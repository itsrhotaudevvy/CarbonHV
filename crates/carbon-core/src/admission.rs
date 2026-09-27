use crate::{BundleRequirements, CpuArch, HostCapabilities};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionFailure {
    ArchitectureMismatch { host: CpuArch, guest: CpuArch },
    HardwareVirtualizationUnavailable,
    GpuAccelerationUnavailable,
    BrokerUnauthorized,
    InsufficientMemory { required_mib: u64, available_mib: u64 },
    InsufficientStorage { required_mib: u64, available_mib: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionDecision {
    Ready,
    Rejected(Vec<AdmissionFailure>),
}

pub fn admit(host: &HostCapabilities, bundle: &BundleRequirements) -> AdmissionDecision {
    let mut failures = Vec::new();

    if host.arch != bundle.arch {
        failures.push(AdmissionFailure::ArchitectureMismatch {
            host: host.arch,
            guest: bundle.arch,
        });
    }
    if bundle.requires_hardware_virtualization && !host.hardware_virtualization {
        failures.push(AdmissionFailure::HardwareVirtualizationUnavailable);
    }
    if bundle.requires_gpu_acceleration && !host.gpu_acceleration {
        failures.push(AdmissionFailure::GpuAccelerationUnavailable);
    }
    if !host.broker_authorized {
        failures.push(AdmissionFailure::BrokerUnauthorized);
    }
    if host.available_memory_mib < bundle.memory_mib {
        failures.push(AdmissionFailure::InsufficientMemory {
            required_mib: bundle.memory_mib,
            available_mib: host.available_memory_mib,
        });
    }
    if host.available_storage_mib < bundle.storage_mib {
        failures.push(AdmissionFailure::InsufficientStorage {
            required_mib: bundle.storage_mib,
            available_mib: host.available_storage_mib,
        });
    }

    if failures.is_empty() {
        AdmissionDecision::Ready
    } else {
        AdmissionDecision::Rejected(failures)
    }
}

