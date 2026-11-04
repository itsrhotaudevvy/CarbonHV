package dev.carbonhv.host

import android.app.Service
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Binder
import android.os.IBinder
import android.os.Process
import android.os.ServiceManager
import android.os.SystemProperties
import android.view.Surface
import java.security.MessageDigest

class CarbonHvHostService : Service() {
    private lateinit var callerPolicy: HostCallerPolicy
    private lateinit var brokerClient: PlatformCarbonHvBrokerClient
    private lateinit var coordinator: HostDisplayCoordinator<Surface>
    private lateinit var runtime: HostVmRuntimeCoordinator<GraphicalVmLaunchConfig>
    private val launchConfigLock = Any()
    private var launchConfig: Pair<DisplaySessionIdentity, GraphicalVmLaunchConfig>? = null

    override fun onCreate() {
        super.onCreate()
        callerPolicy = HostCallerPolicy(
            runtimeUid = Process.SYSTEM_UID,
            allowedUiSigningSha256 = SystemProperties.get(UI_SIGNING_DIGEST_PROPERTY, ""),
        )
        brokerClient = PlatformCarbonHvBrokerClient()
        coordinator = HostDisplayCoordinator(Android17CrosvmDisplayBackend())
        runtime = HostVmRuntimeCoordinator(
            coordinator,
            Android17GraphicalVmBackend(applicationContext),
        ) { identity ->
            brokerClient.notifyRuntimeDied(identity.instanceId, identity.generation)
        }
        ServiceManager.addService(RUNTIME_SERVICE_NAME, runtimeBinder)
    }

    private val runtimeBinder = object : ICarbonHvHostRuntime.Stub() {
        override fun startGraphicalVm(request: GraphicalVmLaunchRequest) {
            enforceRuntimeCaller()
            requireGeneration(request.generation)
            require(validInstanceId(request.instanceId)) { "Invalid CarbonHV instance id" }
            val identity = DisplaySessionIdentity(request.instanceId, request.generation)
            val config = request.toLaunchConfig()
            val validationError = config.validationError(MANAGED_OBJECT_ROOT)
            require(validationError == null) { "Invalid graphical VM launch request: $validationError" }

            when (val result = runtime.start(identity, config)) {
                HostVmStartResult.Started -> synchronized(launchConfigLock) {
                    launchConfig = identity to config
                }
                HostVmStartResult.AlreadyActive -> synchronized(launchConfigLock) {
                    check(launchConfig == identity to config) {
                        "Duplicate start changed the active graphical VM configuration"
                    }
                }
                is HostVmStartResult.Busy -> error(
                    "Graphical VM slot is owned by ${result.active.instanceId}/${result.active.generation}",
                )
            }
        }

        override fun stopGraphicalVm(instanceId: String, generation: Long) {
            enforceRuntimeCaller()
            requireGeneration(generation)
            val identity = DisplaySessionIdentity(instanceId, generation)
            check(runtime.stop(identity)) { "Graphical VM identity is stale or inactive" }
            synchronized(launchConfigLock) {
                if (launchConfig?.first == identity) launchConfig = null
            }
        }
    }

    private val uiBinder = object : ICarbonHvHostService.Stub() {
        override fun startGraphicalVm(request: HostGraphicalVmStartRequest): Long {
            enforceUiCaller()
            return brokerClient.startGraphicalVm(request)
        }

        override fun stopVm(instanceId: String, generation: Long, requestId: Long) {
            enforceUiCaller()
            requireGeneration(generation)
            require(requestId >= 0) { "VM stop request id must be non-negative" }
            brokerClient.stopVm(instanceId, generation, requestId)
        }

        override fun activeDisplayGeneration(instanceId: String): Long {
            enforceUiCaller()
            return brokerClient.activeDisplayGeneration(instanceId)
        }

        override fun attachMainSurface(instanceId: String, generation: Long, surface: Surface) {
            enforceUiCaller()
            requireGeneration(generation)
            checkAuthorized(
                coordinator.attach(DisplaySessionIdentity(instanceId, generation), surface),
            )
        }

        override fun detachMainSurface(instanceId: String, generation: Long) {
            enforceUiCaller()
            requireGeneration(generation)
            checkAuthorized(
                coordinator.detach(DisplaySessionIdentity(instanceId, generation)),
            )
        }

        override fun sendGuestKey(
            instanceId: String,
            generation: Long,
            linuxKeyCode: Int,
            down: Boolean,
        ): Boolean {
            enforceUiCaller()
            requireGeneration(generation)
            if (linuxKeyCode !in 0..0xffff) return false
            return runtime.sendInput(
                DisplaySessionIdentity(instanceId, generation),
                HostGuestInput.Key(linuxKeyCode, down),
            )
        }

        override fun sendGuestTouch(
            instanceId: String,
            generation: Long,
            action: Int,
            actionPointerId: Int,
            viewportWidth: Int,
            viewportHeight: Int,
            pointerIds: IntArray,
            xs: FloatArray,
            ys: FloatArray,
        ): Boolean {
            enforceUiCaller()
            requireGeneration(generation)
            val identity = DisplaySessionIdentity(instanceId, generation)
            if (runtime.current() != identity) return false
            val config = synchronized(launchConfigLock) {
                launchConfig?.takeIf { it.first == identity }?.second
            } ?: return false
            val input = HostTouchMapper.map(
                action = action,
                actionPointerId = actionPointerId,
                viewportWidth = viewportWidth,
                viewportHeight = viewportHeight,
                guestWidth = config.widthPixels,
                guestHeight = config.heightPixels,
                pointerIds = pointerIds,
                xs = xs,
                ys = ys,
            ) ?: return false
            return runtime.sendInput(identity, input)
        }
    }

