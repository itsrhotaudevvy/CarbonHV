package dev.carbonhv.app

/**
 * Tracks whether a host-integration [android.content.ServiceConnection] is registered with the
 * framework. `Context.bindService` registers the connection even when it returns `false` (for
 * example on stock builds where the CarbonHV host integration is absent), and that registration
 * must still be released with `unbindService`; otherwise every destroyed activity leaks it.
 * A `SecurityException` from `bindService` means nothing was registered.
 */
class HostBindingLifecycle(
    private val bind: () -> Boolean,
    private val unbind: () -> Unit,
) {
    var registered = false
        private set

    /** True only when the host service is actually reachable. */
    var connected = false
        private set

    fun bindIfNeeded() {
        if (registered) return
        connected = try {
            val result = bind()
            registered = true
            result
        } catch (_: SecurityException) {
            false
        }
    }

    fun release() {
        if (!registered) return
        registered = false
        connected = false
        try {
            unbind()
        } catch (_: IllegalArgumentException) {
            // Already unregistered by the framework; nothing left to release.
        }
    }

    /** `onBindingDied`: the framework requires an unbind before the connection is reused. */
    fun rebindAfterDeath() {
        release()
        bindIfNeeded()
    }
}
