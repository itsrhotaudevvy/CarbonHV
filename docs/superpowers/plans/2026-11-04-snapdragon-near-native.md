# Snapdragon/Gunyah and Near-Native Performance Plan

**Goal:** Give CarbonHV a source-complete path to run the GrapheneOS-derived and Ubuntu Touch guests on Snapdragon (Gunyah) hosts, and make both host families run guests as close to bare metal as the hypervisor allows, without weakening isolation or qualification honesty.

**Decisions:** ADR-013 (Gunyah tiers) and ADR-014 (near-native via host-side placement), and `docs/ARCHITECTURE.md` §24.3.

**Upstream facts relied on (checked 2026-11-04):**

- crosvm `88c1385`: `--hypervisor gunyah[device=…]` (cargo feature `gunyah`). The default-hypervisor probe tries `/dev/kvm` first, then `/dev/gunyah`. Unprotected VMs get shared memory and no swiotlb. The Gunyah `VmCap` set has no SVE, MTE, or PMU.
- crosvm flags `--cpu-affinity`, `--cpu-capacity`, `--cpus num-cores=,clusters=,freq-domains=`, `--virt-cpufreq-upstream` (requires affinity and capacity), `--boost-uclamp` (`uclamp.min` set to full capacity), `--hugepages`, `--balloon-page-reporting`.
- AOSP virtmgr: passes `--hypervisor gunyah[...]` only for protected QCOM trusted VMs. `CpuTopology` is `cpuCount | matchHost`. Raw config has `hugePages`, `boostUclamp`, `balloon`. The crosvm Android display client registers through `IVirtualizationServiceInternal`.
- Linux `CONFIG_CPUFREQ_VIRT` binds `qemu,virtual-cpufreq`.
- Guest baselines: GrapheneOS on Android 17 (the QPR1 release was held back as of Sept 2026); Ubuntu Touch 24.04 series. Release automation pins exact tags.

## Done in this slice

- [x] Host topology: consistent capacity units, cpufreq domains, offline cores, and 1:1 performance-core placement (`host_resources.rs`).
- [x] `GuestPerformancePlan` with `NearNative` and `Efficient` profiles, `CpuControl`, DVFS/boost exclusivity, and exact crosvm flags (`performance.rs`).
- [x] AVF tunables carried broker → `HostRuntimeGraphicalVmRequest` → AIDL → Kotlin → `VirtualMachineRawConfig`.
- [x] `DirectCrosvmBackend` exact argv: explicit hypervisor, fd-only control and input, no protected VM or swiotlb (`gunyah.rs`).
- [x] `VendorHypervisorAvfReady` classification from `ro.boot.hypervisor.*`; `carbonhvd` reads the properties (`librustutils`, `get_prop(carbonhvd, hypervisor_prop)`).
- [x] ADB probe: hypervisor properties, `/dev/gunyah` SELinux label, cpufreq domains, THP mode, `avf_vendor_path`.
- [x] `carbonctl guest kernel-config` plus shipped fragments. Ubuntu Touch assembly is gated on it.

## Next slices (hardware-gated)

1. Build a controlled Snapdragon host (`host/product/README.md`, "Snapdragon (Gunyah) controlled hosts"), then re-probe the Redmi `kunzite` and record the tuple.
2. Boot the minimal guest (`qualification/gpu/minimal-guest`) through AVF on Gunyah 20 times; confirm vCPU threads, timers, block, vsock, and clean shutdown.
3. Tier 2: `carbonhvd` spawns `DirectCrosvmBackend` invocations in a dedicated crosvm domain. It keeps preserved fds and reuses the host app's display attach through `waitDisplayService`. It supervises exit and reconciles generations exactly as the AVF death path does.
4. Measure each lever on both guests: pinning, DVFS, THP, and page reporting, each against the AVF `cpuCount` + boost baseline. Record p50/p95/p99 frame time, touch latency, thermal state, and power. Remove any lever whose measured cost exceeds its gain.
5. Automate thermal switching from `NearNative` to `Efficient` (§24.2).
