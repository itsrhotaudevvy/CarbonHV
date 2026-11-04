package dev.carbonhv.session

import dev.carbonhv.host.ICarbonHvHostService
import java.lang.reflect.InvocationHandler
import java.lang.reflect.Proxy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PlatformHostGuestInputClientTest {
    @Test
    fun forwards_key_and_touch_with_exact_live_session_identity() {
        val calls = mutableListOf<String>()
        val service = Proxy.newProxyInstance(
            ICarbonHvHostService::class.java.classLoader,
            arrayOf(ICarbonHvHostService::class.java),
            InvocationHandler { _, method, args ->
                when (method.name) {
                    "sendGuestKey" -> {
                        calls += "key:${args!![0]}:${args[1]}:${args[2]}:${args[3]}"
                        true
                    }
                    "sendGuestTouch" -> {
                        calls += "touch:${args!![0]}:${args[1]}:${args[2]}:${args[3]}:${args[4]}x${args[5]}"
                        true
                    }
                    else -> null
                }
            },
        ) as ICarbonHvHostService
        val client = PlatformHostGuestInputClient(service)
        val session = SessionIdentity("vm-4", 9)

        assertTrue(client.sendKey(session, linuxKeyCode = 158, down = true))
        assertTrue(
            client.sendTouch(
                session = session,
                action = 0,
                actionPointerId = 2,
                viewportWidth = 1080,
                viewportHeight = 2400,
                pointerIds = intArrayOf(2),
                xs = floatArrayOf(500f),
                ys = floatArrayOf(600f),
            ),
        )

        assertEquals(
            listOf(
                "key:vm-4:9:158:true",
                "touch:vm-4:9:0:2:1080x2400",
            ),
            calls,
        )
    }
}
