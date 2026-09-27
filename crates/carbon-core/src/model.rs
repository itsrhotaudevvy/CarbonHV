#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArch {
    Arm64,
    X86_64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCapabilities {
    pub arch: CpuArch,
    pub hardware_virtualization: bool,
    pub gpu_acceleration: bool,
    pub broker_authorized: bool,
    pub available_memory_mib: u64,
    pub available_storage_mib: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleRequirements {
    pub arch: CpuArch,
    pub requires_hardware_virtualization: bool,
    pub requires_gpu_acceleration: bool,
    pub memory_mib: u64,
    pub storage_mib: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmState {
    Stopped,
    Starting,
    Running,
    Paused,
    Stopping,
    Crashed,
    Recovering,
}

