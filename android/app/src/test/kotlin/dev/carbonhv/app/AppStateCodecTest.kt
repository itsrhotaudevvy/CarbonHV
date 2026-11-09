package dev.carbonhv.app

import dev.carbonhv.domain.FailureKind
import dev.carbonhv.domain.SessionUiState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AppStateCodecTest {
    private fun populated(): AppState {
        var state = AppState.initial()
        for ((bundle, name) in listOf("grapheneos" to "Daily driver", "ubuntu-touch" to "Ubuntu Touch")) {
            state = reduceApp(state, AppEvent.OpenCreate(bundle))
            state = reduceApp(state, AppEvent.CreateNameChanged(name))
            state = reduceApp(state, AppEvent.ConfirmCreate)
        }
        return state
    }

    @Test
    fun library_survives_configuration_changes() {
        val state = populated()
        assertEquals(state, AppStateCodec.decode(AppStateCodec.encode(state)))
    }

    @Test
    fun an_in_progress_create_form_is_restored() {
        var state = reduceApp(populated(), AppEvent.OpenCreate("grapheneos"))
        state = reduceApp(state, AppEvent.CreateNameChanged("Pix"))
        state = reduceApp(state, AppEvent.CreateMemoryChanged(6144))
        val restored = AppStateCodec.decode(AppStateCodec.encode(state))
        assertEquals(AppScreen.Create, restored.screen)
        assertEquals("grapheneos", restored.createBundleId)
        assertEquals("Pix", restored.createName)
        assertEquals(6144, restored.createMemoryMiB)
    }

    @Test
    fun session_and_recovery_fields_round_trip() {
        var state = populated()
        state = reduceApp(state, AppEvent.OpenSession("vm-2"))
        state = reduceApp(state, AppEvent.SessionStateChanged("vm-2", SessionUiState.Crashed))
        state = reduceApp(state, AppEvent.RecoveryRequired(FailureKind.RendererFailed))
        assertEquals(state, AppStateCodec.decode(AppStateCodec.encode(state)))
    }

    @Test
    fun corrupt_or_foreign_saved_state_falls_back_safely() {
        val initial = AppState.initial()
        assertEquals(initial, AppStateCodec.decode(emptyList()))
        assertEquals(initial, AppStateCodec.decode(listOf("99", "Library", "", "", "", "")))
        assertEquals(initial, AppStateCodec.decode(AppStateCodec.encode(initial) + "dangling"))

        val unknownBundle = listOf("1", "Session", "", "", "vm-1", "", "vm-1", "Old", "removed-os", "Running")
        val restored = AppStateCodec.decode(unknownBundle)
        assertEquals(emptyList<VmInstance>(), restored.instances)
        assertNull(restored.activeInstanceId)
        assertEquals(AppScreen.Library, restored.screen)
    }
}
