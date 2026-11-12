/*
 * Qualification viewer for the emulated engine: runs QEMU as a child of the app (untrusted_app)
 * and puts the guest's frames on the phone's screen.
 *
 * QEMU gets one end of a SOCK_SEQPACKET socketpair as fd 3 (`-display android,frame-fd=3`). The
 * presenter thread reads the ui/android-proto.h stream: for each FRAME it takes the
 * AHardwareBuffer handle off the socket, CPU-copies it into the SurfaceView's ANativeWindow, and
 * only then sends RELEASE, so QEMU can never rewrite a slot that is still being read. Every frame
 * is also kept as a CPU copy, so a surface that attaches later (the activity starts after QEMU, or
 * returns from the background) shows the guest's current screen at once instead of staying black
 * until the guest happens to redraw. Frames are always released, or the ring would stall.
 *
 * This is not the `:vm` service: no broker, no QMP supervision, no fd-passed disks. It exists to
 * show a guest on the screen from inside the app sandbox.
 */
#define _GNU_SOURCE
#include <android/hardware_buffer.h>
#include <android/log.h>
#include <android/native_window.h>
#include <android/native_window_jni.h>
#include <errno.h>
#include <fcntl.h>
#include <jni.h>
#include <pthread.h>
#include <signal.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#include "ui/android-proto.h"

#define TAG "CarbonEmuViewer"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, TAG, __VA_ARGS__)
#define FRAME_FD 3

static pthread_mutex_t g_lock = PTHREAD_MUTEX_INITIALIZER;
static ANativeWindow *g_window;
static uint32_t g_window_w, g_window_h;
static volatile long g_received, g_shown;
/* The latest frame, tightly packed RGBX. Guarded by g_lock. */
static uint8_t *g_last;
static uint32_t g_last_w, g_last_h;
static int g_logged_failures;

static void note_failure(const char *why, uint32_t seq);

/* Locks the window's back buffer and copies one frame into it. Returns false if the window could
 * not be locked; on true the caller must ANativeWindow_unlockAndPost. Caller holds g_lock and has
 * a window. Copy is split from post so the frame's ring slot can be RELEASEd after the copy but
 * before the post, which may block on SurfaceFlinger. */
static bool window_copy(const uint8_t *src, uint32_t stride_px, uint32_t width, uint32_t height,
                        ANativeWindow_Buffer *out)
{
    uint32_t rows, cols, y;

    if (g_window_w != width || g_window_h != height) {
        /* RGBX: the guest scanout has no meaningful alpha. */
        ANativeWindow_setBuffersGeometry(g_window, (int32_t)width, (int32_t)height,
                                         AHARDWAREBUFFER_FORMAT_R8G8B8X8_UNORM);
        g_window_w = width;
        g_window_h = height;
    }
    if (ANativeWindow_lock(g_window, out, NULL) != 0) {
        note_failure("ANativeWindow_lock failed", 0);
        return false;
    }
    rows = height < (uint32_t)out->height ? height : (uint32_t)out->height;
    cols = width < (uint32_t)out->width ? width : (uint32_t)out->width;
    for (y = 0; y < rows; y++) {
        memcpy((uint8_t *)out->bits + (size_t)y * (size_t)out->stride * 4,
               src + (size_t)y * stride_px * 4, (size_t)cols * 4);
    }
    return true;
}

/* Copies one locked frame into the window and posts it. Caller holds g_lock and has a window. */
static void show(const uint8_t *src, uint32_t stride_px, uint32_t width, uint32_t height)
{
    ANativeWindow_Buffer out;

    if (window_copy(src, stride_px, width, height, &out)) {
        ANativeWindow_unlockAndPost(g_window);
        g_shown++;
    }
}

/* Redraws the kept frame on the current window. Caller holds g_lock. */
static void show_last(void)
{
    if (g_window && g_last) {
        show(g_last, g_last_w, g_last_w, g_last_h);
    }
}

static int64_t now_ns(void)
{
    struct timespec ts;

    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000000000 + ts.tv_nsec;
}

/* The kept heap frame exists only so a surface that attaches later shows the guest at once. While
 * a surface is attached it is refreshed at most twice a second; while none is, every frame is kept.
 * This removes the second full-frame copy from the steady-state present path. Caller holds g_lock
 * and the frame's AHardwareBuffer is locked. */
#define SNAPSHOT_INTERVAL_NS 500000000LL
static int64_t g_last_snapshot_ns;

