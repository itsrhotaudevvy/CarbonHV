package dev.carbonhv.host

import org.junit.Assert.assertEquals
import org.junit.Test

class DisplaySessionGateTest {
    @Test
    fun exact_live_generation_is_authorized_and_stale_generation_is_rejected() {
        val gate = DisplaySessionGate()
        gate.activate(DisplaySessionIdentity("vm-1", 7))

        assertEquals(
            DisplaySessionAdmission.Authorized,
            gate.admit(DisplaySessionIdentity("vm-1", 7)),
        )
        assertEquals(
            DisplaySessionAdmission.StaleGeneration(expected = 7, got = 6),
            gate.admit(DisplaySessionIdentity("vm-1", 6)),
        )
    }

    @Test
    fun absent_or_different_live_session_fails_closed() {
        val gate = DisplaySessionGate()

        assertEquals(
            DisplaySessionAdmission.NoActiveSession,
            gate.admit(DisplaySessionIdentity("vm-1", 1)),
        )

        gate.activate(DisplaySessionIdentity("vm-2", 4))
        assertEquals(
            DisplaySessionAdmission.DifferentInstance(activeInstanceId = "vm-2"),
            gate.admit(DisplaySessionIdentity("vm-1", 4)),
        )
    }

    @Test
    fun stale_clear_cannot_revoke_a_newer_generation() {
        val gate = DisplaySessionGate()
        gate.activate(DisplaySessionIdentity("vm-1", 8))

        assertEquals(false, gate.clear(DisplaySessionIdentity("vm-1", 7)))
        assertEquals(
            DisplaySessionAdmission.Authorized,
            gate.admit(DisplaySessionIdentity("vm-1", 8)),
        )
        assertEquals(true, gate.clear(DisplaySessionIdentity("vm-1", 8)))
        assertEquals(
            DisplaySessionAdmission.NoActiveSession,
            gate.admit(DisplaySessionIdentity("vm-1", 8)),
        )
    }

    @Test
    fun active_identity_cannot_be_replaced_until_exactly_cleared() {
        val gate = DisplaySessionGate()
        val first = DisplaySessionIdentity("vm-1", 3)
        val second = DisplaySessionIdentity("vm-2", 1)

        assertEquals(true, gate.activate(first))
        assertEquals(false, gate.activate(second))
        assertEquals(false, gate.activate(DisplaySessionIdentity("vm-1", 4)))
        assertEquals(
            DisplaySessionAdmission.Authorized,
            gate.admit(first),
        )
        assertEquals(true, gate.clear(first))
        assertEquals(true, gate.activate(second))
        assertEquals(
            DisplaySessionAdmission.Authorized,
            gate.admit(second),
        )
    }

    @Test
    fun current_identity_tracks_only_the_exact_active_runtime_session() {
        val gate = DisplaySessionGate()
        val live = DisplaySessionIdentity("vm-9", 12)

        assertEquals(null, gate.current())
        gate.activate(live)
        assertEquals(live, gate.current())
        gate.clear(DisplaySessionIdentity("vm-9", 11))
        assertEquals(live, gate.current())
        gate.clear(live)
        assertEquals(null, gate.current())
    }
}
