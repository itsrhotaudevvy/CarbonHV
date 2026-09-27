package dev.carbonhv.session

enum class NativeCapabilityState(val isSupported: Boolean) {
    UNKNOWN(false),
    UNSUPPORTED(false),
    SUPPORTED(true),
}

data class NativeBufferSupport(
    val format: Int,
    val usage: Long,
)

data class NativeGpuProbe(
    val gpuName: String?,
    val vulkanApiVersion: String?,
    val vulkanDriverVersion: String?,
    val externalMemory: NativeCapabilityState,
    val explicitSync: NativeCapabilityState,
    val hardwareBuffer: NativeCapabilityState,
    val protectedMemory: NativeCapabilityState,
    val nativeBufferFormats: List<NativeBufferSupport>,
    val displayRefreshRatesHz: List<Float>,
) {
    companion object {
        fun unknown() = NativeGpuProbe(
            gpuName = null,
            vulkanApiVersion = null,
            vulkanDriverVersion = null,
            externalMemory = NativeCapabilityState.UNKNOWN,
            explicitSync = NativeCapabilityState.UNKNOWN,
            hardwareBuffer = NativeCapabilityState.UNKNOWN,
            protectedMemory = NativeCapabilityState.UNKNOWN,
            nativeBufferFormats = emptyList(),
            displayRefreshRatesHz = emptyList(),
        )
    }
}

object GraphicsNative {
    private val libraryLoaded = runCatching {
        System.loadLibrary("carbon_graphics")
    }.isSuccess

    fun probe(): NativeGpuProbe {
        if (!libraryLoaded) return NativeGpuProbe.unknown()
        return runCatching { nativeProbe() }.getOrNull() ?: NativeGpuProbe.unknown()
    }

    private external fun nativeProbe(): NativeGpuProbe?
}
