package dev.carbonhv.domain

enum class SessionUiState {
    Stopped,
    Starting,
    Running,
    Paused,
    Stopping,
    Crashed,
    Recovering,
}

enum class SessionEvent {
    StartRequested,
    GuestReady,
    PauseRequested,
    ResumeRequested,
    StopRequested,
    GuestStopped,
    GuestCrashed,
    RecoveryStarted,
    RecoveryFinished,
}

fun reduceSession(state: SessionUiState, event: SessionEvent): SessionUiState = when (state to event) {
    SessionUiState.Stopped to SessionEvent.StartRequested -> SessionUiState.Starting
    SessionUiState.Starting to SessionEvent.GuestReady -> SessionUiState.Running
    SessionUiState.Running to SessionEvent.PauseRequested -> SessionUiState.Paused
    SessionUiState.Paused to SessionEvent.ResumeRequested -> SessionUiState.Running
    SessionUiState.Running to SessionEvent.StopRequested -> SessionUiState.Stopping
    SessionUiState.Paused to SessionEvent.StopRequested -> SessionUiState.Stopping
    SessionUiState.Stopping to SessionEvent.GuestStopped -> SessionUiState.Stopped
    SessionUiState.Starting to SessionEvent.GuestCrashed -> SessionUiState.Crashed
    SessionUiState.Running to SessionEvent.GuestCrashed -> SessionUiState.Crashed
    SessionUiState.Paused to SessionEvent.GuestCrashed -> SessionUiState.Crashed
    SessionUiState.Crashed to SessionEvent.RecoveryStarted -> SessionUiState.Recovering
    SessionUiState.Recovering to SessionEvent.RecoveryFinished -> SessionUiState.Stopped
    else -> state
}

