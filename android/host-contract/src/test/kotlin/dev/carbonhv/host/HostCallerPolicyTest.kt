package dev.carbonhv.host

import org.junit.Assert.assertEquals
import org.junit.Test

class HostCallerPolicyTest {
    private val policy = HostCallerPolicy(
        runtimeUid = 1000,
        allowedUiSigningSha256 = "AA:BB:CC",
    )

    @Test
    fun runtime_operations_require_the_configured_runtime_uid() {
        assertEquals(true, policy.isRuntimeCaller(1000))
        assertEquals(false, policy.isRuntimeCaller(10123))
    }

    @Test
    fun ui_operations_require_an_exact_normalized_signing_digest() {
        assertEquals(true, policy.isUiCaller(setOf("aa:bb:cc")))
        assertEquals(true, policy.isUiCaller(setOf("11:22", "AA:BB:CC")))
        assertEquals(false, policy.isUiCaller(setOf("AA:BB")))
        assertEquals(false, policy.isUiCaller(emptySet()))
    }

    @Test
    fun compact_product_digest_matches_colon_separated_runtime_digest() {
        val compact = HostCallerPolicy(
            runtimeUid = 1000,
            allowedUiSigningSha256 = "aabbcc",
        )

        assertEquals(true, compact.isUiCaller(setOf("AA:BB:CC")))
    }

    @Test
    fun blank_ui_digest_configuration_fails_closed() {
        val missing = HostCallerPolicy(runtimeUid = 1000, allowedUiSigningSha256 = "  ")
        assertEquals(false, missing.isUiCaller(setOf("AA:BB:CC")))
    }
}
