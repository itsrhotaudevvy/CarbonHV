package dev.carbonhv.host

import android.os.ServiceManager
import dev.carbonhv.broker.BrokerStartRequest
import dev.carbonhv.broker.ICarbonHvBroker

class PlatformCarbonHvBrokerClient {
    fun startGraphicalVm(request: HostGraphicalVmStartRequest): Long {
        val brokerRequest = BrokerStartRequest().apply {
            requestId = request.requestId
            instanceId = request.instanceId
            bundleId = request.bundleId
            memoryMib = request.memoryMib
            vcpus = request.vcpus
            widthPixels = request.widthPixels
            heightPixels = request.heightPixels
            densityDpi = request.densityDpi
            refreshRate = request.refreshRate
        }
        return service().startGraphicalVm(brokerRequest)
    }

    fun stopVm(instanceId: String, generation: Long, requestId: Long) {
        service().stopVm(instanceId, generation, requestId)
    }

    fun activeDisplayGeneration(instanceId: String): Long =
        service().activeDisplayGeneration(instanceId)

    fun notifyRuntimeDied(instanceId: String, generation: Long) {
        service().notifyRuntimeDied(instanceId, generation)
    }

    private fun service(): ICarbonHvBroker {
        val binder = ServiceManager.waitForService(BROKER_SERVICE_NAME)
        return checkNotNull(ICarbonHvBroker.Stub.asInterface(binder)) {
            "CarbonHV broker Binder is unavailable"
        }
    }

    private companion object {
        const val BROKER_SERVICE_NAME = "dev.carbonhv.broker"
    }
}
