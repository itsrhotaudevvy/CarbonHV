package dev.carbonhv.host

interface DisplaySessionAuthority {
    fun current(): DisplaySessionIdentity?
    fun activate(identity: DisplaySessionIdentity): Boolean
    fun clear(identity: DisplaySessionIdentity): Boolean
}

sealed interface HostGuestInput {
    data class Key(
        val linuxKeyCode: Int,
        val down: Boolean,
    ) : HostGuestInput

    data class Touch(
        val action: HostTouchAction,
        val actionPointerId: Int,
        val pointers: List<HostTouchPointer>,
    ) : HostGuestInput
}

enum class HostTouchAction {
    Down,
    PointerDown,
    Move,
    PointerUp,
    Up,
    Cancel,
}

data class HostTouchPointer(
    val id: Int,
    val x: Int,
    val y: Int,
)

interface GraphicalVmRuntimeBackend<Config> {
    fun start(
        identity: DisplaySessionIdentity,
        config: Config,
        onDied: () -> Unit,
    )

    fun stop(identity: DisplaySessionIdentity)

    fun sendInput(identity: DisplaySessionIdentity, input: HostGuestInput): Boolean
}

sealed interface HostVmStartResult {
    data object Started : HostVmStartResult
    data object AlreadyActive : HostVmStartResult
    data class Busy(val active: DisplaySessionIdentity) : HostVmStartResult
}

class HostVmRuntimeCoordinator<Config>(
    private val display: DisplaySessionAuthority,
    private val backend: GraphicalVmRuntimeBackend<Config>,
    private val onRuntimeDied: (DisplaySessionIdentity) -> Unit = {},
) {
    private var active: DisplaySessionIdentity? = null

    @Synchronized
    fun current(): DisplaySessionIdentity? = active

    @Synchronized
    fun start(identity: DisplaySessionIdentity, config: Config): HostVmStartResult {
        active?.let { current ->
            return if (current == identity) {
                HostVmStartResult.AlreadyActive
            } else {
                HostVmStartResult.Busy(current)
            }
        }

        if (!display.activate(identity)) {
            return HostVmStartResult.Busy(
                requireNotNull(display.current()) {
                    "Display authority rejected activation without reporting an active owner"
                },
            )
        }

        active = identity
        try {
            backend.start(identity, config) {
                onBackendDied(identity)
            }
        } catch (failure: Throwable) {
            if (active == identity) {
                active = null
                display.clear(identity)
            }
            throw failure
        }
        return HostVmStartResult.Started
    }

    @Synchronized
    fun stop(identity: DisplaySessionIdentity): Boolean {
        if (active != identity) return false
        backend.stop(identity)
        if (active == identity) {
            active = null
            display.clear(identity)
        }
        return true
    }

    @Synchronized
    fun sendInput(identity: DisplaySessionIdentity, input: HostGuestInput): Boolean {
        if (active != identity) return false
        return backend.sendInput(identity, input)
    }

    private fun onBackendDied(identity: DisplaySessionIdentity) {
        val shouldNotify = synchronized(this) {
            if (active != identity) {
                false
            } else {
                active = null
                display.clear(identity)
                true
            }
        }
        if (shouldNotify) onRuntimeDied(identity)
    }
}
