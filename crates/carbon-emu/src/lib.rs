//! The emulated (QEMU TCG) engine's launch planning.
//!
//! This crate is pure policy and pure Rust: no Android, no Binder, no filesystem writes. The
//! `:vm` service and `carbonctl` call it, so the argv that runs on a phone can be reviewed and
//! golden-tested on a laptop.
//!
//! Honesty rules this crate enforces (ADR-017, ADR-012):
//!
//! - The engine is always [`ExecutionEngine::Emulated`](carbon_core::ExecutionEngine) and labelled
//!   "Emulated"; nothing here produces a hardware-VM invocation.
//! - Graphics are hardware or nothing: a plan without a Vulkan device is refused rather than
//!   falling back to a software renderer.
//! - Every resource is an inherited descriptor or a broker-validated absolute path. No shell, no
//!   guest-controlled text in argv.

#![forbid(unsafe_code)]

mod argv;
pub mod bootimg;
mod cpu_model;
mod facts;
pub mod frame_proto;
mod json;
mod memory;
mod placement;
pub mod qmp;

use std::path::PathBuf;

use carbon_core::ProgramInvocation;

pub use argv::{EmuPlan, build_plan};
pub use cpu_model::{CpuModelPlan, for_profile as cpu_model_for_profile};
pub use facts::{EmuHostFacts, FactsError, VulkanFacts, WxMode};
pub use memory::{
    DEFAULT_GUEST_MIB, HOST_RESERVE_MIB, MAX_GUEST_MIB, MemoryBudget, guest_memory_mib,
};
pub use placement::{EmuPlacement, MAX_VCPUS};

/// Which guest-visible posture a plan targets. Every profile keeps CVD1's ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmuProfile {
    /// Default: 4 vCPUs, `pauth-impdef`, MTE off, 256 MiB translation cache.
    #[default]
    Balanced,
    /// Emulated MTE at a measured cost (`-machine virt,mte=on`); still PAC/BTI.
    Hardened,
    /// 1536 MiB guest, 3 vCPUs, 128 MiB cache, 256 MiB gfxstream window.
    LowMemory,
}

impl EmuProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            EmuProfile::Balanced => "balanced",
            EmuProfile::Hardened => "hardened",
            EmuProfile::LowMemory => "low-memory",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "balanced" => Some(EmuProfile::Balanced),
            "hardened" => Some(EmuProfile::Hardened),
            "low-memory" | "lowmemory" => Some(EmuProfile::LowMemory),
            _ => None,
        }
    }
}

/// How composed guest frames reach the app. Both require a GPU; only `ZeroCopy` avoids a readback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationMode {
    /// gfxstream display `ColorBuffer`s exported as `AHardwareBuffer`s plus a sync fence.
    ZeroCopy,
    /// Copy the composed surface into a CPU-writable `AHardwareBuffer`. Bounded fallback, always
    /// reported in diagnostics as "Emulated · readback presentation".
    Readback,
}

impl PresentationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            PresentationMode::ZeroCopy => "zero-copy",
            PresentationMode::Readback => "readback",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "zero-copy" | "zerocopy" => Some(PresentationMode::ZeroCopy),
            "readback" => Some(PresentationMode::Readback),
            _ => None,
        }
    }
}

/// Guest networking. Only the two profiles the emulated engine can honour in v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmuNetwork {
    Offline,
    /// libslirp user-mode NAT: outbound only, DNS from the host, no inbound by default.
    Nat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmuDisplay {
    pub width_pixels: u32,
    pub height_pixels: u32,
    pub density_dpi: u32,
    pub refresh_rate: u32,
}

/// Display geometry used by probes and the desktop dry-run when no device size is known yet.
/// 1080x2392@60 is the reference phone's reported mode.
pub const DEFAULT_DISPLAY: EmuDisplay = EmuDisplay {
    width_pixels: 1080,
    height_pixels: 2392,
    density_dpi: 420,
    refresh_rate: 60,
};

impl EmuDisplay {
    fn validate(self) -> Result<(), EmuPlanError> {
        if self.width_pixels == 0
            || self.height_pixels == 0
            || self.density_dpi == 0
            || self.refresh_rate == 0
            || self.width_pixels > 16_384
            || self.height_pixels > 16_384
            || self.density_dpi > 10_000
            || self.refresh_rate > 1_000
        {
            return Err(EmuPlanError::InvalidDisplay);
        }
        Ok(())
    }
}

