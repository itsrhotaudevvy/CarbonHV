package dev.carbonhv.emuprobe

import android.app.ActivityManager
import android.content.Context
import android.graphics.Bitmap
import android.graphics.ColorSpace
import android.os.Build
import android.os.PowerManager
import android.os.Process as OsProcess
import java.io.File
import java.io.IOException
import java.util.Collections
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread
import org.json.JSONArray
import org.json.JSONObject

/** One spawned-process run: did it start, was it still running at the deadline, what it printed. */
data class RunResult(
    val started: Boolean,
    val aliveAtDeadline: Boolean,
    val exitCode: Int?,
    val output: List<String>,
    val error: String?,
) {
    fun toJson(keep: (String) -> Boolean = { true }): JSONObject = JSONObject().apply {
        put("started", started)
        put("alive_at_deadline", aliveAtDeadline)
        put("exit_code", exitCode ?: JSONObject.NULL)
        put("error", error ?: JSONObject.NULL)
        put("output", JSONArray(output.filter(keep).take(40)))
    }
}

object Probes {
    private val devicePrefix = Regex("^qemu-system-aarch64: -device [^ ]*: ")
    private const val GPU_DEVICE =
        "virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,x-gfxstream-composer=on,cross-domain=off," +
            "hostmem=256M,wsi=surfaceless"
    const val EXPECTED_PIXEL = 0xFF3366CC.toInt()

    fun run(command: List<String>, seconds: Long): RunResult {
        val process = try {
            ProcessBuilder(command).redirectErrorStream(true).start()
        } catch (e: IOException) {
            return RunResult(false, false, null, emptyList(), e.message)
        }
        val lines = Collections.synchronizedList(mutableListOf<String>())
        val reader = thread(name = "probe-output") {
            runCatching {
                process.inputStream.bufferedReader().forEachLine {
                    if (lines.size < 400) lines.add(it.replace(devicePrefix, ""))
                }
            }
        }
        val exited = process.waitFor(seconds, TimeUnit.SECONDS)
        if (!exited) {
            process.destroy()
            if (!process.waitFor(3, TimeUnit.SECONDS)) process.destroyForcibly()
        }
        reader.join(2000)
        return RunResult(true, !exited, if (exited) process.exitValue() else null, lines.toList(), null)
    }

    fun qemu(context: Context) = File(context.applicationInfo.nativeLibraryDir, "libqemu_system_aarch64.so").path

    /** An empty `virt` machine: TCG translates and runs whatever the empty machine executes. */
    fun emptyMachine(qemu: String, splitWx: String, gpu: Boolean): List<String> = buildList {
        addAll(
            listOf(
                qemu, "-nodefaults", "-no-user-config",
                "-machine", "virt,gic-version=3,highmem=on,mte=off",
                "-accel", "tcg,thread=multi,tb-size=64,split-wx=$splitWx",
                "-cpu", "max,pauth-impdef=on,sve=off,sme=off",
                "-smp", "2", "-m", "256",
                "-object", "memory-backend-memfd,id=mem,size=256M,share=on",
                "-numa", "node,memdev=mem",
                "-display", "none", "-monitor", "none", "-serial", "none",
            ),
        )
        if (gpu) addAll(listOf("-device", GPU_DEVICE))
    }

    val gfxLines: (String) -> Boolean = {
        it.contains("Selecting Vulkan device") || it.contains("externalMemoryMode") ||
            it.contains("initialized successfully") || it.contains("FATAL", ignoreCase = true) ||
            it.contains("error", ignoreCase = true) || it.contains("terminating")
    }

    fun gfxstream(qemu: String): JSONObject {
        val result = run(emptyMachine(qemu, "on", gpu = true), 8)
        return result.toJson(gfxLines).apply {
            put("gfxstream_initialized", result.output.any { it.contains("Gfxstream initialized successfully") })
            put("vulkan_device", result.output.firstOrNull { it.contains("Selecting Vulkan device") } ?: JSONObject.NULL)
        }
    }

    fun environment(context: Context): JSONObject = JSONObject().apply {
        put("fingerprint", Build.FINGERPRINT)
        put("sdk", Build.VERSION.SDK_INT)
        put("target_sdk", context.applicationInfo.targetSdkVersion)
        put("uid", OsProcess.myUid())
        put("selinux_context", Native.selinuxContext())
        put("native_library_dir", context.applicationInfo.nativeLibraryDir)
        put(
            "native_library_files",
            JSONArray(File(context.applicationInfo.nativeLibraryDir).list()?.sorted() ?: emptyList<String>()),
        )
    }

    fun jit(qemu: String): JSONObject = JSONObject().apply {
        put("anon_rwx", Native.jitRwx())
        put("anon_mprotect_rx", Native.jitMprotect())
        put("memfd_dual_map", Native.jitMemfdDual())
        for (mode in listOf("on", "off")) {
            put("qemu_tcg_split_wx_$mode", run(emptyMachine(qemu, mode, gpu = false), 5).toJson())
        }
    }

