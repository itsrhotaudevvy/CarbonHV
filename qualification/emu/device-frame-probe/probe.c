/*
 * On-device probe for QEMU's `-display android` backend (Phase 4 / first guest boot).
 *
 * Runs as an unprivileged process that owns one child QEMU: it creates the SOCK_SEQPACKET
 * socketpair, passes one end to QEMU as fd 3 (`-display android,frame-fd=3`), and behaves like the
 * app's frame reader would: it decodes HELLO/MODE/FRAME, receives each AHardwareBuffer with
 * AHardwareBuffer_recvHandleFromUnixSocket, locks it, dumps it as raw RGBA8888, and returns
 * RELEASE so QEMU can reuse the slot.
 *
 * It is deliberately not the `:vm` service. It exists so that the transport, the buffer
 * allocation and the guest's first pixels can be verified on a phone without installing an APK.
 * Its wire format is the same ui/android-proto.h the backend ships, extracted from the patch at
 * build time, so the probe cannot drift from the producer.
 *
 * usage: probe <frame-out-dir> [--max-frames N] [--interval S] -- <qemu> [qemu args...]
 * The QEMU arguments are passed through unchanged; the probe adds `-display android,frame-fd=3`.
 * --interval S keeps at most one frame per S seconds, so a boot that takes many minutes leaves
 * frames spread over the whole boot instead of the first N updates of the first screen.
 */
#define _GNU_SOURCE
#include <android/hardware_buffer.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#include "ui/android-proto.h"

extern char **environ;

#define FRAME_FD 3

/* The FRAME payload, as decoded by this probe. The wire format itself comes from
 * ui/android-proto.h; this is only the reader's view of one message. */
typedef struct FrameHeader {
    uint32_t seq;
    uint32_t width;
    uint32_t height;
    uint32_t format;
    uint64_t usage;
    bool has_fence;
} FrameHeader;

static void log_line(const char *message)
{
    fprintf(stderr, "carbon-frame-probe: %s\n", message);
}

/* Writes `bytes` to `<dir>/<name>` and reports the size. */
static int write_file(const char *dir, const char *name, const void *bytes, size_t len)
{
    char path[512];
    FILE *file;

    snprintf(path, sizeof(path), "%s/%s", dir, name);
    file = fopen(path, "wb");
    if (!file) {
        fprintf(stderr, "carbon-frame-probe: cannot write %s: %s\n", path, strerror(errno));
        return -1;
    }
    if (fwrite(bytes, 1, len, file) != len) {
        fprintf(stderr, "carbon-frame-probe: short write to %s\n", path);
        fclose(file);
        return -1;
    }
    fclose(file);
    return 0;
}

static int send_release(int fd, uint32_t seq)
{
    uint8_t message[ANDROID_PROTO_HEADER_BYTES + ANDROID_RELEASE_BYTES];
    uint8_t *payload = message + ANDROID_PROTO_HEADER_BYTES;

    android_put_u32(payload, seq);
    android_proto_header(message, ANDROID_MSG_RELEASE, ANDROID_RELEASE_BYTES);
    return send(fd, message, sizeof(message), MSG_NOSIGNAL) == (ssize_t)sizeof(message) ? 0 : -1;
}

/* Copies a locked frame into `out`, converting the row stride away. */
static size_t pack_frame(const uint8_t *src, int32_t stride, uint32_t width, uint32_t height,
                         uint8_t *out)
{
    size_t row = (size_t)width * 4;
    size_t len = row * height;
    uint32_t y;

    for (y = 0; y < height; y++) {
        memcpy(out + (size_t)y * row, src + (size_t)y * (size_t)stride, row);
    }
    return len;
}