static void snapshot(const uint8_t *src, uint32_t stride_px, uint32_t width, uint32_t height)
{
    size_t row = (size_t)width * 4;
    uint8_t *copy;
    uint32_t y;
    int64_t now = now_ns();

    if (g_window && g_last && g_last_w == width && g_last_h == height &&
        now - g_last_snapshot_ns < SNAPSHOT_INTERVAL_NS) {
        return;
    }
    copy = (g_last && g_last_w == width && g_last_h == height)
               ? g_last
               : realloc(g_last, row * height);
    if (!copy) {
        return;
    }
    for (y = 0; y < height; y++) {
        memcpy(copy + y * row, src + (size_t)y * stride_px * 4, row);
    }
    g_last = copy;
    g_last_w = width;
    g_last_h = height;
    g_last_snapshot_ns = now;
}

static void note_failure(const char *why, uint32_t seq)
{
    if (g_logged_failures < 20) {
        g_logged_failures++;
        LOGE("frame %u not shown: %s", seq, why);
    }
}

static void release(int fd, uint32_t seq)
{
    uint8_t message[ANDROID_PROTO_HEADER_BYTES + ANDROID_RELEASE_BYTES];

    android_put_u32(message + ANDROID_PROTO_HEADER_BYTES, seq);
    android_proto_header(message, ANDROID_MSG_RELEASE, ANDROID_RELEASE_BYTES);
    send(fd, message, sizeof(message), MSG_NOSIGNAL);
}

JNIEXPORT void JNICALL Java_dev_carbonhv_emuviewer_Native_setSurface(JNIEnv *env, jclass cls,
                                                                     jobject surface)
{
    pthread_mutex_lock(&g_lock);
    if (g_window) {
        ANativeWindow_release(g_window);
        g_window = NULL;
    }
    if (surface) {
        g_window = ANativeWindow_fromSurface(env, surface);
        g_window_w = g_window_h = 0;
        LOGI("surface attached (%s)", g_window ? "ok" : "ANativeWindow_fromSurface failed");
        show_last();
    } else {
        LOGI("surface detached");
    }
    pthread_mutex_unlock(&g_lock);
}

/* Forks QEMU with fd 3 as its frame socket. Returns {pid, our socket end} or null. */
JNIEXPORT jintArray JNICALL Java_dev_carbonhv_emuviewer_Native_startQemu(
    JNIEnv *env, jclass cls, jobjectArray jargv, jstring jworkdir, jstring jlog)
{
    int argc = (*env)->GetArrayLength(env, jargv), pair[2], i;
    char **argv = calloc((size_t)argc + 3, sizeof(char *));
    const char *workdir = (*env)->GetStringUTFChars(env, jworkdir, NULL);
    const char *log_path = (*env)->GetStringUTFChars(env, jlog, NULL);
    jintArray result = NULL;
    pid_t pid;

    /* Everything the child needs is prepared before fork: after it, only exec-safe calls. */
    for (i = 0; i < argc; i++) {
        jstring s = (jstring)(*env)->GetObjectArrayElement(env, jargv, i);
        const char *chars = (*env)->GetStringUTFChars(env, s, NULL);
        argv[i == 0 ? 0 : i + 2] = strdup(chars);
        (*env)->ReleaseStringUTFChars(env, s, chars);
    }
    argv[1] = "-display";
    argv[2] = "android,frame-fd=3";
    argv[argc + 2] = NULL;

    if (socketpair(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0, pair) != 0) {
        LOGE("socketpair: %s", strerror(errno));
        goto out;
    }
    pid = fork();
    if (pid < 0) {
        LOGE("fork: %s", strerror(errno));
        close(pair[0]);
        close(pair[1]);
        goto out;
    }
    if (pid == 0) {
        int logfd, fd;

        setsid();
        if (chdir(workdir) != 0) {
            _exit(126);
        }
        logfd = open(log_path, O_WRONLY | O_CREAT | O_TRUNC, 0600);
        if (logfd >= 0) {
            dup2(logfd, 1);
            dup2(logfd, 2);
        }
        fd = open("/dev/null", O_RDONLY);
        if (fd >= 0) {
            dup2(fd, 0);
        }
        if (dup2(pair[1], FRAME_FD) < 0) {
            _exit(127);
        }
        fcntl(FRAME_FD, F_SETFD, 0);
        /* Nothing of the app's (binder, ashmem, sockets) leaks into the emulator. */
        for (fd = FRAME_FD + 1; fd < 4096; fd++) {
            close(fd);
        }
        execv(argv[0], argv);
        _exit(127);
    }
    close(pair[1]);
    {
        jint values[2] = {pid, pair[0]};
        result = (*env)->NewIntArray(env, 2);
        (*env)->SetIntArrayRegion(env, result, 0, 2, values);
    }
    LOGI("qemu pid=%d frame socket=%d", pid, pair[0]);
out:
    (*env)->ReleaseStringUTFChars(env, jworkdir, workdir);
    (*env)->ReleaseStringUTFChars(env, jlog, log_path);
    for (i = 0; i < argc + 2; i++) {
        if (i != 1 && i != 2) {
            free(argv[i]);
        }
    }
    free(argv);
    return result;
}

