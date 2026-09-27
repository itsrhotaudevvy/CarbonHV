package dev.carbonhv.session

import android.view.Surface

enum class NativeCapabilityState(val isSupported: Boolean) {
    UNKNOWN(false),
    UNSUPPORTED(false),
    SUPPORTED(true),
}

data class NativeBufferSupport(
    val format: Int,
    val usage: Long,
)

data class NativeFrameRelease(
    val frameId: Long,
    val releaseFenceFd: Int,
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

    internal fun attachSurface(surface: Surface, width: Int, height: Int, generation: Long): Boolean =
        if (!libraryLoaded) false else runCatching {
            nativeAttachSurface(surface, width, height, generation)
        }.getOrDefault(false)

    internal fun resizeSurface(surface: Surface, width: Int, height: Int, generation: Long): Boolean =
        if (!libraryLoaded) false else runCatching {
            nativeResizeSurface(surface, width, height, generation)
        }.getOrDefault(false)

    internal fun detachSurface(generation: Long) {
        if (libraryLoaded) runCatching { nativeDetachSurface(generation) }
    }

    internal fun presentFrame(frame: NativeFrame, generation: Long): Boolean =
        if (!libraryLoaded) false else runCatching {
            nativePresentFrame(
                frame.frameId,
                frame.hardwareBuffer,
                frame.width,
                frame.height,
                frame.format,
                frame.usage,
                frame.acquireFenceFd,
                generation,
            )
        }.getOrDefault(false)

    internal fun releaseFrame(frameId: Long) {
        if (libraryLoaded) runCatching { nativeReleaseFrame(frameId) }
    }

    internal fun drainReleasedFrames(): List<NativeFrameRelease> {
        if (!libraryLoaded) return emptyList()
        val values = runCatching { nativeDrainReleasedFrames() }.getOrNull() ?: return emptyList()
        if (values.size % 2 != 0) return emptyList()
        return values.asList().chunked(2).map { pair ->
            NativeFrameRelease(frameId = pair[0], releaseFenceFd = pair[1].toInt())
        }
    }

    private external fun nativeProbe(): NativeGpuProbe?
    private external fun nativeAttachSurface(
        surface: Surface,
        width: Int,
        height: Int,
        generation: Long,
    ): Boolean
    private external fun nativeResizeSurface(
        surface: Surface,
        width: Int,
        height: Int,
        generation: Long,
    ): Boolean
    private external fun nativeDetachSurface(generation: Long)
    private external fun nativePresentFrame(
        frameId: Long,
        hardwareBuffer: android.hardware.HardwareBuffer?,
        width: Int,
        height: Int,
        format: Int,
        usage: Long,
        acquireFenceFd: Int,
        generation: Long,
    ): Boolean
    private external fun nativeReleaseFrame(frameId: Long)
    private external fun nativeDrainReleasedFrames(): LongArray?
}
