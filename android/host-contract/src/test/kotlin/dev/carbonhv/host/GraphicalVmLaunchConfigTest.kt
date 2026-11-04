package dev.carbonhv.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class GraphicalVmLaunchConfigTest {
    @Test
    fun valid_broker_owned_graphical_request_is_accepted() {
        val config = validConfig()

        assertNull(config.validationError(MANAGED_ROOT))
    }

    @Test
    fun paths_must_stay_under_the_broker_managed_object_root() {
        val config = validConfig().copy(kernelPath = "/data/local/tmp/kernel")

        assertEquals(
            GraphicalVmLaunchValidationError.UnmanagedPath,
            config.validationError(MANAGED_ROOT),
        )
    }

    @Test
    fun raw_avf_limits_are_checked_before_platform_parcelables_are_built() {
        assertEquals(
            GraphicalVmLaunchValidationError.InvalidMemory,
            validConfig().copy(memoryMib = Int.MAX_VALUE.toLong() + 1L).validationError(MANAGED_ROOT),
        )
        assertEquals(
            GraphicalVmLaunchValidationError.InvalidVcpuCount,
            validConfig().copy(vcpus = 0).validationError(MANAGED_ROOT),
        )
        assertEquals(
            GraphicalVmLaunchValidationError.InvalidDisplay,
            validConfig().copy(densityDpi = 0).validationError(MANAGED_ROOT),
        )
    }

    @Test
    fun control_characters_in_kernel_parameters_are_rejected_again_at_host_boundary() {
        assertEquals(
            GraphicalVmLaunchValidationError.InvalidKernelParameter,
            validConfig().copy(kernelParams = listOf("console=hvc0\ninit=/bin/sh"))
                .validationError(MANAGED_ROOT),
        )
    }

    private fun validConfig() = GraphicalVmLaunchConfig(
        kernelPath = "$MANAGED_ROOT/kernel-a",
        initrdPath = "$MANAGED_ROOT/initrd-a",
        disks = listOf(
            GraphicalVmDiskConfig("$MANAGED_ROOT/system-a", writable = false),
            GraphicalVmDiskConfig("$MANAGED_ROOT/data-a", writable = true),
        ),
        memoryMib = 4096,
        vcpus = 4,
        kernelParams = listOf("console=hvc0", "gfxstream_enabled"),
        widthPixels = 1080,
        heightPixels = 2400,
        densityDpi = 420,
        refreshRate = 90,
    )

    private companion object {
        const val MANAGED_ROOT = "/data/misc/carbonhv/objects"
    }
}
