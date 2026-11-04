package dev.carbonhv.session

import android.graphics.SurfaceTexture
import android.view.Surface
import dev.carbonhv.host.ICarbonHvHostService
import java.lang.reflect.InvocationHandler
import java.lang.reflect.Proxy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class HostSessionBridgeTest {
    @Test
    fun creates_surface_callbacks_only_for_host_reported_live_generation() {
        val calls = mutableListOf<String>()
        val service = hostService(liveGeneration = 23L, calls = calls)
        val bridge = HostSessionBridge(service)
        val callbacks = bridge.callbacksFor("vm-1")
        val surface = Surface(SurfaceTexture(0))

        requireNotNull(callbacks).onSurfaceAvailable(surface, 1080, 1920)
        callbacks.onSurfaceDestroyed()

        assertEquals(
            listOf("attach:vm-1:23", "detach:vm-1:23"),
            calls,
        )
    }

    @Test
    fun stopped_instance_gets_no_surface_callbacks() {
        val bridge = HostSessionBridge(hostService(liveGeneration = null, calls = mutableListOf()))

        assertNull(bridge.callbacksFor("vm-1"))
    }
}

private fun hostService(liveGeneration: Long?, calls: MutableList<String>): ICarbonHvHostService {
    val handler = InvocationHandler { _, method, args ->
        when (method.name) {
            "activeDisplayGeneration" -> liveGeneration ?: -1L
            "attachMainSurface" -> {
                calls += "attach:${args!![0]}:${args[1]}"
                null
            }
            "detachMainSurface" -> {
                calls += "detach:${args!![0]}:${args[1]}"
                null
            }
            else -> null
        }
    }
    return Proxy.newProxyInstance(
        ICarbonHvHostService::class.java.classLoader,
        arrayOf(ICarbonHvHostService::class.java),
        handler,
    ) as ICarbonHvHostService
}
