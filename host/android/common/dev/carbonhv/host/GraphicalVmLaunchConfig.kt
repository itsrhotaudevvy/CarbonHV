package dev.carbonhv.host

import java.nio.file.Path

data class GraphicalVmDiskConfig(
    val path: String,
    val writable: Boolean,
)

data class GraphicalVmLaunchConfig(
    val kernelPath: String,
    val initrdPath: String,
    val disks: List<GraphicalVmDiskConfig>,
    val memoryMib: Long,
    val vcpus: Int,
    val kernelParams: List<String>,
    val widthPixels: Int,
    val heightPixels: Int,
    val densityDpi: Int,
    val refreshRate: Int,
) {
    fun validationError(managedRoot: String): GraphicalVmLaunchValidationError? {
        if (!isManagedPath(kernelPath, managedRoot) ||
            !isManagedPath(initrdPath, managedRoot) ||
            disks.isEmpty() ||
            disks.any { !isManagedPath(it.path, managedRoot) }
        ) {
            return GraphicalVmLaunchValidationError.UnmanagedPath
        }
        if (memoryMib <= 0 || memoryMib > Int.MAX_VALUE.toLong()) {
            return GraphicalVmLaunchValidationError.InvalidMemory
        }
        if (vcpus <= 0) {
            return GraphicalVmLaunchValidationError.InvalidVcpuCount
        }
        if (widthPixels <= 0 || heightPixels <= 0 ||
            widthPixels > 16_384 || heightPixels > 16_384 ||
            densityDpi <= 0 || densityDpi > 10_000 ||
            refreshRate <= 0 || refreshRate > 1_000
        ) {
            return GraphicalVmLaunchValidationError.InvalidDisplay
        }
        if (kernelParams.size > 64 || kernelParams.any { parameter ->
                parameter.isEmpty() ||
                    parameter.length > 1024 ||
                    parameter.any(Char::isISOControl)
            }
        ) {
            return GraphicalVmLaunchValidationError.InvalidKernelParameter
        }
        if (kernelParams.none { it == "gfxstream_enabled" }) {
            return GraphicalVmLaunchValidationError.MissingGfxstreamParameter
        }
        return null
    }
}

enum class GraphicalVmLaunchValidationError {
    UnmanagedPath,
    InvalidMemory,
    InvalidVcpuCount,
    InvalidDisplay,
    InvalidKernelParameter,
    MissingGfxstreamParameter,
}

private fun isManagedPath(candidate: String, managedRoot: String): Boolean = runCatching {
    val root = Path.of(managedRoot).toAbsolutePath().normalize()
    val path = Path.of(candidate).toAbsolutePath().normalize()
    path != root && path.startsWith(root)
}.getOrDefault(false)
