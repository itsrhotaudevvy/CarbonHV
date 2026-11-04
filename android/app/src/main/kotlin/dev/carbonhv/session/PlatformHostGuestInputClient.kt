package dev.carbonhv.session

import dev.carbonhv.host.ICarbonHvHostService

interface HostGuestInputClient {
    fun sendKey(session: SessionIdentity, linuxKeyCode: Int, down: Boolean): Boolean

    fun sendTouch(
        session: SessionIdentity,
        action: Int,
        actionPointerId: Int,
        viewportWidth: Int,
        viewportHeight: Int,
        pointerIds: IntArray,
        xs: FloatArray,
        ys: FloatArray,
    ): Boolean
}

class PlatformHostGuestInputClient(
    private val service: ICarbonHvHostService,
) : HostGuestInputClient {
    override fun sendKey(session: SessionIdentity, linuxKeyCode: Int, down: Boolean): Boolean =
        service.sendGuestKey(session.instanceId, session.generation, linuxKeyCode, down)

    override fun sendTouch(
        session: SessionIdentity,
        action: Int,
        actionPointerId: Int,
        viewportWidth: Int,
        viewportHeight: Int,
        pointerIds: IntArray,
        xs: FloatArray,
        ys: FloatArray,
    ): Boolean = service.sendGuestTouch(
        session.instanceId,
        session.generation,
        action,
        actionPointerId,
        viewportWidth,
        viewportHeight,
        pointerIds,
        xs,
        ys,
    )
}
