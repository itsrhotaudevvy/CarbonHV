package dev.carbonhv.emuviewer

import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.view.Gravity
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.TextView

/** Full-screen view of the guest. The label says what this is, on every frame. */
class ViewerActivity : Activity(), SurfaceHolder.Callback {
    private lateinit var status: TextView
    private val tick = object : Runnable {
        override fun run() {
            val (received, shown) = Native.counters().let { it[0] to it[1] }
            val running = if (ViewerService.pid != 0) "running" else "stopped"
            status.text = "AOSP Cuttlefish stand-in · NOT GrapheneOS · Emulated (QEMU TCG)\n" +
                "emulator $running · frames $received received, $shown shown"
            status.postDelayed(this, 1000)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        val surface = SurfaceView(this).also { it.holder.addCallback(this) }
        status = TextView(this).apply {
            setTextColor(Color.WHITE)
            setBackgroundColor(0x99000000.toInt())
            textSize = 11f
            setPadding(16, 8, 16, 8)
        }
        setContentView(FrameLayout(this).apply {
            setBackgroundColor(Color.BLACK)
            addView(surface, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
            addView(status, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.TOP))
        })
        startForegroundService(Intent(this, ViewerService::class.java))
    }

    override fun onResume() {
        super.onResume()
        status.post(tick)
    }

    override fun onPause() {
        status.removeCallbacks(tick)
        super.onPause()
    }

    override fun surfaceCreated(holder: SurfaceHolder) = Native.setSurface(holder.surface)
    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) =
        Native.setSurface(holder.surface)
    override fun surfaceDestroyed(holder: SurfaceHolder) = Native.setSurface(null)
}
