package dev.carbonhv.session

data class SessionIdentity(
    val instanceId: String,
    val generation: Int,
)

enum class ReconnectDecision {
    Attach,
    Reconcile,
    ShowStopped,
}

fun decideReconnect(
    requested: SessionIdentity,
    brokerLive: SessionIdentity?,
): ReconnectDecision = when {
    brokerLive == null -> ReconnectDecision.ShowStopped
    requested == brokerLive -> ReconnectDecision.Attach
    else -> ReconnectDecision.Reconcile
}

