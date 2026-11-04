package dev.carbonhv.session

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.graphics.Color
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color as ComposeColor
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import kotlin.math.roundToInt

interface GuestSurfaceCallbacks {
    fun onSurfaceAvailable(surface: Surface, width: Int, height: Int)
    fun onSurfaceChanged(surface: Surface, width: Int, height: Int)
    fun onSurfaceDestroyed()
    fun onTouch(
        action: Int,
        actionPointerId: Int,
        viewportWidth: Int,
        viewportHeight: Int,
        pointerIds: IntArray,
        xs: FloatArray,
        ys: FloatArray,
    ): Boolean
    fun onKey(linuxKeyCode: Int, down: Boolean): Boolean
}

object NoopGuestSurfaceCallbacks : GuestSurfaceCallbacks {
    override fun onSurfaceAvailable(surface: Surface, width: Int, height: Int) = Unit
    override fun onSurfaceChanged(surface: Surface, width: Int, height: Int) = Unit
    override fun onSurfaceDestroyed() = Unit
    override fun onTouch(
        action: Int,
        actionPointerId: Int,
        viewportWidth: Int,
        viewportHeight: Int,
        pointerIds: IntArray,
        xs: FloatArray,
        ys: FloatArray,
    ): Boolean = false
    override fun onKey(linuxKeyCode: Int, down: Boolean): Boolean = false
}

@Composable
fun ImmersiveSessionScreen(
    instanceName: String,
    callbacks: GuestSurfaceCallbacks = NoopGuestSurfaceCallbacks,
    onLeave: () -> Unit,
    onPause: () -> Unit = {},
    onStop: () -> Unit = {},
    onGuestBack: () -> Unit = {},
    onGuestHome: () -> Unit = {},
    onGuestOverview: () -> Unit = {},
) {
    val context = LocalContext.current
    val activity = context.findActivity()
    var controlsVisible by remember { mutableStateOf(false) }
    var handleX by remember { mutableStateOf(0f) }
    var handleY by remember { mutableStateOf(0f) }
    DisposableEffect(activity) {
        if (activity != null) {
            val controller = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
            controller.hide(WindowInsetsCompat.Type.systemBars())
            controller.systemBarsBehavior =
                WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
        onDispose {
            if (activity != null) {
                WindowCompat.getInsetsController(activity.window, activity.window.decorView)
                    .show(WindowInsetsCompat.Type.systemBars())
            }
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(ComposeColor.Black),
    ) {
        key(callbacks) {
            AndroidView(
                modifier = Modifier.fillMaxSize(),
                factory = { viewContext ->
                    SurfaceView(viewContext).apply {
                        setBackgroundColor(Color.BLACK)
                        keepScreenOn = true
                        isFocusable = true
                        isFocusableInTouchMode = true
                        setOnTouchListener { view, event ->
                            val pointerCount = event.pointerCount
                            val pointerIds = IntArray(pointerCount) { index -> event.getPointerId(index) }
                            val xs = FloatArray(pointerCount) { index -> event.getX(index) }
                            val ys = FloatArray(pointerCount) { index -> event.getY(index) }
                            callbacks.onTouch(
                                action = event.actionMasked,
                                actionPointerId = event.getPointerId(event.actionIndex),
                                viewportWidth = view.width.coerceAtLeast(1),
                                viewportHeight = view.height.coerceAtLeast(1),
                                pointerIds = pointerIds,
                                xs = xs,
                                ys = ys,
                            )
                        }
                        setOnKeyListener { _, keyCode, event ->
                            if (event == null || isHostOwnedHardwareKey(keyCode)) {
                                false
                            } else {
                                val scanCode = event.scanCode
                                scanCode > 0 && callbacks.onKey(
                                    linuxKeyCode = scanCode,
                                    down = event.action != KeyEvent.ACTION_UP,
                                )
                            }
                        }
                        holder.addCallback(object : SurfaceHolder.Callback {
                            override fun surfaceCreated(holder: SurfaceHolder) {
                                requestFocus()
                                callbacks.onSurfaceAvailable(
                                    holder.surface,
                                    width.coerceAtLeast(1),
                                    height.coerceAtLeast(1),
                                )
                            }

                            override fun surfaceChanged(
                                holder: SurfaceHolder,
                                format: Int,
                                width: Int,
                                height: Int,
                            ) {
                                callbacks.onSurfaceChanged(holder.surface, width, height)
                            }

                            override fun surfaceDestroyed(holder: SurfaceHolder) {
                                callbacks.onSurfaceDestroyed()
                            }
                        })
                    }
                },
            )
        }

        Button(
            onClick = { controlsVisible = !controlsVisible },
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(12.dp)
                .wrapContentSize()
                .then(
                    Modifier
                        .pointerInput(Unit) {
                            detectDragGestures { change, dragAmount ->
                                change.consume()
                                handleX = (handleX + dragAmount.x).coerceIn(-300f, 0f)
                                handleY = (handleY + dragAmount.y).coerceIn(0f, 500f)
                            }
                        },
                )
                .then(Modifier.wrapContentSize())
                .then(
                    Modifier.offsetCompat(handleX, handleY),
                ),
        ) {
            Text("Host")
        }

        if (controlsVisible) {
            Card(
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .padding(top = 72.dp, start = 12.dp, end = 12.dp),
            ) {
                Column(
                    modifier = Modifier.padding(14.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(instanceName)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = onGuestBack) { Text("Back") }
                        Button(onClick = onGuestHome) { Text("Home") }
                        Button(onClick = onGuestOverview) { Text("Overview") }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = onPause) { Text("Pause") }
                        Button(onClick = onStop) { Text("Stop") }
                        Button(onClick = onLeave) { Text("Android") }
                    }
                }
            }
        }
    }
}

private fun Context.findActivity(): Activity? {
    var current: Context? = this
    while (current is ContextWrapper) {
        if (current is Activity) return current
        current = current.baseContext
    }
    return current as? Activity
}

private fun Modifier.offsetCompat(x: Float, y: Float): Modifier =
    this.then(
        Modifier.offset {
            IntOffset(x.roundToInt(), y.roundToInt())
        },
    )

private fun isHostOwnedHardwareKey(keyCode: Int): Boolean = when (keyCode) {
    KeyEvent.KEYCODE_VOLUME_UP,
    KeyEvent.KEYCODE_VOLUME_DOWN,
    KeyEvent.KEYCODE_VOLUME_MUTE,
    KeyEvent.KEYCODE_POWER -> true
    else -> false
}
