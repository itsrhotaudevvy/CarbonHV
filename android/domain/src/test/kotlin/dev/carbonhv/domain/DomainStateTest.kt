package dev.carbonhv.domain

import org.junit.Assert.assertEquals
import org.junit.Test

class DomainStateTest {
    @Test
    fun admission_labels_are_truthful_about_acceleration() {
        assertEquals(
            "Ready for accelerated sessions",
            AdmissionText.forStatus(HostStatus.ReadyAccelerated),
        )
        assertEquals(
            "CPU acceleration available; graphics experimental",
            AdmissionText.forStatus(HostStatus.CpuAcceleratedGraphicsExperimental),
        )
        assertEquals(
            "This host does not allow full-system VMs",
            AdmissionText.forStatus(HostStatus.Unsupported),
        )
    }

    @Test
    fun session_reducer_tracks_start_pause_resume_and_stop() {
        var state = SessionUiState.Stopped
        state = reduceSession(state, SessionEvent.StartRequested)
        assertEquals(SessionUiState.Starting, state)
        state = reduceSession(state, SessionEvent.GuestReady)
        assertEquals(SessionUiState.Running, state)
        state = reduceSession(state, SessionEvent.PauseRequested)
        assertEquals(SessionUiState.Paused, state)
        state = reduceSession(state, SessionEvent.ResumeRequested)
        assertEquals(SessionUiState.Running, state)
        state = reduceSession(state, SessionEvent.StopRequested)
        assertEquals(SessionUiState.Stopping, state)
        state = reduceSession(state, SessionEvent.GuestStopped)
        assertEquals(SessionUiState.Stopped, state)
    }

    @Test
    fun invalid_ui_transition_keeps_current_state() {
        assertEquals(
            SessionUiState.Stopped,
            reduceSession(SessionUiState.Stopped, SessionEvent.PauseRequested),
        )
    }

    @Test
    fun recovery_actions_preserve_instance_on_host_incompatibility() {
        assertEquals(
            listOf(RecoveryAction.ViewDiagnostics, RecoveryAction.KeepInstance),
            RecoveryPolicy.actionsFor(FailureKind.HostUpdateIncompatible),
        )
        assertEquals(
            listOf(RecoveryAction.Retry, RecoveryAction.ViewDiagnostics),
            RecoveryPolicy.actionsFor(FailureKind.GuestBootFailed),
        )
    }
}
