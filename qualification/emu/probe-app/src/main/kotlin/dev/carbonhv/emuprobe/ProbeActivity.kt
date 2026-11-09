package dev.carbonhv.emuprobe

import android.app.Activity
import android.os.Bundle
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import kotlin.concurrent.thread

/** Foreground (top-app) entry point; it also shows the AHB the child produced. */
class ProbeActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = TextView(this).apply { textSize = 11f; setPadding(32, 32, 32, 32) }
        val image = ImageView(this).apply { minimumHeight = 256 }
        setContentView(
            ScrollView(this).apply {
                addView(
                    LinearLayout(this@ProbeActivity).apply {
                        orientation = LinearLayout.VERTICAL
                        addView(image)
                        addView(text)
                    },
                )
            },
        )
        val runner = ProbeRunner(
            this,
            entryPoint = "activity (top-app)",
            status = { runOnUiThread { text.text = it } },
            show = { runOnUiThread { image.setImageBitmap(it) } },
        )
        thread(name = "probe-main") { runner.runAll() }
    }
}
