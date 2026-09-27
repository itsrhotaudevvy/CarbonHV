package dev.carbonhv.session

import android.graphics.SurfaceTexture
import android.view.Surface
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class GraphicsSessionTest {
    @Test
    fun attachResizeDetachAdvancesPresentationGenerationOnly() {
        val backend = FakePresentationBackend()
        val session = GraphicsSession(vmGeneration = 42, backend = backend, maxInFlightFrames = 2)
        val surface = surface()

        val attachedGeneration = session.attachSurface(surface, 1080, 2400)
        val resizedGeneration = session.resizeSurface(surface, 2400, 1080)
        session.detachSurface()

        assertEquals(42, session.vmGeneration)
        assertEquals(attachedGeneration + 1, resizedGeneration)
        assertEquals(resizedGeneration + 1, session.presentationGeneration)
        assertFalse(session.isSurfaceAttached)
        assertEquals(listOf("attach", "resize", "detach"), backend.lifecycle)
    }

    @Test
    fun stalePresentationGenerationIsRejectedBeforeNativePresentation() {
        val backend = FakePresentationBackend()
        val session = GraphicsSession(vmGeneration = 5, backend = backend)
        val surface = surface()
        val staleGeneration = session.attachSurface(surface, 100, 100)
        val currentGeneration = session.resizeSurface(surface, 120, 100)

        val result = session.presentFrame(frame(id = 1, sequence = 1), staleGeneration)

        assertEquals(PresentResult.Dropped(DropReason.STALE_GENERATION), result)
        assertEquals(currentGeneration, session.presentationGeneration)
        assertTrue(backend.presentedFrameIds.isEmpty())
    }

    @Test
    fun surfaceRecreationChangesPresentationGenerationWithoutChangingVmGeneration() {
        val backend = FakePresentationBackend()
        val session = GraphicsSession(vmGeneration = 9, backend = backend)
        val firstSurface = surface()
        val secondSurface = surface()

        val firstGeneration = session.attachSurface(firstSurface, 100, 200)
        session.detachSurface()
        val secondGeneration = session.attachSurface(secondSurface, 100, 200)

        assertTrue(secondGeneration > firstGeneration)
        assertEquals(9, session.vmGeneration)
    }

    @Test
    fun inFlightQueueIsBoundedAndReleasedFramesMakeRoom() {
        val backend = FakePresentationBackend()
        val session = GraphicsSession(vmGeneration = 1, backend = backend, maxInFlightFrames = 2)
        val generation = session.attachSurface(surface(), 100, 100)

        assertEquals(PresentResult.Presented, session.presentFrame(frame(1, 1), generation))
        assertEquals(PresentResult.Presented, session.presentFrame(frame(2, 2), generation))
        assertEquals(
            PresentResult.Dropped(DropReason.QUEUE_FULL),
            session.presentFrame(frame(3, 3), generation),
        )
        assertEquals(2, session.inFlightFrameCount)

        backend.releaseCompletions += NativeFrameRelease(frameId = 1, releaseFenceFd = -1)
        assertEquals(
            listOf(NativeFrameRelease(frameId = 1, releaseFenceFd = -1)),
            session.drainReleasedFrames(),
        )

        assertEquals(PresentResult.Presented, session.presentFrame(frame(3, 3), generation))
        assertEquals(2, session.inFlightFrameCount)
    }

    @Test
    fun drainingNativeReleaseCompletionsReturnsFenceOwnershipAndMakesQueueRoom() {
        val backend = FakePresentationBackend()
        val session = GraphicsSession(vmGeneration = 1, backend = backend, maxInFlightFrames = 1)
        val generation = session.attachSurface(surface(), 100, 100)
        assertEquals(PresentResult.Presented, session.presentFrame(frame(1, 1), generation))
        backend.releaseCompletions += NativeFrameRelease(frameId = 1, releaseFenceFd = 37)

        val releases = session.drainReleasedFrames()

        assertEquals(listOf(NativeFrameRelease(frameId = 1, releaseFenceFd = 37)), releases)
        assertEquals(0, session.inFlightFrameCount)
        assertEquals(PresentResult.Presented, session.presentFrame(frame(2, 2), generation))
    }

    @Test
    fun obsoleteFrameSequenceIsDropped() {
        val backend = FakePresentationBackend()
        val session = GraphicsSession(vmGeneration = 1, backend = backend)
        val generation = session.attachSurface(surface(), 100, 100)

        assertEquals(PresentResult.Presented, session.presentFrame(frame(2, 2), generation))
        backend.releaseCompletions += NativeFrameRelease(frameId = 2, releaseFenceFd = -1)
        session.drainReleasedFrames()
        assertEquals(
            PresentResult.Dropped(DropReason.OBSOLETE_FRAME),
            session.presentFrame(frame(1, 1), generation),
        )
        assertEquals(listOf(2L), backend.presentedFrameIds)
    }

    @Test
    fun rendererCrashReleasesEveryLogicalFrameLease() {
        val backend = FakePresentationBackend()
        val session = GraphicsSession(vmGeneration = 1, backend = backend, maxInFlightFrames = 3)
        val generation = session.attachSurface(surface(), 100, 100)
        session.presentFrame(frame(11, 1), generation)
        session.presentFrame(frame(12, 2), generation)

        session.rendererCrashed()

        assertEquals(0, session.inFlightFrameCount)
        assertEquals(listOf(11L, 12L), backend.releasedFrameIds)
    }

    @Test
    fun surfaceGenerationChangeDoesNotCancelFramesBeforeNativeReleaseCompletion() {
        val backend = FakePresentationBackend()
        val session = GraphicsSession(vmGeneration = 1, backend = backend, maxInFlightFrames = 2)
        val generation = session.attachSurface(surface(), 100, 100)
        session.presentFrame(frame(11, 1), generation)

        session.resizeSurface(surface(), 120, 100)

        assertEquals(1, session.inFlightFrameCount)
        assertTrue(backend.releasedFrameIds.isEmpty())

        backend.releaseCompletions += NativeFrameRelease(frameId = 11, releaseFenceFd = 41)
        assertEquals(
            listOf(NativeFrameRelease(frameId = 11, releaseFenceFd = 41)),
            session.drainReleasedFrames(),
        )
        assertEquals(0, session.inFlightFrameCount)
    }

    private fun frame(id: Long, sequence: Long) = NativeFrame(
        frameId = id,
        sequence = sequence,
        hardwareBuffer = null,
        width = 100,
        height = 100,
        format = 1,
        usage = 0x300L,
        acquireFenceFd = -1,
    )

    private fun surface() = Surface(SurfaceTexture(0))
}

private class FakePresentationBackend : GraphicsPresentationBackend {
    val lifecycle = mutableListOf<String>()
    val presentedFrameIds = mutableListOf<Long>()
    val releasedFrameIds = mutableListOf<Long>()
    val releaseCompletions = mutableListOf<NativeFrameRelease>()

    override fun attachSurface(surface: Surface, width: Int, height: Int, generation: Long): Boolean {
        lifecycle += "attach"
        return true
    }

    override fun resizeSurface(surface: Surface, width: Int, height: Int, generation: Long): Boolean {
        lifecycle += "resize"
        return true
    }

    override fun detachSurface(generation: Long) {
        lifecycle += "detach"
    }

    override fun presentFrame(frame: NativeFrame, generation: Long): Boolean {
        presentedFrameIds += frame.frameId
        return true
    }

    override fun releaseFrame(frameId: Long) {
        releasedFrameIds += frameId
    }

    override fun drainReleasedFrames(): List<NativeFrameRelease> =
        releaseCompletions.toList().also { releaseCompletions.clear() }
}
