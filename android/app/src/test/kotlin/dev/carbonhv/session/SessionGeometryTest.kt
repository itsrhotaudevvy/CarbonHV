package dev.carbonhv.session

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionGeometryTest {
    @Test
    fun fit_geometry_letterboxes_and_maps_touch_into_guest_space() {
        val geometry = fitGuestGeometry(
            viewportWidth = 1080,
            viewportHeight = 2400,
            guestWidth = 1080,
            guestHeight = 1920,
            generation = 4,
        )

        assertEquals(0f, geometry.offsetX, 0.01f)
        assertEquals(240f, geometry.offsetY, 0.01f)
        assertEquals(1080f, geometry.drawWidth, 0.01f)
        assertEquals(1920f, geometry.drawHeight, 0.01f)

        assertEquals(
            MappedTouch(540f, 960f, 4),
            geometry.mapTouch(hostX = 540f, hostY = 1200f, eventGeneration = 4),
        )
        assertNull(geometry.mapTouch(hostX = 540f, hostY = 100f, eventGeneration = 4))
    }

    @Test
    fun stale_geometry_generation_cancels_touch() {
        val geometry = fitGuestGeometry(1000, 2000, 1000, 2000, generation = 7)

        assertNull(geometry.mapTouch(100f, 100f, eventGeneration = 6))
        assertEquals(MappedTouch(100f, 100f, 7), geometry.mapTouch(100f, 100f, 7))
    }

    @Test
    fun touch_on_exclusive_right_or_bottom_edge_is_rejected() {
        val geometry = fitGuestGeometry(1080, 1920, 1080, 1920, generation = 3)

        assertNull(geometry.mapTouch(1080f, 100f, eventGeneration = 3))
        assertNull(geometry.mapTouch(100f, 1920f, eventGeneration = 3))
        val nearEdge = geometry.mapTouch(1079f, 1919f, eventGeneration = 3)
        assertTrue(requireNotNull(nearEdge).x < 1080f)
        assertTrue(nearEdge.y < 1920f)
    }

    @Test
    fun geometry_tracker_increments_generation_only_when_dimensions_change() {
        val tracker = GeometryTracker()
        val first = tracker.update(1080, 2400, 1080, 1920)
        val same = tracker.update(1080, 2400, 1080, 1920)
        val rotated = tracker.update(2400, 1080, 1920, 1080)

        assertEquals(first.generation, same.generation)
        assertTrue(rotated.generation > same.generation)
    }

    @Test
    fun host_escape_and_system_controls_remain_host_owned() {
        assertEquals(InputOwner.Host, SessionInputPolicy.ownerOf(SessionInput.EscapeHandle))
        assertEquals(InputOwner.Host, SessionInputPolicy.ownerOf(SessionInput.SystemHome))
        assertEquals(InputOwner.Host, SessionInputPolicy.ownerOf(SessionInput.PowerButton))
        assertEquals(InputOwner.Guest, SessionInputPolicy.ownerOf(SessionInput.ViewportTouch))
        assertEquals(InputOwner.Guest, SessionInputPolicy.ownerOf(SessionInput.GuestBack))
    }

    @Test
    fun reconnect_attaches_only_to_matching_live_generation() {
        assertEquals(
            ReconnectDecision.Attach,
            decideReconnect(
                requested = SessionIdentity("vm-1", 9),
                brokerLive = SessionIdentity("vm-1", 9),
            ),
        )
        assertEquals(
            ReconnectDecision.Reconcile,
            decideReconnect(
                requested = SessionIdentity("vm-1", 8),
                brokerLive = SessionIdentity("vm-1", 9),
            ),
        )
        assertEquals(
            ReconnectDecision.ShowStopped,
            decideReconnect(SessionIdentity("vm-1", 9), brokerLive = null),
        )
    }
}
