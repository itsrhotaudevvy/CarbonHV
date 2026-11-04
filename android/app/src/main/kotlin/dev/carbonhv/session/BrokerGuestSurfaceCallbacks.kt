package dev.carbonhv.session

import android.view.Surface

/**
 * Narrow app-side contract for handing the immersive display surface to the privileged host
 * integration. The host integration owns the hidden AVF/crosvm display-service calls.
 */
interface HostDisplaySurfaceClient {
    fun attachMainSurface(session: SessionIdentity, surface: Surface)
    fun detachMainSurface(session: SessionIdentity)
}

class BrokerGuestSurfaceCallbacks(
    private val session: SessionIdentity,
    private val client: HostDisplaySurfaceClient,
    private val inputClient: HostGuestInputClient,
) : GuestSurfaceCallbacks {
    private var attached = false

    override fun onSurfaceAvailable(surface: Surface, width: Int, height: Int) {
        require(width > 0 && height > 0)
        client.attachMainSurface(session, surface)
        attached = true
    }

    override fun onSurfaceChanged(surface: Surface, width: Int, height: Int) {
        require(width > 0 && height > 0)
        // Android 17's crosvm display service keeps the VM display geometry fixed for the current
        // session. Surface size changes do not replace the Binder surface; a later resize-capable
        // AVF contract can add an explicit geometry operation here.
    }

    override fun onSurfaceDestroyed() {
        if (!attached) return
        attached = false
        client.detachMainSurface(session)
    }

    override fun onTouch(
        action: Int,
        actionPointerId: Int,
        viewportWidth: Int,
        viewportHeight: Int,
        pointerIds: IntArray,
        xs: FloatArray,
        ys: FloatArray,
    ): Boolean = inputClient.sendTouch(
        session = session,
        action = action,
        actionPointerId = actionPointerId,
        viewportWidth = viewportWidth,
        viewportHeight = viewportHeight,
        pointerIds = pointerIds,
        xs = xs,
        ys = ys,
    )

    override fun onKey(linuxKeyCode: Int, down: Boolean): Boolean =
        inputClient.sendKey(session, linuxKeyCode, down)
}
