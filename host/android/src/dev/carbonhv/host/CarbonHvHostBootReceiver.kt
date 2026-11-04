package dev.carbonhv.host

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

class CarbonHvHostBootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_LOCKED_BOOT_COMPLETED) {
            context.startService(Intent(context, CarbonHvHostService::class.java))
        }
    }
}