static int handle_frame(int fd, const char *dir, const FrameHeader *header,
                        uint32_t *saved, uint32_t max_frames)
{
    AHardwareBuffer *buffer = NULL;
    AHardwareBuffer_Desc desc;
    uint8_t *dst = NULL;
    const uint8_t *src = NULL;
    int32_t bytes_per_pixel = 0, bytes_per_stride = 0;
    char name[64];
    size_t len;
    int rc;

    rc = AHardwareBuffer_recvHandleFromUnixSocket(fd, &buffer);
    if (rc != 0 || !buffer) {
        fprintf(stderr, "carbon-frame-probe: recvHandle failed: %d (%s)\n", rc,
                strerror(rc < 0 ? -rc : rc));
        return -1;
    }

    AHardwareBuffer_describe(buffer, &desc);
    if (desc.width != header->width || desc.height != header->height) {
        fprintf(stderr, "carbon-frame-probe: descriptor mismatch %ux%u vs %ux%u\n",
                desc.width, desc.height, header->width, header->height);
        AHardwareBuffer_release(buffer);
        return -1;
    }

    rc = AHardwareBuffer_lockAndGetInfo(buffer, AHARDWAREBUFFER_USAGE_CPU_READ_RARELY, -1, NULL,
                                        (void **)&src, &bytes_per_pixel, &bytes_per_stride);
    if (rc != 0) {
        fprintf(stderr, "carbon-frame-probe: lock failed: %d (usage %llx)\n", rc,
                (unsigned long long)desc.usage);
        AHardwareBuffer_release(buffer);
        return -1;
    }

    len = (size_t)header->width * 4 * header->height;
    dst = malloc(len);
    if (!dst) {
        AHardwareBuffer_unlock(buffer, NULL);
        AHardwareBuffer_release(buffer);
        return -1;
    }
    len = pack_frame(src, bytes_per_stride, header->width, header->height, dst);
    AHardwareBuffer_unlock(buffer, NULL);
    AHardwareBuffer_release(buffer);

    snprintf(name, sizeof(name), "frame-%05u-%ux%u.rgba", header->seq, header->width,
             header->height);
    if (write_file(dir, name, dst, len) != 0) {
        free(dst);
        return -1;
    }
    free(dst);
    (*saved)++;
    printf("frame seq=%u %ux%u %zu bytes -> %s\n", header->seq, header->width, header->height,
           len, name);
    fflush(stdout);
    return 0;
}

/* Takes one AHardwareBuffer handle off the socket and drops it, so the stream stays aligned. */
static void drain_handle(int fd)
{
    AHardwareBuffer *buffer = NULL;

    AHardwareBuffer_recvHandleFromUnixSocket(fd, &buffer);
    if (buffer) {
        AHardwareBuffer_release(buffer);
    }
}

