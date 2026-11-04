package dev.carbonhv.host

import android.content.Context
import android.os.IBinder
import android.os.ParcelFileDescriptor
import android.system.Os
import android.system.virtualizationcommon.IGuestAgent
import android.system.virtualizationservice.IVirtualMachine
import android.system.virtualizationservice.IVirtualMachineCallback
import android.system.virtualmachine.CarbonHvVirtualizationServiceBridge
import java.io.IOException
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Android 17 raw-AVF launcher for CarbonHV's controlled system_ext host package.
 *
 * This intentionally mirrors the raw configuration produced by framework-virtualization while
 * using CpuOptions.cpuCount so the broker's exact vCPU limit is preserved. Graphics stay inside
 * crosvm/Rutabaga/gfxstream; CarbonHV does not add a separate renderer process on this path.
 */
class Android17GraphicalVmBackend(
    private val context: Context,
) : GraphicalVmRuntimeBackend<GraphicalVmLaunchConfig> {
    private val lock = Any()
    private var running: RunningVm? = null

    override fun start(
        identity: DisplaySessionIdentity,
        config: GraphicalVmLaunchConfig,
        onDied: () -> Unit,
    ) {
        synchronized(lock) {
            check(running == null) { "A graphical AVF VM is already active" }
        }

        val connection = CarbonHvVirtualizationServiceBridge.connect()
        val resources = mutableListOf<ParcelFileDescriptor>()
        var createdVm: IVirtualMachine? = null
        try {
            val touchPair = socketPair(resources)
            val keyPair = socketPair(resources)
            val mousePair = socketPair(resources)
            val rawConfig = CarbonHvVirtualizationServiceBridge.buildRawConfig(
                context,
                "carbonhv",
                config.kernelPath,
                config.initrdPath,
                config.disks.map { it.path }.toTypedArray(),
                config.disks.map { it.writable }.toBooleanArray(),
                config.kernelParams.toTypedArray(),
                config.memoryMib,
                config.vcpus,
                config.widthPixels,
                config.heightPixels,
                config.densityDpi,
                config.refreshRate,
                GFXSTREAM_BACKEND,
                GFXSTREAM_CONTEXT_TYPES,
                GFXSTREAM_RENDERER_FEATURES,
                config.performance.hugePages,
                config.performance.boostUclamp,
                config.performance.balloon,
                touchPair[1],
                keyPair[1],
                mousePair[1],
            )
            val service = connection.binder
            val vmConfig = android.system.virtualizationservice.VirtualMachineConfig.rawConfig(rawConfig)
            val vm = service.createVm(vmConfig, null, null, null, null)
            createdVm = vm

            val terminated = AtomicBoolean(false)
            val deathAction = {
                if (terminated.compareAndSet(false, true)) {
                    val removed = synchronized(lock) {
                        val current = running
                        if (current?.identity == identity) {
                            running = null
                            current
                        } else {
                            null
                        }
                    }
                    if (removed != null) {
                        removed.close()
                        onDied()
                    }
                }
            }
            val callback = object : IVirtualMachineCallback.Stub() {
                override fun onPayloadStarted(cid: Int) = Unit
                override fun onPayloadReady(cid: Int) = Unit
                override fun onPayloadFinished(cid: Int, exitCode: Int) = Unit
                override fun onError(cid: Int, errorCode: Int, message: String) = Unit
                override fun onDied(cid: Int, reason: Int) = deathAction()
                override fun onGuestAgentRegistered(cid: Int, guestAgent: IGuestAgent) = Unit
            }
            val vmDeathRecipient = IBinder.DeathRecipient { deathAction() }
            val serviceDeathRecipient = IBinder.DeathRecipient { deathAction() }
            vm.registerCallback(callback)
            vm.asBinder().linkToDeath(vmDeathRecipient, 0)
            service.asBinder().linkToDeath(serviceDeathRecipient, 0)

            val handle = RunningVm(
                identity = identity,
                connection = connection,
                vm = vm,
                callback = callback,
                vmDeathRecipient = vmDeathRecipient,
                serviceDeathRecipient = serviceDeathRecipient,
                touchSocket = touchPair[0],
                keySocket = keyPair[0],
                mouseSocket = mousePair[0],
                resources = resources,
                terminated = terminated,
            )
            synchronized(lock) {
                check(running == null) { "A graphical AVF VM became active during launch" }
                check(!terminated.get()) { "Graphical AVF VM died during launch" }
                running = handle
            }
            vm.start()
        } catch (failure: Throwable) {
            synchronized(lock) {
                val current = running
                if (current?.identity == identity) {
                    running = null
                    current.close()
                }
            }
            runCatching { createdVm?.stop() }
            resources.forEach { descriptor -> runCatching { descriptor.close() } }
            throw failure
        }
    }

    override fun stop(identity: DisplaySessionIdentity) {
        val handle = synchronized(lock) {
            val current = running
            if (current?.identity != identity) return
            running = null
            current
        }

        handle.terminated.set(true)
        try {
            handle.vm.stop()
        } finally {
            handle.close()
        }
    }

    override fun sendInput(identity: DisplaySessionIdentity, input: HostGuestInput): Boolean {
        val handle = synchronized(lock) {
            running?.takeIf { it.identity == identity }
        } ?: return false

        val socket = when (input) {
            is HostGuestInput.Key -> handle.keySocket
            is HostGuestInput.Touch -> handle.touchSocket
        }
        val payload = runCatching { EvdevInputEncoder.encode(input) }.getOrElse { return false }
        return runCatching {
            writeFully(socket, payload)
            true
        }.getOrDefault(false)
    }

    private fun socketPair(resources: MutableList<ParcelFileDescriptor>): Array<ParcelFileDescriptor> {
        val pair = ParcelFileDescriptor.createSocketPair()
        resources.addAll(pair)
        return pair
    }

    private fun writeFully(descriptor: ParcelFileDescriptor, payload: ByteArray) {
        var offset = 0
        while (offset < payload.size) {
            val written = Os.write(descriptor.fileDescriptor, payload, offset, payload.size - offset)
            if (written <= 0) throw IOException("Short write to AVF input socket")
            offset += written
        }
    }

    private data class RunningVm(
        val identity: DisplaySessionIdentity,
        val connection: CarbonHvVirtualizationServiceBridge.Connection,
        val vm: IVirtualMachine,
        val callback: IVirtualMachineCallback,
        val vmDeathRecipient: IBinder.DeathRecipient,
        val serviceDeathRecipient: IBinder.DeathRecipient,
        val touchSocket: ParcelFileDescriptor,
        val keySocket: ParcelFileDescriptor,
        val mouseSocket: ParcelFileDescriptor,
        val resources: List<ParcelFileDescriptor>,
        val terminated: AtomicBoolean,
    ) {
        fun close() {
            runCatching { vm.asBinder().unlinkToDeath(vmDeathRecipient, 0) }
            runCatching { connection.binder.asBinder().unlinkToDeath(serviceDeathRecipient, 0) }
            resources.forEach { descriptor -> runCatching { descriptor.close() } }
        }
    }

    private companion object {
        const val GFXSTREAM_BACKEND = "gfxstream"
        val GFXSTREAM_CONTEXT_TYPES = arrayOf("gfxstream-vulkan", "gfxstream-composer")
        const val GFXSTREAM_RENDERER_FEATURES =
            "VulkanDisableCoherentMemoryAndEmulate:enabled;VulkanAllocateHostVisibleAsUdmabuf:enabled;ExternalBlob:enabled"
    }
}
