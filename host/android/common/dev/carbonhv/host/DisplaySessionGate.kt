package dev.carbonhv.host

data class DisplaySessionIdentity(
    val instanceId: String,
    val generation: Long,
)

sealed interface DisplaySessionAdmission {
    data object Authorized : DisplaySessionAdmission
    data object NoActiveSession : DisplaySessionAdmission
    data class DifferentInstance(val activeInstanceId: String) : DisplaySessionAdmission
    data class StaleGeneration(val expected: Long, val got: Long) : DisplaySessionAdmission
}

class DisplaySessionGate {
    private var active: DisplaySessionIdentity? = null

    @Synchronized
    fun current(): DisplaySessionIdentity? = active

    @Synchronized
    fun activate(identity: DisplaySessionIdentity): Boolean {
        val current = active
        if (current != null && current != identity) return false
        active = identity
        return true
    }

    @Synchronized
    fun clear(identity: DisplaySessionIdentity): Boolean {
        if (active != identity) return false
        active = null
        return true
    }

    @Synchronized
    fun admit(requested: DisplaySessionIdentity): DisplaySessionAdmission {
        val current = active ?: return DisplaySessionAdmission.NoActiveSession
        if (current.instanceId != requested.instanceId) {
            return DisplaySessionAdmission.DifferentInstance(current.instanceId)
        }
        if (current.generation != requested.generation) {
            return DisplaySessionAdmission.StaleGeneration(
                expected = current.generation,
                got = requested.generation,
            )
        }
        return DisplaySessionAdmission.Authorized
    }
}
