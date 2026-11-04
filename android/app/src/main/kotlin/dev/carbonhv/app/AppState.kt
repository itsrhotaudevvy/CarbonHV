package dev.carbonhv.app

import dev.carbonhv.domain.FailureKind
import dev.carbonhv.domain.SessionUiState

enum class AppScreen {
    Library,
    Catalog,
    Create,
    Settings,
    Session,
    Recovery,
}

data class CatalogBundle(
    val id: String,
    val name: String,
    val description: String,
    val integrationProfile: String,
    val recommendedMemoryMiB: Int,
)

data class VmInstance(
    val id: String,
    val name: String,
    val bundleId: String,
    val sessionState: SessionUiState = SessionUiState.Stopped,
)

data class AppState(
    val screen: AppScreen,
    val catalog: List<CatalogBundle>,
    val instances: List<VmInstance>,
    val createBundleId: String? = null,
    val createName: String = "",
    val activeInstanceId: String? = null,
    val recoveryFailure: FailureKind? = null,
) {
    val canCreate: Boolean
        get() {
            val name = createName.trim()
            return createBundleId != null &&
                name.isNotEmpty() &&
                catalog.any { it.id == createBundleId } &&
                instances.none { it.name.equals(name, ignoreCase = true) }
        }

    companion object {
        fun initial(): AppState = AppState(
            screen = AppScreen.Library,
            catalog = listOf(
                CatalogBundle(
                    id = "grapheneos",
                    name = "GrapheneOS-derived",
                    description = "CarbonHV-maintained ARM64 virtual-device build",
                    integrationProfile = "Private",
                    recommendedMemoryMiB = 4096,
                ),
                CatalogBundle(
                    id = "ubuntu-touch",
                    name = "Ubuntu Touch",
                    description = "Lomiri mobile Linux virtual-device port",
                    integrationProfile = "Balanced",
                    recommendedMemoryMiB = 3072,
                ),
            ),
            instances = emptyList(),
        )
    }
}

sealed interface AppEvent {
    data object OpenLibrary : AppEvent
    data object OpenCatalog : AppEvent
    data object OpenSettings : AppEvent
    data class OpenCreate(val bundleId: String) : AppEvent
    data class CreateNameChanged(val name: String) : AppEvent
    data object ConfirmCreate : AppEvent
    data class OpenSession(val instanceId: String) : AppEvent
    data class SessionStateChanged(val instanceId: String, val state: SessionUiState) : AppEvent
    data object LeaveSession : AppEvent
    data class RecoveryRequired(val failure: FailureKind) : AppEvent
}

fun reduceApp(state: AppState, event: AppEvent): AppState = when (event) {
    AppEvent.OpenLibrary -> state.copy(
        screen = AppScreen.Library,
        activeInstanceId = null,
        recoveryFailure = null,
    )
    AppEvent.OpenCatalog -> state.copy(screen = AppScreen.Catalog, recoveryFailure = null)
    AppEvent.OpenSettings -> state.copy(screen = AppScreen.Settings, recoveryFailure = null)
    is AppEvent.OpenCreate -> if (state.catalog.any { it.id == event.bundleId }) {
        state.copy(
            screen = AppScreen.Create,
            createBundleId = event.bundleId,
            createName = "",
            recoveryFailure = null,
        )
    } else {
        state
    }
    is AppEvent.CreateNameChanged -> if (state.screen == AppScreen.Create) {
        state.copy(createName = event.name)
    } else {
        state
    }
    AppEvent.ConfirmCreate -> if (state.screen == AppScreen.Create && state.canCreate) {
        val nextIndex = (state.instances.mapNotNull { it.id.removePrefix("vm-").toIntOrNull() }.maxOrNull() ?: 0) + 1
        val instance = VmInstance(
            id = "vm-$nextIndex",
            name = state.createName.trim(),
            bundleId = requireNotNull(state.createBundleId),
        )
        state.copy(
            screen = AppScreen.Library,
            instances = state.instances + instance,
            createBundleId = null,
            createName = "",
        )
    } else {
        state
    }
    is AppEvent.OpenSession -> if (state.instances.any { it.id == event.instanceId }) {
        state.copy(screen = AppScreen.Session, activeInstanceId = event.instanceId)
    } else {
        state
    }
    is AppEvent.SessionStateChanged -> state.copy(
        instances = state.instances.map { instance ->
            if (instance.id == event.instanceId) instance.copy(sessionState = event.state) else instance
        },
    )
    AppEvent.LeaveSession -> state.copy(screen = AppScreen.Library, activeInstanceId = null)
    is AppEvent.RecoveryRequired -> state.copy(
        screen = AppScreen.Recovery,
        recoveryFailure = event.failure,
    )
}
