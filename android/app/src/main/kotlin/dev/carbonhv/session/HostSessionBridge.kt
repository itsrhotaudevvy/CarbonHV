package dev.carbonhv.session

import dev.carbonhv.host.ICarbonHvHostService

class HostSessionBridge(
    service: ICarbonHvHostService,
) {
    private val runtimeState = PlatformHostRuntimeStateClient(service)
    private val control = PlatformHostVmControlClient(service)
    private val displaySurface = PlatformHostDisplaySurfaceClient(service)
    private val guestInput = PlatformHostGuestInputClient(service)

    fun startOrAttach(spec: HostVmStartSpec): HostVmControlResult = control.startOrAttach(spec)

    fun stop(identity: SessionIdentity, requestId: Long): HostVmStopResult =
        control.stop(identity, requestId)

    fun callbacksFor(identity: SessionIdentity): GuestSurfaceCallbacks =
        BrokerGuestSurfaceCallbacks(identity, displaySurface, guestInput)

    fun callbacksFor(instanceId: String): GuestSurfaceCallbacks? {
        val session = runtimeState.liveDisplaySession(instanceId) ?: return null
        return callbacksFor(session)
    }
}