/// One guest disk, identified by the *host* descriptor the service will pass to QEMU.
///
/// QEMU only accepts an `/dev/fdset` descriptor whose `O_ACCMODE | O_DIRECT` flags equal what it
/// asks for (monitor/fds.c), so the service must open each disk exactly as
/// [`EmuDisk::open_flags`] says. Observed on the phone at QEMU 11.1.3 (Gate 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmuDisk {
    pub fd: i32,
    pub writable: bool,
}

/// How the service must `open(2)` a disk before passing it to QEMU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskOpenFlags {
    /// `O_RDWR` when true, `O_RDONLY` otherwise.
    pub read_write: bool,
    /// `O_DIRECT`, required exactly when the drive uses `cache=none`.
    pub direct: bool,
}

impl EmuDisk {
    /// The open flags matching the `-drive` options the argv builder emits for this disk.
    pub fn open_flags(&self, host: &EmuHostFacts) -> DiskOpenFlags {
        DiskOpenFlags {
            read_write: self.writable,
            direct: host.o_direct,
        }
    }
}

/// Descriptors the service hands to QEMU. QEMU never receives a path into app storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmuSessionFds {
    pub qmp: i32,
    pub frame: i32,
    pub input: i32,
    /// vhost-user-vsock socketpair fd (guest agent channel, §3.6 option 1). `None` disables it.
    pub vsock: Option<i32>,
    /// Virtio-console fd for the guest console/logcat (Phase 7). `None` disables it.
    pub console: Option<i32>,
}

impl EmuSessionFds {
    /// Every fd must be a real inherited descriptor, distinct from the others.
    pub fn validate(&self, disks: &[EmuDisk]) -> Result<(), EmuPlanError> {
        let mut all = vec![self.qmp, self.frame, self.input];
        all.extend(self.vsock);
        all.extend(self.console);
        all.extend(disks.iter().map(|disk| disk.fd));
        if all.iter().any(|fd| *fd < 3) {
            return Err(EmuPlanError::InvalidSessionDescriptor);
        }
        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != all.len() {
            return Err(EmuPlanError::InvalidSessionDescriptor);
        }
        Ok(())
    }
}

/// Everything the engine needs to plan one boot. The same CVD1 guest serves hardware and
/// emulated engines; only the transport differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmuVmSpec {
    pub instance_id: String,
    pub generation: u64,
    /// 0 means "the profile default"; QEMU always gets an explicit `-smp`.
    pub vcpus: u16,
    /// 0 means "the profile default"; QEMU always gets an explicit `-m`.
    pub memory_mib: u64,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub kernel_params: Vec<String>,
    pub disks: Vec<EmuDisk>,
    pub display: EmuDisplay,
    pub network: EmuNetwork,
    pub audio: bool,
    pub presentation: PresentationMode,
    pub profile: EmuProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmuPlanError {
    /// No performance core is available, so every vCPU thread would be on an in-order core.
    NoPerformanceCores,
    InsufficientMemory {
        required_mib: u64,
        available_mib: u64,
    },
    GuestMemoryExceedsEmulatedMaximum {
        requested_mib: u64,
        maximum_mib: u64,
    },
    /// `w^x` support was never probed; TCG cannot pick a translation-cache mode blindly.
    UnsupportedWxMode,
    /// No Vulkan device in the emulator process: hardware graphics are impossible.
    NoVulkan,
    /// Zero-copy presentation needs exportable `AHardwareBuffer`s.
    NoHardwareBufferExport,
    InvalidSessionDescriptor,
    DiskDescriptorCountMismatch {
        disks: usize,
        descriptors: usize,
    },
    /// A path the broker did not validate, or one that QEMU would read as an option value list.
    UnsafePath {
        field: &'static str,
    },
    NonUtf8Path {
        field: &'static str,
    },
    UnexpectedQemuProgram,
    InvalidInstanceId,
    InvalidKernelParameter {
        index: usize,
    },
    InvalidDisplay,
}

