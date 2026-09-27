package dev.carbonhv.session

import android.hardware.HardwareBuffer
import android.view.Surface

data class NativeFrame(
    val frameId: Long,
    val sequence: Long,
    val hardwareBuffer: HardwareBuffer?,
    val width: Int,
    val height: Int,
    val format: Int,
    val usage: Long,
    val acquireFenceFd: Int,
)

enum class DropReason {
    NO_SURFACE,
    STALE_GENERATION,
    OBSOLETE_FRAME,
    QUEUE_FULL,
    INVALID_FRAME,
    NATIVE_REJECTED,
}

sealed interface PresentResult {
    data object Presented : PresentResult
    data class Dropped(val reason: DropReason) : PresentResult
}

internal interface GraphicsPresentationBackend {
    fun attachSurface(surface: Surface, width: Int, height: Int, generation: Long): Boolean
    fun resizeSurface(surface: Surface, width: Int, height: Int, generation: Long): Boolean
    fun detachSurface(generation: Long)
    fun presentFrame(frame: NativeFrame, generation: Long): Boolean
    fun releaseFrame(frameId: Long)
}

internal object JniGraphicsPresentationBackend : GraphicsPresentationBackend {
    override fun attachSurface(surface: Surface, width: Int, height: Int, generation: Long): Boolean =
        GraphicsNative.attachSurface(surface, width, height, generation)

    override fun resizeSurface(surface: Surface, width: Int, height: Int, generation: Long): Boolean =
        GraphicsNative.resizeSurface(surface, width, height, generation)

    override fun detachSurface(generation: Long) {
        GraphicsNative.detachSurface(generation)
    }

    override fun presentFrame(frame: NativeFrame, generation: Long): Boolean =
        GraphicsNative.presentFrame(frame, generation)

    override fun releaseFrame(frameId: Long) {
        GraphicsNative.releaseFrame(frameId)
    }
}

class GraphicsSession internal constructor(
    val vmGeneration: Long,
    private val backend: GraphicsPresentationBackend,
    private val maxInFlightFrames: Int = 3,
) {
    constructor(vmGeneration: Long, maxInFlightFrames: Int = 3) : this(
        vmGeneration = vmGeneration,
        backend = JniGraphicsPresentationBackend,
        maxInFlightFrames = maxInFlightFrames,
    )

    init {
        require(maxInFlightFrames > 0)
    }

    var presentationGeneration: Long = 0
        private set

    var isSurfaceAttached: Boolean = false
        private set

    private val inFlightFrameIds = LinkedHashSet<Long>()
    private var lastAcceptedSequence = Long.MIN_VALUE

    val inFlightFrameCount: Int
        get() = inFlightFrameIds.size

    fun attachSurface(surface: Surface, width: Int, height: Int): Long {
        require(width > 0 && height > 0)
        advancePresentationGeneration()
        isSurfaceAttached = backend.attachSurface(surface, width, height, presentationGeneration)
        return presentationGeneration
    }

    fun resizeSurface(surface: Surface, width: Int, height: Int): Long {
        require(width > 0 && height > 0)
        advancePresentationGeneration()
        isSurfaceAttached = backend.resizeSurface(surface, width, height, presentationGeneration)
        return presentationGeneration
    }

    fun detachSurface() {
        advancePresentationGeneration()
        backend.detachSurface(presentationGeneration)
        isSurfaceAttached = false
    }

    fun presentFrame(frame: NativeFrame, presentationGeneration: Long): PresentResult {
        if (!isSurfaceAttached) return PresentResult.Dropped(DropReason.NO_SURFACE)
        if (presentationGeneration != this.presentationGeneration) {
            return PresentResult.Dropped(DropReason.STALE_GENERATION)
        }
        if (!frame.hasValidMetadata()) return PresentResult.Dropped(DropReason.INVALID_FRAME)
        if (frame.sequence <= lastAcceptedSequence) {
            return PresentResult.Dropped(DropReason.OBSOLETE_FRAME)
        }
        if (inFlightFrameIds.size >= maxInFlightFrames) {
            return PresentResult.Dropped(DropReason.QUEUE_FULL)
        }
        if (!backend.presentFrame(frame, presentationGeneration)) {
            return PresentResult.Dropped(DropReason.NATIVE_REJECTED)
        }

        lastAcceptedSequence = frame.sequence
        inFlightFrameIds += frame.frameId
        return PresentResult.Presented
    }

    fun onFrameReleased(frameId: Long) {
        if (inFlightFrameIds.remove(frameId)) {
            backend.releaseFrame(frameId)
        }
    }

    fun rendererCrashed() {
        releaseAllFrameLeases()
    }

    private fun advancePresentationGeneration() {
        releaseAllFrameLeases()
        presentationGeneration += 1
        lastAcceptedSequence = Long.MIN_VALUE
    }

    private fun releaseAllFrameLeases() {
        val frameIds = inFlightFrameIds.toList()
        inFlightFrameIds.clear()
        frameIds.forEach(backend::releaseFrame)
    }

    private fun NativeFrame.hasValidMetadata(): Boolean =
        frameId >= 0 &&
            sequence >= 0 &&
            width > 0 &&
            height > 0 &&
            format > 0 &&
            usage != 0L &&
            acquireFenceFd >= -1
}
