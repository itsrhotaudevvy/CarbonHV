package dev.carbonhv.emuprobe

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import kotlin.concurrent.thread

/**
 * Foreground-service entry point, the same process role the emulated engine's `:vm` service has.
 * Started with `adb shell am start-foreground-service`, so the probe never covers the user's
 * screen; it only posts an ongoing notification while it runs, then stops itself.
 */
class ProbeService : Service() {
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(CHANNEL, "Probe", NotificationManager.IMPORTANCE_LOW))
        val notification = Notification.Builder(this, CHANNEL)
            .setContentTitle("CarbonHV emu probe running")
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setOngoing(true)
            .build()
        startForeground(1, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        thread(name = "probe-main") {
            ProbeRunner(this, entryPoint = "foreground service (specialUse)").runAll()
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf(startId)
        }
        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private companion object {
        const val CHANNEL = "probe"
    }
}