    override fun onBind(intent: Intent?): IBinder = uiBinder

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY

    override fun onDestroy() {
        runtime.current()?.let { identity -> runCatching { runtime.stop(identity) } }
        synchronized(launchConfigLock) { launchConfig = null }
        super.onDestroy()
    }

    private fun enforceRuntimeCaller() {
        val uid = Binder.getCallingUid()
        if (!callerPolicy.isRuntimeCaller(uid)) {
            throw SecurityException("Caller uid $uid cannot publish CarbonHV runtime state")
        }
    }

    private fun enforceUiCaller() {
        val uid = Binder.getCallingUid()
        val digests = signingDigestsForUid(uid)
        if (!callerPolicy.isUiCaller(digests)) {
            throw SecurityException("Caller uid $uid is not the configured CarbonHV UI signer")
        }
    }

    private fun signingDigestsForUid(uid: Int): Set<String> {
        val packages = packageManager.getPackagesForUid(uid)?.asIterable().orEmpty()
        return packages.flatMapTo(mutableSetOf()) { packageName ->
            val info = packageManager.getPackageInfo(
                packageName,
                PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()),
            )
            info.signingInfo?.apkContentsSigners.orEmpty().map(::sha256Digest)
        }
    }

    private fun sha256Digest(signature: android.content.pm.Signature): String {
        val digest = MessageDigest.getInstance("SHA-256").digest(signature.toByteArray())
        return digest.joinToString(":") { byte -> "%02X".format(byte.toInt() and 0xff) }
    }

    private fun requireGeneration(generation: Long) {
        require(generation >= 0) { "VM generation must be non-negative" }
    }

    private fun validInstanceId(instanceId: String): Boolean =
        instanceId.length in 1..64 && instanceId.all { character ->
            character.isLetterOrDigit() || character == '_' || character == '-' || character == '.'
        }

    private fun GraphicalVmLaunchRequest.toLaunchConfig(): GraphicalVmLaunchConfig =
        GraphicalVmLaunchConfig(
            kernelPath = kernelPath,
            initrdPath = initrdPath,
            disks = disks.map { disk -> GraphicalVmDiskConfig(disk.path, disk.writable) },
            memoryMib = memoryMib,
            vcpus = vcpus,
            kernelParams = kernelParams.toList(),
            widthPixels = widthPixels,
            heightPixels = heightPixels,
            densityDpi = densityDpi,
            refreshRate = refreshRate,
            performance = GraphicalVmPerformanceConfig(
                hugePages = hugePages,
                boostUclamp = boostUclamp,
                balloon = balloon,
            ),
        )

    private fun checkAuthorized(admission: DisplaySessionAdmission) {
        if (admission != DisplaySessionAdmission.Authorized) {
            throw SecurityException("Display session rejected: $admission")
        }
    }

    companion object {
        const val RUNTIME_SERVICE_NAME = "dev.carbonhv.host.runtime"
        private const val UI_SIGNING_DIGEST_PROPERTY = "ro.carbonhv.ui_signing_sha256"
        private const val MANAGED_OBJECT_ROOT = "/data/misc/carbonhv/objects"
    }
}
