package dev.carbonhv.session

import android.view.Surface

/** JNI bridge to QEMU and its AHardwareBuffer presenter. */
object EmulatedNative {
    init { System.loadLibrary("carbon_emulated") }

    @JvmStatic external fun startQemu(argv: Array<String>, workDir: String, logPath: String): IntArray?
    @JvmStatic external fun present(fd: Int): Long
    @JvmStatic external fun setSurface(surface: Surface?)
    @JvmStatic external fun waitQemu(pid: Int): Int
    @JvmStatic external fun signalQemu(pid: Int, signal: Int)
}
