package dev.carbonhv.session

import android.view.Surface
import dev.carbonhv.host.ICarbonHvHostService

class PlatformHostDisplaySurfaceClient(
    private val service: ICarbonHvHostService,
) : HostDisplaySurfaceClient {
    override fun attachMainSurface(session: SessionIdentity, surface: Surface) {
        service.attachMainSurface(session.instanceId, session.generation, surface)
    }

    override fun detachMainSurface(session: SessionIdentity) {
        service.detachMainSurface(session.instanceId, session.generation)
    }
}
