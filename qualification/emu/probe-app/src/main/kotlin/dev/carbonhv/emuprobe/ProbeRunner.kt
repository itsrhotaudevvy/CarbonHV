package dev.carbonhv.emuprobe

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.graphics.Bitmap
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.Message
import android.os.Messenger
import android.util.Log
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.json.JSONObject

/**
 * Runs every Gate 3 check once from [context] (an Activity or the foreground service), saving
 * files/report.json after each step and logging under [TAG]. Touches nothing outside the sandbox.
 */
class ProbeRunner(
    private val context: Context,
    private val entryPoint: String,
    private val status: (String) -> Unit = {},
    private val show: (Bitmap) -> Unit = {},
) {
    fun runAll(): JSONObject {
        val report = JSONObject()
        fun step(name: String, block: () -> Any) {
            status("running $name…")
            report.put(name, runCatching { block() }.getOrElse { "error: ${it.javaClass.simpleName}: ${it.message}" })
            // Saved after every step, so a crash in a later check keeps the earlier results.
            File(context.filesDir, "report.json").writeText(report.toString(2))
            Log.i(TAG, "step done: $name")
        }
        val qemu = Probes.qemu(context)
        report.put("probe", "CarbonHV emulated engine Gate 3 probe v1")
        report.put("entry_point", entryPoint)
        step("environment") { Probes.environment(context) }
        step("p1_exec_qemu_version") { Probes.run(listOf(qemu, "--version"), 10).toJson() }
        step("p2_jit") { Probes.jit(qemu) }
        step("p3_placement") { Probes.placement() }
        step("p4_gfxstream_in_qemu") { Probes.gfxstream(qemu) }
        step("p4_vulkan_ahb_child_to_app") { Probes.ahbFromChild(context, show) }
        step("p5_memory") { Probes.memory(context) }
        step("p6_io") { Probes.io(context) }
        step("p7_isolated_process") { isolated() }
        step("p7_app_process_comparison") {
            JSONObject().put("inet_socket", Native.inetSocket()).put("open_kgsl", Native.openDevice("/dev/kgsl-3d0"))
        }
        step("p8_qemu_sandbox_flag") { Probes.sandboxFlag(qemu) }
        step("p9_thermal") { Probes.thermal(context) }
        report.put("done", true)

        val json = report.toString(2)
        File(context.filesDir, "report.json").writeText(json)
        json.chunked(3000).forEach { Log.i(TAG, it) }
        Log.i(TAG, "REPORT-COMPLETE")
        status(json)
        return report
    }

    private fun isolated(): Any {
        val replies = HandlerThread("probe-iso-reply").apply { start() }
        val latch = CountDownLatch(1)
        var json: String? = null
        val replyTo = Messenger(
            object : Handler(replies.looper) {
                override fun handleMessage(msg: Message) {
                    json = msg.data.getString("json")
                    latch.countDown()
                }
            },
        )
        val connection = object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName, binder: IBinder) {
                Messenger(binder).send(Message.obtain().apply { this.replyTo = replyTo })
            }

            override fun onServiceDisconnected(name: ComponentName) = Unit
        }
        val bound = context.bindService(
            Intent(context, IsolatedProbeService::class.java),
            connection,
            Context.BIND_AUTO_CREATE,
        )
        val answered = bound && latch.await(60, TimeUnit.SECONDS)
        if (bound) context.unbindService(connection)
        replies.quitSafely()
        return when {
            !bound -> "error: bindService returned false"
            !answered -> "error: no reply within 60 s"
            else -> JSONObject(json!!)
        }
    }

    companion object {
        const val TAG = "CarbonEmuProbe"
    }
}
