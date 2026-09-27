package dev.carbonhv.session

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

class GraphicsNativeContractTest {
    @Test
    fun probeRepresentsObservedGpuAndNativeBufferFacts() {
        val probe = NativeGpuProbe(
            gpuName = "Mali-G710",
            vulkanApiVersion = "1.3.0",
            vulkanDriverVersion = "226504704",
            externalMemory = NativeCapabilityState.SUPPORTED,
            explicitSync = NativeCapabilityState.SUPPORTED,
            hardwareBuffer = NativeCapabilityState.SUPPORTED,
            protectedMemory = NativeCapabilityState.UNSUPPORTED,
            nativeBufferFormats = listOf(
                NativeBufferSupport(format = 1, usage = 0x300L),
                NativeBufferSupport(format = 22, usage = 0x100L),
            ),
            displayRefreshRatesHz = listOf(60.0f, 90.0f),
        )

        assertEquals("Mali-G710", probe.gpuName)
        assertEquals("1.3.0", probe.vulkanApiVersion)
        assertEquals("226504704", probe.vulkanDriverVersion)
        assertEquals(NativeCapabilityState.SUPPORTED, probe.externalMemory)
        assertEquals(NativeCapabilityState.SUPPORTED, probe.explicitSync)
        assertEquals(NativeCapabilityState.SUPPORTED, probe.hardwareBuffer)
        assertEquals(NativeCapabilityState.UNSUPPORTED, probe.protectedMemory)
        assertEquals(2, probe.nativeBufferFormats.size)
        assertEquals(listOf(60.0f, 90.0f), probe.displayRefreshRatesHz)
    }

    @Test
    fun unknownProbeValuesStayUnknownInsteadOfBecomingSupported() {
        val probe = NativeGpuProbe.unknown()

        assertNull(probe.gpuName)
        assertNull(probe.vulkanApiVersion)
        assertNull(probe.vulkanDriverVersion)
        assertEquals(NativeCapabilityState.UNKNOWN, probe.externalMemory)
        assertEquals(NativeCapabilityState.UNKNOWN, probe.explicitSync)
        assertEquals(NativeCapabilityState.UNKNOWN, probe.hardwareBuffer)
        assertEquals(NativeCapabilityState.UNKNOWN, probe.protectedMemory)
        assertFalse(probe.externalMemory.isSupported)
        assertFalse(probe.explicitSync.isSupported)
        assertFalse(probe.hardwareBuffer.isSupported)
        assertFalse(probe.protectedMemory.isSupported)
        assertEquals(emptyList<NativeBufferSupport>(), probe.nativeBufferFormats)
        assertEquals(emptyList<Float>(), probe.displayRefreshRatesHz)
    }
}
