package dev.carbonhv.emuprobe

import android.hardware.HardwareBuffer

/** JNI checks implemented in native/probe_jni.c. Each returns a short result string. */
object Native {
    init {
        System.loadLibrary("emuprobe_jni")
    }

    @JvmStatic external fun jitRwx(): String
    @JvmStatic external fun jitMprotect(): String
    @JvmStatic external fun jitMemfdDual(): String
    @JvmStatic external fun affinity(cpus: IntArray, uclampMin: Int): String
    @JvmStatic external fun oDirect(dir: String): String
    @JvmStatic external fun inetSocket(): String
    @JvmStatic external fun openDevice(path: String): String
    @JvmStatic external fun selinuxContext(): String
    @JvmStatic external fun listenAbstract(name: String): Int
    @JvmStatic external fun acceptHardwareBuffer(listenFd: Int, timeoutMs: Int): HardwareBuffer?
    @JvmStatic external fun closeFd(fd: Int)
}
