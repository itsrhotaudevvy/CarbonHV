# CarbonHV support matrix and architecture coverage

**Evidence date:** 2026-09-27  
**Repository channel:** development / pre-release  
**Supported CarbonHV v1 release:** **No**

This matrix reports what the current repository proves. It intentionally separates host-independent source tests, a static physical-host capability probe, integration templates, guest build recipes, and full hardware qualification.

Status vocabulary:

- **Verified** — exercised by an automated repository test/build in this implementation.
- **Observed** — measured directly on the named physical host, but not enough for full qualification.
- **Partial** — meaningful implementation exists, but required integration or end-to-end evidence is missing.
- **Recipe only** — source/build contract exists; no claim that the full guest artifact was built and booted here.
- **Unverified** — architecture requirement has no qualifying runtime evidence yet.
- **Excluded v1** — deliberately outside the mandatory v1 scope.

## 1. Host-independent implementation status

| Area | Status | Current evidence | Remaining gate |
| --- | --- | --- | --- |
| ARM64-only admission / no CPU emulation fallback | Verified | Rust admission + bundle tests reject cross-ISA, missing virtualization, and emulation-capable runtime metadata | Physical backend must still boot an actual ARM64 guest |
| Lifecycle generation/state model | Verified | Rust lifecycle tests cover legal transitions, stale generations, idempotent requests, single disk writer | Wire to real platform service/runtime and test process death |
| Bundle manifest parsing | Verified | Strict parser and both starter manifests pass `carbonctl bundle verify` | Production signatures/catalog trust not implemented |
| CVD1 virtual-device contracts | Verified | Contract tests cover major version, ARM64, required virtio devices, guest family | Real backend device realization not yet proven |
| Broker request policy | Verified | Caller identity, resource caps, managed IDs, generation replay, double-start rejection | Real Binder/platform identity and SELinux integration |
| AVF launch planning | Verified | Tests cover exact argv, control-character handling, missing AVF tool, runtime path validation | Actual CarbonHV service spawning and owning a VM on device |
| Android manager domain/state | Verified | JVM unit tests | Connect live broker/session data |
| Compose library/catalog/create/settings/recovery UI | Verified build | App JVM tests + debug APK build | Real bundle install/runtime flows and accessibility qualification |
| Immersive `SurfaceView` session shell | Verified build | Geometry/input/reconnect policy unit tests + APK build | Real scanout/guest input, physical gesture/IME/accessibility tests |
| Update transaction model | Verified | Interruption, health/commit, rollback and migration-epoch tests | Real signed guest updates and storage fault injection |
| Backup/restore/clone metadata | Verified | Manifest integrity and identity semantics tests | Real stopped-disk backup/restore on physical guest data |
| Qualification fixtures | Verified | `qualification/test-dry-run.sh` | Fixtures are policy tests, not phone certification |
| GPU capability/profile model | Verified | Host-independent tests cover Mali profile selection, exact tuple invalidation, and no software fallback | Physical renderer execution still required |
| Mali renderer adapter | Verified source/build | Pinned crosvm/Rutabaga/gfxstream profile; broker emits Android 17's gfxstream Vulkan/composer configuration and keeps the guest ABI on `virtio-gpu` | App-to-AVF Android display-service surface handoff is not yet wired on the phone |
| Android GPU probe + presentation boundary | Verified build / physical lifecycle observed | ARM64 JNI library builds; generation/queue tests pass; acquire fences are handed to SurfaceFlinger and native release completions/fences can be drained back to the renderer-facing session; debug APK cold-launched on Pixel 7a and survived rotate, background/foreground, and activity-process recreation without fatal/native errors | No guest-produced `AHardwareBuffer` has reached the physical presentation path yet |
| GPU proof harness | Verified | Positive/no-proof fixtures enforce hardware-evidence gating; physical Pixel report records blocked bring-up without promoting capabilities | Needs real guest/renderer evidence, then sustained and both-guest qualification |
| Near-native performance plan | Verified | Host topology reader (consistent capacity units, cpufreq domains, offline cores) and 1:1 performance-core placement; DVFS/boost exclusivity; exact crosvm flags; AVF `hugePages`/`boostUclamp`/`balloon` carried broker → AIDL → raw config | Measure the per-lever gain and thermal cost on real guests (§24.3/§24.4); thermal profile switching is not yet automatic |
| Snapdragon / Gunyah backend | Verified source / physical run blocked | `VendorHypervisorAvfReady` vs `VendorHypervisorWithoutAvf` classification from `ro.boot.hypervisor.*`; exact `crosvm --hypervisor gunyah` argv with fd-only inputs; ADB probe records hypervisor props, `/dev/gunyah` label, cpufreq domains, THP mode | Controlled Snapdragon host build; `carbonhvd` spawning/supervising tier-2 crosvm; first Gunyah guest boot |
| Guest kernel near-native config | Verified | `carbonctl guest kernel-config` with shared and per-family fragments; Ubuntu Touch assembly refuses a kernel config that fails it; the Android build verifies when `CARBON_GRAPHENE_KERNEL_CONFIG` is supplied | Verify the actual GKI and Ubuntu Touch kernels used for release builds |
| Starter guest graphics contracts | Verified source contract | Android-derived CVD1 requires virtual GLES/Vulkan/native-buffer/explicit-sync; Ubuntu Touch requires GLES/native-buffer/explicit-sync through Mesa/Mir on the same `virtio-gpu` ABI | Real guest builds and runtime renderer identity still unverified |
| Sustained GPU qualification policy | Verified policy / physical run blocked | 30-minute workload classes, metric definitions, threshold evaluator, exact-tuple physical evidence gate, and fail-closed fixture tests are present; Pixel profile has no invented thresholds | Requires both real guests, guest-produced frames, recovery injection, measured baseline, then calibrated thresholds |

