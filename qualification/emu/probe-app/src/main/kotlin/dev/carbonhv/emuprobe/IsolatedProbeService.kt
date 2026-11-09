package dev.carbonhv.emuprobe

import android.app.Service
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import kotlin.concurrent.thread

/** Runs [Probes.isolated] in an isolated_app process and replies with the JSON. */
class IsolatedProbeService : Service() {
    private val handler = object : Handler(Looper.getMainLooper()) {
        override fun handleMessage(msg: Message) {
            val reply = msg.replyTo ?: return
            thread(name = "probe-isolated") {
                val json = runCatching { Probes.isolated(this@IsolatedProbeService).toString() }
                    .getOrElse { "{\"error\":\"${it.javaClass.simpleName}: ${it.message}\"}" }
                reply.send(Message.obtain().apply { data = Bundle().apply { putString("json", json) } })
            }
        }
    }

    override fun onBind(intent: Intent): IBinder = Messenger(handler).binder
}
