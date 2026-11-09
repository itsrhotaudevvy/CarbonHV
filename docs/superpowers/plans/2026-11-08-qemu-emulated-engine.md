# Emulated Engine: QEMU TCG with Hardware Graphics on Stock Phones

> **For agentic workers:** Work through this plan phase by phase. Steps use checkbox (`- [ ]`) syntax for tracking. Every phase ends in a **gate** with measurable exit criteria; do not start a phase whose predecessor's gate has not passed, except where a phase says it can run in parallel. When a gate fails, record the failure in a report under `qualification/emu/reports/` and stop to ask the user. Do not quietly change the plan.

**Goal:** Boot the GrapheneOS-derived CVD1 guest inside the unprivileged CarbonHV app on a **stock, unrooted, unflashed** phone with no usable hypervisor, starting with the user's Redmi `kunzite` (Snapdragon 6 Gen 3, Adreno 710). Use QEMU system emulation for the CPU and the host GPU for graphics. Every guest-visible ABI stays identical to CVD1, so the same guest image also runs on the AVF/crosvm hardware path.

**One-paragraph honest summary:** This is not a hypervisor. On a stock `kunzite`, Android blocks every hardware virtualization interface from apps (details in §2), so the guest CPU must be emulated with QEMU's TCG JIT (dynamic binary translation, arm64 guest on an arm64 host). "Hardware-accelerated" in this plan means that everything that *can* use hardware does:

- **Graphics:** the GPU (Adreno through gfxstream/Vulkan), zero-copy presentation through `AHardwareBuffer`, and composition on the GPU.
- **CPU translation:** a JIT running on the big cores with multi-threaded TCG.
- **Guest tuning:** the guest is tuned to avoid what TCG does badly.

CPU-bound guest work will be several times slower than native, while graphics stays GPU-accelerated. The product must label this engine **"Emulated"** everywhere and must never present it as native (ADR-012, amended by the new ADR-017 in Phase 0).

**Architecture in one line:** `CarbonHV app (Kotlin UI, SurfaceView)` ⇄ `:vm` service process ⇄ spawned `qemu-system-aarch64` executable (TCG, `virt` machine, `virtio-gpu-rutabaga` with gfxstream on Adreno, custom `android` UI backend that ships composed frames as `AHardwareBuffer` + fence to the app) ⇄ unchanged CVD1 guest.

**Tech stack:**
- QEMU, pinned to the latest stable 10.x tag at start; record the exact tag.
- Android NDK r28 or newer, cross-building with meson/ninja.
- glib, pixman, libslirp, zlib and libfdt cross-built for `aarch64-linux-android`.
- `rutabaga_gfx` FFI and gfxstream host, the same lineage as `crates/carbon-renderer`.
- Rust 1.98 (workspace), Kotlin 2.3.20, Compose, Android SDK 36 (`minSdk 30`, `targetSdk 36`).

**Spec / context to read first (in this order):**
1. `docs/ARCHITECTURE.md`: §7 (admission), §8 (virtual hardware platform), §12 (graphics), §17 (GrapheneOS bundle), §23 (security), the ADR table (ADR-001, ADR-012, ADR-014, ADR-016).
2. `docs/SUPPORT_MATRIX.md` §2b (`kunzite`), the 2026-11-04 re-probe.
3. `docs/superpowers/plans/2026-11-04-snapdragon-near-native.md` (the hardware path this engine complements).
4. `host/product/snapdragon/KUNZITE.md`. This is the **flashing** route. The user has **rejected** it. It is context only, never the plan.
5. `guest/android/README.md`, `guest/android/device/cvd1.toml`, `guest/common/kernel/cvd1-near-native.fragment`.
6. `crates/carbon-broker/src/direct_crosvm.rs`, `backend.rs`, `avf.rs` (argv-building patterns to mirror).
7. `android/app/src/main/kotlin/dev/carbonhv/session/GraphicsSession.kt` (`NativeFrame` takes `HardwareBuffer` + `acquireFenceFd`; the emulated engine feeds exactly this).

---

## 0. Rules the user has set (non-negotiable)

These come from the user directly. Breaking any of them is a failure of the task even if the code works.

1. **Stock phones only.** No flashing, no root, no patched `boot`/`vendor_boot`/`vbmeta`, no custom host builds, no Magisk/KernelSU, no exploits. The user's words were "dont flash any custom image. it should work on any phone". The engine must work for an ordinary user who installs an APK. ADB/Shizuku-granted permissions are acceptable only when strictly necessary; this engine should need none.
2. **The phone is the user's personal daily device** (Redmi `kunzite`, serial `8b227126`).
   - You may run **read-only** `adb` probes (`getprop`, reading `/proc`/`/sys`, `ls -Z`, `dumpsys`).
   - You may take screenshots **only of an actually running guest**.
   - **Ask the user before installing anything on the phone**, including test APKs, and before sending any UI input. Check the foreground app first.
   - Never kill, stop or disturb processes you did not start, and never change settings without restoring them.
   - Do all UI work, screenshots and iterative testing on an **Android emulator (AVD)** or on the desktop QEMU first (Phase 1).
3. **Honesty labels.** Everything stays `EXPERIMENTAL` until measured on hardware. The engine is always shown as "Emulated" and is never selected silently as a fallback. Every `gpu.*` capability claim must be backed by a qualification report.
4. **No blocking waits.** Run long builds, AOSP syncs, downloads and CI watches in the background and keep working; use quick status checks.
5. **Publishing.** Finished, verified work (`scripts/verify.sh` PASS) is committed on `main` and pushed to `origin main` (public repo `itsrhotaudevvy/CarbonHV`). Scan for secrets before every push. Commits are authored as `itsrhotaudevvy <12345678+itsrhotaudevvy@users.noreply.github.com>` (check `git config user.name/user.email`, and set it in the repo if it is missing). **Note (2026-11-08):** local `main` history was rewritten to that author and the user still has to force-push it. If `git status` shows `main` diverged from `origin/main`, do **not** force-push and do not merge the old history back in. Ask the user.

---

## 1. What "hardware-accelerated" can and cannot mean here

| Subsystem | Hardware path on a stock phone? | This plan's choice | Expected cost vs. native |
| --- | --- | --- | --- |
| CPU execution | **No.** The hypervisor is unreachable from apps (§2) | QEMU TCG, multi-threaded, on the A78 cores | Large. Plan for roughly 3–10× slower on CPU-bound code and worse on syscall/MMU-heavy code. **Measure it; do not quote these numbers as results.** |
| Guest memory / MMU | No (no stage-2 tables) | TCG soft-MMU with a large TLB; guest kernel tuned | Part of the CPU cost |
| GPU rendering (GLES/Vulkan) | **Yes** | `virtio-gpu-rutabaga` + gfxstream → host Vulkan on Adreno 710 | Near native for GPU-bound work; per-call overhead is serialization over virtio-gpu, which TCG makes slower on the guest side |
| Composition | **Yes** | Guest ranchu HWC → gfxstream composer capset → host GPU | Low |
| Presentation | **Yes** | Composed frame in an `AHardwareBuffer` plus a sync fence → app `SurfaceView` (existing `GraphicsSession`) | Zero-copy target; a CPU-readback fallback for bring-up |
| Video decode | Partially (later) | Software in the guest first; host `MediaCodec` through virtio-media is out of scope (§12 lists it as future work) | High until offloaded |
| Disk I/O | Host kernel does the I/O | virtio-blk, raw images, `aio=threads` | Low/moderate; guest-side CPU cost dominates |
| Network | Host sockets | virtio-net + libslirp (user networking) | Moderate |
| Audio | Host AAudio | virtio-snd + a new QEMU `aaudio` audiodev | Low |
| Input | Host touch events | virtio-multitouch + virtio-keyboard fed from the app | Low |

What this is **not**:
- It is not a way around the hypervisor lock.
- It is not KVM, not Gunyah, and not "near-native".
- The near-native work (ADR-014, `direct_crosvm.rs`) still applies only to hardware-VM hosts.

---

## 2. Facts this plan rests on

### 2.1 Verified on the phone (read-only probes, 2026-11-04 and 2026-11-08)

- Device: Redmi `kunzite` `25098RA98G`, xiaomi.eu `OS3.0.304.0.WPQCNXM`, Android 16 (SDK 36), kernel `6.6.118-android15-8` with **4 KiB pages**.
- CPU: 4× Cortex-A55 (CPUs 0–3, capacity 405) + 4× Cortex-A78 (CPUs 4–7, capacity 1024, 2.4 GHz); cpufreq domains `0-3;4-7`. `/proc/cpuinfo` features: `fp asimd evtstrm aes pmull sha1 sha2 crc32 atomics fphp asimdhp cpuid asimdrdm lrcpc dcpop asimddp`.
  - It has LSE `atomics`, which QEMU's aarch64 TCG backend uses.
  - It has **no** SVE, PAC, BTI or MTE.
