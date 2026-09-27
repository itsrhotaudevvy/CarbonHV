use carbon_core::{CapabilityEvidence, GpuHostCapabilities, GpuIdentity, GraphicsCapability};

fn unknown_host(identity: GpuIdentity) -> GpuHostCapabilities {
    GpuHostCapabilities {
        identity,
        vulkan: CapabilityEvidence::Unknown,
        gles: CapabilityEvidence::Unknown,
        native_buffer: CapabilityEvidence::Unknown,
        explicit_sync: CapabilityEvidence::Unknown,
        zero_copy: CapabilityEvidence::Unknown,
        sustained: CapabilityEvidence::Unknown,
    }
}

#[test]
fn unknown_probe_values_do_not_grant_capabilities() {
    let host = unknown_host(GpuIdentity {
        vendor: "ARM".into(),
        renderer: "Mali-G710".into(),
        driver_version: None,
    });

    assert!(!host.supports(GraphicsCapability::Vulkan));
    assert!(!host.supports(GraphicsCapability::NativeBuffer));
    assert!(!host.supports(GraphicsCapability::ExplicitSync));
    assert!(!host.supports(GraphicsCapability::Qualified));
}

#[test]
fn mali_identity_is_host_only_metadata() {
    let mut mali = unknown_host(GpuIdentity {
        vendor: "ARM".into(),
        renderer: "Mali-G710".into(),
        driver_version: Some("226504704".into()),
    });
    mali.vulkan = CapabilityEvidence::Observed(true);

    let mut adreno = mali.clone();
    adreno.identity = GpuIdentity {
        vendor: "Qualcomm".into(),
        renderer: "Adreno".into(),
        driver_version: Some("future".into()),
    };

    assert_eq!(
        mali.available_capabilities(),
        adreno.available_capabilities(),
        "host vendor/model metadata must not change the guest-visible capability contract",
    );
}

#[test]
fn capability_evidence_distinguishes_observed_from_qualified() {
    let mut host = unknown_host(GpuIdentity {
        vendor: "ARM".into(),
        renderer: "Mali-G710".into(),
        driver_version: None,
    });
    host.vulkan = CapabilityEvidence::Observed(true);
    host.native_buffer = CapabilityEvidence::Qualified(true);

    assert!(host.supports(GraphicsCapability::Vulkan));
    assert!(!host.is_qualified(GraphicsCapability::Vulkan));
    assert!(host.supports(GraphicsCapability::NativeBuffer));
    assert!(host.is_qualified(GraphicsCapability::NativeBuffer));
}
