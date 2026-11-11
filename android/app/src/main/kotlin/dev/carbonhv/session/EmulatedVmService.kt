package dev.carbonhv.session

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.util.Log
import java.io.File

/** Owns one QEMU child in the app sandbox; independent of the AVF host service. */
class EmulatedVmService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(1, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        if (intent?.action == ACTION_STOP) {
            stopGuest()
            return START_NOT_STICKY
        }
        val id = intent?.getStringExtra(EXTRA_INSTANCE_ID) ?: return START_NOT_STICKY
        if (activeId == id && activePid != 0) {
            EmulatedEngineClient.signal(id, Result.success(Unit))
        } else if (activeId != null) {
            fail(id, "Another emulated guest is already running.")
        } else {
            launch(id)
        }
        return START_NOT_STICKY
    }

    private fun launch(id: String) {
        val guest = File(filesDir, "guests/$id")
        val argsFile = File(guest, "qemu.args")
        val emulator = File(applicationInfo.nativeLibraryDir, "libqemu_system_aarch64.so")
        if (!argsFile.isFile) {
            fail(id, "No staged guest image. Import a CarbonHV guest before starting this VM.")
            return
        }
        if (!emulator.isFile) {
            fail(id, "The emulated engine is not included in this APK.")
            return
        }
        val args = argsFile.readLines().filter { it.isNotBlank() }
            .map { it.replace("@GUEST@", guest.path) }
        val started = EmulatedNative.startQemu(
            (listOf(emulator.absolutePath) + args).toTypedArray(), guest.path,
            File(guest, "qemu.log").path)
        if (started == null) {
            fail(id, "QEMU could not start. See ${guest.path}/qemu.log")
            return
        }
        activeId = id
        activePid = started[0]
        EmulatedEngineClient.signal(id, Result.success(Unit))
        Thread({
            EmulatedNative.present(started[1])
            val status = EmulatedNative.waitQemu(started[0])
            Log.i(TAG, "QEMU exited for $id: status=$status")
            activePid = 0
            activeId = null
            stopSelf()
        }, "carbonhv-emu-presenter").start()
    }

    private fun stopGuest() {
        if (activePid != 0) EmulatedNative.signalQemu(activePid, 15)
    }

    override fun onDestroy() {
        stopGuest()
        super.onDestroy()
    }

    private fun notification(): Notification {
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, "Emulated guest", NotificationManager.IMPORTANCE_LOW))
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle("CarbonHV emulated guest")
            .setContentText("Running in the app sandbox · software emulation")
            .setOngoing(true).build()
    }

    companion object {
        const val EXTRA_INSTANCE_ID = "dev.carbonhv.session.INSTANCE_ID"
        const val ACTION_STOP = "dev.carbonhv.session.STOP"
        private const val CHANNEL = "emulated-guest"
        private const val TAG = "CarbonHvEmulated"
        @Volatile private var activePid = 0
        @Volatile private var activeId: String? = null

        private fun fail(id: String, message: String) {
            EmulatedEngineClient.signal(id, Result.failure(IllegalStateException(message)))
        }
    }
}