## 2. Physical host observation

One attached development host was re-probed on 2026-09-27:

| Property | Observed value |
| --- | --- |
| Device | Google Pixel 7a (`lynx`) |
| CPU ABI | `arm64-v8a` |
| Android release | 17 |
| SDK | 37 |
| Build fingerprint | `google/lynx/lynx:17/CP3A.260905.009/16091614:user/release-keys` |
| Bootloader / vbmeta | unlocked / `orange` verified-boot state |
| `/apex/com.android.virt/bin/vm` | Present/executable |
| `/dev/kvm` | Present |
| SoC | `GS201` (Tensor G2 family) |
| GPU | `Mali-G710` |
| EGL provider | `mali` |
| Vulkan provider | `mali` |
| Vulkan driver version | `226504704` |
| Kernel | `6.1.162-android14-11-g2ec90535fa34-ab15810641` |
| AVF revision | `com.android.virt@371899999` |
| Built-in display | 1080 × 2400; 60/90 Hz modes observed |
| CPU qualification result | `hardware-virtualized` |
| Graphics | `unverified` |
| Overall probe result | `EXPERIMENTAL` |

The ADB qualifier deliberately exits with its experimental status after these static checks. This host is a viable bring-up candidate; it is **not** a supported CarbonHV host tuple yet.

The initial physical GPU bring-up was attempted through `qualification/gpu/run-minimal.sh`. The reproducible report is `qualification/gpu/reports/pixel-7a-mali-g710-2026-09-27.txt`; it remains `EXPERIMENTAL` and grants no GPU capability. A fresh 2026-09-28 readiness check is recorded in `qualification/gpu/reports/pixel-7a-platform-readiness-2026-09-28.txt`. The Android 17 Surface handoff, Rust host-runtime Binder publisher, framework-owned raw-config bridge, and CarbonHV-owned SELinux/service/property contracts now exist in source. The Pixel bootloader is unlocked, so a controlled host image is installable once built, but the stock user build still lacks the privileged `CarbonHvHostIntegration` package, `/system_ext/bin/carbonhvd`, and both CarbonHV Binder services. The controlled product image and real guest artifacts have not yet been built. Android 17's supported gfxstream configuration is emitted by the broker; a separate `/system/bin/carbon-renderer` is not part of the initial Mali path. Direct shell access to AVF's bundled crosvm binary is also denied on this stock user build.

The 30-minute sustained entrypoint was also executed on the same Pixel and recorded in `qualification/gpu/reports/pixel-7a-mali-g710-sustained-2026-09-27.txt`. It stopped at the same preflight blockers before any guest workload ran. All performance/thermal/copy metrics therefore remain `unknown` or `not-run`, the threshold profile remains `uncalibrated`, and both `gpu.sustained` and `gpu.qualified` remain false.