- Memory: `MemTotal` 7.2 GiB; `MemAvailable` 2.6–2.7 GiB at probe time (1.8 GiB on 2026-09-30). **Memory is the tightest resource.**
- GPU: Adreno 710, `ro.hardware.vulkan=adreno`, `ro.hardware.egl=adreno`; Vulkan 1.3 device / 1.4 instance (2026-09-30 report).
- Hypervisor:
  - `/dev/gunyah` exists, `crw-rw-rw-`, SELinux label `u:object_r:vendor_gunyah_dev:s0`; SELinux is `Enforcing`.
  - `ro.boot.hypervisor.vm.supported` is unset and there is no `/dev/kvm`.
  - `vm info` panics in virtmgr with "Device doesn't support protected or non-protected VMs".
  - **Note:** `vm info` then hangs, so always run it with a timeout.
  - The shell domain is denied `/dev/gunyah`. Untrusted apps are denied it as well, since only `vm_manager_device_type` users (AVF's virtualizationmanager/crosvm) may open it.
- Bootloader unlocked (`orange`). **Irrelevant:** the user forbids using it.

### 2.2 Known from upstream, but VERIFY at the pinned revision before relying on it

These are recalled from upstream knowledge, not re-checked for this plan. Each has a verification step in the phases below. Mark each one ✅ or ❌ in the Phase 0 report with the source path and commit.

| ID | Claim | Where to verify |
| --- | --- | --- |
| V1 | QEMU has `-device virtio-gpu-rutabaga` (added in QEMU 8.1) with gfxstream properties (`gfxstream-vulkan=on`, `gfxstream-gles=on`?, `cross-domain=on`, `hostmem=<size>`, `wsi=surfaceless`). It links the `rutabaga_gfx_ffi` C library. | `hw/display/virtio-gpu-rutabaga.c`, `meson_options.txt` (`rutabaga_gfx`) |
| V2 | QEMU has `virtio-multitouch-device`/`-pci` (8.2+) and `virtio-sound` (8.2+). | `hw/input/virtio-input-hid.c`, `hw/audio/virtio-snd.c` |
| V3 | `-accel tcg,thread=multi,tb-size=N,split-wx=on|off` exist; `split-wx` uses a memfd-backed double mapping so no page is ever W+X. | `accel/tcg/tcg-all.c`, `tcg/region.c` |
| V4 | `-cpu max` accepts `pauth-impdef=on` (much cheaper than QARMA5 in TCG), `pauth=off`, `sve=off`, `sme=off`. `-machine virt,mte=on` enables emulated MTE. | `target/arm/cpu64.c`, `hw/arm/virt.c` |
| V5 | Cuttlefish `launch_cvd --vm_manager=qemu_cli` builds an arm64 QEMU command line, with a gfxstream mode that uses `virtio-gpu-rutabaga`. It can boot arm64 cuttlefish with a QEMU-specific U-Boot (`bootloader_qemu_aarch64` or similar in `device/google/cuttlefish_prebuilts`). | AOSP `device/google/cuttlefish/host/libs/vm_manager/qemu_manager.cpp`, `cuttlefish_prebuilts/bootloader/` |
| V6 | Termux (`termux-packages`, `packages/qemu-system-*`) cross-builds QEMU + glib for Android with the NDK, and its patches are a usable reference. Open-source Android apps (for example Vectras VM, Limbo) already run QEMU TCG inside unprivileged apps. This is an existence proof only; **do not copy their code or branding**, because licenses differ. | GitHub repos |
| V7 | Untrusted apps with `targetSdk ≥ 29` cannot `execve` files from their writable data dir (W^X for app data). They **can** execute files shipped in the APK's `nativeLibraryDir` when `useLegacyPackaging`/`extractNativeLibs=true`. | AOSP `system/sepolicy` `untrusted_app*.te`, the Android 10 behavior-changes page; confirm on device in Gate 0 |
| V8 | Untrusted apps may create executable anonymous or memfd mappings (ART's JIT uses a memfd dual mapping). Whether plain RWX anonymous memory (`execmem`) is allowed for `targetSdk 36` must be **tested**, not assumed. `split-wx=on` avoids needing it. | `system/sepolicy` `app.te`/`untrusted_app_all.te`; Gate 0 test |
| V9 | Android blocks `io_uring` for apps via seccomp/SELinux (Android 14+). | Gate 0 test; use `aio=threads` regardless |
| V10 | An app may `sched_setaffinity` its own threads onto CPUs 4–7 while in the `top-app` cpuset, and the `top-app` cpuset includes the big cores on this device. | Gate 0 test reading `/dev/cpuset/top-app/cpus` (read-only) and the thread's `Cpus_allowed_list` |
| V11 | `isolatedProcess=true` services (`isolated_app` domain) cannot open the KGSL GPU device or network sockets. This decides the hardening split in Phase 14. | `system/sepolicy` `isolated_app*.te`; Gate 0 test |
| V12 | A public AOSP CI build of `aosp_cf_arm64_only_phone-userdebug` (or the current arm64 cuttlefish phone target) provides `*-img-*.zip` and `cvd-host_package.tar.gz` downloadable without login. | `ci.android.com` |
| V13 | The gfxstream host library can build for an Android host (bionic) against the Android Vulkan loader, the way AVF's virt APEX crosvm uses it. `crates/carbon-renderer` already has a `native-gfxstream` feature path. | `crates/carbon-renderer`, AOSP `hardware/google/gfxstream`, `packages/modules/Virtualization` |
| V14 | Android bootconfig: the kernel reads a bootconfig block appended to the initrd (trailer magic `#BOOTCONFIG\n`) when `bootconfig` is on the cmdline. `androidboot.*` keys go there, not on the cmdline, for Android 12+ GKI. | `Documentation/admin-guide/bootconfig.rst`, AOSP `system/tools/mkbootimg` |

### 2.3 What is explicitly ruled out (do not re-investigate)

- Using `/dev/gunyah` or `/dev/kvm` from the app: SELinux denies it on stock firmware (§2.1).
- QEMU's Gunyah accelerator: Qualcomm posted RFC patches, but they are not upstream and would hit the same SELinux denial.
- AVF on this phone: `ro.boot.hypervisor.vm.supported` is unset, and only a firmware change sets it.
- The `vendor_boot` patch in `KUNZITE.md`: rejected by the user.
- Containers (Waydroid/LXC/namespaces): these need root. Unprivileged user namespaces are not available to Android apps.
- SwiftShader or any CPU rasterizer as the guest GPU: this violates ADR-016 and would make the engine useless. If gfxstream on Adreno fails, the engine fails Gate 6. It does **not** fall back to software rendering.

---

## 3. Architecture

### 3.1 Process model

```
┌──────────────────────── CarbonHV app (uid u0_aXXX, untrusted_app) ────────────────────────┐
│                                                                                           │
│  Main process  dev.carbonhv.app                                                          │
│   ├─ Compose UI, ImmersiveSessionScreen, SurfaceView                                     │
│   ├─ GraphicsSession  ◀── NativeFrame(HardwareBuffer, acquireFenceFd)                     │
│   └─ EmulatedEngineClient (Binder to :vm)                                                 │
│                                                                                           │
│  :vm process  (foreground service, type "specialUse", ongoing notification)              │
│   ├─ EmulatedVmService (Kotlin): lifecycle, generations, fd plumbing, thermal policy      │
│   ├─ libcarbonemu.so (Rust, JNI): argv build/validate (crates/carbon-emu),                │
│   │     QMP client, frame socket reader, input/audio forwarders                          │
│   └─ spawns ▼                                                                              │
│                                                                                           │
│  qemu-system-aarch64  (executable shipped as libqemu_system_aarch64.so in nativeLibraryDir)│
│   ├─ TCG MTTCG vCPU threads ×N, pinned to CPUs 4–7                                        │
│   ├─ virtio-gpu-rutabaga → rutabaga_gfx_ffi → gfxstream host → Vulkan (Adreno 710)        │
│   ├─ ui/android.c: composed scanout as AHardwareBuffer + sync_file → frame socket         │
│   ├─ virtio-multitouch / virtio-keyboard ← input socket                                   │
│   ├─ virtio-snd → audio/aaudio.c → AAudio                                                 │
│   ├─ virtio-blk ← disk fds (system ro, vendor ro, userdata rw, metadata rw, misc rw)      │
│   ├─ virtio-net → libslirp                                                                │
│   ├─ vhost-vsock? NO (needs /dev/vhost-vsock) → virtio-vsock emulated alternative §3.6    │
│   └─ QMP on an inherited socketpair fd                                                    │
└───────────────────────────────────────────────────────────────────────────────────────────┘
                      guest: GrapheneOS-derived CVD1 (unchanged ABI)
```

Why a **separate executable** and not a JNI library loaded into `:vm`:
1. **Crash containment.** A QEMU abort kills only that process; `:vm` reports the failure and reconciles the generation, the same way the AVF death path does.
2. **QEMU is not re-entrant.** Its `main()` and global state assume one VM per process lifetime. A fresh process per boot avoids leaked state.
3. **License boundary.** QEMU is GPL-2.0, CarbonHV is Apache-2.0. A separately executed program, talking over sockets/QMP, is the conventional aggregate boundary. Ship QEMU's source offer and NOTICE (Phase 13). **Have the user confirm the licensing approach before the first public release that ships the binary.**

Why `AHardwareBuffer` frames instead of handing QEMU the `Surface`: an `ANativeWindow` cannot be passed to a non-Binder child process. `AHardwareBuffer_sendHandleToUnixSocket` / `AHardwareBuffer_recvHandleFromUnixSocket` (NDK, API 26+) can be passed, and the app already presents `NativeFrame(HardwareBuffer, acquireFenceFd)` through `GraphicsSession`. So the emulated engine plugs into the existing presentation path with **no new presentation code in the app**.

### 3.2 Engine selection and admission (ADR-017)

- Add `ExecutionEngine` to the domain:
  - `HardwareVm { backend: Avf | DirectCrosvm, hypervisor: HypervisorKind }`
  - `Emulated { accelerator: Tcg }`
- `HostVirtualizationClass` keeps its meaning. Emulation availability is a **separate** probe result (`EmulationReadiness`: `Ready`, `InsufficientMemory`, `NoVulkan`, `UnsupportedAbi`), because it depends on app-side facts, not on the hypervisor.
- Selection rules:
  1. If a hardware VM is ready → use it. Never offer emulation silently as a "faster start".
  2. If not → the instance setup screen shows **"Run emulated (slow)"** as an explicit opt-in. It explains the cost and lists the security differences (§6). The choice is stored per instance.
  3. An instance created as `Emulated` stays emulated until the user switches it. Switching is allowed both ways, because the guest ABI is identical (CVD1). It needs a cold boot. Disk images are shared; checkpoints are not.
- Capability reporting: an emulated instance may report `gpu`, `gpu.gles`, `gpu.vulkan`, `gpu.native-buffer`, `gpu.explicit-sync` **only** after Gate 6/7 reports prove them on this host. `cpu.native` is always `false`.

### 3.3 Guest ABI (unchanged CVD1)

The same `grapheneos-cvd1` image runs on both engines. The emulated machine must present:

- arm64 CPU, GICv3, ARM generic timer, PSCI;
- virtio-gpu with the gfxstream capsets (GLES, Vulkan, composer);
- virtio-input (multitouch + keyboard);
- virtio-net, virtio-blk, virtio-snd, vsock (§3.6);
- a virtio-console/hvc for logcat/kmsg, matching cuttlefish port numbering (verify against `qemu_manager.cpp`, V5).

Differences that the guest can see (record them in the security/difference matrix):

- **Guest-physical memory map.** QEMU `virt` puts RAM at `0x4000_0000`; crosvm uses `0x8000_0000`. The guest gets its layout from the FDT, so this is fine for Linux.
  - **Do not** try to force crosvm's PCI windows (`AARCH64_PCI_WINDOWS` in `direct_crosvm.rs`) onto QEMU.
  - Prefer the virtio-mmio or PCI transport that cuttlefish's QEMU mode uses (V5), so the guest kernel config already matches.
- **CPU model.** Emulated features differ (§5.3). GrapheneOS feature detection must handle that, and it does at runtime.
- **No `CONFIG_CPUFREQ_VIRT` device.** TCG has no meaningful guest DVFS. The guest boots fine, and `carbonctl guest kernel-config` already treats the near-native extras as optional (exit 5).

### 3.4 Boot flow

Preferred, if V5 holds: cuttlefish's QEMU U-Boot loads `boot`/`init_boot`/`vendor_boot` from the `misc`/`boot` disks, exactly as on cuttlefish. Guest-side A/B and bootconfig work unchanged.

Fallback (direct kernel boot):
1. Extract the kernel from `boot.img`.
2. Build the ramdisk as `vendor_boot` ramdisk ‖ `init_boot` ramdisk ‖ bootconfig trailer (V14).
3. Pass them with `-kernel`, `-initrd` and `-append "... bootconfig"`.
4. Do this extraction **at install time** in Rust (`crates/carbon-emu/src/bootimg.rs`). Never parse boot images in the QEMU process.

In the fallback there is no guest bootloader AVB. Instead, the app verifies the bundle's image digests before every boot, as it does today, and the difference matrix records "verified by host before boot; no in-guest AVB chain".

### 3.5 Storage layout

These follow ADR-007:
- `system`, `vendor`, `product` (and the `super` image if the guest uses dynamic partitions) are **read-only raw**, opened by the `:vm` service and passed as fds (`-add-fd` + `/dev/fdset/N`). QEMU never gets a path into app storage.
- `userdata` and `metadata` are raw sparse files (no qcow2: qcow2 metadata costs CPU under TCG) with `discard=unmap,detect-zeroes=unmap`.
- `misc` and `boot` partitions are raw.
- All disks use `cache=none` where `O_DIRECT` works on the app's f2fs volume (test in Gate 0) and `cache=writeback` otherwise, with `aio=threads`. Never `io_uring` (V9).
- Instance state stays under the app's private storage, using the existing `carbon-core::storage` layout. **Add no new path schemes.**

### 3.6 Host/guest agent channel (vsock)

CVD1 requires vsock (`vsock = true`) for the CarbonHV guest agent (`guest/common/PROTOCOL.md`). QEMU's `vhost-vsock-pci` needs `/dev/vhost-vsock`, which apps cannot open. Options, in order of preference:

1. **`vhost-user-vsock` in-process.** Run a userspace vsock device backend in the `:vm` process; the `vhost-device-vsock` crate from rust-vmm maps guest vsock to host unix sockets. QEMU connects to it over a socketpair fd (`-chardev socket,fd=N` + `-device vhost-user-vsock-device,chardev=...`). vhost-user needs a shared-memory guest RAM backend (`-object memory-backend-memfd,share=on`), which works unprivileged.
2. If (1) fails Gate 0/7: **virtio-console fallback.** Carry the agent protocol over a dedicated virtio-serial port. That requires an agent transport abstraction in the guest. It is a guest change that must stay behind a CVD1-minor capability flag, so avoid it if possible.

Decide with a measured spike (Phase 7). Do not fork the agent protocol.

### 3.7 Repository layout (new and changed)

```
crates/carbon-emu/                     NEW  pure Rust, no Android deps; unit-tested on the Mac
  src/lib.rs                                 public API: EmuPlan, EmuPlanError
  src/argv.rs                                QEMU argv builder (mirrors direct_crosvm.rs rules)
  src/cpu_model.rs                           guest CPU model + TCG options policy
  src/placement.rs                           vCPU count / affinity from HostResources
  src/memory.rs                              RAM / TB-cache / hostmem budget vs MemAvailable
  src/bootimg.rs                             boot/init_boot/vendor_boot parsing, bootconfig
  src/qmp.rs                                 minimal QMP client (no shell, typed commands)
  src/frame_proto.rs                         frame/input/audio socket wire formats
  tests/argv_golden.rs                       golden argv per profile
crates/carbon-core/src/model.rs        CHANGE ExecutionEngine, EmulationReadiness
crates/carbon-broker/src/backend.rs    CHANGE engine selection; never auto-emulate
android/emu/                           NEW  Android library module
  src/main/kotlin/dev/carbonhv/emu/EmulatedVmService.kt
  src/main/kotlin/dev/carbonhv/emu/EmulatedEngineClient.kt
  src/main/kotlin/dev/carbonhv/emu/EmulationProbe.kt
  src/main/cpp/ (or Rust via cargo-ndk) libcarbonemu.so JNI
  src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so   (built artifact, not committed)
android/app/...                        CHANGE engine choice UI, "Emulated" badges, setup copy
emu/qemu/                              NEW  QEMU build recipe for Android
  build-android.sh                           NDK cross-build (meson cross file)
  cross/aarch64-android.ini
  patches/0001-ui-add-android-ahb-backend.patch
  patches/0002-audio-add-aaudio-backend.patch
  patches/00xx-*.patch                       minimal Android portability fixes
  deps/ (glib, pixman, libslirp, libfdt, zlib, rutabaga_gfx_ffi, gfxstream) recipes
  VERSIONS.toml                              exact pinned tags + sha256
qualification/emu/                     NEW  gates, workloads, reports for this engine
docs/ARCHITECTURE.md                   CHANGE ADR-017, §7/§8/§23 updates
docs/SUPPORT_MATRIX.md                 CHANGE Emulated engine column
scripts/verify.sh                      CHANGE run carbon-emu tests + golden argv
```

Do **not** commit QEMU binaries, guest images or downloaded AOSP artifacts. Add them to `.gitignore`. Record the hashes in reports instead.

---

## 4. Phases at a glance

| Phase | Name | Where it runs | Gate |
| --- | --- | --- | --- |
| 0 | Decisions, ADR-017, verify V1–V14 | Mac | G0: report with every V-claim ✅/❌ |
| 1 | Desktop bring-up: AOSP cuttlefish arm64 under QEMU TCG + gfxstream | Mac (arm64) / Linux | G1: boots to launcher under TCG, with GPU-accelerated graphics |
| 2 | QEMU + deps cross-built for Android | Mac/Linux builder | G2: `qemu-system-aarch64 --version` runs on an AVD |
| 3 | On-device feasibility probes (test APK) | AVD, then phone **with user consent** | G3: JIT, exec, affinity, Vulkan, memory all ✅ on `kunzite` |
| 4 | Android UI backend: frames to `GraphicsSession` (readback first) | AVD | G4: guest framebuffer visible in the app |
| 5 | Input | AVD | G5: touch + keyboard round-trip |
| 6 | gfxstream on Adreno through `virtio-gpu-rutabaga`, zero-copy AHB | phone (consent) | G6: guest GLES+Vulkan on Adreno, zero-copy present |
| 7 | Storage, network, audio, vsock agent | AVD + phone | G7: CVD1 devices all functional |
| 8 | `crates/carbon-emu` + admission + engine model | Mac | G8: unit/golden tests, `verify.sh` PASS |
| 9 | App integration: service, lifecycle, UI | AVD | G9: start/stop/reattach/crash all reconcile |
| 10 | GrapheneOS-derived guest built and tuned for TCG | Linux x86_64 builder | G10: guest boots on desktop QEMU TCG |
| 11 | Performance work | phone | G11: measured targets (§9) |
| 12 | Thermal, power, memory pressure, lifecycle | phone | G12: 30-min sustained run with no kill/throttle collapse |
| 13 | Qualification, docs, licensing, release | Mac + phone | G13: reports + docs + `verify.sh` PASS, pushed |
| 14 | Hardening: process split / isolation | AVD + phone | G14: security matrix improved without breaking G6 |
| 15 | Research track (optional): faster CPU paths | anywhere | go/no-go memo only |

Phases 2 and 8 can run in parallel with Phase 1. Phase 10 (the guest build) needs a Linux x86_64 machine with about 400 GB of free disk, 64 GB of RAM recommended, and many hours. **Ask the user** whether they have one (a cloud VM is fine) before starting it. Until then, use the AOSP CI cuttlefish image (V12) as a clearly labelled **stand-in guest**. It is never shipped and never claimed as GrapheneOS.

---

## 5. Performance strategy (what makes TCG as fast as it can be)

This is the core of "accelerated". Each lever has an owner phase and must be **measured** in Phase 11: on vs. off, p50/p95, with the report in `qualification/emu/reports/`. Remove any lever that does not pay for itself (the same discipline as ADR-014).

### 5.1 Host-side TCG configuration

| Lever | Setting | Rationale | Risk |
| --- | --- | --- | --- |
| Multi-threaded TCG | `-accel tcg,thread=multi` | One host thread per vCPU; arm64-on-arm64 MTTCG is supported (memory model matches) | none |
| vCPU count | default 4 (= A78 count); measure 3 | QEMU's I/O thread, gfxstream render threads and the app UI also need big-core time; 3 vCPUs plus free big cores may beat 4 | measure |
| Affinity | vCPU threads → CPUs 4–7; I/O + gfxstream threads → unpinned or 4–7 minus one | Big cores are about 2.5× the A55s; keep the A55s for the host UI | V10 |
| Translation cache | `tb-size=256` MiB to start; measure 128/512 | Android's large code footprint thrashes a small cache; memory is tight (§5.5) | memory |
| W^X | `split-wx=on` if anonymous RWX is denied (V8); measure the overhead if both work | Required on hardened kernels | small cost |
| Thread priority | vCPU threads at normal priority with `uclamp.min` boost while foreground (`sched_setattr` on own threads, test in Gate 3); none in background | Same idea as `--boost-uclamp` in ADR-014 | thermal |
| Clock | `-rtc base=utc,clock=host`; no `-icount` | `icount` serializes and kills performance | none |
| Guest RAM backing | `-object memory-backend-memfd,id=mem,size=...,share=on` (needed for vhost-user vsock, §3.6) | Shared, unprivileged | none |
| Huge pages | none (THP is `never` on this firmware; no hugetlbfs for apps) | — | — |

### 5.2 Guest kernel (TCG profile fragment)

Create `guest/common/kernel/cvd1-emulated.fragment` (the guest must still boot on hardware VMs with the same image; these options are all safe there):
- `CONFIG_NO_HZ_IDLE=y` (or `NO_HZ_FULL` off) and `CONFIG_HZ_250` or lower. Fewer timer interrupts means fewer TB exits. Measure 100 vs. 250.
- No debug options: no `KASAN`, `KCSAN`, `LOCKDEP`, `DEBUG_PAGEALLOC`, `PAGE_POISONING` or `SLUB_DEBUG_ON`. Production guests should not have them anyway.
- Make sure the guest `cpuidle`/`WFI` path is used. A guest idling in WFI lets TCG halt the vCPU thread, which matters greatly for battery and heat.
- **Do not** disable security mitigations (ADR-014's rule stands), KASLR, or `init_on_alloc`/`init_on_free`. GrapheneOS enables zeroing; measure its cost and record it, but do not remove it.
- `CONFIG_ARM64_PTR_AUTH` stays on. Choose the cheap emulated algorithm on the host side (§5.3) instead of disabling it.

### 5.3 Guest CPU model

Use `-cpu max` with expensive features tuned on the host side, keeping the guest's security features present wherever possible:

| Feature | Setting | Note |
| --- | --- | --- |
| Pointer authentication | `pauth-impdef=on` | Keeps PAC (GrapheneOS uses it) at a fraction of QARMA5's emulation cost. Verify V4 |
| SVE/SME | `sve=off,sme=off` | Large state and slow emulation; Android does not need them |
| MTE | **off by default**; `-machine virt,mte=on` as an opt-in "Hardened (slower)" profile | MTE under TCG is emulated and costs memory (tag storage) and speed. Also check the interaction with gfxstream host-visible blobs: the AVF path keeps MTE off because MTE guests cannot map non-MTE blobs (`direct_crosvm.rs` header). Record it in the difference matrix either way. GrapheneOS hardened_malloc uses MTE when present |
| BTI | on (cheap) | |
| Alternative | `-cpu cortex-a76` or `neoverse-n1` | A fixed, older model; measure against `max` with features off |

### 5.4 Guest userspace (biggest wins for Android under TCG)

Make these changes in the guest **product configuration for the emulated profile**, set at runtime via bootconfig/sysprops where possible, so one image serves both engines.

1. **No JIT in the guest; everything AOT.** A guest JIT under TCG means double translation plus self-modifying code, and every guest JIT write invalidates QEMU translation blocks, which is expensive.
   - Build-time: compile system_server, SystemUI, the launcher, settings and all preinstalled apps with the `speed` filter (`PRODUCT_DEX_PREOPT_DEFAULT_COMPILER_FILTER := speed`, plus boot image `speed`).
   - Runtime (bootconfig → sysprops): `dalvik.vm.usejit=false`, `pm.dexopt.*` set so background dexopt does not run during interaction, and `dalvik.vm.dex2oat-threads` limited.
   - **Measure image growth**; userdata and system sizes are bounded by the bundle's `storage_mib = 24576`.
2. **Boot path:** disable the boot animation (`debug.sf.nobootanimation=1` or the equivalent for the emulated profile) and skip first-boot dexopt by shipping a fully compiled image.
3. **UI:**
   - Default animator, transition and window animation scale to `0.5` for the emulated profile (a user-visible setting that can be changed back).
   - Disable window blur (`ro.surface_flinger.supports_background_blur=0` for the profile).
   - Keep HWUI on the GPU (it is hardware-accelerated through gfxstream). **Verify** that HWUI is not falling back to the CPU: `dumpsys gfxinfo` in the guest shows the GPU pipeline.
4. **Services:** turn off what a VM never needs (telephony, NFC, UWB, fingerprint, face, camera HALs absent, location providers idle). Most of this is already true of a CVD1 product; audit `dumpsys activity services` for busy services and list them in the report.
5. **Memory:** set `ro.config.low_ram=false`, but tune `dalvik.vm.heapsize`/`heapgrowthlimit` for a 2 GiB guest. Use zram inside the guest (`zram` swap, `lz4`), since compression in the guest costs TCG CPU. Measure on vs. off with 2 GiB RAM.

### 5.5 Memory budget (this device)

`MemAvailable` ≈ 2.7 GiB. A budget that fits:

| Item | Budget |
| --- | --- |
| Guest RAM | 2048 MiB (emulated profile default; bundle's 4096 is for hardware VMs) |
| TCG translation cache | 256 MiB |
| QEMU + glib + slirp + device state | ~150 MiB |
| gfxstream host (Vulkan objects, staging, host-visible `hostmem` window) | 256–512 MiB, measure. `hostmem` is address space; resident use is what matters |
| `:vm` service + JNI | ~60 MiB |
| **Total** | **~2.8–3.0 GiB resident at peak**. Tight: the guest must use zram, and admission must check live `MemAvailable` |

Admission rule (`crates/carbon-emu/src/memory.rs`): refuse to start with a clear message if `MemAvailable` < guest RAM + 768 MiB. Re-check under memory pressure (`onTrimMemory` in the service) and checkpoint-then-stop instead of being killed mid-write (§Phase 12).

---

## 6. Security posture (record in `docs/SECURITY.md` and the difference matrix)

| Property | Hardware VM (AVF/crosvm) | Emulated engine (this plan) |
| --- | --- | --- |
| Guest↔host isolation boundary | Hypervisor (stage-2) + crosvm sandbox | **Software MMU (TCG) + the app's own sandbox.** A bug in QEMU device emulation or TCG gives code execution as the CarbonHV app, not as system |
| Privileges of the emulator process | crosvm domain | `untrusted_app` (CarbonHV's uid); later `isolated_app` (Phase 14) |
| Guest memory confidentiality from host | None (non-protected VM) | None |
| Verified boot | Guest bootloader AVB (cuttlefish U-Boot) | Same if V5 holds; otherwise host-verified digests only (§3.4) |
| MTE in guest | Off (Gunyah: none) | Off by default; emulated opt-in |
| PAC/BTI in guest | Hardware (where present) | Emulated (`pauth-impdef`) |
| Attack surface | crosvm devices | QEMU devices. **Minimize:** `-nodefaults`, `-no-user-config`, explicitly listed devices only, no USB, no legacy, no `-monitor`, QMP only on an inherited fd, `-sandbox` is Linux-seccomp. Test whether it works on Android or conflicts with the app seccomp policy; Android apps already run under a zygote seccomp filter |

Rules:
- The QEMU argv is built only by `crates/carbon-emu`. No shell. Every path is an fd. Guest-controlled strings never reach argv (mirror `direct_crosvm.rs` and the plan-v1 review focus).
- Validate the QMP responses' schema strictly. Treat everything from QEMU as untrusted input to the `:vm` process.
- Frame socket: validate every `AHardwareBuffer` description (size, format, usage) before handing it to `GraphicsSession` (`hasValidMetadata` already exists).

---

## Phase 0: Decisions, ADR-017, upstream verification

**Files:** modify `docs/ARCHITECTURE.md`; create `qualification/emu/reports/2026-11-XX-phase0-upstream-verification.md`.

- [ ] Read §2.2 and verify each of V1–V14 against the pinned upstream sources (QEMU tag, AOSP branch, termux-packages commit). Record ✅/❌ with file path, commit and the exact flag spellings. **Any ❌ on V1, V3, V7 or V13 is a plan-changing finding: stop and report.**
- [ ] Pick and record pins in `emu/qemu/VERSIONS.toml`:
  - the QEMU tag;
  - glib, pixman, libslirp, libfdt and zlib versions;
  - the `rutabaga_gfx` revision (match `Cargo.lock`'s `rutabaga_gfx` where possible);
  - the gfxstream revision;
  - the NDK version.
- [ ] Add **ADR-017** to the ADR table:
  - *Decision:* "On hosts with no app-reachable hypervisor, CarbonHV offers an explicit, opt-in **Emulated** engine: QEMU TCG for the CPU, with gfxstream on the host GPU for graphics. The same CVD1 guest ABI is used. It is never selected automatically, never labelled native, and never uses a software GPU."
  - *Why:* "Stock Snapdragon phones (`kunzite`) deny `/dev/gunyah` to apps and leave AVF disabled; the user requires stock phones."
  - *Revisit when:* "AVF becomes available on the host, or measured emulated performance makes the engine unusable for its starter guests."
- [ ] Amend ADR-001's and ADR-012's text to reference ADR-017. Hardware virtualization stays the production target; emulation is an explicitly labelled, separately qualified engine.
- [ ] Update `§7 Capability discovery and admission` with the selection rules from §3.2.
- [ ] Commit (`docs(arch): ADR-017 explicit emulated engine for stock hosts without a hypervisor`).

**G0:**
- All V-claims are resolved.
- The ADR is merged.
- The user has seen the Phase 0 report. **Ask them to confirm the GPL/Apache approach (§3.1) and the "Emulated (slow)" product framing before Phase 13.**

## Phase 1: Desktop bring-up (prove the stack before touching Android)

Goal: the cheapest possible proof that the whole guest stack (QEMU TCG + `virtio-gpu-rutabaga` + gfxstream + an arm64 cuttlefish guest) boots to a launcher with GPU-accelerated graphics. Do it first on the Mac, where iteration is fast. Use `-accel tcg` **explicitly**, not HVF, so the measurements reflect TCG.

**Files:** `qualification/emu/desktop/run-desktop.sh`, a report.

- [ ] Download the stand-in guest (V12): the latest green `aosp_cf_arm64_only_phone-trunk_staging-userdebug` (or the current name) `*-img-*.zip` + `cvd-host_package.tar.gz`. Record the build ID and sha256. Store it outside the repo (for example `~/carbonhv-cache/`), in its own new directory. Downloads are untrusted data: never run scripts from inside them with the CWD set there.
- [ ] Build QEMU for macOS from the pinned tag with `--enable-rutabaga-gfx` (needs `rutabaga_gfx_ffi` + gfxstream built for macOS with MoltenVK). **If gfxstream on macOS is impractical, do Phase 1 on a Linux arm64 or x86_64 machine instead**, using that host's Vulkan. Ask the user which machine. The point of the phase is the guest stack, not the host OS.
- [ ] Produce the QEMU command line. Reference: cuttlefish `qemu_manager.cpp` (V5); port the arm64 branch faithfully. Start from this **draft**, then correct it against the source:
  ```
  qemu-system-aarch64 -nodefaults -no-user-config -nographic(no) \
    -machine virt,gic-version=3,highmem=on,mte=off \
    -accel tcg,thread=multi,tb-size=256 \
    -cpu max,pauth-impdef=on,sve=off,sme=off \
    -smp 4 -m 2048 \
    -object memory-backend-memfd,id=mem,size=2048M,share=on -numa node,memdev=mem \
    -device virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,x-gfxstream-composer=on,cross-domain=off,hostmem=512M,wsi=surfaceless \
    -device virtio-multitouch-pci -device virtio-keyboard-pci \
    -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
    -audiodev none,id=snd0 -device virtio-sound-pci,audiodev=snd0 \
    -drive if=none,id=d0,file=...,format=raw,readonly=on,aio=threads -device virtio-blk-pci,drive=d0 \
    ... (one per cuttlefish disk, in cuttlefish's order) \
    -chardev ... -device virtio-serial-pci ... (console/logcat/kmsg ports as cuttlefish defines) \
    -bios <cuttlefish qemu u-boot>   # or -kernel/-initrd/-append per §3.4 fallback
  ```
- [ ] Boot. Capture serial/kmsg/logcat to files. Record:
  - (a) time to kernel, to `init` second stage, to `sys.boot_completed=1`, and to launcher drawn;
  - (b) in the guest, `dumpsys SurfaceFlinger` GLES/Vulkan strings. They must name gfxstream/"Android Emulator OpenGL ES Translator", **not** SwiftShader;
  - (c) host CPU utilization per thread.
- [ ] Run the same guest with `-accel hvf` (Mac) or `kvm` (Linux) once as the **native reference**, and record the ratio. This gives the first honest estimate of the TCG penalty for this guest.
- [ ] Try `-smp 2/3/4`, `tb-size` 128/256/512, and `pauth-impdef` vs. `pauth=off`. Record the boot times.

**G1:**
- The stand-in guest reaches `sys.boot_completed=1` and shows the launcher under TCG.
- The guest GLES/Vulkan renderer is gfxstream (hardware).
- There is a report with boot timings and the TCG/native ratio.

**Kill criterion:** if the guest cannot boot on desktop TCG even with generous resources, stop. The phone will not do better.

## Phase 2: QEMU and dependencies cross-built for Android

**Files:** `emu/qemu/build-android.sh`, `emu/qemu/cross/aarch64-android.ini`, `emu/qemu/deps/*`, `emu/qemu/patches/*`.

- [ ] Write the meson cross file for `aarch64-linux-android30` (`minSdk 30`) using NDK clang. Build the static deps in this order: zlib, libffi, pcre2, glib (meson, `-Dtests=false -Dselinux=disabled -Dlibmount=disabled -Dxattr=false`), pixman, libslirp, dtc/libfdt.
- [ ] Build `rutabaga_gfx_ffi` for `aarch64-linux-android` with gfxstream enabled (V13). Reuse whatever `crates/carbon-renderer` already does for `native-gfxstream`. If that crate builds gfxstream for Android, build the FFI from the same revision so the guest-visible capsets are bit-identical to the AVF path.
- [ ] Configure QEMU, enabling exactly what is needed:
  - **Enable:** `--target-list=aarch64-softmmu --enable-tcg --enable-rutabaga-gfx --enable-slirp --enable-fdt=system`.
  - **Disable:** `--disable-kvm --disable-xen --disable-docs --disable-tools --disable-guest-agent --disable-sdl --disable-gtk --disable-vnc --disable-spice --disable-opengl --disable-virglrenderer --disable-libusb --disable-usb-redir --disable-linux-io-uring --disable-curl --disable-capstone`. Add `--disable-…` for everything else that is not needed.
  - **Use static deps; strip; use LTO.**
- [ ] Use Termux's patch set (V6) **as a reference only** for Android portability issues: shm, `sys/shm.h`, `getloadavg`, `pthread_cancel`, `posix_memalign`, `tmpfile`, and other bionic gaps. Write minimal, separately reviewed patches in `emu/qemu/patches/` with a header comment explaining each one.
- [ ] Output: `libqemu_system_aarch64.so`. It is an **executable** (PIE) named `lib*.so` so the Android packager installs it into `nativeLibraryDir` (V7). Set `packaging { jniLibs { useLegacyPackaging = true } }` in the `android/emu` module.
- [ ] Add `emu/qemu/build-android.sh --check`. It prints pinned versions, verifies the sha256 of every source tarball, and is safe to call from `scripts/verify.sh` (no network; it skips if tarballs are absent).
- [ ] Smoke test on an **arm64 AVD** (Apple Silicon host). Push the binary to `/data/local/tmp`; that is fine on an emulator, never on the phone. Run `qemu-system-aarch64 --version` and `-machine help`, `-device help | grep -E 'rutabaga|multitouch|sound'`.

**G2:** The binary runs on an arm64 AVD and lists `virtio-gpu-rutabaga`, `virtio-multitouch`, `virtio-sound` and `tcg`. The size is recorded (target: < 40 MiB stripped, excluding gfxstream).

## Phase 3: On-device feasibility probes (a test APK)

Goal: prove, **on the real phone**, every assumption that Android's app sandbox could break. This is the first time anything is installed on the user's phone: **ask for consent first**, explain exactly what the test APK does, and uninstall it afterwards.

**Files:** `qualification/emu/probe-app/` (a tiny separate Gradle module or app, `dev.carbonhv.emuprobe`), a report.

The probe app runs each check in a background thread, writes JSON to its own files dir, and shows it on screen. Retrieve the result via `adb shell run-as dev.carbonhv.emuprobe cat files/report.json` (debuggable build) or via a share intent. Checks:

- [ ] **P1 exec:** `ProcessBuilder(nativeLibraryDir + "/libqemu_system_aarch64.so", "--version")` succeeds (V7).
- [ ] **P2 JIT memory:** in a native test binary also shipped as `lib*.so`:
  - (a) `mmap(PROT_READ|PROT_WRITE|PROT_EXEC, MAP_ANON)`; write and run a `ret` stub;
  - (b) memfd double mapping (RW + RX views); write and run.
  
  Record which of them work (V8). Then run `qemu-system-aarch64 -accel tcg,split-wx=on|off -machine virt -cpu max -m 64 -nographic -kernel <tiny test kernel>` for both values. Ship a tiny arm64 Linux kernel + initramfs that prints and powers off. Build it in Phase 1 or reuse `qualification/gpu/minimal-guest`.
- [ ] **P3 affinity/uclamp:** read `/dev/cpuset/top-app/cpus`, set a thread's affinity to 4–7, read back `Cpus_allowed_list`, try `sched_setattr` with `uclamp.min`, and record what is permitted (V10). Do this with the app both foreground and background.
- [ ] **P4 Vulkan in a child process:** a native child (spawned like QEMU) creates a Vulkan instance + device on Adreno and allocates an image exportable as `AHardwareBuffer` (`VK_ANDROID_external_memory_android_hardware_buffer`). It sends the AHB over a unix socket to the parent and passes a `sync_file` fence. The parent wraps it in a `HardwareBuffer` and presents it on a `SurfaceView`, showing a solid color. **This de-risks the zero-copy path in Phase 6.**
- [ ] **P5 memory:** read `MemAvailable` and `ActivityManager.getMemoryInfo`. Allocate and touch 2.5 GiB (memfd) in the child while the app is foreground with a foreground service, and record whether LMK kills it and at what level.
- [ ] **P6 io:** open a file in app storage with `O_DIRECT`, do 4 KiB aligned reads/writes; record the result. Try `io_uring_setup` and expect EPERM/ENOSYS (V9).
- [ ] **P7 isolated process:** the same P1/P4 from an `isolatedProcess=true` service (V11). Record whether the KGSL/Vulkan device opens and whether sockets are allowed.
- [ ] **P8 seccomp:** run QEMU with `-sandbox on` and record whether it starts, given the zygote's existing filter.
- [ ] **P9 thermal:** register `PowerManager.addThermalStatusListener` and `getThermalHeadroom` and record them on a 5-minute 4-thread spin. This is the baseline for Phase 12.

Run all of the above on the AVD first, then (with consent) on `kunzite`. Uninstall afterwards: `adb uninstall dev.carbonhv.emuprobe`.

**G3:** On `kunzite`, P1, P2 (at least one W^X mode), P4 and P5 (2 GiB guest viable) pass.
- P3, P6, P7, P8 and P9 are recorded, and their results set Phase 5/7/11/14 defaults.
- **Kill criterion:** if P1 and P2 both fail, there is no in-app emulator on this phone. Stop and report.
- If P4 fails, the zero-copy path is dead; fall back to readback-only (§Phase 6 fallback) and tell the user the graphics cost.

## Phase 4: Android UI backend: frames to the app

**Files:** `emu/qemu/patches/0001-ui-add-android-ahb-backend.patch` (`ui/android.c`, `ui/meson.build`, the `qapi` display type), `crates/carbon-emu/src/frame_proto.rs`, the `android/emu` JNI frame reader.

Design (`frame_proto` v1, versioned and fixed-size, little-endian):
- The socket is a `SOCK_SEQPACKET` socketpair inherited by QEMU as fd N (`-display android,frame-fd=N`).
- QEMU → app messages:
  - `HELLO{version}`
  - `MODE{width,height,format}`
  - `FRAME{seq, width, height, format, usage, has_fence}`, followed immediately by the AHB handle (`AHardwareBuffer_sendHandleToUnixSocket`) and, if `has_fence`, an `SCM_RIGHTS` `sync_file` fd
  - `CURSOR{...}` (unused for touch guests)
- App → QEMU messages: `RELEASE{seq}` (buffer is free to reuse) and `RESIZE{width,height,dpi}` (forwarded to the guest as a display hotplug or mode change, if supported; otherwise ignored in v1).
- QEMU keeps a ring of 3 AHBs per mode, never reuses one without `RELEASE`, and drops (does not block) when all are in flight. This mirrors `GraphicsSession`'s backpressure (`QUEUE_FULL`).

Steps:
- [ ] Implement the backend first in **readback mode**: on `dpy_gfx_update`/`gfx_switch`, copy the pixman `DisplaySurface` into a CPU-writable AHB (`AHARDWAREBUFFER_USAGE_CPU_WRITE_OFTEN | GPU_SAMPLED_IMAGE`), send it, and send no fence. With `virtio-gpu-rutabaga`, verify how scanout reaches the UI layer (V1): resource readback (`rutabaga_resource_transfer_read`) or a GL/dmabuf scanout callback. Implement whatever the pinned QEMU exposes.
- [ ] JNI in `:vm`: read `FRAME`s, wrap them as `HardwareBuffer` + fence fd, and send them via Binder (`HardwareBuffer` is `Parcelable`; pass the fence as a `ParcelFileDescriptor`) to the main process. There they become `NativeFrame`s for `GraphicsSession.presentFrame`. Return `RELEASE` on `drainReleasedFrames`.
- [ ] Unit-test `frame_proto` encode/decode in Rust (golden bytes, truncated message, wrong version, oversize dimensions → reject).
- [ ] On the AVD: boot the stand-in guest (fewer vCPUs, it is an emulator in an emulator) and see the boot screen and launcher inside CarbonHV's session surface. Take screenshots **on the AVD only**.

**G4:**
- The guest display is visible and updating in the app's `ImmersiveSessionScreen` on the AVD.
- No frame is presented with a stale generation; reuse `GraphicsSession`'s generation tests.
- Readback cost per frame is recorded.

## Phase 5: Input

**Files:** in `ui/android.c` (input part), `frame_proto.rs` (input messages) or a separate `input_proto.rs`, `android/emu` forwarders, reusing `SessionInputPolicy.kt`.

- [ ] Use a separate `SOCK_SEQPACKET` input socket (app → QEMU) so input never queues behind frames. Messages:
  - `TOUCH{slot, tracking_id, x, y, pressure, major, action}`, batched per `MotionEvent`;
  - `KEY{linux_keycode, down}`;
  - `SYN`.
  
  Map Android `MotionEvent` pointer IDs to MT slots. Coordinates are absolute in guest display pixels after `SessionGeometry` mapping.
- [ ] In QEMU, inject via `qemu_input_queue_mtt`/`qemu_input_queue_abs` + `qemu_input_event_sync` into `virtio-multitouch` and `virtio-keyboard`. Verify the API names at the pin.
- [ ] Reuse the existing input policy: the escape gesture and immersive ownership rules (§11.1) are handled in the app before forwarding. **Do not duplicate them.**
- [ ] Test on the AVD: tap, scroll, pinch and keyboard typing in the guest Settings search. Record touch-to-photon latency with the method from `qualification/gpu/workloads` if one exists; otherwise add a simple guest test app that flashes on touch, and measure with frame timestamps.

**G5:** Multi-touch and keyboard work on the AVD. Latency is recorded. The escape gesture still returns to CarbonHV.

## Phase 6: gfxstream on Adreno, zero-copy presentation

This is the "hardware-accelerated graphics" phase, and the most important one for usability.

**Files:** `emu/qemu/patches/0003-virtio-gpu-rutabaga-ahb-scanout.patch` (or a gfxstream patch, whichever layer owns the scanout image), `crates/carbon-renderer` (shared profile), a report.

- [ ] Confirm on the phone (consent, debug build of CarbonHV) that `virtio-gpu-rutabaga` initializes gfxstream on Adreno 710 inside the QEMU child process. The guest must report the gfxstream GLES/Vulkan strings. Run the existing GPU qualification workloads (`qualification/gpu/workloads`) inside the guest if they apply to an Android guest.
- [ ] Zero-copy scanout. The guest's ranchu HWC composes through the gfxstream **composer** capset into a host `ColorBuffer` designated as the display, then posts it.
  - Patch the host side so the display ColorBuffers are allocated as Vulkan images **backed by `AHardwareBuffer`s** (a ring of 3).
  - On post, export a `sync_file` from the composition fence (`VK_KHR_external_semaphore_fd`/`vkGetSemaphoreFdKHR` with `SYNC_FD` handle type, or `VK_ANDROID_native_buffer` semantics) and send `FRAME` with the AHB + fence (Phase 4 protocol, `has_fence=1`).
  - This replaces readback. The exact hook depends on gfxstream's display/post API at the pinned revision: find the "post color buffer" callback path. **Spike it first and write down the hook before coding.**
- [ ] Keep readback as the fallback, selectable by profile, and **say so in diagnostics** when it is active ("Emulated · readback presentation").
- [ ] Renderer profile: add `gfxstream-vulkan-adreno-emulated`, sharing the `gfxstream-vulkan-adreno` contract (`docs/SUPPORT_MATRIX.md` §2b). The guest ABI is identical; the only differences are the host process model and the presentation transport.
- [ ] Measure: frame time p50/p95/p99 for a scrolling list (SystemUI settings), a GLES game-like workload and a Vulkan workload. Compare readback vs. zero-copy. Record GPU utilization (`/sys/class/kgsl/kgsl-3d0/gpu_busy_percentage`, read-only) if readable.
- [ ] **No SwiftShader check:** fail the gate if any guest renderer string mentions SwiftShader, llvmpipe, or a software renderer.

**G6:**
- Guest GLES and Vulkan run on Adreno through gfxstream on `kunzite`.
- Zero-copy presentation works with fences, or the readback fallback is measured and the user is told the cost.
- A report with frame-time numbers is written.
- Screenshots of the **running guest on the phone** are now permitted. Take them, store them under `docs/screenshots/` only if the user approves using them in the README, and label them "Emulated engine".

## Phase 7: Storage, network, audio, agent channel

**Files:** `crates/carbon-emu/src/argv.rs` (devices), `emu/qemu/patches/0002-audio-add-aaudio-backend.patch`, `android/emu` (fd plumbing, vsock backend), reports.

- [ ] **Storage:**
  - Open every disk in `:vm` (Kotlin `ParcelFileDescriptor` / Rust `open`) with the right mode.
  - Pass each one via `-add-fd fd=N,set=K` + `file=/dev/fdset/K`.
  - Read-only images use `readonly=on`, and the fd must be opened `O_RDONLY`.
  - Apply the cache/aio settings from P6 results.
  - Test: guest `userdata` persists across a cold boot; a crash mid-write followed by a cold boot gives a consistent fs (f2fs/ext4 fsck clean) at least 10 times in a row.
- [ ] **Network:**
  - `-netdev user` (libslirp): DNS from the host, IPv4 NAT, no inbound by default (ADR-010 privacy).
  - Map the existing network profiles (§21.1) to slirp options: "Offline" means no netdev; "NAT" means slirp; others are not supported in the emulated engine v1. Say so in the UI.
  - Test: guest browser loads an HTTPS page; host VPN changes propagate (slirp uses host sockets, so they follow the host).
- [ ] **Audio:**
  - Implement QEMU audiodev `aaudio` (`audio/aaudio.c`): output via the AAudio NDK (low-latency, `AAUDIO_PERFORMANCE_MODE_LOW_LATENCY`), with input only if the CVD1 contract asks for it later (it does not today: `audio-output` only).
  - Handle device changes and errors by reopening the stream.
  - Test: guest media playback, latency measured with a click test.
- [ ] **Agent channel (vsock), per §3.6:**
  - Spike `vhost-user-vsock` with the rust-vmm `vhost-device-vsock` backend inside `:vm` (cross-compiled for Android).
  - Confirm the guest agent connects (`guest/common/PROTOCOL.md` handshake) and that the host side is the existing agent client code.
  - If the spike fails, write a decision memo before touching the guest.
- [ ] **Console/logcat:** virtio-serial ports to files in the instance's log dir, size-capped and rotated. Never expose them to other apps.

**G7:** Every CVD1 device in `guest/android/device/cvd1.toml` is functional under the emulated engine on `kunzite`, with a report per device. The `required_capability` list is satisfiable, or the unsatisfied entries are listed and the bundle is refused admission for this engine with a clear reason.

## Phase 8: `crates/carbon-emu` and the engine model (pure Rust, Mac-testable)

Can run in parallel from Phase 1 onwards.

**Files:** see §3.7.

- [ ] `EmuPlan::new(spec: &ValidatedVmSpec, host: &HostResources, emu_host: &EmulationHostFacts, profile: EmuProfile) -> Result<EmuPlan, EmuPlanError>`.
  - `EmulationHostFacts`: `mem_available_mib`, `big_cores: Vec<u32>`, `w_xor_x: WxMode`, `o_direct: bool`, `vulkan: VulkanFacts`, `affinity_allowed: bool`, `uclamp_allowed: bool`.
  - `EmuProfile`: `Balanced` (default), `Hardened` (MTE emulated, slower) and `LowMemory`.
- [ ] `argv.rs`:
  - Build the exact argv as `ProgramInvocation` (reuse the type from `backend.rs`).
  - Every resource is an fd number from an `EmuSessionFds` struct, like `DirectSessionFds`.
  - Use no string interpolation of guest-provided values and no paths except `nativeLibraryDir`'s QEMU executable (validated absolute path, no `..`; reuse the `Component` checks from `direct_crosvm.rs`).
  - Always pass `-nodefaults -no-user-config` and never `-monitor`/`-serial stdio`/`-display gtk|sdl|vnc`.
- [ ] `placement.rs`: vCPUs = min(requested, big cores, profile cap), and the affinity mask = the big cores. Reuse `host_resources.rs` MIDR classification (A55 `0xd05` excluded, A78 `0xd41` included).
- [ ] `memory.rs`: the §5.5 budget and admission rule, with typed errors that the UI renders.
- [ ] `cpu_model.rs`: §5.3 per profile.
- [ ] `bootimg.rs`: parse boot v4 / vendor_boot v4 / init_boot headers with strict bounds checks, and produce kernel + concatenated ramdisk + bootconfig (V14). Fuzz with `cargo fuzz` (or proptest) over malformed headers. Reuse the parsing knowledge in `host/product/snapdragon/patch_vendor_boot.py`, but write it fresh in Rust.
- [ ] `qmp.rs`: a typed subset: `qmp_capabilities`, `query-status`, `system_powerdown`, `quit`, `stop`/`cont`, `query-cpus-fast`. Strict JSON parsing and a size cap per message.
- [ ] Golden tests (`tests/argv_golden.rs`): for `kunzite` facts (from the 2026-11-04 probe report fixture) × each profile, assert the full argv. Add negative tests: insufficient memory, no big cores, a path with `..`, vCPUs above the cap, MTE with readback-only presentation (allowed) vs. MTE with zero-copy blobs (reject if Phase 6 proved incompatibility).
- [ ] `carbon-core`: add the `ExecutionEngine` and `EmulationReadiness` types and serialize them in instance metadata with a migration (existing instances default to `HardwareVm`). Add tests in `crates/carbon-core/tests/lifecycle.rs`.
- [ ] `carbon-broker/backend.rs`: engine selection that **never** returns `Emulated` unless the instance's stored choice is `Emulated`. Add a test: a host with `VendorHypervisorWithoutAvf` + an instance with no stored engine gives an admission error ("needs hardware virtualization or explicit emulated mode"), **not** emulation.
- [ ] CLI: `carbonctl emu plan --facts <json> --profile balanced` prints the argv for review (dry-run only).
- [ ] Extend `scripts/verify.sh` with `cargo test -p carbon-emu` (already covered by `cargo test`, but add the golden check and the `build-android.sh --check` step).

**G8:** `cargo test` passes, `scripts/verify.sh` PASS, and golden argvs are reviewed. Then commit and push (rules in §0).

## Phase 9: App integration

**Files:** `android/emu/**`, `android/app/**` (engine choice, badges), `android/domain/**` (state).

- [ ] `EmulatedVmService`:
  - A foreground service (`foregroundServiceType="specialUse"` with the `PROPERTY_SPECIAL_USE_FGS_SUBTYPE` explanation) in `android:process=":vm"`.
  - It owns one QEMU child at a time (ADR-009: one foreground VM).
  - It spawns via JNI `posix_spawn` with an explicit fd map (only the fds in `EmuSessionFds`; everything else `O_CLOEXEC`).
  - It supervises with `pidfd` where available, `waitpid` otherwise.
  - On exit it reconciles the session generation exactly like the AVF death path (`SessionReconnect.kt`, `SessionControlState.kt`).
- [ ] `EmulatedEngineClient` implements the same surface the app uses for `PlatformHostVmControlClient` (`startOrAttach(HostVmStartSpec)`, `stop(identity, requestId)`) so `HostSessionBridge` can be given either engine. Introduce a small `VmEngineClient` interface if needed; do not fork `HostSessionBridge`.
- [ ] Lifecycle:
  - Stop means QMP `system_powerdown` with a timeout (30 s under TCG; measure), then `quit`, then `SIGKILL`.
  - Pause in the background means QMP `stop` when the user leaves the session, if the instance policy says pause (§9.3), and `cont` on return. A stopped TCG VM costs 0 CPU.
  - When the service is killed, disks are left consistent because the next start runs fsck (a guest responsibility) and the host never writes images itself.
- [ ] UI:
  - Engine choice on instance creation when no hardware VM is available: "Run emulated (slow)" with a one-paragraph explanation, an estimated-performance note (from G11 numbers once they exist; until then "much slower than a hardware VM; graphics use your GPU") and the security differences link.
  - An "Emulated" badge on the instance card and in the session HUD.
  - Diagnostics show the TCG settings, presentation mode, vCPU placement and memory budget.
  - Keep the icon-led design from `eb842fd`/`17fcfd9`.
- [ ] Tests:
  - Kotlin unit tests for the reducer/state (`AppReducerTest`, `AppStateCodecTest`).
  - The service state machine with a fake process.
  - Generation reconciliation on crash, kill and restart.

**G9:** On the AVD: create an emulated instance, start, attach, detach, reattach, stop, crash-kill the QEMU process (`kill` **on the AVD**) and recover. All generations are consistent. Screenshots are from the AVD only.

## Phase 10: The GrapheneOS-derived guest, built and tuned for TCG

**Prerequisite:** a Linux x86_64 builder. **Ask the user** for access, or for approval to rent a cloud VM (cost!), before starting.

**Files:** `guest/android/build.sh`, `guest/android/device/*`, `guest/common/kernel/cvd1-emulated.fragment`, `guest/aosp-overlay/**` if the CVD1 overlay is shared (ADR-016).

- [ ] Run `guest/android/build.sh` as designed (GrapheneOS Android 17 line, pinned tag; `CARBON_GRAPHENE_TARGET` per README). Background the sync and build; run status checks only.
- [ ] Add the §5.4 changes as an **emulated profile** that is selected at boot by bootconfig (`androidboot.carbonhv.engine=emulated`) where possible, and at build time only where it must be (full AOT `speed` compilation can stay on for both engines; it also helps hardware VMs boot faster). Record the image size impact.
- [ ] Merge `cvd1-emulated.fragment` (§5.2) and verify with `carbonctl guest kernel-config android …` (it must still pass the CVD1 contract, exit 0 or 5).
- [ ] Desktop TCG boot (Phase 1 harness) with the real GrapheneOS-derived image: setup wizard, lock screen, encrypted userdata (§17.5 graduation criteria apply).
- [ ] Security/difference matrix entries for the emulated engine (§6, §5.3 MTE/PAC).

**G10:** The GrapheneOS-derived guest boots to setup/launcher under desktop TCG with gfxstream, and the matrix is updated. Then build the bundle for the phone.

## Phase 11: Performance work on the phone

**Files:** `qualification/emu/workloads/*`, `qualification/emu/run-sustained.sh` (mirror `qualification/gpu/run-sustained.sh`), reports.

Metrics (all p50/p95/p99 where applicable, 3 runs, phone at a similar temperature (wait for cool-down, record `thermal_status`), on power and on battery):
- Cold boot to `sys.boot_completed`; to launcher interactive.
- App launch: Settings, the guest browser, a small test app (cold and warm).
- Scroll jank: `dumpsys gfxinfo` in the guest during a scripted scroll (`input swipe` **inside the guest** via `adb` over the guest's network or agent, not on the host phone).
- Touch-to-photon latency (Phase 5 method).
- Host: CPU % per QEMU thread, `MemAvailable` floor, battery drain %/10 min, thermal status over 30 min.

Levers to sweep, one at a time, against a fixed baseline. Keep a lever only if it helps:
- [ ] vCPUs 3 vs. 4; affinity on/off; uclamp boost on/off (if P3 allowed).
- [ ] `tb-size` 128/256/512.
- [ ] `pauth-impdef` vs. `pauth=off` (record the security trade-off; the default stays `pauth-impdef`).
- [ ] Guest `HZ` 100 vs. 250; guest zram on/off; RAM 1536/2048/2560.
- [ ] AOT `speed` vs. `speed-profile`; JIT off vs. on.
- [ ] Presentation zero-copy vs. readback.
- [ ] Animation scale 1.0 vs. 0.5.

Targets. These are **targets, not promises**: fill in measured values, and if a target is missed, report it honestly with the numbers. The user then decides whether the engine is worth shipping.

| Metric | Target on `kunzite` |
| --- | --- |
| Cold boot to launcher | ≤ 5 min (first boot ≤ 10 min) |
| Settings cold launch | ≤ 4 s |
| Scroll in Settings | p95 frame time ≤ 33 ms (30 fps) |
| Touch-to-photon | p95 ≤ 120 ms |
| 30-min sustained | no LMK kill, no thermal `SEVERE`, guest responsive |

**G11:** A report with every metric and lever, the defaults updated in `carbon-emu` profiles (with golden tests updated), and the targets met or misses explained.

## Phase 12: Thermal, power, memory pressure, lifecycle robustness

- [ ] Thermal policy: on `THERMAL_STATUS_MODERATE`, drop uclamp boost and cap to 3 vCPUs on the next boot, and suggest a lower refresh rate. On `SEVERE`, QMP `stop` (pause), tell the user, and resume when it is cool. This mirrors the §24.2 NearNative→Efficient idea for hardware VMs.
- [ ] Idle: when the session surface is not visible and the policy is "keep running", rely on the guest idling in WFI. Verify that the host CPU of an idle guest is below 5% of one core (otherwise find the busy guest thread).
- [ ] Memory pressure: on `onTrimMemory(RUNNING_CRITICAL)` / `TRIM_MEMORY_COMPLETE`, ask the guest to drop caches via the agent if available, then pause. Never let LMK kill QEMU mid-write without the user being told on the next start ("the emulated VM was stopped by Android to free memory").
- [ ] Battery: show the measured drain in diagnostics. Default to pausing in the background on battery.
- [ ] Checkpoints (optional, gated): QEMU `migrate` to file for a fast resume could hide the slow boot.
  - Blockers: virtio-gpu-rutabaga/gfxstream state may not be migratable (check for a migration blocker at the pin), and vhost-user devices complicate it.
  - Spike only. If it is infeasible, write it down; ADR-009 (cold boot baseline) stays.

**G12:** 30-minute sustained runs on battery and on charger with no uncontrolled kills, the thermal policy observed acting, and a report.

## Phase 13: Qualification, documentation, licensing, release

- [ ] `qualification/emu/README.md`: how to run every gate, with report schema (reuse `qualification/gpu/schema` style).
- [ ] `docs/SUPPORT_MATRIX.md`: an **Emulated engine** column/section per host. `kunzite`: the measured numbers, `EXPERIMENTAL`, renderer profile, presentation mode, vCPU placement and security differences.
- [ ] `docs/ARCHITECTURE.md`: §8 (the emulated machine), §12 (presentation over AHB socket), §23 (the §6 table).
- [ ] `docs/SECURITY.md`: the emulated threat model.
- [ ] `NOTICE`: QEMU (GPL-2.0), glib (LGPL-2.1+), pixman (MIT), libslirp (BSD-3), dtc (GPL-2.0/BSD-2), gfxstream/rutabaga (Apache-2.0/BSD), plus a written source offer and the exact source tarballs + patches published with every release that ships the binary. **The user confirms the licensing approach (G0) before this ships.**
- [ ] `README.md`: a short, honest section: "No hardware virtualization on your phone? CarbonHV can run guests in Emulated mode: much slower CPU, GPU-accelerated graphics."
- [ ] `CHANGELOG.md` entry.
- [ ] Release build: the APK includes `libqemu_system_aarch64.so` built reproducibly from `emu/qemu/VERSIONS.toml`, with provenance recorded (source hashes, NDK version, build host).
- [ ] `scripts/verify.sh` PASS, then commit and push (§0 rules).

**G13:** Docs and reports are merged and pushed; the user has reviewed the README wording.

## Phase 14: Hardening (process split)

Use the P7 results:
- If `isolated_app` can run QEMU but cannot open the GPU (likely), split the work:
  - QEMU runs in an `isolatedProcess` service with no network, no files and fds only.
  - The GPU runs as **`vhost-user-gpu`** in the `:vm` process (a rutabaga/gfxstream backend that speaks vhost-user to QEMU). Evaluate whether crosvm's `crosvm device gpu` vhost-user backend or a rust-vmm based backend interoperates with QEMU's `vhost-user-gpu-pci` frontend at the pin. Spike it.
  - Network goes over `-netdev stream`/`dgram` with an fd to a slirp/passt-like userspace stack in `:vm`.
- QEMU `-sandbox on` if P8 allows it.
- Re-run G6/G7 checks after the split, with the frame-time regression recorded.

**G14:** The emulated process runs with fewer privileges than `untrusted_app`, or a memo explains why it cannot, and graphics performance is within 10% of pre-split.

## Phase 15: Research track (optional; produce a memo, not product code)

Only after G11, and only if the user asks. These are ways to go beyond TCG's ceiling without a hypervisor; each is high-risk.

1. **Systrap-style userspace kernel (gVisor-like) for Android userspace.** Guest user code runs **natively** on the host CPU (same ISA), and a userspace "sentry" implements the Linux syscall ABI. gVisor's systrap platform uses seccomp `SECCOMP_RET_TRAP` + signals with no KVM.
   - Android userspace needs binder, ashmem/memfd, ion/dma-buf, SELinux fs, cgroups, FUSE and much more, which is enormous to emulate.
   - It would also abandon the CVD1 VM ABI and full-guest isolation.
   - Memo: feasibility, effort, and the security comparison.
2. **Faster TCG for same-ISA guests:** QEMU's "TCG plugin" API is observation-only. A same-ISA translation fast path (copying most user-mode instructions verbatim, rewriting memory ops for soft-MMU) is a research project upstream. Memo: who is working on it and whether anything is usable.
3. **Re-check the hypervisor at runtime:** keep `EmulationProbe` re-checking `ro.boot.hypervisor.vm.supported` and AVF availability on every app start. If a future OTA ever enables AVF on this phone, offer switching the instance to the hardware engine (the same CVD1 image, §3.2).

---

## 7. Risk register

| # | Risk | Likelihood | Impact | Mitigation / detection |
| --- | --- | --- | --- | --- |
| R1 | TCG is too slow for an Android guest to be usable on a mid-range phone | **High** | Engine not worth shipping | Desktop TCG/native ratio in G1 gives an early estimate; the §5 levers; honest G11 report and user decision |
| R2 | Not enough RAM (`MemAvailable` ~2.7 GiB) | High | LMK kills, OOM | 2 GiB guest + zram, admission check, foreground service, pause on trim (G3 P5, G12) |
| R3 | JIT memory blocked for `targetSdk 36` apps | Low–Med | No TCG at all | `split-wx=on` (memfd dual map, ART's own technique); P2 in G3 is the kill gate |
| R4 | `virtio-gpu-rutabaga` / gfxstream does not build or run on an Android host in a child process | Med | No hardware graphics → no engine (no SwiftShader allowed) | Reuse the `carbon-renderer` native-gfxstream lineage; P4 de-risks Vulkan+AHB in a child; G6 |
| R5 | No zero-copy hook in gfxstream's display path | Med | Readback costs CPU/memory bandwidth | Readback fallback, measured; spike before coding |
| R6 | vsock agent cannot work without `/dev/vhost-vsock` | Med | Agent features missing | vhost-user-vsock in `:vm` (§3.6); console fallback memo |
| R7 | Thermal throttling collapses performance after minutes | High | Bad UX | Thermal policy (G12); vCPU 3 default if measured better |
| R8 | GPL obligations misunderstood | Low–Med | Legal | Separate executable, source offer, NOTICE, **user confirms** (G0, G13) |
| R9 | Cuttlefish QEMU mode bit-rotted at the pinned AOSP revision | Med | Boot flow work | Direct kernel boot fallback (§3.4) |
| R10 | Guest JIT/dexopt still runs in the background | Med | CPU spikes | AOT everything, dexopt policy, audit `top` in the guest |
| R11 | User's phone disturbed during testing | Low | Trust | §0 rule 2: consent before installs, AVD first, read-only probes |
| R12 | Scope creep into "make it native" | Med | Wasted effort | §2.3 ruled-out list; Phase 15 is memo-only |

## 8. Definition of done (for the whole plan)

- On a stock `kunzite`, an ordinary user can:
  - install the CarbonHV APK (no ADB, no Shizuku, no flashing);
  - create a GrapheneOS-derived instance in **Emulated** mode;
  - boot it, use it with touch/keyboard, with GPU-accelerated graphics, audio and network;
  - stop it, and resume it later from a cold boot with their data intact.
- Every gate report exists under `qualification/emu/reports/`, with real numbers.
- Docs, the matrix, the security model, NOTICE and the README honestly describe the engine as **emulated**.
- `scripts/verify.sh` PASS; pushed to `origin main` per §0.
- Screenshots of the running guest on the phone exist and are labelled "Emulated engine".

## 9. Verification commands (quick reference)

```bash
# Host repo checks
scripts/verify.sh
cargo test -p carbon-emu
cargo run -q -p carbon-cli -- emu plan --facts qualification/emu/fixtures/kunzite.json --profile balanced

# Phone: read-only probes only (no installs without consent)
adb -s 8b227126 shell getprop ro.boot.hypervisor.vm.supported
adb -s 8b227126 shell 'grep -E "MemTotal|MemAvailable" /proc/meminfo'
adb -s 8b227126 shell 'ls -Z /dev/gunyah; getenforce'
adb -s 8b227126 shell 'timeout 10 /apex/com.android.virt/bin/vm info'   # always with a timeout: it hangs after virtmgr panics

# AVD (all UI/iteration work)
emulator -avd <arm64 AVD> &   # background
adb -s emulator-5554 install -r android/app/build/outputs/apk/debug/app-debug.apk
```

## 10. Things not to do

- Do not flash, root, patch boot images or use exploits, even "just to test".
- Do not install anything on the user's phone or touch its UI without asking; do not kill processes you did not start there.
- Do not use SwiftShader/llvmpipe or any software GPU, even as a temporary fallback that "will be removed later".
- Do not auto-select emulation or call it native, near-native or hardware-virtualized anywhere in UI, docs or commit messages.
- Do not fork the CVD1 guest ABI or the agent protocol for the emulated engine.
- Do not commit binaries, guest images, AOSP downloads or anything over a few MB.
- Do not force-push `main`.
- Do not quote the performance estimates in §1 as results. Only measured numbers go in reports and docs.
