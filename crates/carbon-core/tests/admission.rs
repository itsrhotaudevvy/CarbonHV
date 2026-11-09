use carbon_core::{
    AdmissionDecision, AdmissionFailure, BundleRequirements, CpuArch, EmulationAccelerator,
    EmulationReadiness, ExecutionEngine, HardwareVmBackend, HostCapabilities, admit,
    admit_with_engine,
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
    assert_eq!(
        admit(&ready_host(), &starter_bundle()),
        AdmissionDecision::Ready
    );
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

fn emulated_engine() -> ExecutionEngine {
    ExecutionEngine::Emulated(EmulationAccelerator::Tcg)
}

fn host_without_hardware_virtualization() -> HostCapabilities {
    let mut host = ready_host();
    host.hardware_virtualization = false;
    host
}

#[test]
fn a_stored_emulated_engine_admits_a_host_without_virtualization() {
    assert_eq!(
        admit_with_engine(
            &host_without_hardware_virtualization(),
            &starter_bundle(),
            Some(emulated_engine()),
            EmulationReadiness::Ready,
        ),
        AdmissionDecision::Ready
    );
}

#[test]
fn a_stored_emulated_engine_still_fails_closed_when_emulation_is_not_ready() {
    let readiness = EmulationReadiness::InsufficientMemory {
        required_mib: 2816,
        available_mib: 2750,
    };

    assert_eq!(
        admit_with_engine(
            &host_without_hardware_virtualization(),
            &starter_bundle(),
            Some(emulated_engine()),
            readiness,
        ),
        AdmissionDecision::Rejected(vec![AdmissionFailure::EmulationNotReady(readiness)])
    );
}

#[test]
fn an_instance_without_a_stored_engine_never_starts_emulated() {
    assert_eq!(
        admit_with_engine(
            &host_without_hardware_virtualization(),
            &starter_bundle(),
            None,
            EmulationReadiness::Ready,
        ),
        AdmissionDecision::Rejected(vec![AdmissionFailure::HardwareVirtualizationUnavailable])
    );
}

#[test]
fn a_hardware_instance_never_falls_back_to_emulation() {
    let engine = ExecutionEngine::HardwareVm(HardwareVmBackend::Avf);

    assert_eq!(
        admit_with_engine(
            &host_without_hardware_virtualization(),
            &starter_bundle(),
            Some(engine),
            EmulationReadiness::Ready,
        ),
        AdmissionDecision::Rejected(vec![AdmissionFailure::HardwareVirtualizationUnavailable])
    );
}

#[test]
fn engine_metadata_round_trips_and_never_labels_emulation_native() {
    for engine in [
        ExecutionEngine::HardwareVm(HardwareVmBackend::Avf),
        ExecutionEngine::HardwareVm(HardwareVmBackend::DirectCrosvm),
        emulated_engine(),
    ] {
        assert_eq!(ExecutionEngine::parse(&engine.as_str()), Some(engine));
    }

    assert_eq!(emulated_engine().label(), "Emulated");
    assert!(!emulated_engine().cpu_native());
    assert!(emulated_engine().is_emulated());
    assert!(!ExecutionEngine::HardwareVm(HardwareVmBackend::Avf).is_emulated());
    assert_eq!(ExecutionEngine::parse("emulated:kvm"), None);
    assert_eq!(ExecutionEngine::parse("emulated"), None);
}
