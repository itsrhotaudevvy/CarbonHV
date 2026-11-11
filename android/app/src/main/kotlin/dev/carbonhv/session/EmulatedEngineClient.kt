package dev.carbonhv.session

import android.content.Context
import android.content.Intent
import android.view.Surface
import dev.carbonhv.app.VmInstance
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/** Starts the app-sandbox emulator and connects its presenter to a Compose SurfaceView. */
class EmulatedEngineClient(private val context: Context) {
    fun start(instance: VmInstance): Result<Unit> {
        val args = File(context.filesDir, "guests/${instance.id}/qemu.args")
        if (!args.isFile) return Result.failure(IllegalStateException(
            "No staged ${instance.bundleId} image. Import the CarbonHV guest before starting."))
        val intent = Intent(context, EmulatedVmService::class.java).apply {
            putExtra(EmulatedVmService.EXTRA_INSTANCE_ID, instance.id)
        }
        val latch = CountDownLatch(1)
        outcomes[instance.id] = Result.failure(IllegalStateException("Guest start timed out."))
        pending[instance.id] = { outcome ->
            outcomes[instance.id] = outcome
            latch.countDown()
        }
        return try {
            context.startForegroundService(intent)
            if (!latch.await(30, TimeUnit.SECONDS)) {
                Result.failure(IllegalStateException("The emulated engine did not report startup."))
            } else outcomes[instance.id] ?: Result.failure(IllegalStateException("Guest startup failed."))
        } catch (failure: Exception) {
            Result.failure(failure)
        }
    }

    fun stop(instanceId: String) {
        context.startService(Intent(context, EmulatedVmService::class.java).setAction(EmulatedVmService.ACTION_STOP))
    }

    fun callbacks(): GuestSurfaceCallbacks = EmulatedSurfaceCallbacks

    private object EmulatedSurfaceCallbacks : GuestSurfaceCallbacks {
        override fun onSurfaceAvailable(surface: Surface, width: Int, height: Int) = EmulatedNative.setSurface(surface)
        override fun onSurfaceChanged(surface: Surface, width: Int, height: Int) = EmulatedNative.setSurface(surface)
        override fun onSurfaceDestroyed() = EmulatedNative.setSurface(null)
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

    companion object {
        private val pending = ConcurrentHashMap<String, (Result<Unit>) -> Unit>()
        private val outcomes = ConcurrentHashMap<String, Result<Unit>>()

        fun signal(instanceId: String, result: Result<Unit>) {
            pending.remove(instanceId)?.invoke(result)
        }
    }
}
