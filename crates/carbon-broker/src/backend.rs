use carbon_core::{EmulationReadiness, ExecutionEngine, HardwareVmBackend};

use crate::PlacementError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostVirtualizationClass {
    Unavailable,
    DevelopmentReady,
    /// A vendor hypervisor (Gunyah, GenieZone, Halla) exists but the build does not declare AVF
    /// VM support, as on stock Snapdragon firmware. Reportable, never ready.
    VendorHypervisorWithoutAvf,
    /// A vendor-hypervisor host whose (controlled or vendor) build declares non-protected AVF VM
    /// support. crosvm falls back from the absent `/dev/kvm` to the vendor device, so the AVF
    /// display/gfxstream/input path is reused unchanged. Still EXPERIMENTAL until a guest boots
    /// and renders on it.
    VendorHypervisorAvfReady,
}

/// The aarch64 hypervisor backends crosvm can drive, in crosvm's own default-probe order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HypervisorKind {
    /// Upstream KVM / pKVM (Google Tensor, most AVF reference devices).
    Kvm,
    /// MediaTek GenieZone (`/dev/gzvm`), an AVF backend on Dimensity SoCs.
    GenieZone,
    /// Samsung Exynos Halla (`/dev/halla`).
    Halla,
    /// Qualcomm Gunyah (`/dev/gunyah`).
    Gunyah,
    None,
}

impl HypervisorKind {
    /// Vendor hypervisors in the order crosvm's `get_default_hypervisor` tries them after KVM.
    pub const VENDOR_PROBE_ORDER: [HypervisorKind; 3] = [
        HypervisorKind::GenieZone,
        HypervisorKind::Halla,
        HypervisorKind::Gunyah,
    ];

    /// The `--hypervisor` key crosvm accepts.
    pub fn crosvm_name(self) -> Option<&'static str> {
        match self {
            HypervisorKind::Kvm => Some("kvm"),
            HypervisorKind::GenieZone => Some("geniezone"),
            HypervisorKind::Halla => Some("halla"),
            HypervisorKind::Gunyah => Some("gunyah"),
            HypervisorKind::None => None,
        }
    }

    pub fn default_device(self) -> Option<&'static str> {
        match self {
            HypervisorKind::Kvm => Some("/dev/kvm"),
            HypervisorKind::GenieZone => Some("/dev/gzvm"),
            HypervisorKind::Halla => Some("/dev/halla"),
            HypervisorKind::Gunyah => Some("/dev/gunyah"),
            HypervisorKind::None => None,
        }
    }

    /// Whether AOSP's `com.android.virt` crosvm is built with this backend (external/crosvm
    /// `Android.bp` features: `geniezone`, `gunyah`; KVM is always present). A Halla host needs a
    /// vendor-built APEX or crosvm.
    pub fn in_aosp_apex_crosvm(self) -> bool {
        matches!(
            self,
            HypervisorKind::Kvm | HypervisorKind::GenieZone | HypervisorKind::Gunyah
        )
    }

    fn version_tokens(self) -> &'static [&'static str] {
        match self {
            HypervisorKind::Kvm => &["kvm"],
            HypervisorKind::GenieZone => &["geniezone", "gzvm"],
            HypervisorKind::Halla => &["halla"],
            HypervisorKind::Gunyah => &["gunyah"],
            HypervisorKind::None => &[],
        }
    }
}

/// `ro.boot.hypervisor.*` values read by the Android daemon; `None` means unreadable/absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HypervisorProperties {
    pub vm_supported: Option<bool>,
    pub protected_vm_supported: Option<bool>,
    pub version: Option<String>,
}

impl HypervisorProperties {
    /// Android property booleans: `1`/`true` and `0`/`false`; anything else is unknown.
    pub fn parse_bool(value: &str) -> Option<bool> {
        match value.trim() {
            "1" | "true" => Some(true),
            "0" | "false" => Some(false),
            _ => None,
        }
    }

    /// The version property is advisory (crosvm's device probe selects the hypervisor), so an
    /// absent or unfamiliar value is accepted; a value naming a *different* known hypervisor is
    /// an inconsistent build and is not.
    pub fn consistent_with(&self, kind: HypervisorKind) -> bool {
        let Some(version) = self.version.as_deref() else {
            return true;
        };
        let version = version.to_ascii_lowercase();
        let names = |other: HypervisorKind| {
            other
                .version_tokens()
                .iter()
                .any(|token| version.contains(token))
        };
        names(kind)
            || ![
                HypervisorKind::Kvm,
                HypervisorKind::GenieZone,
                HypervisorKind::Halla,
                HypervisorKind::Gunyah,
            ]
            .into_iter()
            .filter(|other| *other != kind)
            .any(names)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProbe {
    pub vm_tool_present: bool,
    pub kvm_present: bool,
    /// The hypervisor crosvm would select by default on this host.
    pub hypervisor: HypervisorKind,
    pub class: HostVirtualizationClass,
}

/// Why an instance cannot be started with the engine it asked for.
///
/// The rule that matters: an instance with **no stored engine** never falls back to emulation.
/// The user has to choose "Run emulated (slow)" once, and that choice is stored per instance
/// (ADR-017). This keeps a hardware-VM instance from silently becoming an emulated one when a
/// phone's firmware changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineSelectionError {
    /// No hardware VM is ready on this host and the instance has no stored engine.
    RequiresExplicitEngineChoice,
    /// The instance is emulated, but this install cannot run the emulated engine right now.
    EmulationNotReady(EmulationReadiness),
}

impl EngineSelectionError {
    pub fn message(self) -> String {
        match self {
            EngineSelectionError::RequiresExplicitEngineChoice => {
                "This phone cannot run a hardware VM. Create the instance as Emulated (slow) or pick a host with an app-reachable hypervisor.".into()
            }
            EngineSelectionError::EmulationNotReady(readiness) => readiness.explanation().into(),
        }
    }
}

/// Chooses the engine for one start.
pub fn select_execution_engine(
    stored: Option<ExecutionEngine>,
    probe: &HostProbe,
    emulation: EmulationReadiness,
) -> Result<ExecutionEngine, EngineSelectionError> {
    match stored {
        Some(engine) if engine.is_emulated() => {
            if emulation.is_ready() {
                Ok(engine)
            } else {
                Err(EngineSelectionError::EmulationNotReady(emulation))
            }
        }
        // A hardware-VM instance keeps its backend even when emulation would now be possible.
        Some(engine) => Ok(engine),
        None => match probe.class {
            // AVF is the platform-preferred baseline (ADR-004).
            HostVirtualizationClass::DevelopmentReady
            | HostVirtualizationClass::VendorHypervisorAvfReady => {
                Ok(ExecutionEngine::HardwareVm(HardwareVmBackend::Avf))
            }
            HostVirtualizationClass::Unavailable
            | HostVirtualizationClass::VendorHypervisorWithoutAvf => {
                Err(EngineSelectionError::RequiresExplicitEngineChoice)
            }
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    UnsafeRuntimeDirectory,
    InvalidGraphicsConfig,
    UnsupportedCliVcpuCount {
        requested: u16,
    },
    NoHypervisor,
    InvalidSessionDescriptor,
    NonUtf8Path,
    Placement(PlacementError),
    /// The emulated engine's own planning refused the request; the message is user-facing.
    Emulation(carbon_emu::EmuPlanError),
    EngineSelection(EngineSelectionError),
    DiskDescriptorCountMismatch {
        disks: usize,
        descriptors: usize,
    },
}
