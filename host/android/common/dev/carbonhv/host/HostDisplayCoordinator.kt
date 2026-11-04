package dev.carbonhv.host

interface MainSurfaceBackend<T> {
    fun attach(surface: T)
    fun detach()
}

class HostDisplayCoordinator<T>(
    private val backend: MainSurfaceBackend<T>,
    private val gate: DisplaySessionGate = DisplaySessionGate(),
) : DisplaySessionAuthority {
    @Synchronized
    override fun current(): DisplaySessionIdentity? = gate.current()

    @Synchronized
    override fun activate(identity: DisplaySessionIdentity): Boolean = gate.activate(identity)

    @Synchronized
    override fun clear(identity: DisplaySessionIdentity): Boolean {
        if (gate.admit(identity) != DisplaySessionAdmission.Authorized) return false
        backend.detach()
        return gate.clear(identity)
    }

    @Synchronized
    fun attach(identity: DisplaySessionIdentity, surface: T): DisplaySessionAdmission {
        val admission = gate.admit(identity)
        if (admission == DisplaySessionAdmission.Authorized) {
            backend.attach(surface)
        }
        return admission
    }

    @Synchronized
    fun detach(identity: DisplaySessionIdentity): DisplaySessionAdmission {
        val admission = gate.admit(identity)
        if (admission == DisplaySessionAdmission.Authorized) {
            backend.detach()
        }
        return admission
    }
}
