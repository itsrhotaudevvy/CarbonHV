use std::path::PathBuf;

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

/// A program plus its exact argument vector. The broker is the only component that builds one,
/// so a caller can never interpolate guest-controlled text into a launch command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramInvocation {
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// The two ways a guest can be driven by a hypervisor crosvm talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HardwareVmBackend {
    /// AVF's `virtualizationservice`, the platform-preferred path (ADR-004).
    Avf,
    /// A CarbonHV-owned crosvm launch, needed for per-vCPU pinning on vendor hypervisors.
    DirectCrosvm,
}

/// What accelerates a guest's CPU instructions.
///
/// Emulation is never a fallback and never "near-native": it is a separately admitted,
/// explicitly requested engine (ADR-017). Every surface that displays it must use
/// [`ExecutionEngine::label`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionEngine {
    HardwareVm(HardwareVmBackend),
    Emulated(EmulationAccelerator),
}

/// The only CPU acceleration a stock, unrooted phone can offer an app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmulationAccelerator {
    /// QEMU dynamic binary translation (`-accel tcg`), multi-threaded.
    Tcg,
}

impl HardwareVmBackend {
    /// Stable metadata token. Existing instances default to `avf`.
    pub fn as_str(self) -> &'static str {
        match self {
            HardwareVmBackend::Avf => "avf",
            HardwareVmBackend::DirectCrosvm => "direct-crosvm",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "avf" => Some(HardwareVmBackend::Avf),
            "direct-crosvm" => Some(HardwareVmBackend::DirectCrosvm),
            _ => None,
        }
    }
}

impl EmulationAccelerator {
    pub fn as_str(self) -> &'static str {
        match self {
            EmulationAccelerator::Tcg => "tcg",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "tcg" => Some(EmulationAccelerator::Tcg),
            _ => None,
        }
    }
}

impl ExecutionEngine {
    /// `hardware-vm:<backend>` or `emulated:<accelerator>`; used in instance metadata.
    pub fn as_str(self) -> String {
        match self {
            ExecutionEngine::HardwareVm(backend) => {
                format!("hardware-vm:{}", backend.as_str())
            }
            ExecutionEngine::Emulated(accelerator) => {
                format!("emulated:{}", accelerator.as_str())
            }
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let (kind, detail) = value.split_once(':')?;
        match kind {
            "hardware-vm" => HardwareVmBackend::parse(detail).map(ExecutionEngine::HardwareVm),
            "emulated" => EmulationAccelerator::parse(detail).map(ExecutionEngine::Emulated),
            _ => None,
        }
    }

    pub fn is_emulated(self) -> bool {
        matches!(self, ExecutionEngine::Emulated(_))
    }

    /// The only user-facing name this engine may be shown under.
    pub fn label(self) -> &'static str {
        match self {
            ExecutionEngine::HardwareVm(_) => "Hardware VM",
            ExecutionEngine::Emulated(_) => "Emulated",
        }
    }

    /// `cpu.native` capability: true only when guest instructions run on the host CPU as-is.
    pub fn cpu_native(self) -> bool {
        matches!(self, ExecutionEngine::HardwareVm(_))
    }
}

/// Whether *this* install can run the emulated engine. Independent of `HostVirtualizationClass`:
/// it depends on app-side facts (memory, GPU, ABI), not on the hypervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmulationReadiness {
    Ready,
    InsufficientMemory {
        required_mib: u64,
        available_mib: u64,
    },
    /// No usable Vulkan device in the emulator process, so no hardware graphics (ADR-016).
    NoVulkan,
    /// The host ABI is not arm64, so a CVD1 guest cannot be emulated usefully.
    UnsupportedAbi,
}

impl EmulationReadiness {
    pub fn is_ready(self) -> bool {
        matches!(self, EmulationReadiness::Ready)
    }

    /// The reason, phrased for a setup screen. Emulation is only ever offered with this text.
    pub fn explanation(self) -> &'static str {
        match self {
            EmulationReadiness::Ready => {
                "Emulated mode runs the guest CPU in software (QEMU TCG) with GPU-accelerated graphics."
            }
            EmulationReadiness::InsufficientMemory { .. } => {
                "Not enough free memory to run an emulated guest on this phone right now."
            }
            EmulationReadiness::NoVulkan => {
                "This device exposes no Vulkan device to apps, so it cannot render an emulated guest on the GPU."
            }
            EmulationReadiness::UnsupportedAbi => {
                "This device is not arm64, so an ARM64 guest cannot be emulated usefully."
            }
        }
    }
}
