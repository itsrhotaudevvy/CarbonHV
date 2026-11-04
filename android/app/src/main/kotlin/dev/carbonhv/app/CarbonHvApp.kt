package dev.carbonhv.app

import android.os.SystemClock
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.carbonhv.domain.AdmissionText
import dev.carbonhv.domain.HostStatus
import dev.carbonhv.domain.RecoveryPolicy
import dev.carbonhv.domain.SessionUiState
import dev.carbonhv.host.ICarbonHvHostService
import dev.carbonhv.session.HostVmControlResult
import dev.carbonhv.session.HostVmStartSpec
import dev.carbonhv.session.HostVmStopResult
import dev.carbonhv.session.HostSessionBridge
import dev.carbonhv.session.ImmersiveSessionScreen
import dev.carbonhv.session.SessionControlEvent
import dev.carbonhv.session.SessionControlState
import dev.carbonhv.session.reduceSessionControl
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.roundToInt

@Composable
fun CarbonHvApp(hostService: ICarbonHvHostService? = null) {
    var state by remember { mutableStateOf(AppState.initial()) }
    val dispatch: (AppEvent) -> Unit = { event -> state = reduceApp(state, event) }

    MaterialTheme {
        Surface(modifier = Modifier.fillMaxSize()) {
            val showNavigation = state.screen in setOf(
                AppScreen.Library,
                AppScreen.Catalog,
                AppScreen.Settings,
            )
            Scaffold(
                bottomBar = {
                    if (showNavigation) {
                        MainNavigation(state.screen, dispatch)
                    }
                },
            ) { padding ->
                when (state.screen) {
                    AppScreen.Library -> LibraryScreen(state, dispatch, padding)
                    AppScreen.Catalog -> CatalogScreen(state, dispatch, padding)
                    AppScreen.Create -> CreateScreen(state, dispatch, padding)
                    AppScreen.Settings -> SettingsScreen(padding)
                    AppScreen.Session -> SessionScreen(state, dispatch, hostService)
                    AppScreen.Recovery -> RecoveryScreen(state, dispatch, padding)
                }
            }
        }
    }
}

@Composable
private fun MainNavigation(screen: AppScreen, dispatch: (AppEvent) -> Unit) {
    NavigationBar {
        NavigationBarItem(
            selected = screen == AppScreen.Library,
            onClick = { dispatch(AppEvent.OpenLibrary) },
            icon = { Text("VM") },
            label = { Text("Library") },
        )
        NavigationBarItem(
            selected = screen == AppScreen.Catalog,
            onClick = { dispatch(AppEvent.OpenCatalog) },
            icon = { Text("+") },
            label = { Text("Catalog") },
        )
        NavigationBarItem(
            selected = screen == AppScreen.Settings,
            onClick = { dispatch(AppEvent.OpenSettings) },
            icon = { Text("≡") },
            label = { Text("Settings") },
        )
    }
}

@Composable
private fun Page(
    title: String,
    padding: PaddingValues,
    content: @Composable () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(padding)
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(title, style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold)
        content()
    }
}

