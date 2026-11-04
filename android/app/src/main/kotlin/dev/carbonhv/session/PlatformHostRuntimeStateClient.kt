package dev.carbonhv.session

import dev.carbonhv.host.ICarbonHvHostService

class PlatformHostRuntimeStateClient(
    private val service: ICarbonHvHostService,
) {
    fun liveDisplaySession(instanceId: String): SessionIdentity? {
        val generation = service.activeDisplayGeneration(instanceId)
        if (generation < 0) return null
        return SessionIdentity(instanceId, generation)
    }
}
