package dev.carbonhv.session

sealed interface SessionControlState {
    data object Starting : SessionControlState
    data class Running(val identity: SessionIdentity) : SessionControlState
    data object HostUnavailable : SessionControlState
    data class StaleGeneration(
        val started: SessionIdentity,
        val authoritative: SessionIdentity?,
    ) : SessionControlState
    data class RuntimeFailure(val message: String) : SessionControlState
}

sealed interface SessionControlEvent {
    data object HostUnavailable : SessionControlEvent
    data class Running(val identity: SessionIdentity) : SessionControlEvent
    data class StaleGeneration(
        val started: SessionIdentity,
        val authoritative: SessionIdentity?,
    ) : SessionControlEvent
    data class RuntimeFailure(val message: String) : SessionControlEvent
    data object Retry : SessionControlEvent
}

fun reduceSessionControl(
    state: SessionControlState,
    event: SessionControlEvent,
): SessionControlState = when (event) {
    SessionControlEvent.HostUnavailable -> SessionControlState.HostUnavailable
    is SessionControlEvent.Running -> SessionControlState.Running(event.identity)
    is SessionControlEvent.StaleGeneration ->
        SessionControlState.StaleGeneration(event.started, event.authoritative)
    is SessionControlEvent.RuntimeFailure -> SessionControlState.RuntimeFailure(event.message)
    SessionControlEvent.Retry -> when (state) {
        SessionControlState.HostUnavailable,
        is SessionControlState.StaleGeneration,
        is SessionControlState.RuntimeFailure -> SessionControlState.Starting
        else -> state
    }
}
