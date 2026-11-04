package dev.carbonhv.host

import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.junit.Assert.assertEquals
import org.junit.Test

class EvdevInputEncoderTest {
    @Test
    fun key_event_is_ev_key_followed_by_syn_report() {
        val events = decode(EvdevInputEncoder.encode(HostGuestInput.Key(158, down = true)))

        assertEquals(
            listOf(
                Event(type = 0x01, code = 158, value = 1),
                Event(type = 0x00, code = 0, value = 0),
            ),
            events,
        )
    }

    @Test
    fun touch_down_uses_multitouch_slot_tracking_and_absolute_coordinates() {
        val events = decode(
            EvdevInputEncoder.encode(
                HostGuestInput.Touch(
                    action = HostTouchAction.Down,
                    actionPointerId = 3,
                    pointers = listOf(HostTouchPointer(id = 3, x = 540, y = 1200)),
                ),
            ),
        )

        assertEquals(Event(0x01, 0x14a, 1), events[0])
        assertEquals(Event(0x03, 0x2f, 3), events[1])
        assertEquals(Event(0x03, 0x39, 3), events[2])
        assertEquals(Event(0x03, 0x35, 540), events[3])
        assertEquals(Event(0x03, 0x36, 1200), events[4])
        assertEquals(Event(0x00, 0, 0), events.last())
    }

    @Test
    fun move_serializes_every_pointer_then_one_syn_report() {
        val events = decode(
            EvdevInputEncoder.encode(
                HostGuestInput.Touch(
                    action = HostTouchAction.Move,
                    actionPointerId = 0,
                    pointers = listOf(
                        HostTouchPointer(id = 0, x = 10, y = 20),
                        HostTouchPointer(id = 1, x = 30, y = 40),
                    ),
                ),
            ),
        )

        assertEquals(13, events.size)
        assertEquals(Event(0x03, 0x2f, 0), events[0])
        assertEquals(Event(0x03, 0x2f, 1), events[6])
        assertEquals(Event(0x00, 0, 0), events.last())
    }

    @Test
    fun cancel_releases_each_reported_slot() {
        val events = decode(
            EvdevInputEncoder.encode(
                HostGuestInput.Touch(
                    action = HostTouchAction.Cancel,
                    actionPointerId = 0,
                    pointers = listOf(
                        HostTouchPointer(id = 0, x = 10, y = 20),
                        HostTouchPointer(id = 2, x = 30, y = 40),
                    ),
                ),
            ),
        )

        assertEquals(Event(0x01, 0x14a, 0), events[0])
        assertEquals(Event(0x03, 0x2f, 0), events[1])
        assertEquals(Event(0x03, 0x39, -1), events[2])
        assertEquals(Event(0x03, 0x2f, 2), events[3])
        assertEquals(Event(0x03, 0x39, -1), events[4])
        assertEquals(Event(0x00, 0, 0), events.last())
    }

    private fun decode(bytes: ByteArray): List<Event> {
        val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        return buildList {
            while (buffer.remaining() >= 8) {
                add(
                    Event(
                        type = buffer.short.toInt() and 0xffff,
                        code = buffer.short.toInt() and 0xffff,
                        value = buffer.int,
                    ),
                )
            }
        }
    }

    private data class Event(val type: Int, val code: Int, val value: Int)
}
