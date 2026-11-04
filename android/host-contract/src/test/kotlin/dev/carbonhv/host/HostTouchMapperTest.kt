package dev.carbonhv.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class HostTouchMapperTest {
    @Test
    fun maps_viewport_coordinates_into_fixed_guest_display() {
        assertEquals(
            HostGuestInput.Touch(
                action = HostTouchAction.Down,
                actionPointerId = 7,
                pointers = listOf(HostTouchPointer(7, 540, 1200)),
            ),
            HostTouchMapper.map(
                action = HostTouchMapper.ACTION_DOWN,
                actionPointerId = 7,
                viewportWidth = 540,
                viewportHeight = 1200,
                guestWidth = 1080,
                guestHeight = 2400,
                pointerIds = intArrayOf(7),
                xs = floatArrayOf(270f),
                ys = floatArrayOf(600f),
            ),
        )
    }

    @Test
    fun clamps_rounding_at_the_exclusive_guest_edge() {
        val touch = HostTouchMapper.map(
            action = HostTouchMapper.ACTION_MOVE,
            actionPointerId = 0,
            viewportWidth = 1080,
            viewportHeight = 2400,
            guestWidth = 1080,
            guestHeight = 2400,
            pointerIds = intArrayOf(0),
            xs = floatArrayOf(1080f),
            ys = floatArrayOf(2400f),
        )

        assertEquals(1079, touch?.pointers?.single()?.x)
        assertEquals(2399, touch?.pointers?.single()?.y)
    }

    @Test
    fun malformed_pointer_arrays_and_unknown_actions_fail_closed() {
        assertNull(
            HostTouchMapper.map(
                action = HostTouchMapper.ACTION_MOVE,
                actionPointerId = 0,
                viewportWidth = 100,
                viewportHeight = 100,
                guestWidth = 100,
                guestHeight = 100,
                pointerIds = intArrayOf(0, 1),
                xs = floatArrayOf(10f),
                ys = floatArrayOf(10f, 20f),
            ),
        )
        assertNull(
            HostTouchMapper.map(
                action = 99,
                actionPointerId = 0,
                viewportWidth = 100,
                viewportHeight = 100,
                guestWidth = 100,
                guestHeight = 100,
                pointerIds = intArrayOf(0),
                xs = floatArrayOf(10f),
                ys = floatArrayOf(10f),
            ),
        )
    }
}
