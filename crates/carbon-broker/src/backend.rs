use std::path::PathBuf;

use crate::PlacementError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramInvocation {
    pub program: PathBuf,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostVirtualizationClass {
    Unavailable,
    DevelopmentReady,
    /// A Gunyah hypervisor device exists but AVF has no KVM to run on. This is a distinct,
    /// reportable state, never a ready one: no Gunyah VM backend is qualified.
    GunyahWithoutBackend,
    /// A Gunyah host whose controlled build declares non-protected AVF VM support. crosvm falls
    /// back from the absent `/dev/kvm` to `/dev/gunyah`, so the AVF display/gfxstream/input path
    /// is reused unchanged. Still EXPERIMENTAL until a guest boots and renders on it.
    GunyahAvfReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypervisorKind {
    None,
    Kvm,
    Gunyah,
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

    /// When the build names its hypervisor, it must say Gunyah; an empty version is accepted
    /// because crosvm's device probe, not the property, selects the hypervisor.
    pub fn declares_gunyah(&self) -> bool {
        self.version.as_deref().is_none_or(|version| {
            version.is_empty() || version.to_ascii_lowercase().contains("gunyah")
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProbe {
    pub vm_tool_present: bool,
    pub kvm_present: bool,
    pub gunyah_present: bool,
    pub hypervisor: HypervisorKind,
    pub class: HostVirtualizationClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    UnsafeRuntimeDirectory,
    InvalidGraphicsConfig,
    UnsupportedCliVcpuCount { requested: u16 },
    InvalidSessionDescriptor,
    NonUtf8Path,
    Placement(PlacementError),
}
