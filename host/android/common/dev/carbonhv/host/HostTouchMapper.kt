package dev.carbonhv.host

object HostTouchMapper {
    const val ACTION_DOWN = 0
    const val ACTION_UP = 1
    const val ACTION_MOVE = 2
    const val ACTION_CANCEL = 3
    const val ACTION_POINTER_DOWN = 5
    const val ACTION_POINTER_UP = 6

    fun map(
        action: Int,
        actionPointerId: Int,
        viewportWidth: Int,
        viewportHeight: Int,
        guestWidth: Int,
        guestHeight: Int,
        pointerIds: IntArray,
        xs: FloatArray,
        ys: FloatArray,
    ): HostGuestInput.Touch? {
        if (viewportWidth <= 0 || viewportHeight <= 0 || guestWidth <= 0 || guestHeight <= 0) {
            return null
        }
        if (pointerIds.size != xs.size || pointerIds.size != ys.size || pointerIds.isEmpty()) {
            return null
        }
        if (pointerIds.size > MAX_POINTERS || pointerIds.toSet().size != pointerIds.size) return null

        val mappedAction = when (action) {
            ACTION_DOWN -> HostTouchAction.Down
            ACTION_UP -> HostTouchAction.Up
            ACTION_MOVE -> HostTouchAction.Move
            ACTION_CANCEL -> HostTouchAction.Cancel
            ACTION_POINTER_DOWN -> HostTouchAction.PointerDown
            ACTION_POINTER_UP -> HostTouchAction.PointerUp
            else -> return null
        }
        if (mappedAction != HostTouchAction.Move && mappedAction != HostTouchAction.Cancel &&
            actionPointerId !in pointerIds
        ) {
            return null
        }

        val pointers = pointerIds.indices.map { index ->
            val x = xs[index]
            val y = ys[index]
            if (!x.isFinite() || !y.isFinite()) return null
            HostTouchPointer(
                id = pointerIds[index],
                x = mapAxis(x, viewportWidth, guestWidth),
                y = mapAxis(y, viewportHeight, guestHeight),
            )
        }
        return HostGuestInput.Touch(
            action = mappedAction,
            actionPointerId = actionPointerId,
            pointers = pointers,
        )
    }

    private fun mapAxis(value: Float, viewportExtent: Int, guestExtent: Int): Int =
        ((value / viewportExtent.toFloat()) * guestExtent.toFloat())
            .toInt()
            .coerceIn(0, guestExtent - 1)

    private const val MAX_POINTERS = 32
}
