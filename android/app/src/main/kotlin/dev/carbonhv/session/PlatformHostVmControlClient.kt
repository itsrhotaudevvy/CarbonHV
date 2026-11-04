package dev.carbonhv.session

import dev.carbonhv.host.HostGraphicalVmStartRequest
import dev.carbonhv.host.ICarbonHvHostService

data class HostVmStartSpec(
    val requestId: Long,
    val instanceId: String,
    val bundleId: String,
    val memoryMib: Int,
    val vcpus: Int,
    val widthPixels: Int,
    val heightPixels: Int,
    val densityDpi: Int,
    val refreshRate: Int,
)

sealed interface HostVmControlResult {
    data class Running(val identity: SessionIdentity) : HostVmControlResult
    data class StaleGeneration(
        val started: SessionIdentity,
        val authoritative: SessionIdentity?,
    ) : HostVmControlResult
    data class RuntimeFailure(val message: String) : HostVmControlResult
}

sealed interface HostVmStopResult {
    data object Stopped : HostVmStopResult
    data class StaleGeneration(val authoritative: SessionIdentity) : HostVmStopResult
    data class RuntimeFailure(val message: String) : HostVmStopResult
}

class PlatformHostVmControlClient(
    private val service: ICarbonHvHostService,
) {
    private val runtimeState = PlatformHostRuntimeStateClient(service)

    fun startOrAttach(spec: HostVmStartSpec): HostVmControlResult = try {
        runtimeState.liveDisplaySession(spec.instanceId)?.let { live ->
            return HostVmControlResult.Running(live)
        }

        val request = HostGraphicalVmStartRequest().apply {
            requestId = spec.requestId
            instanceId = spec.instanceId
            bundleId = spec.bundleId
            memoryMib = spec.memoryMib
            vcpus = spec.vcpus
            widthPixels = spec.widthPixels
            heightPixels = spec.heightPixels
            densityDpi = spec.densityDpi
            refreshRate = spec.refreshRate
        }
        val started = SessionIdentity(spec.instanceId, service.startGraphicalVm(request))
        val authoritative = runtimeState.liveDisplaySession(spec.instanceId)
        if (authoritative == started) {
            HostVmControlResult.Running(started)
        } else {
            HostVmControlResult.StaleGeneration(started, authoritative)
        }
    } catch (failure: Exception) {
        HostVmControlResult.RuntimeFailure(failure.message ?: failure.javaClass.simpleName)
    }

    fun stop(identity: SessionIdentity, requestId: Long): HostVmStopResult = try {
        service.stopVm(identity.instanceId, identity.generation, requestId)
        when (val authoritative = runtimeState.liveDisplaySession(identity.instanceId)) {
            null -> HostVmStopResult.Stopped
            else -> HostVmStopResult.StaleGeneration(authoritative)
        }
    } catch (failure: Exception) {
        HostVmStopResult.RuntimeFailure(failure.message ?: failure.javaClass.simpleName)
    }
}
