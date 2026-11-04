package dev.carbonhv.app

import android.os.SystemClock
import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import dev.carbonhv.app.ui.CarbonTheme
import dev.carbonhv.app.ui.osAccent
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
    var state by rememberSaveable(stateSaver = AppStateSaver) { mutableStateOf(AppState.initial()) }
    val dispatch: (AppEvent) -> Unit = { event -> state = reduceApp(state, event) }

    CarbonTheme {
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            when (state.screen) {
                AppScreen.Library, AppScreen.Catalog, AppScreen.Settings ->
                    TopLevelScaffold(state, dispatch)
                AppScreen.Create -> CreateScreen(state, dispatch)
                AppScreen.Session -> SessionScreen(state, dispatch, hostService)
                AppScreen.Recovery -> RecoveryScreen(state, dispatch)
            }
        }
    }
}

private val AppStateSaver: Saver<AppState, List<String>> = Saver(
    save = { AppStateCodec.encode(it) },
    restore = { AppStateCodec.decode(it) },
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TopLevelScaffold(state: AppState, dispatch: (AppEvent) -> Unit) {
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val title = when (state.screen) {
        AppScreen.Catalog -> "Systems"
        AppScreen.Settings -> "Settings"
        else -> "Virtual machines"
    }
    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            LargeTopAppBar(
                title = { Text(title) },
                scrollBehavior = scrollBehavior,
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainer,
                ),
            )
        },
        bottomBar = { MainNavigation(state.screen, dispatch) },
        floatingActionButton = {
            if (state.screen == AppScreen.Library && state.instances.isNotEmpty()) {
                ExtendedFloatingActionButton(
                    onClick = { dispatch(AppEvent.OpenCatalog) },
                    icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                    text = { Text("New VM") },
                )
            }
        },
    ) { padding ->
        when (state.screen) {
            AppScreen.Catalog -> CatalogScreen(state, dispatch, padding)
            AppScreen.Settings -> SettingsScreen(state, padding)
            else -> LibraryScreen(state, dispatch, padding)
        }
    }
}

@Composable
private fun MainNavigation(screen: AppScreen, dispatch: (AppEvent) -> Unit) {
    NavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainer) {
        NavigationBarItem(
            selected = screen == AppScreen.Library,
            onClick = { dispatch(AppEvent.OpenLibrary) },
            icon = { Icon(Icons.Filled.Home, contentDescription = null) },
            label = { Text("Library") },
        )
        NavigationBarItem(
            selected = screen == AppScreen.Catalog,
            onClick = { dispatch(AppEvent.OpenCatalog) },
            icon = { Icon(Icons.Filled.Add, contentDescription = null) },
            label = { Text("Systems") },
        )
        NavigationBarItem(
            selected = screen == AppScreen.Settings,
            onClick = { dispatch(AppEvent.OpenSettings) },
            icon = { Icon(Icons.Filled.Settings, contentDescription = null) },
            label = { Text("Settings") },
        )
    }
}

/** Rounded monogram tile identifying a guest OS without borrowing its logo. */
@Composable
private fun OsBadge(bundleId: String, size: Int = 48) {
    val accent = osAccent(bundleId)
    Box(
        modifier = Modifier
            .size(size.dp)
            .background(accent.container, RoundedCornerShape((size / 3.4f).dp)),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            accent.monogram,
            color = accent.content,
            style = if (size >= 56) MaterialTheme.typography.headlineSmall else MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
        )
    }
}

/** Small rounded label, for example "Unofficial" or "4 GB RAM". */
@Composable
private fun Tag(text: String, emphasized: Boolean = false) {
    val colors = MaterialTheme.colorScheme
    Surface(
        shape = RoundedCornerShape(8.dp),
        color = if (emphasized) colors.tertiaryContainer else colors.surfaceContainerHighest,
        contentColor = if (emphasized) colors.onTertiaryContainer else colors.onSurfaceVariant,
    ) {
        Text(
            text,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
            style = MaterialTheme.typography.labelMedium,
        )
    }
}

