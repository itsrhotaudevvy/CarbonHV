package dev.carbonhv.emuviewer

import android.view.Surface

/** native/viewer_jni.c: QEMU as a child of this app, and its frames onto a Surface. */
object Native {
    init {
        System.loadLibrary("emuviewer_jni")
    }

    /** Forks [argv] (argv[0] is the emulator) with `-display android,frame-fd=3`; {pid, fd}. */
    @JvmStatic external fun startQemu(argv: Array<String>, workDir: String, logPath: String): IntArray?
    /** Blocks, showing frames, until QEMU closes the frame socket. Returns frames received. */
    @JvmStatic external fun present(fd: Int): Long
    @JvmStatic external fun setSurface(surface: Surface?)
    /** {frames received, frames shown}. */
    @JvmStatic external fun counters(): LongArray
    @JvmStatic external fun waitQemu(pid: Int): Int
    @JvmStatic external fun signalQemu(pid: Int, signal: Int)
}