Separately, the current debug APK was installed and cold-launched on the Pixel. Portrait/landscape changes, background/foreground, and activity process recreation completed with no fatal/JNI/native presentation errors in the captured log. Because there was no live VM or guest frame source, this is only Android surface-lifecycle evidence; it does not prove VM continuity, buffer reuse, frame presentation, or Mali execution.

## 2b. Second physical host: Snapdragon 6 Gen 3

A Qualcomm development phone was probed read-only on 2026-09-30 (`qualification/gpu/reports/redmi-kunzite-sm6475-host-probe-2026-09-30.txt`, `qualification/gpu/reports/redmi-kunzite-sm6475-adreno710-2026-09-30.txt`):

| Property | Observed value |
| --- | --- |
| Device | Redmi `kunzite` (`25098RA98G`) |
| SoC | `SM6475` (Snapdragon 6 Gen 3): 4× Cortex-A78 (capacity 1024, 2.4 GHz) + 4× Cortex-A55 (capacity 405, 1.8 GHz) |
| Memory | 7.2 GiB total (about 1.8 GiB available at probe time) |
| GPU | `Adreno (TM) 710`, OpenGL ES 3.2, Vulkan 1.3 device (1.4 instance), driver `2150760518` |
| Android release / kernel | 16 (SDK 36) / `6.6.118-android15-8` |
| Bootloader / vbmeta | unlocked / `orange` |
| Display | 1080 × 2392; 60/90/120 Hz |
| Hypervisor | **Gunyah** (`/dev/gunyah` present; vendor `qti.qvirt` service registered) |
| `/dev/kvm` | **Absent** |
| AVF `vm info` | `VMs are not supported.` (`Device doesn't support protected or non-protected VMs`) |
| CPU qualification result | `UNSUPPORTED` — Gunyah present, AVF has no KVM to run on |
| Renderer profile | `gfxstream-vulkan-adreno` selected as **experimental** only |

What this means:

- **The stock firmware cannot run any CarbonHV guest.** The limit is the hypervisor and boot configuration, not the SoC. AVF refuses VMs because the build does not declare `ro.boot.hypervisor.vm.supported`, and there is no `/dev/kvm`. The broker reports this state as `VendorHypervisorWithoutAvf` and never treats it as ready.
- **Snapdragon path, source-complete and hardware-gated (ADR-013):**
  - *Tier 1, AVF on Gunyah.* A controlled host build declares `androidboot.hypervisor.vm.supported=1` and grants the AVF crosvm domain the vendor `/dev/gunyah` label. The probe now records that label as `vendor_hypervisor_selinux_label`. crosvm then falls back from the absent `/dev/kvm` to `/dev/gunyah`, and CarbonHV's existing AVF display, gfxstream, and input path is reused unchanged. The broker classifies this as `VendorHypervisorAvfReady`; the probe classifies it as `avf_vendor_path=declared`. Both still mean `EXPERIMENTAL`.
  - *Tier 2, CarbonHV-owned crosvm.* `DirectCrosvmBackend` builds an exact `crosvm run --hypervisor gunyah[device=/dev/gunyah]` argv. It pins 4 vCPUs to the A78 cores 4–7, mirrors capacity and the frequency domain, and forwards guest DVFS. It never requests a protected or trusted VM and never uses swiotlb. Spawning and supervising that process from `carbonhvd` is not yet implemented.
- **Re-probed 2026-11-04** (`qualification/gpu/reports/redmi-kunzite-sm6475-host-probe-2026-11-04.txt`):
  - **CPU:** MIDR parts `0xd05`×4 (A55) + `0xd41`×4 (A78) in cpufreq domains `0-3;4-7`. This confirms the broker would pin 4 vCPUs to CPUs 4–7.
  - **Gunyah device:** `/dev/gunyah` is labeled `vendor_gunyah_dev`, and SELinux denies the shell.
  - **Kernel:** `CONFIG_GUNYAH=y` with resource-manager modules loaded. `CONFIG_KVM=y`, but there is no `/dev/kvm`.
  - **THP:** runtime mode is `never`, so the plan now drops huge-page advice on such hosts.
  - **Device state:** bootloader unlocked, no root, SELinux enforcing.
  - The complete controlled-build delta is in `host/product/snapdragon/`: one bootconfig line, one `vm_manager_device_type` attribute line, and THP set to `madvise`.
