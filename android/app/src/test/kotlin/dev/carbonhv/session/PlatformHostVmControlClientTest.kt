package dev.carbonhv.session

import dev.carbonhv.host.HostGraphicalVmStartRequest
import dev.carbonhv.host.ICarbonHvHostService
import java.lang.reflect.InvocationHandler
import java.lang.reflect.Proxy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PlatformHostVmControlClientTest {
    @Test
    fun start_uses_high_level_intent_and_confirms_broker_authoritative_generation() {
        var captured: HostGraphicalVmStartRequest? = null
        var queryCount = 0
        val service = hostService { method, args ->
            when (method) {
                "activeDisplayGeneration" -> if (queryCount++ == 0) -1L else 7L
                "startGraphicalVm" -> {
                    captured = args!![0] as HostGraphicalVmStartRequest
                    7L
                }
                else -> null
            }
        }
        val client = PlatformHostVmControlClient(service)

        val result = client.startOrAttach(startSpec())

        assertEquals(HostVmControlResult.Running(SessionIdentity("vm-1", 7)), result)
        assertEquals("grapheneos", captured?.bundleId)
        assertEquals(4096, captured?.memoryMib)
        assertEquals(4, captured?.vcpus)
        assertEquals(1080, captured?.widthPixels)
        assertEquals(2400, captured?.heightPixels)
    }

    @Test
    fun returned_generation_is_rejected_when_broker_query_reports_a_different_live_generation() {
        var queryCount = 0
        val service = hostService { method, _ ->
            when (method) {
                "activeDisplayGeneration" -> if (queryCount++ == 0) -1L else 9L
                "startGraphicalVm" -> 8L
                else -> null
            }
        }
        val client = PlatformHostVmControlClient(service)

        val result = client.startOrAttach(startSpec())

        assertEquals(
            HostVmControlResult.StaleGeneration(
                started = SessionIdentity("vm-1", 8),
                authoritative = SessionIdentity("vm-1", 9),
            ),
            result,
        )
    }

    @Test
    fun runtime_exception_becomes_recoverable_client_failure() {
        val service = hostService { method, _ ->
            when (method) {
                "activeDisplayGeneration" -> -1L
                "startGraphicalVm" -> error("AVF launch rejected")
                else -> null
            }
        }
        val client = PlatformHostVmControlClient(service)

        val result = client.startOrAttach(startSpec())

        assertTrue(result is HostVmControlResult.RuntimeFailure)
        assertTrue((result as HostVmControlResult.RuntimeFailure).message.contains("AVF launch rejected"))
    }

    private fun startSpec() = HostVmStartSpec(
        requestId = 100,
        instanceId = "vm-1",
        bundleId = "grapheneos",
        memoryMib = 4096,
        vcpus = 4,
        widthPixels = 1080,
        heightPixels = 2400,
        densityDpi = 420,
        refreshRate = 90,
    )
}

private fun hostService(
    handler: (method: String, args: Array<out Any?>?) -> Any?,
): ICarbonHvHostService {
    val invocation = InvocationHandler { _, method, args -> handler(method.name, args) }
    return Proxy.newProxyInstance(
        ICarbonHvHostService::class.java.classLoader,
        arrayOf(ICarbonHvHostService::class.java),
        invocation,
    ) as ICarbonHvHostService
}
