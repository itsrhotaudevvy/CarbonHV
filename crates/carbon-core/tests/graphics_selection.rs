use carbon_core::{
    CapabilityEvidence, GpuHostCapabilities, GpuIdentity, GpuQualificationTuple,
    GpuRendererProfile, GraphicsCapability, QualificationRecord, RendererQualification,
    RendererSelection, RendererSelectionMode, select_renderer,
};

fn mali_host(qualified: bool) -> GpuHostCapabilities {
    let evidence = |value| {
        if qualified {
            CapabilityEvidence::Qualified(value)
        } else {
            CapabilityEvidence::Observed(value)
        }
    };
    GpuHostCapabilities {
        identity: GpuIdentity {
            vendor: "ARM".into(),
            renderer: "Mali-G710".into(),
            driver_version: Some("226504704".into()),
        },
        vulkan: evidence(true),
        gles: evidence(true),
        native_buffer: evidence(true),
        explicit_sync: evidence(true),
        zero_copy: CapabilityEvidence::Unknown,
        sustained: CapabilityEvidence::Unknown,
    }
}

fn tuple() -> GpuQualificationTuple {
    GpuQualificationTuple {
        carbonhv_commit: "abc123".into(),
        device_model: "Pixel 7a".into(),
        soc: "GS201".into(),
        gpu: "Mali-G710".into(),
        host_build_fingerprint: "google/lynx/test".into(),
        kernel_version: "6.1-test".into(),
        avf_revision: "avf-17-r1".into(),
        renderer_id: "gfxstream-vulkan-mali".into(),
        renderer_version: "renderer-a".into(),
        graphics_driver_version: "226504704".into(),
        android_guest_bundle_version: "android-a".into(),
        ubuntu_touch_guest_bundle_version: "ubuntu-touch-a".into(),
        cvd_major: 1,
        display_mode: "1080x2400@90".into(),
        workload_version: "gpu-workload-v1".into(),
    }
}

fn record() -> QualificationRecord {
    QualificationRecord {
        tuple: tuple(),
        capabilities: vec![
            GraphicsCapability::Vulkan,
            GraphicsCapability::NativeBuffer,
            GraphicsCapability::ExplicitSync,
            GraphicsCapability::Qualified,
        ],
    }
}

#[test]
fn release_selects_qualified_mali_profile() {
    let profile = GpuRendererProfile::gfxstream_vulkan_mali();
    let record = record();
    let current = tuple();
    let selected = select_renderer(
        &mali_host(true),
        &[profile.clone()],
        RendererSelectionMode::Release,
        Some(RendererQualification {
            record: &record,
            current: &current,
        }),
    );

    assert_eq!(profile.id, "gfxstream-vulkan-mali");
    assert_eq!(selected, RendererSelection::Selected(profile));
}

#[test]
fn release_does_not_select_from_capability_evidence_alone() {
    let profile = GpuRendererProfile::gfxstream_vulkan_mali();

    assert_eq!(
        select_renderer(
            &mali_host(true),
            &[profile],
            RendererSelectionMode::Release,
            None,
        ),
        RendererSelection::Unavailable,
        "release selection needs the exact qualification tuple, not only qualified-looking host facts",
    );
}

#[test]
fn missing_required_capability_fails_closed() {
    let mut host = mali_host(true);
    host.native_buffer = CapabilityEvidence::Unknown;
    let profile = GpuRendererProfile::gfxstream_vulkan_mali();
    let record = record();
    let current = tuple();
    let qualification = Some(RendererQualification {
        record: &record,
        current: &current,
    });

    assert_eq!(
        select_renderer(
            &host,
            &[profile.clone()],
            RendererSelectionMode::DeveloperExperimental,
            qualification,
        ),
        RendererSelection::Unavailable,
    );
    assert_eq!(
        select_renderer(
            &host,
            &[profile],
            RendererSelectionMode::Release,
            qualification
        ),
        RendererSelection::Unavailable,
    );
}

#[test]
fn developer_mode_explicitly_allows_compatible_unqualified_profile() {
    let profile = GpuRendererProfile::gfxstream_vulkan_mali();

    assert_eq!(
        select_renderer(
            &mali_host(false),
            &[profile.clone()],
            RendererSelectionMode::DeveloperExperimental,
            None,
        ),
        RendererSelection::Experimental(profile.clone()),
    );
    assert_eq!(
        select_renderer(
            &mali_host(false),
            &[profile],
            RendererSelectionMode::Release,
            None,
        ),
        RendererSelection::Unavailable,
    );
}

#[test]
fn there_is_no_automatic_software_fallback() {
    assert_eq!(
        select_renderer(
            &mali_host(true),
            &[],
            RendererSelectionMode::DeveloperExperimental,
            None,
        ),
        RendererSelection::Unavailable,
    );
}

#[test]
fn mali_profile_does_not_select_on_adreno_identity() {
    let mut host = mali_host(true);
    host.identity.vendor = "Qualcomm".into();
    host.identity.renderer = "Adreno 830".into();

    assert_eq!(
        select_renderer(
            &host,
            &[GpuRendererProfile::gfxstream_vulkan_mali()],
            RendererSelectionMode::Release,
            None,
        ),
        RendererSelection::Unavailable,
    );
}

#[test]
fn release_rejects_qualification_from_a_different_material_tuple() {
    let profile = GpuRendererProfile::gfxstream_vulkan_mali();
    let record = record();
    let mut current = tuple();
    current.kernel_version = "different-kernel".into();

    assert_eq!(
        select_renderer(
            &mali_host(true),
            &[profile],
            RendererSelectionMode::Release,
            Some(RendererQualification {
                record: &record,
                current: &current,
            }),
        ),
        RendererSelection::Unavailable,
    );
}
