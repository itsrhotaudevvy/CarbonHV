use carbon_core::{
    CapabilityEvidence, GpuHostCapabilities, GpuIdentity, GpuRendererProfile, RendererSelection,
    RendererSelectionMode, select_renderer,
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

#[test]
fn release_selects_qualified_mali_profile() {
    let profile = GpuRendererProfile::gfxstream_vulkan_mali();
    let selected = select_renderer(
        &mali_host(true),
        &[profile.clone()],
        RendererSelectionMode::Release,
    );

    assert_eq!(profile.id, "gfxstream-vulkan-mali");
    assert_eq!(selected, RendererSelection::Selected(profile));
}

#[test]
fn missing_required_capability_fails_closed() {
    let mut host = mali_host(true);
    host.native_buffer = CapabilityEvidence::Unknown;
    let profile = GpuRendererProfile::gfxstream_vulkan_mali();

    assert_eq!(
        select_renderer(
            &host,
            &[profile.clone()],
            RendererSelectionMode::DeveloperExperimental,
        ),
        RendererSelection::Unavailable,
    );
    assert_eq!(
        select_renderer(&host, &[profile], RendererSelectionMode::Release),
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
        ),
        RendererSelection::Experimental(profile.clone()),
    );
    assert_eq!(
        select_renderer(
            &mali_host(false),
            &[profile],
            RendererSelectionMode::Release,
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
        ),
        RendererSelection::Unavailable,
    );
}
