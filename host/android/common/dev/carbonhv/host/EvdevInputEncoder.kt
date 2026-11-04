package dev.carbonhv.host

import java.nio.ByteBuffer
import java.nio.ByteOrder

object EvdevInputEncoder {
    fun encode(input: HostGuestInput): ByteArray = when (input) {
        is HostGuestInput.Key -> encodeEvents(
            listOf(
                Event(EV_KEY, input.linuxKeyCode, if (input.down) 1 else 0),
                syn(),
            ),
        )
        is HostGuestInput.Touch -> encodeTouch(input)
    }

    private fun encodeTouch(input: HostGuestInput.Touch): ByteArray {
        val events = when (input.action) {
            HostTouchAction.Move -> buildList {
                input.pointers.forEach { pointer ->
                    addAll(positionEvents(pointer))
                }
                add(syn())
            }
            HostTouchAction.Down,
            HostTouchAction.PointerDown,
            HostTouchAction.PointerUp,
            HostTouchAction.Up -> {
                val pointer = requireNotNull(input.pointers.firstOrNull { it.id == input.actionPointerId }) {
                    "Touch action pointer is missing from the pointer set"
                }
                val down = input.action == HostTouchAction.Down || input.action == HostTouchAction.PointerDown
                buildList {
                    add(Event(EV_KEY, BTN_TOUCH, if (down) 1 else 0))
                    add(Event(EV_ABS, ABS_MT_SLOT, pointer.id))
                    add(Event(EV_ABS, ABS_MT_TRACKING_ID, if (down) pointer.id else -1))
                    add(Event(EV_ABS, ABS_MT_POSITION_X, pointer.x))
                    add(Event(EV_ABS, ABS_MT_POSITION_Y, pointer.y))
                    add(Event(EV_ABS, ABS_X, pointer.x))
                    add(Event(EV_ABS, ABS_Y, pointer.y))
                    add(syn())
                }
            }
            HostTouchAction.Cancel -> buildList {
                add(Event(EV_KEY, BTN_TOUCH, 0))
                input.pointers.forEach { pointer ->
                    add(Event(EV_ABS, ABS_MT_SLOT, pointer.id))
                    add(Event(EV_ABS, ABS_MT_TRACKING_ID, -1))
                }
                add(syn())
            }
        }
        return encodeEvents(events)
    }

    private fun positionEvents(pointer: HostTouchPointer): List<Event> = listOf(
        Event(EV_ABS, ABS_MT_SLOT, pointer.id),
        Event(EV_ABS, ABS_MT_TRACKING_ID, pointer.id),
        Event(EV_ABS, ABS_MT_POSITION_X, pointer.x),
        Event(EV_ABS, ABS_MT_POSITION_Y, pointer.y),
        Event(EV_ABS, ABS_X, pointer.x),
        Event(EV_ABS, ABS_Y, pointer.y),
    )

    private fun encodeEvents(events: List<Event>): ByteArray {
        val buffer = ByteBuffer.allocate(events.size * EVENT_BYTES).order(ByteOrder.LITTLE_ENDIAN)
        events.forEach { event ->
            buffer.putShort(event.type.toShort())
            buffer.putShort(event.code.toShort())
            buffer.putInt(event.value)
        }
        return buffer.array()
    }

    private fun syn(): Event = Event(EV_SYN, SYN_REPORT, 0)

    private data class Event(val type: Int, val code: Int, val value: Int)

    private const val EVENT_BYTES = 8
    private const val EV_SYN = 0x00
    private const val EV_KEY = 0x01
    private const val EV_ABS = 0x03
    private const val SYN_REPORT = 0x00
    private const val BTN_TOUCH = 0x14a
    private const val ABS_X = 0x00
    private const val ABS_Y = 0x01
    private const val ABS_MT_SLOT = 0x2f
    private const val ABS_MT_POSITION_X = 0x35
    private const val ABS_MT_POSITION_Y = 0x36
    private const val ABS_MT_TRACKING_ID = 0x39
}
