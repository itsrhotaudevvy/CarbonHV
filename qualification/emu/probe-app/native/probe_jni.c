// Gate 3 probe: native checks that only make sense inside the app sandbox (untrusted_app).
// Every function returns a short human-readable result; nothing here changes system state.
#define _GNU_SOURCE
#include <android/hardware_buffer.h>
#include <android/hardware_buffer_jni.h>
#include <errno.h>
#include <fcntl.h>
#include <jni.h>
#include <poll.h>
#include <sched.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/un.h>
#include <unistd.h>

static jstring str(JNIEnv *env, const char *s) { return (*env)->NewStringUTF(env, s); }

// mov w0, #42 ; ret
static const uint32_t kStub[] = {0x52800540u, 0xd65f03c0u};
typedef int (*stub_fn)(void);

static int run_stub(void *exec_view, void *write_view) {
    memcpy(write_view, kStub, sizeof(kStub));
    __builtin___clear_cache((char *)exec_view, (char *)exec_view + sizeof(kStub));
    return ((stub_fn)exec_view)();
}

// P2a: anonymous RWX mapping (SELinux execmem).
JNIEXPORT jstring JNICALL Java_dev_carbonhv_emuprobe_Native_jitRwx(JNIEnv *env, jclass cls) {
    char out[128];
    void *p = mmap(NULL, 4096, PROT_READ | PROT_WRITE | PROT_EXEC, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (p == MAP_FAILED) {
        snprintf(out, sizeof(out), "denied: mmap RWX errno=%d (%s)", errno, strerror(errno));
        return str(env, out);
    }
    int r = run_stub(p, p);
    munmap(p, 4096);
    snprintf(out, sizeof(out), "ok: stub returned %d", r);
    return str(env, out);
}

// P2b: RW anonymous mapping later flipped to RX with mprotect (also execmem).
JNIEXPORT jstring JNICALL Java_dev_carbonhv_emuprobe_Native_jitMprotect(JNIEnv *env, jclass cls) {
    char out[128];
    void *p = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (p == MAP_FAILED) {
        snprintf(out, sizeof(out), "error: mmap RW errno=%d", errno);
        return str(env, out);
    }
    memcpy(p, kStub, sizeof(kStub));
    if (mprotect(p, 4096, PROT_READ | PROT_EXEC) != 0) {
        snprintf(out, sizeof(out), "denied: mprotect RX errno=%d (%s)", errno, strerror(errno));
        munmap(p, 4096);
        return str(env, out);
    }
    __builtin___clear_cache((char *)p, (char *)p + sizeof(kStub));
    int r = ((stub_fn)p)();
    munmap(p, 4096);
    snprintf(out, sizeof(out), "ok: stub returned %d", r);
    return str(env, out);
}

// P2c: memfd double mapping, one RW view and one RX view (what QEMU split-wx=on does).
JNIEXPORT jstring JNICALL Java_dev_carbonhv_emuprobe_Native_jitMemfdDual(JNIEnv *env, jclass cls) {
    char out[160];
    int fd = memfd_create("emuprobe-jit", MFD_CLOEXEC);
    if (fd < 0) {
        snprintf(out, sizeof(out), "denied: memfd_create errno=%d (%s)", errno, strerror(errno));
        return str(env, out);
    }
    if (ftruncate(fd, 4096) != 0) {
        snprintf(out, sizeof(out), "error: ftruncate errno=%d", errno);
        close(fd);
        return str(env, out);
    }
    void *rw = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    void *rx = mmap(NULL, 4096, PROT_READ | PROT_EXEC, MAP_SHARED, fd, 0);
    close(fd);
    if (rw == MAP_FAILED || rx == MAP_FAILED) {
        snprintf(out, sizeof(out), "denied: rw=%s rx=%s errno=%d (%s)", rw == MAP_FAILED ? "fail" : "ok",
                 rx == MAP_FAILED ? "fail" : "ok", errno, strerror(errno));
        if (rw != MAP_FAILED) munmap(rw, 4096);
        if (rx != MAP_FAILED) munmap(rx, 4096);
        return str(env, out);
    }
    int r = run_stub(rx, rw);
    munmap(rw, 4096);
    munmap(rx, 4096);
    snprintf(out, sizeof(out), "ok: stub returned %d", r);
    return str(env, out);
}

static void cpulist(const cpu_set_t *set, char *buf, size_t len) {
    buf[0] = 0;
    for (int cpu = 0; cpu < CPU_SETSIZE && cpu < 64; cpu++) {
        if (CPU_ISSET(cpu, set)) {
            size_t used = strlen(buf);
            snprintf(buf + used, len - used, used ? ",%d" : "%d", cpu);
        }
    }
}

struct carbon_sched_attr {
    uint32_t size, sched_policy;
    uint64_t sched_flags;
    int32_t sched_nice;
    uint32_t sched_priority;
    uint64_t sched_runtime, sched_deadline, sched_period;
    uint32_t sched_util_min, sched_util_max;
};
#define CARBON_SCHED_FLAG_KEEP_ALL 0x18
#define CARBON_SCHED_FLAG_UTIL_CLAMP_MIN 0x20

// P3: pin the calling thread to `cpus`, read it back, then try uclamp.min on the same thread.
JNIEXPORT jstring JNICALL Java_dev_carbonhv_emuprobe_Native_affinity(JNIEnv *env, jclass cls,
                                                                    jintArray cpus, jint uclamp_min) {
    char out[512], before[128], after[128];
    cpu_set_t set;
    CPU_ZERO(&set);
    sched_getaffinity(0, sizeof(set), &set);
    cpulist(&set, before, sizeof(before));

    jsize n = (*env)->GetArrayLength(env, cpus);
    jint *values = (*env)->GetIntArrayElements(env, cpus, NULL);
    CPU_ZERO(&set);
    for (jsize i = 0; i < n; i++) CPU_SET(values[i], &set);
    (*env)->ReleaseIntArrayElements(env, cpus, values, JNI_ABORT);
    int set_rc = sched_setaffinity(0, sizeof(set), &set);
    int set_errno = errno;
    CPU_ZERO(&set);
    sched_getaffinity(0, sizeof(set), &set);
    cpulist(&set, after, sizeof(after));

    struct carbon_sched_attr attr = {0};
    attr.size = sizeof(attr);
    attr.sched_flags = CARBON_SCHED_FLAG_KEEP_ALL | CARBON_SCHED_FLAG_UTIL_CLAMP_MIN;
    attr.sched_util_min = (uint32_t)uclamp_min;
    int uc_rc = (int)syscall(__NR_sched_setattr, 0, &attr, 0);
    int uc_errno = errno;
    struct carbon_sched_attr got = {0};
    int get_rc = (int)syscall(__NR_sched_getattr, 0, &got, sizeof(got), 0);

    snprintf(out, sizeof(out),
             "before=[%s] setaffinity=%s%s after=[%s] uclamp_set=%s%s uclamp_min_now=%s%u",
             before, set_rc == 0 ? "ok" : "errno=", set_rc == 0 ? "" : strerror(set_errno), after,
             uc_rc == 0 ? "ok" : "errno=", uc_rc == 0 ? "" : strerror(uc_errno),
             get_rc == 0 ? "" : "?", got.sched_util_min);
    return str(env, out);
}

// P6a: O_DIRECT on the app's own storage, with an aligned 4 KiB write and read.
JNIEXPORT jstring JNICALL Java_dev_carbonhv_emuprobe_Native_oDirect(JNIEnv *env, jclass cls, jstring jdir) {
    char out[192], path[512];
    const char *dir = (*env)->GetStringUTFChars(env, jdir, NULL);
    snprintf(path, sizeof(path), "%s/odirect.bin", dir);
    (*env)->ReleaseStringUTFChars(env, jdir, dir);
    int fd = open(path, O_RDWR | O_CREAT | O_TRUNC | O_DIRECT | O_CLOEXEC, 0600);
    if (fd < 0) {
        snprintf(out, sizeof(out), "open O_DIRECT failed errno=%d (%s)", errno, strerror(errno));
        return str(env, out);
    }
    void *buf = NULL;
    posix_memalign(&buf, 4096, 4096);
    memset(buf, 0xA5, 4096);
    ssize_t w = pwrite(fd, buf, 4096, 0);
    int werr = errno;
    memset(buf, 0, 4096);
    ssize_t r = pread(fd, buf, 4096, 0);
    int rerr = errno;
    int match = r == 4096 && ((unsigned char *)buf)[4095] == 0xA5;
    free(buf);
    close(fd);
    unlink(path);
    snprintf(out, sizeof(out), "write=%zd%s read=%zd%s data=%s", w, w == 4096 ? "" : strerror(werr), r,
             r == 4096 ? "" : strerror(rerr), match ? "match" : "MISMATCH");
    return str(env, out);
}

// P7 helpers.
JNIEXPORT jstring JNICALL Java_dev_carbonhv_emuprobe_Native_inetSocket(JNIEnv *env, jclass cls) {
    char out[128];
    int fd = socket(AF_INET, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0) {
        snprintf(out, sizeof(out), "denied errno=%d (%s)", errno, strerror(errno));
        return str(env, out);
    }
    close(fd);
    return str(env, "allowed");
}

JNIEXPORT jstring JNICALL Java_dev_carbonhv_emuprobe_Native_openDevice(JNIEnv *env, jclass cls, jstring jpath) {
    char out[192];
    const char *path = (*env)->GetStringUTFChars(env, jpath, NULL);
    int fd = open(path, O_RDWR | O_CLOEXEC);
    int err = errno;
    (*env)->ReleaseStringUTFChars(env, jpath, path);
    if (fd < 0) {
        snprintf(out, sizeof(out), "denied errno=%d (%s)", err, strerror(err));
        return str(env, out);
    }
    close(fd);
    return str(env, "opened");
}

JNIEXPORT jstring JNICALL Java_dev_carbonhv_emuprobe_Native_selinuxContext(JNIEnv *env, jclass cls) {
    char buf[256] = {0};
    int fd = open("/proc/self/attr/current", O_RDONLY | O_CLOEXEC);
    if (fd >= 0) {
        ssize_t n = read(fd, buf, sizeof(buf) - 1);
        if (n > 0 && buf[n - 1] == '\n') buf[n - 1] = 0;
        close(fd);
    }
    return str(env, buf);
}

// P4: listen on an abstract SOCK_SEQPACKET socket, accept one child connection and receive one
// AHardwareBuffer with the public NDK API, returned as android.hardware.HardwareBuffer.
JNIEXPORT jint JNICALL Java_dev_carbonhv_emuprobe_Native_listenAbstract(JNIEnv *env, jclass cls, jstring jname) {
    const char *name = (*env)->GetStringUTFChars(env, jname, NULL);
    struct sockaddr_un addr = {.sun_family = AF_UNIX};
    size_t len = strlen(name);
    if (len > sizeof(addr.sun_path) - 2) len = sizeof(addr.sun_path) - 2;
    memcpy(addr.sun_path + 1, name, len);
    (*env)->ReleaseStringUTFChars(env, jname, name);
    int fd = socket(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0);
    if (fd < 0) return -errno;
    if (bind(fd, (struct sockaddr *)&addr, (socklen_t)(offsetof(struct sockaddr_un, sun_path) + 1 + len)) != 0 ||
        listen(fd, 1) != 0) {
        int err = errno;
        close(fd);
        return -err;
    }
    return fd;
}

JNIEXPORT jobject JNICALL Java_dev_carbonhv_emuprobe_Native_acceptHardwareBuffer(JNIEnv *env, jclass cls,
                                                                                jint listen_fd, jint timeout_ms) {
    struct pollfd pfd = {.fd = listen_fd, .events = POLLIN};
    if (poll(&pfd, 1, timeout_ms) != 1) return NULL;
    int conn = accept4(listen_fd, NULL, NULL, SOCK_CLOEXEC);
    if (conn < 0) return NULL;
    AHardwareBuffer *buffer = NULL;
    int rc = AHardwareBuffer_recvHandleFromUnixSocket(conn, &buffer);
    char ack = rc == 0 ? 'k' : 'e';
    write(conn, &ack, 1);
    close(conn);
    if (rc != 0 || buffer == NULL) return NULL;
    jobject java_buffer = AHardwareBuffer_toHardwareBuffer(env, buffer);
    AHardwareBuffer_release(buffer);
    return java_buffer;
}

JNIEXPORT void JNICALL Java_dev_carbonhv_emuprobe_Native_closeFd(JNIEnv *env, jclass cls, jint fd) { close(fd); }