/* Reads frames until QEMU closes the socket. Returns the number of frames received. */
JNIEXPORT jlong JNICALL Java_dev_carbonhv_emuviewer_Native_present(JNIEnv *env, jclass cls,
                                                                   jint fd)
{
    uint8_t incoming[ANDROID_PROTO_MAX_PAYLOAD];

    for (;;) {
        ssize_t got = recv(fd, incoming, sizeof(incoming), 0);
        const uint8_t *payload = incoming + ANDROID_PROTO_HEADER_BYTES;

        if (got < 0 && errno == EINTR) {
            continue;
        }
        if (got <= 0) {
            LOGI("frame socket closed (%s)", got < 0 ? strerror(errno) : "eof");
            break;
        }
        if (android_proto_expect(incoming, (size_t)got, ANDROID_MSG_MODE, ANDROID_MODE_BYTES)) {
            LOGI("mode %ux%u", android_get_u32(payload), android_get_u32(payload + 4));
        } else if (android_proto_expect(incoming, (size_t)got, ANDROID_MSG_FRAME,
                                        ANDROID_FRAME_BYTES)) {
            uint32_t seq = android_get_u32(payload);
            uint32_t width = android_get_u32(payload + 4);
            uint32_t height = android_get_u32(payload + 8);
            AHardwareBuffer *buffer = NULL;
            bool released = false;

            /* The handle is its own datagram and must be consumed even if it is not shown. */
            if (AHardwareBuffer_recvHandleFromUnixSocket(fd, &buffer) == 0 && buffer) {
                AHardwareBuffer_Desc desc;
                void *src = NULL;

                AHardwareBuffer_describe(buffer, &desc);
                pthread_mutex_lock(&g_lock);
                if (desc.width != width || desc.height != height) {
                    note_failure("buffer size differs from the FRAME header", seq);
                } else if (AHardwareBuffer_lock(buffer, AHARDWAREBUFFER_USAGE_CPU_READ_RARELY, -1,
                                                NULL, &src) != 0) {
                    note_failure("AHardwareBuffer_lock failed", seq);
                } else {
                    ANativeWindow_Buffer out;
                    bool locked = false;

                    /* One copy, straight into the window. The kept heap frame is throttled. */
                    snapshot((const uint8_t *)src, desc.stride, width, height);
                    if (g_window) {
                        locked = window_copy((const uint8_t *)src, desc.stride, width, height, &out);
                    } else {
                        note_failure("no surface attached yet (kept for when one is)", seq);
                    }
                    AHardwareBuffer_unlock(buffer, NULL);
                    /* The pixels are out of the buffer: free QEMU's ring slot now, before the
                     * post, which can block on SurfaceFlinger. */
                    release(fd, seq);
                    released = true;
                    if (locked) {
                        ANativeWindow_unlockAndPost(g_window);
                        g_shown++;
                    }
                }
                pthread_mutex_unlock(&g_lock);
                AHardwareBuffer_release(buffer);
            }
            g_received++;
            if (!released) {
                release(fd, seq);
            }
        }
    }
    close(fd);
    return g_received;
}

JNIEXPORT jlongArray JNICALL Java_dev_carbonhv_emuviewer_Native_counters(JNIEnv *env, jclass cls)
{
    jlong values[2] = {g_received, g_shown};
    jlongArray result = (*env)->NewLongArray(env, 2);

    (*env)->SetLongArrayRegion(env, result, 0, 2, values);
    return result;
}

/* Blocks until QEMU exits. Returns the wait status, or -1. */
JNIEXPORT jint JNICALL Java_dev_carbonhv_emuviewer_Native_waitQemu(JNIEnv *env, jclass cls,
                                                                   jint pid)
{
    int status = 0;

    while (waitpid(pid, &status, 0) < 0) {
        if (errno != EINTR) {
            return -1;
        }
    }
    return status;
}

JNIEXPORT void JNICALL Java_dev_carbonhv_emuviewer_Native_signalQemu(JNIEnv *env, jclass cls,
                                                                     jint pid, jint sig)
{
    if (pid > 0) {
        kill(pid, sig);
    }
}
