package dev.carbonhv.session

import kotlin.math.min

data class MappedTouch(
    val x: Float,
    val y: Float,
    val generation: Int,
)

data class GuestGeometry(
    val viewportWidth: Int,
    val viewportHeight: Int,
    val guestWidth: Int,
    val guestHeight: Int,
    val offsetX: Float,
    val offsetY: Float,
    val drawWidth: Float,
    val drawHeight: Float,
    val generation: Int,
) {
    fun mapTouch(hostX: Float, hostY: Float, eventGeneration: Int): MappedTouch? {
        if (eventGeneration != generation) return null
        if (hostX < offsetX || hostY < offsetY) return null
        if (hostX > offsetX + drawWidth || hostY > offsetY + drawHeight) return null
        if (drawWidth <= 0f || drawHeight <= 0f) return null

        val x = ((hostX - offsetX) / drawWidth) * guestWidth
        val y = ((hostY - offsetY) / drawHeight) * guestHeight
        return MappedTouch(x = x, y = y, generation = generation)
    }
}

fun fitGuestGeometry(
    viewportWidth: Int,
    viewportHeight: Int,
    guestWidth: Int,
    guestHeight: Int,
    generation: Int,
): GuestGeometry {
    require(viewportWidth > 0 && viewportHeight > 0)
    require(guestWidth > 0 && guestHeight > 0)
    val scale = min(
        viewportWidth.toFloat() / guestWidth.toFloat(),
        viewportHeight.toFloat() / guestHeight.toFloat(),
    )
    val drawWidth = guestWidth * scale
    val drawHeight = guestHeight * scale
    return GuestGeometry(
        viewportWidth = viewportWidth,
        viewportHeight = viewportHeight,
        guestWidth = guestWidth,
        guestHeight = guestHeight,
        offsetX = (viewportWidth - drawWidth) / 2f,
        offsetY = (viewportHeight - drawHeight) / 2f,
        drawWidth = drawWidth,
        drawHeight = drawHeight,
        generation = generation,
    )
}

class GeometryTracker {
    private var generation = 0
    private var lastDimensions: List<Int>? = null

    fun update(
        viewportWidth: Int,
        viewportHeight: Int,
        guestWidth: Int,
        guestHeight: Int,
    ): GuestGeometry {
        val dimensions = listOf(viewportWidth, viewportHeight, guestWidth, guestHeight)
        if (dimensions != lastDimensions) {
            generation += 1
            lastDimensions = dimensions
        }
        return fitGuestGeometry(
            viewportWidth,
            viewportHeight,
            guestWidth,
            guestHeight,
            generation,
        )
    }
}

