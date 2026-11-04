package dev.carbonhv.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HostVmRuntimeCoordinatorTest {
    @Test
    fun failed_start_rolls_back_display_ownership() {
        val display = RecordingDisplayAuthority()
        val backend = RecordingVmBackend(failStart = true)
        val coordinator = HostVmRuntimeCoordinator(display, backend)
        val identity = DisplaySessionIdentity("vm-1", 4)

        val failure = runCatching { coordinator.start(identity, "config-a") }.exceptionOrNull()

        assertEquals("start failed", failure?.message)
        assertEquals(null, coordinator.current())
        assertEquals(listOf(identity), display.activated)
        assertEquals(listOf(identity), display.cleared)
    }

    @Test
    fun stale_backend_death_cannot_clear_a_newer_generation() {
        val display = RecordingDisplayAuthority()
        val backend = RecordingVmBackend()
        val coordinator = HostVmRuntimeCoordinator(display, backend)
        val first = DisplaySessionIdentity("vm-1", 4)
        val next = DisplaySessionIdentity("vm-1", 5)

        assertEquals(HostVmStartResult.Started, coordinator.start(first, "config-a"))
        assertTrue(coordinator.stop(first))
        assertEquals(HostVmStartResult.Started, coordinator.start(next, "config-b"))

        backend.fireDied(first)

        assertEquals(next, coordinator.current())
        assertEquals(next, display.current())
        assertEquals(listOf(first), display.cleared)
    }

    @Test
    fun death_observer_receives_only_the_exact_active_generation() {
        val display = RecordingDisplayAuthority()
        val backend = RecordingVmBackend()
        val deaths = mutableListOf<DisplaySessionIdentity>()
        val coordinator = HostVmRuntimeCoordinator(display, backend) { identity -> deaths += identity }
        val first = DisplaySessionIdentity("vm-1", 4)
        val next = DisplaySessionIdentity("vm-1", 5)

        assertEquals(HostVmStartResult.Started, coordinator.start(first, "config-a"))
        assertTrue(coordinator.stop(first))
        assertEquals(HostVmStartResult.Started, coordinator.start(next, "config-b"))

        backend.fireDied(first)
        assertTrue(deaths.isEmpty())

        backend.fireDied(next)
        assertEquals(listOf(next), deaths)
        assertEquals(null, coordinator.current())
    }

    @Test
    fun only_active_generation_can_forward_guest_input() {
        val display = RecordingDisplayAuthority()
        val backend = RecordingVmBackend()
        val coordinator = HostVmRuntimeCoordinator(display, backend)
        val live = DisplaySessionIdentity("vm-9", 12)

        assertEquals(HostVmStartResult.Started, coordinator.start(live, "config"))
        assertFalse(
            coordinator.sendInput(
                DisplaySessionIdentity("vm-9", 11),
                HostGuestInput.Key(linuxKeyCode = 158, down = true),
            ),
        )
        assertTrue(
            coordinator.sendInput(
                live,
                HostGuestInput.Key(linuxKeyCode = 158, down = true),
            ),
        )

        assertEquals(
            listOf(live to HostGuestInput.Key(linuxKeyCode = 158, down = true)),
            backend.inputs,
        )
    }

    @Test
    fun duplicate_start_is_idempotent_but_other_session_is_busy() {
        val display = RecordingDisplayAuthority()
        val backend = RecordingVmBackend()
        val coordinator = HostVmRuntimeCoordinator(display, backend)
        val live = DisplaySessionIdentity("vm-1", 8)

        assertEquals(HostVmStartResult.Started, coordinator.start(live, "config-a"))
        assertEquals(HostVmStartResult.AlreadyActive, coordinator.start(live, "config-a"))
        assertEquals(
            HostVmStartResult.Busy(live),
            coordinator.start(DisplaySessionIdentity("vm-2", 1), "config-b"),
        )
        assertEquals(listOf(live to "config-a"), backend.started)
    }
}

private class RecordingDisplayAuthority : DisplaySessionAuthority {
    private var active: DisplaySessionIdentity? = null
    val activated = mutableListOf<DisplaySessionIdentity>()
    val cleared = mutableListOf<DisplaySessionIdentity>()

    override fun current(): DisplaySessionIdentity? = active

    override fun activate(identity: DisplaySessionIdentity): Boolean {
        if (active != null && active != identity) return false
        active = identity
        activated += identity
        return true
    }

    override fun clear(identity: DisplaySessionIdentity): Boolean {
        if (active != identity) return false
        active = null
        cleared += identity
        return true
    }
}

private class RecordingVmBackend(
    private val failStart: Boolean = false,
) : GraphicalVmRuntimeBackend<String> {
    val started = mutableListOf<Pair<DisplaySessionIdentity, String>>()
    val inputs = mutableListOf<Pair<DisplaySessionIdentity, HostGuestInput>>()
    private val deathCallbacks = mutableMapOf<DisplaySessionIdentity, () -> Unit>()

    override fun start(
        identity: DisplaySessionIdentity,
        config: String,
        onDied: () -> Unit,
    ) {
        if (failStart) error("start failed")
        started += identity to config
        deathCallbacks[identity] = onDied
    }

    override fun stop(identity: DisplaySessionIdentity) = Unit

    override fun sendInput(identity: DisplaySessionIdentity, input: HostGuestInput): Boolean {
        inputs += identity to input
        return true
    }

    fun fireDied(identity: DisplaySessionIdentity) {
        deathCallbacks[identity]?.invoke()
    }
}
