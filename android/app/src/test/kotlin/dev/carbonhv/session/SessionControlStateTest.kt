package dev.carbonhv.session

import org.junit.Assert.assertEquals
import org.junit.Test

class SessionControlStateTest {
    @Test
    fun unavailable_host_is_presented_and_can_retry() {
        var state: SessionControlState = SessionControlState.Starting

        state = reduceSessionControl(state, SessionControlEvent.HostUnavailable)
        assertEquals(SessionControlState.HostUnavailable, state)

        state = reduceSessionControl(state, SessionControlEvent.Retry)
        assertEquals(SessionControlState.Starting, state)
    }

    @Test
    fun stale_generation_has_explicit_recovery_state() {
        val started = SessionIdentity("vm-1", 4)
        val authoritative = SessionIdentity("vm-1", 5)

        val state = reduceSessionControl(
            SessionControlState.Starting,
            SessionControlEvent.StaleGeneration(started, authoritative),
        )

        assertEquals(SessionControlState.StaleGeneration(started, authoritative), state)
    }

    @Test
    fun runtime_failure_is_presented_and_retry_returns_to_starting() {
        var state: SessionControlState = SessionControlState.Starting
        state = reduceSessionControl(state, SessionControlEvent.RuntimeFailure("AVF failed"))

        assertEquals(SessionControlState.RuntimeFailure("AVF failed"), state)

        state = reduceSessionControl(state, SessionControlEvent.Retry)
        assertEquals(SessionControlState.Starting, state)
    }

    @Test
    fun successful_start_enters_running_state() {
        val identity = SessionIdentity("vm-1", 6)

        val state = reduceSessionControl(
            SessionControlState.Starting,
            SessionControlEvent.Running(identity),
        )

        assertEquals(SessionControlState.Running(identity), state)
    }
}