- The pinned crosvm's Gunyah backend exposes no SVE, MTE, or PMU to guests. This is a recorded guest difference, not a qualification failure.
- Adreno support is a host provider/profile: the guest ABI stays CVD1 `virtio-gpu`. The `gfxstream-vulkan-adreno` profile shares the Mali profile's transport, buffer, sync and sandbox contract and carries no Adreno quirks. Gfxstream vs Mesa/Venus is still to be chosen from measurements, which cannot start without a bootable guest.
- Chip-aware limits are implemented and unit-tested: the broker caps guest vCPUs at the number of performance cores (4 here) and leaves the Android host at least 3 GiB, so this phone grants at most 4319 MiB. That admits Ubuntu Touch (3072 MiB) and only just the GrapheneOS-derived bundle (4096 MiB). Low `MemAvailable` on this firmware is an admission-time risk.
- Nothing above is a capability claim: every `gpu.*` capability is `false` and the overall result stays `EXPERIMENTAL`/`UNSUPPORTED`.

## 2c. SoC family coverage (source level only)

Every SoC family below shares one broker policy, one CVD1 guest ABI, and one gfxstream transport. What differs per vendor is probe facts and a profile ID, never a guest-visible fork. Only the Pixel 7a and the Redmi `kunzite` have been probed physically; every other row is **source and fixture coverage with no runtime claim**.

| Family | Example SoCs | Hypervisor (crosvm backend, device) | In AOSP `com.android.virt` crosvm | Host GPU → profile | Pinnable cores (in-order efficiency cores excluded by MIDR) |
| --- | --- | --- | --- | --- | --- |
| Google Tensor G1–G4 | GS201 (Pixel 7a, observed), Tensor G3/G4 | KVM/pKVM, `/dev/kvm` | yes | Mali-G710/G715 → `gfxstream-vulkan-mali` | G2: 2× X1 + 2× A78; G3: X3 + 4× A715; G4: X4 + 3× A720 |
| Google Tensor G5 | Pixel 10 | KVM/pKVM, `/dev/kvm` | yes | PowerVR D-Series DXT-48-1536 → `gfxstream-vulkan-powervr` | X4 + 5× A725 |
| Samsung Exynos | Exynos 2400/2500 (Xclipse), Exynos 1380 (Mali) | Halla, `/dev/halla`, where the build exposes it | **no**: needs a vendor crosvm built with `halla` | Samsung Xclipse 9x0 → `gfxstream-vulkan-xclipse`; Mali → `gfxstream-vulkan-mali` | 2400: X4 + 5× A720; 2500: X925 + 7× A725 |
| MediaTek Dimensity | 9300, 9400 | GenieZone, `/dev/gzvm` | yes | Immortalis-G925 → `gfxstream-vulkan-immortalis`; `Mali-G720-Immortalis` → `gfxstream-vulkan-mali` | all-big designs: every core |
| Qualcomm Snapdragon | SM6475 (observed), 8 Elite | Gunyah, `/dev/gunyah` | yes | Adreno → `gfxstream-vulkan-adreno` | SM6475: 4× A78; Kryo Silver excluded; Oryon: every core |

Implications that hold across families:

- **Probe order matches crosvm's:** `/dev/kvm`, then `gzvm`, `halla`, `gunyah`. On every vendor hypervisor, AVF readiness needs `ro.boot.hypervisor.vm.supported=1` (`VendorHypervisorAvfReady`). Stock firmware without it reports `VendorHypervisorWithoutAvf`.
- **Direct-crosvm tier:** `DirectCrosvmBackend` names the hypervisor explicitly (`kvm[…]`, `geniezone[…]`, `halla[…]`, `gunyah[…]`), applies the same pinned plan, and never requests a protected VM.
- **Guest CPU features differ by hypervisor.** On the pinned crosvm revision, KVM exposes SVE in auto mode; GenieZone, Halla, and Gunyah expose no SVE and no PMU. MTE stays off on every backend, because an MTE guest cannot map the non-MTE gfxstream blobs used for zero-copy presentation. The GrapheneOS-derived bundle records this as a security difference.
- **Driver versions are part of every qualification tuple.** For example, Tensor G5's PowerVR driver changed during 2025–2026, so evidence from one driver version never carries over to another.
- **Core classification:** the ADB probe records `cpu_parts`, `cpu_freq_domains`, `renderer_profile` and the vendor device's SELinux label, so a first physical run on each family can confirm or correct the classification above.