    fun placement(): JSONObject {
        val result = JSONObject()
        result.put("top_app_cpus", readText("/dev/cpuset/top-app/cpus"))
        result.put("own_cpuset", readText("/proc/self/cpuset"))
        // A fresh thread, so the UI thread keeps its scheduling.
        val worker = thread(name = "probe-affinity") {
            result.put(
                "pin_4_7_uclamp_512",
                runCatching { Native.affinity(intArrayOf(4, 5, 6, 7), 512) }.getOrElse { "error: $it" },
            )
        }
        worker.join()
        return result
    }

    fun ahbFromChild(context: Context, show: (Bitmap) -> Unit): JSONObject {
        val result = JSONObject()
        val name = "carbonhv.emuprobe.${OsProcess.myPid()}"
        val listenFd = Native.listenAbstract(name)
        if (listenFd < 0) return result.put("error", "listen failed errno=${-listenFd}")
        val child = File(context.applicationInfo.nativeLibraryDir, "libemuprobe_child.so").path
        var childRun: RunResult? = null
        val runner = thread(name = "probe-child") { childRun = run(listOf(child, name), 15) }
        val buffer = Native.acceptHardwareBuffer(listenFd, 15_000)
        runner.join()
        Native.closeFd(listenFd)
        result.put("child", childRun?.toJson() ?: JSONObject.NULL)
        if (buffer == null) return result.put("received", false)
        result.put("received", true)
        result.put("width", buffer.width)
        result.put("height", buffer.height)
        result.put("format", buffer.format)
        result.put("usage", "0x" + java.lang.Long.toHexString(buffer.usage))
        val wrapped = Bitmap.wrapHardwareBuffer(buffer, ColorSpace.get(ColorSpace.Named.SRGB))
        if (wrapped == null) {
            buffer.close()
            return result.put("wrapped_as_bitmap", false)
        }
        val pixel = wrapped.copy(Bitmap.Config.ARGB_8888, false).getPixel(128, 128)
        result.put("wrapped_as_bitmap", true)
        result.put("center_pixel", String.format("0x%08X", pixel))
        result.put("pixel_matches_gpu_clear", pixel == EXPECTED_PIXEL)
        show(wrapped)
        buffer.close()
        return result
    }

    fun memory(context: Context): JSONObject = JSONObject().apply {
        val info = ActivityManager.MemoryInfo()
        val manager = context.getSystemService(ActivityManager::class.java)
        manager.getMemoryInfo(info)
        put("meminfo", JSONArray(readText("/proc/meminfo").lines().filter {
            it.startsWith("MemTotal") || it.startsWith("MemAvailable") || it.startsWith("SwapFree")
        }))
        put("am_avail_mib", info.availMem / (1024 * 1024))
        put("am_threshold_mib", info.threshold / (1024 * 1024))
        put("am_low_memory", info.lowMemory)
        put("large_memory_class_mib", manager.largeMemoryClass)
        put(
            "not_run",
            "2.5 GiB allocate-and-touch skipped on the owner's daily phone: it would make LMK kill their apps",
        )
    }

    fun io(context: Context): JSONObject = JSONObject().apply {
        put("o_direct", Native.oDirect(context.filesDir.path))
        // In a child: the app's seccomp filter answers io_uring_setup with SIGSYS, killing the caller.
        val child = File(context.applicationInfo.nativeLibraryDir, "libemuprobe_child.so").path
        put("io_uring_setup_in_child", run(listOf(child, "--io-uring"), 5).toJson())
    }

    fun sandboxFlag(qemu: String): JSONObject =
        run(listOf(qemu, "-sandbox", "on", "-machine", "none", "-display", "none", "-S"), 3).toJson()

    fun thermal(context: Context): JSONObject = JSONObject().apply {
        val power = context.getSystemService(PowerManager::class.java)
        put("status", power.currentThermalStatus)
        put("headroom_10s", power.getThermalHeadroom(10).toDouble())
        put("not_run", "5-minute 4-thread spin skipped on the owner's daily phone (heat); Phase 12 measures it")
    }

    /** P7 subset, run inside the isolatedProcess service. */
    fun isolated(context: Context): JSONObject {
        val qemu = qemu(context)
        return JSONObject().apply {
            put("selinux_context", Native.selinuxContext())
            put("qemu_version", run(listOf(qemu, "--version"), 10).toJson())
            put("memfd_dual_map", Native.jitMemfdDual())
            put("inet_socket", Native.inetSocket())
            put("open_kgsl", Native.openDevice("/dev/kgsl-3d0"))
            put("gfxstream", gfxstream(qemu))
        }
    }

    private fun readText(path: String): String = runCatching { File(path).readText().trim() }.getOrElse { "unreadable: ${it.message}" }
}
