package dev.carbonhv.emuviewer

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.util.Log
import java.io.File

/**
 * Owns one emulated guest. The guest is staged under files/guest by
 * qualification/emu/android-standin/run-in-viewer.sh: kernel, initrd, disk.img and qemu.args
 * (one QEMU argument per line, `@GUEST@` standing for this directory, the emulator executable
 * implied). The process is a specialUse foreground service, the role the `:vm` service will have.
 */
class ViewerService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(1, notification("AOSP stand-in guest (not GrapheneOS) · Emulated"),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        if (intent?.action == ACTION_STOP) {
            stopGuest()
            stopSelf()
            return START_NOT_STICKY
        }
        synchronized(Companion) {
            if (pid == 0) launch()
        }
        return START_NOT_STICKY
    }

    private fun launch() {
        val guest = File(filesDir, "guest")
        val argsFile = File(guest, "qemu.args")
        if (!argsFile.isFile) {
            Log.e(TAG, "no guest staged: ${argsFile.path} is missing")
            stopSelf()
            return
        }
        val emulator = File(applicationInfo.nativeLibraryDir, "libqemu_system_aarch64.so").path
        val args = argsFile.readLines().filter { it.isNotEmpty() }.map { it.replace("@GUEST@", guest.path) }
        val started = Native.startQemu((listOf(emulator) + args).toTypedArray(), guest.path,
            File(guest, "qemu.log").path)
        if (started == null) {
            Log.e(TAG, "could not start the emulator")
            stopSelf()
            return
        }
        pid = started[0]
        Log.i(TAG, "emulator pid=$pid, ${args.size} arguments, guest dir ${guest.path}")
        Thread({
            val frames = Native.present(started[1])
            val status = Native.waitQemu(started[0])
            Log.i(TAG, "emulator exited: wait status=$status, frames received=$frames")
            synchronized(Companion) { pid = 0 }
            stopSelf()
        }, "emu-presenter").start()
    }

    private fun stopGuest() {
        synchronized(Companion) {
            if (pid != 0) Native.signalQemu(pid, 15)
        }
    }

    override fun onDestroy() {
        stopGuest()
        super.onDestroy()
    }

    private fun notification(text: String): Notification {
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, "Emulated guest", NotificationManager.IMPORTANCE_LOW))
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle("CarbonHV emulated engine")
            .setContentText(text)
            .setOngoing(true)
            .build()
    }

    companion object {
        const val TAG = "CarbonEmuViewer"
        const val CHANNEL = "emu"
        const val ACTION_STOP = "dev.carbonhv.emuviewer.STOP"
        @Volatile var pid = 0
    }
}
