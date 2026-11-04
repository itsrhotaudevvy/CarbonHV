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
    /** Upstream base shown under the name, for example "Android 17". */
    val platform: String = "",
    /** Built by CarbonHV from a project that does not publish this virtual device itself. */
    val unofficial: Boolean = false,
)

/** "4 GB" for whole gibibytes, otherwise "3.5 GB"; never fractional noise like "3.999". */
fun formatMemory(mib: Int): String {
    val tenths = (mib * 10 + 512) / 1024
    return if (tenths % 10 == 0) "${tenths / 10} GB" else "${tenths / 10}.${tenths % 10} GB"
}

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
                    description = "Hardened Android as a CarbonHV-maintained ARM64 virtual device.",
                    integrationProfile = "Private",
                    recommendedMemoryMiB = 4096,
                    platform = "Android 17",
                ),
                CatalogBundle(
                    id = "lineageos",
                    name = "LineageOS",
                    description = "The community Android distribution, built for the CarbonHV virtual device.",
                    integrationProfile = "Balanced",
                    recommendedMemoryMiB = 4096,
                    platform = "LineageOS 24 · Android 17",
                    unofficial = true,
                ),
                CatalogBundle(
                    id = "pixelos",
                    name = "PixelOS",
                    description = "A Pixel-style AOSP experience, built for the CarbonHV virtual device.",
                    integrationProfile = "Balanced",
                    recommendedMemoryMiB = 4096,
                    platform = "PixelOS 17 · Android 17",
                    unofficial = true,
                ),
                CatalogBundle(
                    id = "ubuntu-touch",
                    name = "Ubuntu Touch",
                    description = "Mobile Linux with the Lomiri shell, ported to the CarbonHV virtual device.",
                    integrationProfile = "Balanced",
                    recommendedMemoryMiB = 3072,
                    platform = "Ubuntu Touch 24.04",
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

/**
 * Flattens the user-visible state into saveable primitives so the library, an in-progress create
 * form and the open screen survive configuration changes (rotation, dark mode) and process death.
 * The static catalog is not saved; restored instances whose bundle disappeared are dropped.
 */
object AppStateCodec {
    private const val FIELDS_PER_INSTANCE = 4
    private const val HEADER = 6
    private const val VERSION = "1"

    fun encode(state: AppState): List<String> = buildList {
        add(VERSION)
        add(state.screen.name)
        add(state.createBundleId.orEmpty())
        add(state.createName)
        add(state.activeInstanceId.orEmpty())
        add(state.recoveryFailure?.name.orEmpty())
        state.instances.forEach { instance ->
            add(instance.id)
            add(instance.name)
            add(instance.bundleId)
            add(instance.sessionState.name)
        }
    }

    fun decode(saved: List<String>, base: AppState = AppState.initial()): AppState {
        if (saved.size < HEADER || saved[0] != VERSION || (saved.size - HEADER) % FIELDS_PER_INSTANCE != 0) {
            return base
        }
        val bundleIds = base.catalog.map { it.id }.toSet()
        val instances = saved.drop(HEADER).chunked(FIELDS_PER_INSTANCE).mapNotNull { (id, name, bundleId, session) ->
            if (bundleId !in bundleIds) return@mapNotNull null
            VmInstance(
                id = id,
                name = name,
                bundleId = bundleId,
                sessionState = enumValueOrNull<SessionUiState>(session) ?: SessionUiState.Stopped,
            )
        }
        val activeId = saved[4].ifEmpty { null }?.takeIf { id -> instances.any { it.id == id } }
        val screen = enumValueOrNull<AppScreen>(saved[1]) ?: AppScreen.Library
        return base.copy(
            screen = if (screen == AppScreen.Session && activeId == null) AppScreen.Library else screen,
            instances = instances,
            createBundleId = saved[2].ifEmpty { null }?.takeIf { it in bundleIds },
            createName = saved[3],
            activeInstanceId = activeId,
            recoveryFailure = enumValueOrNull<FailureKind>(saved[5]),
        )
    }

    private inline fun <reified T : Enum<T>> enumValueOrNull(name: String): T? =
        enumValues<T>().firstOrNull { it.name == name }
}
