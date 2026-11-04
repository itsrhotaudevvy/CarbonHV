package dev.carbonhv.host

import org.junit.Assert.assertEquals
import org.junit.Test

class HostDisplayCoordinatorTest {
    @Test
    fun only_the_active_generation_can_touch_the_platform_surface_backend() {
        val backend = RecordingSurfaceBackend<String>()
        val coordinator = HostDisplayCoordinator(backend)
        val live = DisplaySessionIdentity("vm-1", 8)

        assertEquals(true, coordinator.activate(live))
        assertEquals(
            DisplaySessionAdmission.StaleGeneration(expected = 8, got = 7),
            coordinator.attach(DisplaySessionIdentity("vm-1", 7), "stale"),
        )
        assertEquals(emptyList<String>(), backend.attached)

        assertEquals(DisplaySessionAdmission.Authorized, coordinator.attach(live, "surface-a"))
        assertEquals(listOf("surface-a"), backend.attached)
        assertEquals(DisplaySessionAdmission.Authorized, coordinator.detach(live))
        assertEquals(1, backend.detachCount)
    }

    @Test
    fun stale_detach_and_clear_cannot_affect_a_new_active_session() {
        val backend = RecordingSurfaceBackend<String>()
        val coordinator = HostDisplayCoordinator(backend)
        val first = DisplaySessionIdentity("vm-1", 3)
        val next = DisplaySessionIdentity("vm-2", 1)

        coordinator.activate(first)
        coordinator.attach(first, "surface-a")
        assertEquals(true, coordinator.clear(first))
        assertEquals(1, backend.detachCount)
        assertEquals(true, coordinator.activate(next))
        coordinator.attach(next, "surface-b")

        assertEquals(
            DisplaySessionAdmission.DifferentInstance(activeInstanceId = "vm-2"),
            coordinator.detach(first),
        )
        assertEquals(false, coordinator.clear(first))
        assertEquals(1, backend.detachCount)
        assertEquals(listOf("surface-a", "surface-b"), backend.attached)
    }
}

private class RecordingSurfaceBackend<T> : MainSurfaceBackend<T> {
    val attached = mutableListOf<T>()
    var detachCount = 0

    override fun attach(surface: T) {
        attached += surface
    }

    override fun detach() {
        detachCount += 1
    }
}
