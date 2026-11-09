//! Engine selection never turns a hardware-VM instance into an emulated one.
//!
//! This is ADR-017's admission rule, tested as policy: emulation is an explicit, stored choice.

use carbon_broker::{
    EngineSelectionError, HostProbe, HostVirtualizationClass, HypervisorKind,
    select_execution_engine,
};
use carbon_core::{EmulationAccelerator, EmulationReadiness, ExecutionEngine, HardwareVmBackend};

fn probe(class: HostVirtualizationClass) -> HostProbe {
    HostProbe {
        vm_tool_present: class != HostVirtualizationClass::Unavailable,
        kvm_present: class == HostVirtualizationClass::DevelopmentReady,
        hypervisor: if class == HostVirtualizationClass::DevelopmentReady {
            HypervisorKind::Kvm
        } else {
            HypervisorKind::Gunyah
        },
        class,
    }
}

#[test]
fn a_stock_snapdragon_host_refuses_an_instance_without_a_stored_engine() {
    // The kunzite case: /dev/gunyah exists, AVF VM support is unset, emulation would be possible.
    let error = select_execution_engine(
        None,
        &probe(HostVirtualizationClass::VendorHypervisorWithoutAvf),
        EmulationReadiness::Ready,
    )
    .unwrap_err();
    assert_eq!(error, EngineSelectionError::RequiresExplicitEngineChoice);
    assert!(error.message().contains("Emulated"));
}

#[test]
fn an_explicit_emulated_instance_is_honoured_when_readiness_allows() {
    let stored = ExecutionEngine::Emulated(EmulationAccelerator::Tcg);
    assert_eq!(
        select_execution_engine(
            Some(stored),
            &probe(HostVirtualizationClass::VendorHypervisorWithoutAvf),
            EmulationReadiness::Ready,
        )
        .unwrap(),
        stored
    );
    assert_eq!(
        select_execution_engine(
            Some(stored),
            &probe(HostVirtualizationClass::VendorHypervisorWithoutAvf),
            EmulationReadiness::InsufficientMemory {
                required_mib: 2816,
                available_mib: 2400
            },
        )
        .unwrap_err(),
        EngineSelectionError::EmulationNotReady(EmulationReadiness::InsufficientMemory {
            required_mib: 2816,
            available_mib: 2400
        })
    );
}

#[test]
fn a_hardware_ready_host_uses_avf_and_keeps_a_stored_engine() {
    assert_eq!(
        select_execution_engine(
            None,
            &probe(HostVirtualizationClass::DevelopmentReady),
            EmulationReadiness::Ready
        )
        .unwrap(),
        ExecutionEngine::HardwareVm(HardwareVmBackend::Avf)
    );
    // Emulation is never selected just because it is possible.
    assert!(
        !select_execution_engine(
            None,
            &probe(HostVirtualizationClass::DevelopmentReady),
            EmulationReadiness::Ready
        )
        .unwrap()
        .is_emulated()
    );
    assert_eq!(
        select_execution_engine(
            Some(ExecutionEngine::HardwareVm(HardwareVmBackend::DirectCrosvm)),
            &probe(HostVirtualizationClass::VendorHypervisorAvfReady),
            EmulationReadiness::NoVulkan,
        )
        .unwrap(),
        ExecutionEngine::HardwareVm(HardwareVmBackend::DirectCrosvm)
    );
}

#[test]
fn engine_metadata_round_trips_and_labels_are_honest() {
    for engine in [
        ExecutionEngine::HardwareVm(HardwareVmBackend::Avf),
        ExecutionEngine::HardwareVm(HardwareVmBackend::DirectCrosvm),
        ExecutionEngine::Emulated(EmulationAccelerator::Tcg),
    ] {
        assert_eq!(ExecutionEngine::parse(&engine.as_str()), Some(engine));
        assert_eq!(engine.is_emulated(), engine.label() == "Emulated");
        assert_eq!(engine.cpu_native(), !engine.is_emulated());
    }
    assert_eq!(
        ExecutionEngine::parse("emulated:kvm"),
        None,
        "emulation must not borrow a hypervisor's name"
    );
}

#[test]
fn admission_accepts_emulation_only_from_the_stored_engine() {
    use carbon_broker::BackendError;
    // The broker's emulated adapter compiles the engine into its own label, so nothing can
    // produce an "emulated" plan that is presented as a hardware VM.
    let plan = carbon_broker::EmulatedBackend::engine();
    assert!(plan.is_emulated());
    let _unused: fn(carbon_emu::EmuPlanError) -> BackendError = BackendError::Emulation;
}
