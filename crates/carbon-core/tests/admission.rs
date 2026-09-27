use carbon_core::{
    AdmissionDecision, AdmissionFailure, BundleRequirements, CpuArch, HostCapabilities,
    admit,
};

fn ready_host() -> HostCapabilities {
    HostCapabilities {
        arch: CpuArch::Arm64,
        hardware_virtualization: true,
        gpu_acceleration: true,
        broker_authorized: true,
        available_memory_mib: 8192,
        available_storage_mib: 64 * 1024,
    }
}

fn starter_bundle() -> BundleRequirements {
    BundleRequirements {
        arch: CpuArch::Arm64,
        requires_hardware_virtualization: true,
        requires_gpu_acceleration: true,
        memory_mib: 4096,
        storage_mib: 16 * 1024,
    }
}

#[test]
fn admits_supported_accelerated_arm64_host() {
    assert_eq!(admit(&ready_host(), &starter_bundle()), AdmissionDecision::Ready);
}

#[test]
fn rejects_missing_hardware_virtualization_without_fallback() {
    let mut host = ready_host();
    host.hardware_virtualization = false;

    assert_eq!(
        admit(&host, &starter_bundle()),
        AdmissionDecision::Rejected(vec![AdmissionFailure::HardwareVirtualizationUnavailable])
    );
}

#[test]
fn rejects_missing_gpu_when_bundle_requires_acceleration() {
    let mut host = ready_host();
    host.gpu_acceleration = false;

    assert_eq!(
        admit(&host, &starter_bundle()),
        AdmissionDecision::Rejected(vec![AdmissionFailure::GpuAccelerationUnavailable])
    );
}

#[test]
fn rejects_unauthorized_broker_and_reports_resource_shortfall() {
    let mut host = ready_host();
    host.broker_authorized = false;
    host.available_memory_mib = 3072;
    host.available_storage_mib = 8 * 1024;

    assert_eq!(
        admit(&host, &starter_bundle()),
        AdmissionDecision::Rejected(vec![
            AdmissionFailure::BrokerUnauthorized,
            AdmissionFailure::InsufficientMemory {
                required_mib: 4096,
                available_mib: 3072,
            },
            AdmissionFailure::InsufficientStorage {
                required_mib: 16 * 1024,
                available_mib: 8 * 1024,
            },
        ])
    );
}

#[test]
fn rejects_cross_isa_bundle() {
    let mut bundle = starter_bundle();
    bundle.arch = CpuArch::X86_64;

    assert_eq!(
        admit(&ready_host(), &bundle),
        AdmissionDecision::Rejected(vec![AdmissionFailure::ArchitectureMismatch {
            host: CpuArch::Arm64,
            guest: CpuArch::X86_64,
        }])
    );
}