impl EmuPlanError {
    /// One sentence the setup screen or `carbonctl` can show verbatim.
    pub fn message(&self) -> String {
        match self {
            EmuPlanError::NoPerformanceCores => {
                "This phone exposes no performance core to run emulated vCPU threads.".into()
            }
            EmuPlanError::InsufficientMemory {
                required_mib,
                available_mib,
            } => format!(
                "An emulated guest needs {required_mib} MiB free; this phone reports {available_mib} MiB. Close other apps or pick the low-memory profile."
            ),
            EmuPlanError::GuestMemoryExceedsEmulatedMaximum {
                requested_mib,
                maximum_mib,
            } => format!(
                "The emulated engine plans at most {maximum_mib} MiB of guest RAM; {requested_mib} MiB was requested."
            ),
            EmuPlanError::UnsupportedWxMode => {
                "Writable-then-executable memory for the JIT has not been probed on this phone.".into()
            }
            EmuPlanError::NoVulkan => {
                "No Vulkan device is reachable from the emulator process, so graphics cannot be hardware-accelerated. The emulated engine refuses to fall back to a software renderer.".into()
            }
            EmuPlanError::NoHardwareBufferExport => {
                "This GPU cannot export Vulkan images as AHardwareBuffer, so zero-copy presentation is unavailable. Use the readback profile.".into()
            }
            EmuPlanError::InvalidSessionDescriptor => {
                "A session descriptor is invalid: descriptors must be distinct and at least 3.".into()
            }
            EmuPlanError::DiskDescriptorCountMismatch {
                disks,
                descriptors,
            } => format!("{descriptors} disk descriptors were supplied for {disks} disks."),
            EmuPlanError::UnsafePath { field } => {
                format!("The {field} path is not a plain absolute path.")
            }
            EmuPlanError::NonUtf8Path { field } => {
                format!("The {field} path is not valid UTF-8.")
            }
            EmuPlanError::UnexpectedQemuProgram => {
                "The emulator executable must be qemu-system-aarch64 or the shipped libqemu_system_aarch64.so.".into()
            }
            EmuPlanError::InvalidInstanceId => {
                "The instance id contains characters a QEMU name may not carry.".into()
            }
            EmuPlanError::InvalidKernelParameter { index } => format!(
                "Kernel parameter {index} contains whitespace, a comma, a leading '-' or a control character."
            ),
            EmuPlanError::InvalidDisplay => "The display geometry is out of range.".into(),
        }
    }
}

/// A warning is never a silent default: the service and diagnostics show every one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmuWarning {
    ReadbackPresentation,
    MteWithZeroCopyUnverified,
    NoSparePerformanceCore,
    AffinityNotPermitted,
    UclampNotPermitted,
    AnonymousRwxTranslationCache,
    ODirectUnavailable,
    SoftwareFallbackImpossible,
}

impl EmuWarning {
    pub fn code(self) -> &'static str {
        match self {
            EmuWarning::ReadbackPresentation => "presentation.readback",
            EmuWarning::MteWithZeroCopyUnverified => "security.mte-zero-copy-unverified",
            EmuWarning::NoSparePerformanceCore => "cpu.no-spare-core",
            EmuWarning::AffinityNotPermitted => "cpu.affinity-blocked",
            EmuWarning::UclampNotPermitted => "cpu.uclamp-blocked",
            EmuWarning::AnonymousRwxTranslationCache => "jit.anonymous-rwx",
            EmuWarning::ODirectUnavailable => "io.o-direct-unavailable",
            EmuWarning::SoftwareFallbackImpossible => "gpu.no-software-fallback",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            EmuWarning::ReadbackPresentation => {
                "Presentation uses a CPU readback: expect extra frame latency and memory bandwidth."
            }
            EmuWarning::MteWithZeroCopyUnverified => {
                "MTE plus zero-copy gfxstream blobs is unproven on this engine; Phase 6 must confirm it before this profile is qualified."
            }
            EmuWarning::NoSparePerformanceCore => {
                "No performance core is left for QEMU I/O, gfxstream and the UI; expect contention."
            }
            EmuWarning::AffinityNotPermitted => {
                "The app may not set thread affinity here; vCPU threads float across little and big cores."
            }
            EmuWarning::UclampNotPermitted => {
                "uclamp boosting is unavailable; vCPU threads may be scheduled on little cores."
            }
            EmuWarning::AnonymousRwxTranslationCache => {
                "TCG's translation cache uses writable-executable pages instead of a split mapping."
            }
            EmuWarning::ODirectUnavailable => {
                "O_DIRECT is unavailable in app storage; disks use the page cache (cache=writeback)."
            }
            EmuWarning::SoftwareFallbackImpossible => {
                "If gfxstream cannot initialise on this GPU the session fails; there is no software GPU fallback."
            }
        }
    }
}

/// Plan one boot. The returned invocation is complete and reviewable.
pub fn plan(
    qemu: PathBuf,
    spec: &EmuVmSpec,
    host: &EmuHostFacts,
    fds: &EmuSessionFds,
) -> Result<EmuPlan, EmuPlanError> {
    build_plan(qemu, spec, host, fds)
}

/// Convenience: the program invocation only, for callers that do not need diagnostics.
pub fn invocation(
    qemu: PathBuf,
    spec: &EmuVmSpec,
    host: &EmuHostFacts,
    fds: &EmuSessionFds,
) -> Result<ProgramInvocation, EmuPlanError> {
    Ok(build_plan(qemu, spec, host, fds)?.invocation)
}