@Composable
private fun LibraryScreen(state: AppState, dispatch: (AppEvent) -> Unit, padding: PaddingValues) {
    Page("Virtual machines", padding) {
        if (state.instances.isEmpty()) {
            Card(modifier = Modifier.fillMaxWidth()) {
                Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text("No virtual machines yet", style = MaterialTheme.typography.titleMedium)
                    Text("Install a curated mobile OS bundle and create an isolated local instance.")
                    Button(onClick = { dispatch(AppEvent.OpenCatalog) }) { Text("Browse systems") }
                }
            }
        } else {
            LazyColumn(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                items(state.instances, key = { it.id }) { instance ->
                    Card(modifier = Modifier.fillMaxWidth()) {
                        Row(
                            modifier = Modifier.padding(16.dp).fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Column {
                                Text(instance.name, style = MaterialTheme.typography.titleMedium)
                                Text(instance.bundleId)
                                Text(instance.sessionState.name, style = MaterialTheme.typography.bodySmall)
                            }
                            Button(onClick = { dispatch(AppEvent.OpenSession(instance.id)) }) {
                                Text("Open")
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CatalogScreen(state: AppState, dispatch: (AppEvent) -> Unit, padding: PaddingValues) {
    Page("Systems", padding) {
        LazyColumn(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            items(state.catalog, key = { it.id }) { bundle ->
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(bundle.name, style = MaterialTheme.typography.titleLarge)
                        Text(bundle.description)
                        Text("${bundle.integrationProfile} integration · ${bundle.recommendedMemoryMiB} MiB recommended")
                        Button(onClick = { dispatch(AppEvent.OpenCreate(bundle.id)) }) {
                            Text("Create VM")
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CreateScreen(state: AppState, dispatch: (AppEvent) -> Unit, padding: PaddingValues) {
    val bundle = state.catalog.firstOrNull { it.id == state.createBundleId }
    Page("Create ${bundle?.name ?: "VM"}", padding) {
        OutlinedTextField(
            value = state.createName,
            onValueChange = { dispatch(AppEvent.CreateNameChanged(it)) },
            modifier = Modifier.fillMaxWidth(),
            label = { Text("Name") },
            singleLine = true,
        )
        Text("CPU virtualization and accelerated graphics are required before this instance can launch.")
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(onClick = { dispatch(AppEvent.OpenCatalog) }) { Text("Cancel") }
            Button(
                onClick = { dispatch(AppEvent.ConfirmCreate) },
                enabled = state.canCreate,
            ) { Text("Create") }
        }
    }
}

@Composable
private fun SettingsScreen(padding: PaddingValues) {
    Page("Settings", padding) {
        Card(modifier = Modifier.fillMaxWidth()) {
            Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Host compatibility", style = MaterialTheme.typography.titleMedium)
                Text(AdmissionText.forStatus(HostStatus.Unsupported))
                Text("A qualified CarbonHV host integration will replace this diagnostic state at runtime.")
            }
        }
        Card(modifier = Modifier.fillMaxWidth()) {
            Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Privacy", style = MaterialTheme.typography.titleMedium)
                Text("GrapheneOS-derived instances default to Private sharing. LineageOS, PixelOS and Ubuntu Touch default to Balanced sharing.")
            }
        }
    }
}

@Composable
private fun SessionScreen(
    state: AppState,
    dispatch: (AppEvent) -> Unit,
    hostService: ICarbonHvHostService?,
) {
    val instance = state.instances.firstOrNull { it.id == state.activeInstanceId }
    val bundle = state.catalog.firstOrNull { it.id == instance?.bundleId }
    val context = LocalContext.current
    val metrics = context.resources.displayMetrics
    val startRequestId = remember(instance?.id) { SystemClock.elapsedRealtimeNanos().coerceAtLeast(0L) }
    val stopRequestId = remember(startRequestId) {
        if (startRequestId == Long.MAX_VALUE) startRequestId else startRequestId + 1
    }
    var retryToken by remember(instance?.id) { mutableStateOf(0) }
    var stopRequested by remember(instance?.id) { mutableStateOf(false) }
    var controlState by remember(instance?.id) {
        mutableStateOf<SessionControlState>(SessionControlState.Starting)
    }

    LaunchedEffect(hostService, instance?.id, retryToken) {
        if (instance == null || bundle == null) return@LaunchedEffect
        if (hostService == null) {
            controlState = reduceSessionControl(controlState, SessionControlEvent.HostUnavailable)
            return@LaunchedEffect
        }
        controlState = SessionControlState.Starting
        dispatch(AppEvent.SessionStateChanged(instance.id, SessionUiState.Starting))
        val refreshRate = context.display.refreshRate.roundToInt().coerceAtLeast(1)
        val spec = HostVmStartSpec(
            requestId = startRequestId,
            instanceId = instance.id,
            bundleId = instance.bundleId,
            memoryMib = bundle.recommendedMemoryMiB,
            vcpus = 4,
            widthPixels = metrics.widthPixels.coerceAtLeast(1),
            heightPixels = metrics.heightPixels.coerceAtLeast(1),
            densityDpi = metrics.densityDpi.coerceAtLeast(1),
            refreshRate = refreshRate,
        )
        when (val result = withContext(Dispatchers.IO) { HostSessionBridge(hostService).startOrAttach(spec) }) {
            is HostVmControlResult.Running -> {
                controlState = reduceSessionControl(
                    controlState,
                    SessionControlEvent.Running(result.identity),
                )
                dispatch(AppEvent.SessionStateChanged(instance.id, SessionUiState.Running))
            }
            is HostVmControlResult.StaleGeneration -> {
                controlState = reduceSessionControl(
                    controlState,
                    SessionControlEvent.StaleGeneration(result.started, result.authoritative),
                )
                dispatch(AppEvent.SessionStateChanged(instance.id, SessionUiState.Crashed))
            }
            is HostVmControlResult.RuntimeFailure -> {
                controlState = reduceSessionControl(
                    controlState,
                    SessionControlEvent.RuntimeFailure(result.message),
                )
                dispatch(AppEvent.SessionStateChanged(instance.id, SessionUiState.Crashed))
            }
        }
    }

    LaunchedEffect(stopRequested) {
        if (!stopRequested) return@LaunchedEffect
        stopRequested = false
        val running = controlState as? SessionControlState.Running ?: return@LaunchedEffect
        if (hostService == null || instance == null) {
            controlState = reduceSessionControl(controlState, SessionControlEvent.HostUnavailable)
            return@LaunchedEffect
        }
        dispatch(AppEvent.SessionStateChanged(instance.id, SessionUiState.Stopping))
        when (val result = withContext(Dispatchers.IO) {
            HostSessionBridge(hostService).stop(running.identity, stopRequestId)
        }) {
            HostVmStopResult.Stopped -> {
                dispatch(AppEvent.SessionStateChanged(instance.id, SessionUiState.Stopped))
                dispatch(AppEvent.LeaveSession)
            }
            is HostVmStopResult.StaleGeneration -> {
                controlState = reduceSessionControl(
                    controlState,
                    SessionControlEvent.StaleGeneration(running.identity, result.authoritative),
                )
                dispatch(AppEvent.SessionStateChanged(instance.id, SessionUiState.Crashed))
            }
            is HostVmStopResult.RuntimeFailure -> {
                controlState = reduceSessionControl(
                    controlState,
                    SessionControlEvent.RuntimeFailure(result.message),
                )
                dispatch(AppEvent.SessionStateChanged(instance.id, SessionUiState.Crashed))
            }
        }
    }

    when (val current = controlState) {
        SessionControlState.Starting -> SessionStatusScreen(
            title = "Starting ${instance?.name ?: "virtual machine"}",
            detail = "Connecting to the native CarbonHV host runtime…",
            onRetry = null,
            onLeave = { dispatch(AppEvent.LeaveSession) },
        )
        SessionControlState.HostUnavailable -> SessionStatusScreen(
            title = "Host integration unavailable",
            detail = "This Android build does not currently expose the CarbonHV platform service.",
            onRetry = {
                controlState = reduceSessionControl(controlState, SessionControlEvent.Retry)
                retryToken += 1
            },
            onLeave = { dispatch(AppEvent.LeaveSession) },
        )
        is SessionControlState.StaleGeneration -> SessionStatusScreen(
            title = "Session changed",
            detail = "The broker moved from generation ${current.started.generation} to ${current.authoritative?.generation ?: "stopped"}. Reconnect before attaching the display.",
            onRetry = {
                controlState = reduceSessionControl(controlState, SessionControlEvent.Retry)
                retryToken += 1
            },
            onLeave = { dispatch(AppEvent.LeaveSession) },
        )
        is SessionControlState.RuntimeFailure -> SessionStatusScreen(
            title = "Virtual machine could not start",
            detail = current.message,
            onRetry = {
                controlState = reduceSessionControl(controlState, SessionControlEvent.Retry)
                retryToken += 1
            },
            onLeave = { dispatch(AppEvent.LeaveSession) },
        )
        is SessionControlState.Running -> {
            val callbacks = remember(hostService, current.identity) {
                hostService?.let { HostSessionBridge(it).callbacksFor(current.identity) }
            }
            if (callbacks == null) {
                SessionStatusScreen(
                    title = "Host integration unavailable",
                    detail = "The host service disconnected from this running session.",
                    onRetry = {
                        controlState = reduceSessionControl(controlState, SessionControlEvent.HostUnavailable)
                        retryToken += 1
                    },
                    onLeave = { dispatch(AppEvent.LeaveSession) },
                )
            } else {
                ImmersiveSessionScreen(
                    instanceName = instance?.name ?: "Session",
                    callbacks = callbacks,
                    onLeave = { dispatch(AppEvent.LeaveSession) },
                    onStop = { stopRequested = true },
                )
            }
        }
    }
}

@Composable
private fun SessionStatusScreen(
    title: String,
    detail: String,
    onRetry: (() -> Unit)?,
    onLeave: () -> Unit,
) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Card(modifier = Modifier.padding(24.dp)) {
            Column(
                modifier = Modifier.padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(title, style = MaterialTheme.typography.titleLarge)
                Text(detail)
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    if (onRetry != null) Button(onClick = onRetry) { Text("Retry") }
                    OutlinedButton(onClick = onLeave) { Text("Library") }
                }
            }
        }
    }
}

@Composable
private fun RecoveryScreen(state: AppState, dispatch: (AppEvent) -> Unit, padding: PaddingValues) {
    Page("Recovery", padding) {
        Text("CarbonHV preserved the VM instance and blocked an unsafe automatic action.")
        state.recoveryFailure?.let { failure ->
            Text("Reason: ${failure.name}")
            Text("Available actions: ${RecoveryPolicy.actionsFor(failure).joinToString { it.name }}")
        }
        Button(onClick = { dispatch(AppEvent.OpenLibrary) }) { Text("Return to library") }
    }
}
