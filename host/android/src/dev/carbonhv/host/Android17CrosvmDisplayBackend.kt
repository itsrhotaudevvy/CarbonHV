package dev.carbonhv.host

import android.crosvm.ICrosvmAndroidDisplayService
import android.os.DeadObjectException
import android.os.ServiceManager
import android.system.virtualizationservice_internal.IVirtualizationServiceInternal
import android.view.Surface

/** Android 17 AVF display path used by VmTerminalApp on android17-release. */
class Android17CrosvmDisplayBackend : MainSurfaceBackend<Surface> {
    @Volatile
    private var cachedDisplayService: ICrosvmAndroidDisplayService? = null

    override fun attach(surface: Surface) {
        try {
            displayService().setSurface(surface, false /* forCursor */)
        } catch (dead: DeadObjectException) {
            cachedDisplayService = null
            throw dead
        }
    }

    override fun detach() {
        val service = cachedDisplayService ?: return
        try {
            service.removeSurface(false /* forCursor */)
        } catch (_: DeadObjectException) {
            cachedDisplayService = null
        }
    }

    @Synchronized
    private fun displayService(): ICrosvmAndroidDisplayService {
        cachedDisplayService?.let { return it }
        val virtualizationBinder =
            requireNotNull(ServiceManager.waitForService(VIRTUALIZATION_SERVICE)) {
                "Android virtualization service is unavailable"
            }
        val virtualizationService =
            IVirtualizationServiceInternal.Stub.asInterface(virtualizationBinder)
        val displayBinder = virtualizationService.waitDisplayService()
        return ICrosvmAndroidDisplayService.Stub.asInterface(displayBinder).also {
            cachedDisplayService = it
        }
    }

    private companion object {
        const val VIRTUALIZATION_SERVICE = "android.system.virtualizationservice"
    }
}
