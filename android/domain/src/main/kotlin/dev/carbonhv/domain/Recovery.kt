package dev.carbonhv.domain

enum class FailureKind {
    HostUpdateIncompatible,
    GuestBootFailed,
    RendererFailed,
    BundleVerificationFailed,
    InsufficientResources,
}

enum class RecoveryAction {
    Retry,
    ViewDiagnostics,
    KeepInstance,
    UsePreviousSystem,
    FreeSpace,
}

object RecoveryPolicy {
    fun actionsFor(failure: FailureKind): List<RecoveryAction> = when (failure) {
        FailureKind.HostUpdateIncompatible ->
            listOf(RecoveryAction.ViewDiagnostics, RecoveryAction.KeepInstance)
        FailureKind.GuestBootFailed ->
            listOf(RecoveryAction.Retry, RecoveryAction.ViewDiagnostics)
        FailureKind.RendererFailed ->
            listOf(RecoveryAction.Retry, RecoveryAction.ViewDiagnostics)
        FailureKind.BundleVerificationFailed ->
            listOf(RecoveryAction.ViewDiagnostics, RecoveryAction.KeepInstance)
        FailureKind.InsufficientResources ->
            listOf(RecoveryAction.FreeSpace, RecoveryAction.KeepInstance)
    }
}