static double now_seconds(void)
{
    struct timespec ts;

    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

static volatile pid_t qemu_pid;

/* Running under `timeout` must not leave the emulator behind: take it down with us. */
static void on_terminate(int signo)
{
    pid_t pid = qemu_pid;

    (void)signo;
    if (pid > 0) {
        kill(pid, SIGKILL);
    }
    _exit(1);
}

int main(int argc, char **argv)
{
    const char *dir = NULL;
    uint32_t max_frames = 32;
    char **qemu_argv = NULL;
    int i, frame_fd = -1, status = 0, qemu_argc = 0;
    pid_t child = 0;
    uint32_t saved = 0, width = 0, height = 0;
    double interval = 0.0, last_saved = -1.0;
    char **qemu_argv_alloc = NULL;
    uint8_t incoming[ANDROID_PROTO_MAX_PAYLOAD];

    if (argc < 4) {
        fprintf(stderr,
                "usage: %s <frame-out-dir> [--max-frames N] [--interval S] -- <qemu> [args...]\n",
                argv[0]);
        return 64;
    }
    dir = argv[1];
    for (i = 2; i < argc; i++) {
        if (strcmp(argv[i], "--max-frames") == 0 && i + 1 < argc) {
            max_frames = (uint32_t)strtoul(argv[++i], NULL, 10);
        } else if (strcmp(argv[i], "--interval") == 0 && i + 1 < argc) {
            interval = strtod(argv[++i], NULL);
        } else if (strcmp(argv[i], "--") == 0) {
            qemu_argv = &argv[i + 1];
            qemu_argc = argc - (i + 1);
            break;
        }
    }
    if (!qemu_argv || qemu_argc == 0) {
        fprintf(stderr, "carbon-frame-probe: no command after --\n");
        return 64;
    }
    /* A Cuttlefish-shaped command line has dozens of console ports, so no fixed cap. */
    qemu_argv_alloc = calloc((size_t)qemu_argc + 3, sizeof(*qemu_argv_alloc));
    if (!qemu_argv_alloc) {
        log_line("out of memory for QEMU arguments");
        return 1;
    }
    mkdir(dir, 0775);

    /* -display android,frame-fd=3 comes first so a later -display cannot silently win. */
    qemu_argv_alloc[0] = qemu_argv[0];
    qemu_argv_alloc[1] = "-display";
    qemu_argv_alloc[2] = "android,frame-fd=3";
    for (i = 1; i < qemu_argc; i++) {
        qemu_argv_alloc[i + 2] = qemu_argv[i];
    }
    qemu_argv_alloc[qemu_argc + 2] = NULL;

    {
        int pair[2];

        if (socketpair(AF_UNIX, SOCK_SEQPACKET, 0, pair) != 0) {
            fprintf(stderr, "carbon-frame-probe: socketpair: %s\n", strerror(errno));
            return 1;
        }
        qemu_pid = fork();
        if (qemu_pid < 0) {
            fprintf(stderr, "carbon-frame-probe: fork: %s\n", strerror(errno));
            return 1;
        }
        if (qemu_pid == 0) {
            /* Make the socket QEMU's end of the conversation land on a known descriptor. The
             * socketpair can hand us fd 3 itself, and dup2 onto the same descriptor does not
             * clear close-on-exec, so both cases are handled explicitly. Nothing in this child is
             * kept: QEMU's own opens must not be able to reuse fd 3 by accident. */
            int parent_end = pair[0], child_end = pair[1];

            if (child_end != FRAME_FD && dup2(child_end, FRAME_FD) < 0) {
                _exit(127);
            }
            fcntl(FRAME_FD, F_SETFD, 0);
            if (parent_end != FRAME_FD) {
                close(parent_end);
            }
            if (child_end != FRAME_FD) {
                close(child_end);
            }
            execv(qemu_argv_alloc[0], qemu_argv_alloc);
            _exit(127);
        }
        close(pair[1]);
        frame_fd = pair[0];
        child = qemu_pid;
        signal(SIGTERM, on_terminate);
        signal(SIGINT, on_terminate);
        printf("spawned qemu pid=%d frame-fd=%d\n", (int)child, frame_fd);
        fflush(stdout);
    }

    for (;;) {
        ssize_t got = recv(frame_fd, incoming, sizeof(incoming), 0);

        if (got < 0) {
            if (errno == EINTR) {
                continue;
            }
            fprintf(stderr, "carbon-frame-probe: recv: %s\n", strerror(errno));
            break;
        }
        if (got == 0) {
            log_line("frame socket closed by qemu");
            break;
        }

        /* The header kinds are checked in order of arrival; a truncated datagram is ignored. */
        if (android_proto_expect(incoming, (size_t)got, ANDROID_MSG_HELLO, ANDROID_HELLO_BYTES)) {
            printf("hello version=%u\n", android_get_u16(incoming + ANDROID_PROTO_HEADER_BYTES));
            fflush(stdout);
        } else if (android_proto_expect(incoming, (size_t)got, ANDROID_MSG_MODE,
                                        ANDROID_MODE_BYTES)) {
            const uint8_t *payload = incoming + ANDROID_PROTO_HEADER_BYTES;

            width = android_get_u32(payload);
            height = android_get_u32(payload + 4);
            printf("mode %ux%u format=%u\n", width, height, android_get_u32(payload + 8));
            fflush(stdout);
        } else if (android_proto_expect(incoming, (size_t)got, ANDROID_MSG_FRAME,
                                        ANDROID_FRAME_BYTES)) {
            const uint8_t *payload = incoming + ANDROID_PROTO_HEADER_BYTES;
            FrameHeader header = {
                .seq = android_get_u32(payload),
                .width = android_get_u32(payload + 4),
                .height = android_get_u32(payload + 8),
                .format = android_get_u32(payload + 12),
                .usage = (uint64_t)android_get_u32(payload + 16) |
                         ((uint64_t)android_get_u32(payload + 20) << 32),
                .has_fence =
                    (android_get_u32(payload + 24) & ANDROID_FRAME_FLAG_HAS_FENCE) != 0,
            };

            if (saved < max_frames &&
                (last_saved < 0 || now_seconds() - last_saved >= interval)) {
                if (handle_frame(frame_fd, dir, &header, &saved, max_frames) == 0) {
                    last_saved = now_seconds();
                }
            } else {
                /* Keep reading: the handle is a separate datagram and must be taken off the
                 * socket, or every later datagram is off by one. */
                drain_handle(frame_fd);
            }
            /* Always acknowledge, or the ring never drains and QEMU drops every later frame. */
            send_release(frame_fd, header.seq);
        } else if (android_proto_expect(incoming, (size_t)got, ANDROID_MSG_CURSOR,
                                        ANDROID_CURSOR_BYTES)) {
            const uint8_t *payload = incoming + ANDROID_PROTO_HEADER_BYTES;

            printf("cursor %u,%u visible=%u\n", android_get_u32(payload),
                   android_get_u32(payload + 4), android_get_u32(payload + 8));
            fflush(stdout);
        } else {
            fprintf(stderr, "carbon-frame-probe: %zd-byte datagram is not a known message\n",
                    got);
        }
    }

    if (waitpid(child, &status, 0) < 0) {
        fprintf(stderr, "carbon-frame-probe: waitpid: %s\n", strerror(errno));
    }
    close(frame_fd);

    printf("saved %u frame(s) into %s\n", saved, dir);
    if (WIFEXITED(status)) {
        printf("qemu exit=%d\n", WEXITSTATUS(status));
    } else if (WIFSIGNALED(status)) {
        printf("qemu killed by signal=%d\n", WTERMSIG(status));
    }
    return saved > 0 ? 0 : 1;
}
