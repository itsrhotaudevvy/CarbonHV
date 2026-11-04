package dev.carbonhv.host

class HostCallerPolicy(
    private val runtimeUid: Int,
    allowedUiSigningSha256: String,
) {
    private val allowedUiSigningSha256 = normalizeDigest(allowedUiSigningSha256)

    fun isRuntimeCaller(uid: Int): Boolean = uid == runtimeUid

    fun isUiCaller(signingDigests: Set<String>): Boolean {
        if (allowedUiSigningSha256.isEmpty()) return false
        return signingDigests.any { normalizeDigest(it) == allowedUiSigningSha256 }
    }

    private fun normalizeDigest(value: String): String =
        value.trim().replace(":", "").uppercase()
}
