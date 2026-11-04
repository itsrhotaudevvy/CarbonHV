use std::path::PathBuf;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProbe {
    pub vm_tool_present: bool,
    pub kvm_present: bool,
    pub gunyah_present: bool,
    pub class: HostVirtualizationClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    UnsafeRuntimeDirectory,
    InvalidGraphicsConfig,
    UnsupportedCliVcpuCount { requested: u16 },
}