## 3. Starter guest status

| Guest | Source/contract | Build | Boot | Mobile shell | GPU/input | Security/confinement | Update/recovery | Current label |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| CarbonHV GrapheneOS-derived ARM64 | CVD1 contract verified; Linux source recipe present; virtual gralloc/HWC/EGL/Vulkan requirements locked | Recipe only | Unverified | Unverified | Contract requires virtual GLES/Vulkan + native-buffer/explicit-sync; runtime unverified | Unverified on VM target | Transaction model only | Development recipe |
| Ubuntu Touch / Lomiri ARM64 | CVD1 contract verified; rootfs+kernel assembler recipe present; Mesa/Mir virtio-gpu requirements locked | Recipe only | Unverified | Unverified | Contract requires GLES + native-buffer/explicit-sync; Vulkan tested if exposed; runtime unverified | AppArmor/mobile-service behavior unverified | Transaction model only | Development recipe |

| LineageOS ARM64 (unofficial) | CVD1 contract verified. The shared AOSP-derived overlay (`lineage_carbonhv_cvd1`) sits on the cuttlefish arm64-only phone; the recipe defaults to `lineage-24.0` (Android 17, `cp2a`), with `lineage-23.2` selectable | Recipe only | Unverified | Unverified | gfxstream guest GLES/Vulkan via the cuttlefish base (Lineage's Mesa/virgl virtio target deliberately not used); runtime unverified | Unverified; bundle boot parameters not yet carried by the broker | Transaction model only | Additional development recipe |
| PixelOS ARM64 (unofficial) | CVD1 contract verified. Same overlay (`aosp_carbonhv_cvd1`); the recipe defaults to `seventeen` (`android-17.0.0_r1` + `lineage-24.0`), with `sixteen-qpr2` (PixelOS 16.2) selectable | Recipe only | Unverified | Unverified | Same as LineageOS | Same as LineageOS; no Pixel-exclusive or Google-certified features | Transaction model only | Additional development recipe |

The GrapheneOS-derived row must not be interpreted as official GrapheneOS device support. Likewise, the LineageOS and PixelOS rows are unofficial CarbonHV builds, not official LineageOS or PixelOS devices or releases. The Ubuntu Touch row means the intended guest is the genuine mobile session, not generic Ubuntu desktop.

## 4. CarbonHV v1 release-definition coverage

The architecture defines v1 only when all mandatory statements below are true.

| v1 criterion from architecture | Coverage | Evidence / gap |
| --- | --- | --- |
| At least one host tuple has local ARM64 hardware execution and qualified guest GPU acceleration | Partial | Pixel 7a exposes AVF `vm` + `/dev/kvm`; no CarbonHV guest boot or qualified GPU path yet. The Snapdragon 6 Gen 3 phone is Gunyah-only (no KVM) and is unsupported until a Gunyah backend exists |
| Both starter bundles are available with truthful provenance/maintenance | Partial | Manifests/contracts/recipes exist; real pinned release images and maintenance baselines do not |
| Native UI can create/start/use/leave/stop/update/backup/restore/delete | Partial | Native flows/reducers and transaction models exist; no live broker/guest end-to-end path |
| Shells/apps/touch/keyboard/audio/rotation/networking match published matrix | Unverified | Requires both real guest ports on physical hardware |
| Immersion has reliable host-owned escape and coherent gestures | Partial | Host escape/input policy exists; physical UX qualification not performed |
| VMs survive activity/surface loss and tested crash/recovery | Partial | Reconnect/state/storage invariants are tested; live VM/process/surface recovery is unverified |
| Guest data isolated; sharing explicit; no generic privileged shell | Partial | Storage IDs/leases plus CarbonHV service/file/property SELinux contracts exist; they are not yet compiled/installed in a controlled host image and sharing bridges remain unimplemented |
| Signing/migration/rollback/recovery exercised | Partial | Migration/rollback transaction tests exist; production signing and real-image recovery are missing |
| Sustained performance targets pass per guest | Unverified | Harness and workload definitions exist, but the Pixel profile is uncalibrated because no real guest performance/thermal baseline has run |
| Unsupported configurations are blocked/experimental; no silent CPU emulation | Verified in policy | Admission/manifest rules fail closed; real product wiring must preserve these rules |

Because the mandatory GPU, real-guest, end-to-end, and sustained-performance criteria remain open, the repository must not be labeled a supported v1 release.

## 5. Architecture section coverage

This is the Task 12 requirements pass against `docs/ARCHITECTURE.md`.

| Architecture area | Current implementation | Status / remaining work |
| --- | --- | --- |
| §2 evidence boundaries | Qualification harness separates fixture/static host facts from GPU/guest evidence | Verified discipline; full evidence still pending |
| §4 host tiers/distribution | H0/H1-style capability result and host integration templates | Partial; no H2 signed integrated host build |
| §5–6 decomposition/module boundaries | Rust core/protocol/broker/CLI + Kotlin domain/app + guest/host/qualification trees | Verified source structure |
| §7 admission | Explicit host/bundle/resource capability intersection; unknown required capabilities fail closed | Verified policy; real backend authorization probe remains |
| §8 CVD contract | CVD major 1, ARM64, virtio GPU/input/net/blk/snd + vsock contracts | Verified contracts; real AVF realization pending |
| §9 lifecycle/persistence | State, generations, disk leases, idempotent operations | Verified model; broker-to-host runtime publication types/Binder client exist, while the daemon's authenticated app-facing request loop remains pending |
| §10 native host experience | Compose library/catalog/create/settings/recovery surfaces | Partial; live runtime, shortcuts, real install/download flows pending |
| §11 immersion | SurfaceView, host escape affordance, geometry generations, input ownership policy | Partial; physical gesture modes, external display/fold/IME pending |
| §12 graphics/display | Mali capability/profile model, Android 17 AVF gfxstream config, generation-scoped privileged display-service handoff, native SurfaceView reconnect path, AHardwareBuffer/fence fallback state machine, and proof harness | Partial; runtime publication is implemented in source but the controlled host package/daemon are not installed on the Pixel and no physical guest-produced frame has reached the SurfaceView |
| §13 input/accessibility | Coordinate transforms and input policy tests | Partial; keyboard/IME/stylus/screen-reader/large-font physical acceptance pending |
| §14 audio/camera/sensors/haptics | No end-to-end bridges qualified | Unverified / optional subset for v1 except audio output is mandatory |
| §15 host/guest protocol | CVD1 vsock protocol document and bounded handshake/message contract | Partial; production agent implementations and adversarial runtime tests pending |
| §16 bundles/catalog | Strict manifests and starter metadata | Partial; signed catalog/download/install pipeline pending |
| §17 GrapheneOS-derived guest | Honest downstream identity, CVD1 contract, Linux build recipe | Recipe only; full ARM64 port/security matrix/boot/update pending |
| §18 Ubuntu Touch guest | Lomiri-specific contract, direct virtio recipe, verified rootfs input requirement | Recipe only; mobile-session boot/confinement/update pending |
| §19 storage | Disk ownership/install transaction models | Partial; real allocation/encryption/quota/deletion behavior pending |
| §20 update/rollback/backup/clone | Two-phase candidate model, data epochs, backup manifest, clone identity semantics | Verified model; signed artifacts and real disk fault injection pending |
| §21 networking | Architecture only | Unverified on real runtime; VPN/lockdown/IPv6/offline tests pending |
| §22 sharing/notifications | Protocol families described | Unverified; bridge implementations/grant revocation pending |
| §23 security | Narrow typed broker and input/path/resource checks | Partial; dedicated Binder/service-manager/file/property SELinux source contracts now exist, but product-policy compilation/enforcement, renderer hardening, target-product policy compilation/enforcement and release-key controls remain pending |
| §24 performance/resources | Resource caps represented; no performance result claimed | Unverified on real guests; 60 Hz/latency/boot/thermal/reliability targets remain gates |
| §25 failures/diagnostics | Recovery reducers and transaction failure semantics | Partial; structured runtime diagnostics/export and real failure injection pending |
| §26 validation | Rust/Kotlin/contract/fixture tests plus ADB static probe | Partial; physical host/guest/adversarial/experience matrices pending |
| §27 build/release | Deterministic source verifier, debug APK, release Rust build path | Partial; CI provenance, SBOM, production signing/promotion pipeline pending |
| §28 roadmap gates | Gates 0–7 can now be evaluated honestly | Gates 1–3 and 7 remain open; see next section |
| §31 ADRs | ARM64-only, native Compose+surface, narrow broker, AVF-first, CVD1, one foreground VM reflected in code/contracts | Partial runtime realization but architecture alignment is preserved |
| §33 v1 release definition | Policy/source pieces exist | Not met overall |

## 6. Roadmap gate status

| Gate | State | What would close it |
| --- | --- | --- |
| Gate 0 — authorized host path | Partial | Android 17 AVF display API, runtime Binder publisher, caller policy, and CarbonHV-owned SELinux/service/property contracts are pinned in source; compile the supplied product/policy integration in a controlled Android 17 product and build/install that `system_ext` image |
| Gate 1 — actual hardware execution | Open | Boot a minimal ARM64 guest repeatedly through the real CarbonHV backend; verify vCPUs, timers, block, vsock, shutdown, cleanup |
| Gate 2 — local graphics path | Open | Build a bootable ARM64 graphics guest, install the Android 17 host integration, publish the broker-owned display generation before crosvm launch, then prove gfxstream-rendered guest frames reach the Android surface with measured copy/fence behavior |
| Gate 3 — both starter guest ports | Open | Real GrapheneOS-derived and Ubuntu Touch images boot to genuine mobile shells, persist data, preserve declared confinement, and use qualified display/input |
| Gate 4 — complete native product flow | Partial | Wire the app to live broker/bundle install/runtime; qualify text input, escape, recreation, return, accessibility |
| Gate 5 — durable data/updates | Partial | Signed real-image update/backup/restore plus interruption/storage exhaustion/migration recovery tests |
| Gate 6 — bounded integrations | Open/optional sequencing | Implement only selected bridges and prove per-feature grant/revocation under compromise/process death/lock |
| Gate 7 — supported release | Open | Both guests pass mandatory experience/security/update/performance matrix on at least one exact host tuple |

## 7. Hardware-only evidence still required

The following cannot be truthfully completed by building the repository on the macOS development machine:

- real Android host service authorization and SELinux enforcement;
- hardware-backed guest execution through the actual integrated service;
- AVF/CVD1 virtual-device compatibility beyond static launch planning;
- guest GPU acceleration, renderer selection, buffer sharing, fences, scanout, and surface presentation;
- touch-to-visible latency and geometry/input behavior against real rendered frames;
- real audio output and optional capture paths;
- network/VPN/IPv6/offline behavior through the privileged runtime;
- activity recreation, surface loss, host process death, screen lock, and host OTA recovery with a live VM;
- low-memory and storage-exhaustion behavior;
- 20-boot cold-boot measurements, 100 start/stop cycles, 30-minute audio and thermal workloads, and frame-latency targets;
- full GrapheneOS-derived setup/lock/reboot/confinement/security-feature matrix;
- full Ubuntu Touch setup/Lomiri gestures/session/AppArmor matrix;
- real signed guest update, rollback, repair, backup/restore, and migration behavior;
- physical accessibility/gesture-mode/external-keyboard/fold/external-display acceptance.

Until those gates are closed with recorded device/build/renderer/guest tuples, `EXPERIMENTAL` is the strongest truthful physical-host label.

## 8. Deliberately excluded or later work

These do not block v1 when their absence is communicated accurately:

- protected GUI VMs;
- durable RAM checkpoints;
- multiple concurrent foreground VMs;
- microphone, camera, location and notification mirroring;
- host IME bridging beyond the mandatory usable keyboard/text-input experience;
- additional guest operating systems beyond the starter pair, which do not gate v1. The LineageOS and PixelOS ports are additional recipes built on the same CVD1 overlay (ADR-016).

They should not be enabled by weakening the core native-execution, isolation, or qualification requirements.
