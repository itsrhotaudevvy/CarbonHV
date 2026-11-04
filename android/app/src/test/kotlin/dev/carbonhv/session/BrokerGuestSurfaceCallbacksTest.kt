package dev.carbonhv.session

import android.graphics.SurfaceTexture
import android.view.Surface
import org.junit.Assert.assertEquals
import org.junit.Test

class BrokerGuestSurfaceCallbacksTest {
    @Test
    fun lifecycle_hands_main_surface_to_host_once_and_detaches_once() {
        val client = FakeHostDisplaySurfaceClient()
        val input = FakeHostGuestInputClient()
        val session = SessionIdentity("vm-1", 7)
        val callbacks = BrokerGuestSurfaceCallbacks(session, client, input)
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
        val input = FakeHostGuestInputClient()
        val session = SessionIdentity("vm-1", 9)
        val callbacks = BrokerGuestSurfaceCallbacks(session, client, input)
        val first = Surface(SurfaceTexture(0))
        val second = Surface(SurfaceTexture(0))

        callbacks.onSurfaceAvailable(first, 1080, 2400)
        callbacks.onSurfaceDestroyed()
        callbacks.onSurfaceAvailable(second, 1080, 2400)

        assertEquals(listOf(first, second), client.attachedSurfaces)
        assertEquals(listOf(session), client.detachedSessions)
    }

    @Test
    fun input_uses_the_same_generation_scoped_session_as_the_surface() {
        val display = FakeHostDisplaySurfaceClient()
        val input = FakeHostGuestInputClient()
        val session = SessionIdentity("vm-3", 11)
        val callbacks = BrokerGuestSurfaceCallbacks(session, display, input)

        callbacks.onKey(158, down = true)
        callbacks.onTouch(
            action = 0,
            actionPointerId = 4,
            viewportWidth = 1080,
            viewportHeight = 2400,
            pointerIds = intArrayOf(4),
            xs = floatArrayOf(500f),
            ys = floatArrayOf(600f),
        )

        assertEquals(listOf(session), input.keySessions)
        assertEquals(listOf(session), input.touchSessions)
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

private class FakeHostGuestInputClient : HostGuestInputClient {
    val keySessions = mutableListOf<SessionIdentity>()
    val touchSessions = mutableListOf<SessionIdentity>()

    override fun sendKey(session: SessionIdentity, linuxKeyCode: Int, down: Boolean): Boolean {
        keySessions += session
        return true
    }

    override fun sendTouch(
        session: SessionIdentity,
        action: Int,
        actionPointerId: Int,
        viewportWidth: Int,
        viewportHeight: Int,
        pointerIds: IntArray,
        xs: FloatArray,
        ys: FloatArray,
    ): Boolean {
        touchSessions += session
        return true
    }
}
