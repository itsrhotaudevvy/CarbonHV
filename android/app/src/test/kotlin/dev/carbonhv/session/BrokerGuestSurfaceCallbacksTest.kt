package dev.carbonhv.session

import android.graphics.SurfaceTexture
import android.view.Surface
import org.junit.Assert.assertEquals
import org.junit.Test

class BrokerGuestSurfaceCallbacksTest {
    @Test
    fun lifecycle_hands_main_surface_to_host_once_and_detaches_once() {
        val client = FakeHostDisplaySurfaceClient()
        val session = SessionIdentity("vm-1", 7)
        val callbacks = BrokerGuestSurfaceCallbacks(session, client)
        val surface = Surface(SurfaceTexture(0))

        callbacks.onSurfaceAvailable(surface, 1080, 2400)
        callbacks.onSurfaceChanged(surface, 2400, 1080)
        callbacks.onSurfaceDestroyed()
        callbacks.onSurfaceDestroyed()

        assertEquals(listOf(session), client.attachedSessions)
        assertEquals(listOf(surface), client.attachedSurfaces)
        assertEquals(listOf(session), client.detachedSessions)
    }

    @Test
    fun recreated_surface_is_attached_as_a_new_surface_lifecycle() {
        val client = FakeHostDisplaySurfaceClient()
        val session = SessionIdentity("vm-1", 9)
        val callbacks = BrokerGuestSurfaceCallbacks(session, client)
        val first = Surface(SurfaceTexture(0))
        val second = Surface(SurfaceTexture(0))

        callbacks.onSurfaceAvailable(first, 1080, 2400)
        callbacks.onSurfaceDestroyed()
        callbacks.onSurfaceAvailable(second, 1080, 2400)

        assertEquals(listOf(first, second), client.attachedSurfaces)
        assertEquals(listOf(session), client.detachedSessions)
    }
}

private class FakeHostDisplaySurfaceClient : HostDisplaySurfaceClient {
    val attachedSessions = mutableListOf<SessionIdentity>()
    val attachedSurfaces = mutableListOf<Surface>()
    val detachedSessions = mutableListOf<SessionIdentity>()

    override fun attachMainSurface(session: SessionIdentity, surface: Surface) {
        attachedSessions += session
        attachedSurfaces += surface
    }

    override fun detachMainSurface(session: SessionIdentity) {
        detachedSessions += session
    }
}