@Composable
private fun StatusPill(state: SessionUiState) {
    val colors = MaterialTheme.colorScheme
    val (label, dot) = when (state) {
        SessionUiState.Running -> "Running" to Color(0xFF2E9E5B)
        SessionUiState.Starting -> "Starting" to colors.primary
        SessionUiState.Paused -> "Paused" to colors.tertiary
        SessionUiState.Stopping -> "Stopping" to colors.tertiary
        SessionUiState.Crashed -> "Needs attention" to colors.error
        SessionUiState.Recovering -> "Recovering" to colors.tertiary
        SessionUiState.Stopped -> "Stopped" to colors.outline
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Box(Modifier.size(8.dp).background(dot, CircleShape))
        Text(label, style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant)
    }
}

@Composable
private fun cardColors() = CardDefaults.cardColors(
    containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
)

@Composable
private fun LibraryScreen(state: AppState, dispatch: (AppEvent) -> Unit, padding: PaddingValues) {
    if (state.instances.isEmpty()) {
        EmptyState(
            modifier = Modifier.padding(padding),
            icon = Icons.Filled.PlayArrow,
            title = "No virtual machines yet",
            body = "Pick a mobile OS and CarbonHV creates an isolated instance that runs on this phone's own hardware.",
            actionLabel = "Browse systems",
            onAction = { dispatch(AppEvent.OpenCatalog) },
        )
        return
    }
    LazyColumn(
        contentPadding = PaddingValues(
            start = 16.dp,
            end = 16.dp,
            top = padding.calculateTopPadding() + 4.dp,
            bottom = padding.calculateBottomPadding() + 88.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        items(state.instances, key = { it.id }) { instance ->
            val bundle = state.catalog.firstOrNull { it.id == instance.bundleId }
            Card(
                onClick = { dispatch(AppEvent.OpenSession(instance.id)) },
                modifier = Modifier.fillMaxWidth(),
                colors = cardColors(),
            ) {
                Row(
                    modifier = Modifier.padding(16.dp).fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    OsBadge(instance.bundleId)
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(instance.name, style = MaterialTheme.typography.titleMedium)
                        Text(
                            instanceSubtitle(instance, bundle),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(Modifier.height(4.dp))
                        StatusPill(instance.sessionState)
                    }
                    FilledTonalButton(onClick = { dispatch(AppEvent.OpenSession(instance.id)) }) {
                        Icon(Icons.Filled.PlayArrow, contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.size(6.dp))
                        Text("Open")
                    }
                }
            }
        }
    }
}

/** The system behind an instance, without repeating a name the user kept unchanged. */
internal fun instanceSubtitle(instance: VmInstance, bundle: CatalogBundle?): String = when {
    bundle == null -> instance.bundleId
    bundle.platform.isEmpty() -> bundle.name
    instance.name.equals(bundle.name, ignoreCase = true) -> bundle.platform
    else -> "${bundle.name} · ${bundle.platform}"
}

@Composable
private fun CatalogScreen(state: AppState, dispatch: (AppEvent) -> Unit, padding: PaddingValues) {
    LazyColumn(
        contentPadding = PaddingValues(
            start = 16.dp,
            end = 16.dp,
            top = padding.calculateTopPadding() + 4.dp,
            bottom = padding.calculateBottomPadding() + 16.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item {
            Text(
                "Each system runs as a hardware-virtualized ARM64 guest with its own isolated storage.",
                modifier = Modifier.padding(start = 4.dp, end = 4.dp, bottom = 6.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        items(state.catalog, key = { it.id }) { bundle ->
            Card(modifier = Modifier.fillMaxWidth(), colors = cardColors()) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        OsBadge(bundle.id)
                        Column(Modifier.weight(1f)) {
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Text(bundle.name, style = MaterialTheme.typography.titleMedium)
                                if (bundle.unofficial) Tag("Unofficial", emphasized = true)
                            }
                            Text(
                                bundle.platform,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                    Text(bundle.description, style = MaterialTheme.typography.bodyMedium)
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Tag("${bundle.integrationProfile} sharing")
                        Tag("${formatMemory(bundle.recommendedMemoryMiB)} RAM")
                        Spacer(Modifier.weight(1f))
                        FilledTonalButton(onClick = { dispatch(AppEvent.OpenCreate(bundle.id)) }) {
                            Text("Create")
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CreateScreen(state: AppState, dispatch: (AppEvent) -> Unit) {
    val bundle = state.catalog.firstOrNull { it.id == state.createBundleId }
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = { Text("New virtual machine") },
                navigationIcon = {
                    IconButton(onClick = { dispatch(AppEvent.OpenCatalog) }) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            if (bundle != null) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    OsBadge(bundle.id, size = 56)
                    Column {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Text(bundle.name, style = MaterialTheme.typography.titleLarge)
                            if (bundle.unofficial) Tag("Unofficial", emphasized = true)
                        }
                        Text(
                            "${bundle.platform} · ${formatMemory(bundle.recommendedMemoryMiB)} RAM · ${bundle.integrationProfile} sharing",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
            OutlinedTextField(
                value = state.createName,
                onValueChange = { dispatch(AppEvent.CreateNameChanged(it)) },
                modifier = Modifier.fillMaxWidth(),
                label = { Text("Name") },
                placeholder = { Text(bundle?.name ?: "My phone") },
                supportingText = { Text("Shown in your library. Each name must be unique.") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Words,
                    imeAction = ImeAction.Done,
                ),
            )
            Card(
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.secondaryContainer),
            ) {
                Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Icon(Icons.Filled.Info, contentDescription = null, tint = MaterialTheme.colorScheme.onSecondaryContainer)
                    Text(
                        "Starting this VM needs hardware CPU virtualization and accelerated graphics on this phone. CarbonHV never falls back to slow software emulation.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                    )
                }
            }
            Button(
                onClick = { dispatch(AppEvent.ConfirmCreate) },
                enabled = state.canCreate,
                modifier = Modifier.fillMaxWidth().height(52.dp),
            ) { Text("Create virtual machine") }
        }
    }
}

@Composable
private fun SettingsScreen(state: AppState, padding: PaddingValues) {
    val context = LocalContext.current
    val version = remember {
        runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }
            .getOrNull() ?: "unknown"
    }
    LazyColumn(
        contentPadding = PaddingValues(
            start = 16.dp,
            end = 16.dp,
            top = padding.calculateTopPadding(),
            bottom = padding.calculateBottomPadding() + 16.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item { SectionHeader("This phone") }
        item {
            SettingsGroup {
                SettingsRow(
                    icon = Icons.Filled.Warning,
                    iconTint = MaterialTheme.colorScheme.error,
                    headline = "Host compatibility",
                    supporting = "${AdmissionText.forStatus(HostStatus.Unsupported)}. A CarbonHV-enabled host build replaces this diagnostic at runtime.",
                )
            }
        }
        item { SectionHeader("Privacy defaults") }
        item {
            SettingsGroup {
                state.catalog.forEachIndexed { index, bundle ->
                    if (index > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                    ListItem(
                        leadingContent = { OsBadge(bundle.id, size = 36) },
                        headlineContent = { Text(bundle.name) },
                        supportingContent = { Text("${bundle.integrationProfile} sharing by default") },
                        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                    )
                }
            }
        }
        item { SectionHeader("About") }
        item {
            SettingsGroup {
                SettingsRow(
                    icon = Icons.Filled.Info,
                    headline = "CarbonHV $version",
                    supporting = "Pre-release. No phone is qualified yet; every guest runs only on hardware virtualization.",
                )
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                SettingsRow(
                    icon = Icons.Filled.Lock,
                    headline = "Isolation",
                    supporting = "Each instance has private storage. This app never receives host paths or launch parameters.",
                )
            }
        }
    }
}

@Composable
private fun SectionHeader(text: String) {
    Text(
        text,
        modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 2.dp),
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
    )
}

@Composable
private fun SettingsGroup(content: @Composable () -> Unit) {
    Card(modifier = Modifier.fillMaxWidth(), colors = cardColors()) {
        Column { content() }
    }
}

@Composable
private fun SettingsRow(
    icon: ImageVector,
    headline: String,
    supporting: String,
    iconTint: Color = MaterialTheme.colorScheme.onSurfaceVariant,
) {
    ListItem(
        leadingContent = { Icon(icon, contentDescription = null, tint = iconTint) },
        headlineContent = { Text(headline) },
        supportingContent = { Text(supporting) },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
    )
}

@Composable
private fun EmptyState(
    modifier: Modifier = Modifier,
    icon: ImageVector,
    title: String,
    body: String,
    actionLabel: String,
    onAction: () -> Unit,
) {
    Box(modifier.fillMaxSize().padding(horizontal = 32.dp), contentAlignment = Alignment.Center) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(14.dp),
            modifier = Modifier.widthIn(max = 420.dp),
        ) {
            Box(
                Modifier.size(88.dp).background(MaterialTheme.colorScheme.primaryContainer, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    icon,
                    contentDescription = null,
                    modifier = Modifier.size(40.dp),
                    tint = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            }
            Text(title, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
            Text(
                body,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(4.dp))
            Button(onClick = onAction, modifier = Modifier.height(48.dp)) { Text(actionLabel) }
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
            kind = StatusKind.Progress,
            title = "Starting ${instance?.name ?: "virtual machine"}",
            detail = "Connecting to the native CarbonHV host runtime…",
            onRetry = null,
            onLeave = { dispatch(AppEvent.LeaveSession) },
        )
        SessionControlState.HostUnavailable -> SessionStatusScreen(
            kind = StatusKind.Warning,
            title = "Host integration unavailable",
            detail = "This Android build doesn't include the CarbonHV platform service, so virtual machines can't start here. A CarbonHV-enabled host build is required.",
            onRetry = {
                controlState = reduceSessionControl(controlState, SessionControlEvent.Retry)
                retryToken += 1
            },
            onLeave = { dispatch(AppEvent.LeaveSession) },
        )
        is SessionControlState.StaleGeneration -> SessionStatusScreen(
            kind = StatusKind.Info,
            title = "Session changed",
            detail = "The broker moved from generation ${current.started.generation} to ${current.authoritative?.generation ?: "stopped"}. Reconnect before attaching the display.",
            onRetry = {
                controlState = reduceSessionControl(controlState, SessionControlEvent.Retry)
                retryToken += 1
            },
            onLeave = { dispatch(AppEvent.LeaveSession) },
        )
        is SessionControlState.RuntimeFailure -> SessionStatusScreen(
            kind = StatusKind.Error,
            title = "Virtual machine couldn't start",
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
                    kind = StatusKind.Warning,
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

private enum class StatusKind { Progress, Info, Warning, Error }

@Composable
private fun SessionStatusScreen(
    kind: StatusKind,
    title: String,
    detail: String,
    onRetry: (() -> Unit)?,
    onLeave: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Box(
        modifier = Modifier.fillMaxSize().background(colors.background).padding(horizontal = 32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(14.dp),
            modifier = Modifier.widthIn(max = 420.dp),
        ) {
            val (container, content) = when (kind) {
                StatusKind.Error -> colors.errorContainer to colors.onErrorContainer
                StatusKind.Warning -> colors.tertiaryContainer to colors.onTertiaryContainer
                else -> colors.primaryContainer to colors.onPrimaryContainer
            }
            Box(Modifier.size(88.dp).background(container, CircleShape), contentAlignment = Alignment.Center) {
                when (kind) {
                    StatusKind.Progress -> CircularProgressIndicator(
                        modifier = Modifier.size(40.dp),
                        color = content,
                        strokeWidth = 3.dp,
                    )
                    StatusKind.Info -> Icon(Icons.Filled.Info, null, Modifier.size(40.dp), tint = content)
                    StatusKind.Warning, StatusKind.Error ->
                        Icon(Icons.Filled.Warning, null, Modifier.size(40.dp), tint = content)
                }
            }
            Text(title, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
            Text(
                detail,
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(4.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedButton(onClick = onLeave) { Text("Back to library") }
                if (onRetry != null) Button(onClick = onRetry) { Text("Try again") }
            }
        }
    }
}

@Composable
private fun RecoveryScreen(state: AppState, dispatch: (AppEvent) -> Unit) {
    val failure = state.recoveryFailure
    val detail = buildString {
        append("CarbonHV kept your instance and blocked an unsafe automatic action.")
        if (failure != null) {
            append("\n\nReason: ").append(failure.name.humanized())
            append("\nAvailable: ").append(RecoveryPolicy.actionsFor(failure).joinToString { it.name.humanized() })
        }
    }
    SessionStatusScreen(
        kind = StatusKind.Error,
        title = "Recovery needed",
        detail = detail,
        onRetry = null,
        onLeave = { dispatch(AppEvent.OpenLibrary) },
    )
}

/** "HostUpdateIncompatible" -> "Host update incompatible". */
internal fun String.humanized(): String =
    replace(Regex("(?<=[a-z])(?=[A-Z])"), " ").lowercase().replaceFirstChar { it.uppercase() }
