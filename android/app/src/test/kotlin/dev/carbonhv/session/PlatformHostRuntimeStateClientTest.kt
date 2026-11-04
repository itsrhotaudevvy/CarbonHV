package dev.carbonhv.session

import dev.carbonhv.host.ICarbonHvHostService
import java.lang.reflect.InvocationHandler
import java.lang.reflect.Proxy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PlatformHostRuntimeStateClientTest {
    @Test
    fun reads_exact_64_bit_generation_from_host_truth() {
        val service = runtimeService(mapOf("vm-1" to 5_000_000_000L))
        val client = PlatformHostRuntimeStateClient(service)

        assertEquals(
            SessionIdentity("vm-1", 5_000_000_000L),
            client.liveDisplaySession("vm-1"),
        )
    }

    @Test
    fun missing_live_session_is_not_synthesized() {
        val client = PlatformHostRuntimeStateClient(runtimeService(emptyMap()))

        assertNull(client.liveDisplaySession("vm-stopped"))
    }
}

private fun runtimeService(generations: Map<String, Long>): ICarbonHvHostService {
    val handler = InvocationHandler { _, method, args ->
        when (method.name) {
            "activeDisplayGeneration" -> generations[args!![0] as String] ?: -1L
            else -> null
        }
    }
    return Proxy.newProxyInstance(
        ICarbonHvHostService::class.java.classLoader,
        arrayOf(ICarbonHvHostService::class.java),
        handler,
    ) as ICarbonHvHostService
}
