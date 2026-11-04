package dev.carbonhv.app

import dev.carbonhv.domain.FailureKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AppReducerTest {
    @Test
    fun initial_catalog_contains_both_starter_bundles() {
        val state = AppState.initial()

        assertEquals(
            listOf("grapheneos", "lineageos", "pixelos", "ubuntu-touch"),
            state.catalog.map { it.id },
        )
        assertEquals(AppScreen.Library, state.screen)
    }

    @Test
    fun grapheneos_catalog_identity_is_explicitly_downstream() {
        val bundle = AppState.initial().catalog.single { it.id == "grapheneos" }

        assertEquals("GrapheneOS-derived", bundle.name)
        assertTrue(bundle.description.contains("CarbonHV-maintained"))
    }

    @Test
    fun create_flow_requires_non_blank_unique_name() {
        var state = AppState.initial()
        state = reduceApp(state, AppEvent.OpenCreate("grapheneos"))
        assertEquals(AppScreen.Create, state.screen)
        assertFalse(state.canCreate)

        state = reduceApp(state, AppEvent.CreateNameChanged("Private phone"))
        assertTrue(state.canCreate)
        state = reduceApp(state, AppEvent.ConfirmCreate)

        assertEquals(AppScreen.Library, state.screen)
        assertEquals(1, state.instances.size)
        assertEquals("Private phone", state.instances.single().name)

        state = reduceApp(state, AppEvent.OpenCreate("ubuntu-touch"))
        state = reduceApp(state, AppEvent.CreateNameChanged("Private phone"))
        assertFalse(state.canCreate)
    }

    @Test
    fun selecting_instance_opens_session_without_changing_runtime_state() {
        var state = AppState.initial()
        state = reduceApp(state, AppEvent.OpenCreate("ubuntu-touch"))
        state = reduceApp(state, AppEvent.CreateNameChanged("Lomiri"))
        state = reduceApp(state, AppEvent.ConfirmCreate)
        val id = state.instances.single().id

        state = reduceApp(state, AppEvent.OpenSession(id))

        assertEquals(AppScreen.Session, state.screen)
        assertEquals(id, state.activeInstanceId)
        assertEquals(dev.carbonhv.domain.SessionUiState.Stopped, state.instances.single().sessionState)
    }

    @Test
    fun runtime_state_updates_are_scoped_to_the_selected_instance() {
        var state = AppState.initial()
        state = reduceApp(state, AppEvent.OpenCreate("grapheneos"))
        state = reduceApp(state, AppEvent.CreateNameChanged("Private"))
        state = reduceApp(state, AppEvent.ConfirmCreate)
        val id = state.instances.single().id

        state = reduceApp(
            state,
            AppEvent.SessionStateChanged(id, dev.carbonhv.domain.SessionUiState.Running),
        )

        assertEquals(dev.carbonhv.domain.SessionUiState.Running, state.instances.single().sessionState)
        assertEquals("grapheneos", state.instances.single().bundleId)
    }

    @Test
    fun recovery_event_routes_to_recovery_screen_with_failure_context() {
        var state = AppState.initial()
        state = reduceApp(state, AppEvent.RecoveryRequired(FailureKind.HostUpdateIncompatible))

        assertEquals(AppScreen.Recovery, state.screen)
        assertEquals(FailureKind.HostUpdateIncompatible, state.recoveryFailure)
    }

    @Test
    fun custom_rom_ports_are_labelled_unofficial() {
        val catalog = AppState.initial().catalog
        for (id in listOf("lineageos", "pixelos")) {
            val bundle = catalog.single { it.id == id }
            assertTrue("${bundle.name} must be marked unofficial", bundle.unofficial)
            assertFalse(bundle.name.contains("official", ignoreCase = true))
            assertEquals("Balanced", bundle.integrationProfile)
            assertTrue(bundle.platform.endsWith("Android 17"))
        }
        assertFalse(catalog.single { it.id == "grapheneos" }.unofficial)
    }

    @Test
    fun library_subtitle_names_the_system_when_the_instance_was_renamed() {
        val catalog = AppState.initial().catalog
        val graphene = catalog.single { it.id == "grapheneos" }
        val lineage = catalog.single { it.id == "lineageos" }
        assertEquals(
            "GrapheneOS-derived · Android 17",
            instanceSubtitle(VmInstance("vm-1", "Daily driver", "grapheneos"), graphene),
        )
        assertEquals(
            "LineageOS 24 · Android 17",
            instanceSubtitle(VmInstance("vm-2", "LineageOS", "lineageos"), lineage),
        )
        assertEquals("unknown", instanceSubtitle(VmInstance("vm-3", "x", "unknown"), null))
    }

    @Test
    fun enum_names_are_humanized_for_display() {
        assertEquals("Host update incompatible", "HostUpdateIncompatible".humanized())
        assertEquals("Retry", "Retry".humanized())
    }

    @Test
    fun memory_is_shown_in_readable_gigabytes() {
        assertEquals("4 GB", formatMemory(4096))
        assertEquals("3 GB", formatMemory(3072))
        assertEquals("3.5 GB", formatMemory(3584))
        assertEquals("0.5 GB", formatMemory(512))
    }
}
