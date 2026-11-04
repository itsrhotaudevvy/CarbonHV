package dev.carbonhv.session

import android.graphics.SurfaceTexture
import android.view.Surface
import dev.carbonhv.host.ICarbonHvHostService
import java.lang.reflect.InvocationHandler
import java.lang.reflect.Proxy
import org.junit.Assert.assertEquals
import org.junit.Test

class PlatformHostDisplaySurfaceClientTest {
    @Test
    fun forwards_exact_session_identity_and_surface_to_privileged_host_service() {
        val recorder = RecordingHostService()
        val service = Proxy.newProxyInstance(
            ICarbonHvHostService::class.java.classLoader,
            arrayOf(ICarbonHvHostService::class.java),
            recorder,
        ) as ICarbonHvHostService
        val client = PlatformHostDisplaySurfaceClient(service)
        val session = SessionIdentity("vm-7", 42)
        val surface = Surface(SurfaceTexture(0))

        client.attachMainSurface(session, surface)
        client.detachMainSurface(session)

        assertEquals(listOf("vm-7" to 42L), recorder.attachSessions)
        assertEquals(listOf(surface), recorder.attachSurfaces)
        assertEquals(listOf("vm-7" to 42L), recorder.detachSessions)
    }
}

private class RecordingHostService : InvocationHandler {
    val attachSessions = mutableListOf<Pair<String, Long>>()
    val attachSurfaces = mutableListOf<Surface>()
    val detachSessions = mutableListOf<Pair<String, Long>>()

    override fun invoke(proxy: Any, method: java.lang.reflect.Method, args: Array<out Any?>?): Any? {
        val values = requireNotNull(args)
        when (method.name) {
            "attachMainSurface" -> {
                attachSessions += values[0] as String to values[1] as Long
                attachSurfaces += values[2] as Surface
            }
            "detachMainSurface" -> {
                detachSessions += values[0] as String to values[1] as Long
            }
        }
        return null
    }
}
