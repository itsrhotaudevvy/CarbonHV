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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProbe {
    pub vm_tool_present: bool,
    pub kvm_present: bool,
    pub class: HostVirtualizationClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    UnsafeRuntimeDirectory,
}
