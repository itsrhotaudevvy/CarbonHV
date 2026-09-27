use carbon_core::{
    CapabilityEvidence, GpuHostCapabilities, GpuIdentity, GraphicsCapability, HostGraphicsApi,
};
use carbon_renderer::{
    GFXSTREAM_MALI_ENTRY_POINT, RendererAdmissionError, gfxstream_vulkan_mali_backend,
};

fn mali_host() -> GpuHostCapabilities {
    GpuHostCapabilities {
        identity: GpuIdentity {
            vendor: "ARM".into(),
            renderer: "Mali-G710".into(),
            driver_version: Some("226504704".into()),
        },
        vulkan: CapabilityEvidence::Observed(true),
        gles: CapabilityEvidence::Observed(true),
        native_buffer: CapabilityEvidence::Observed(true),
        explicit_sync: CapabilityEvidence::Observed(true),
        zero_copy: CapabilityEvidence::Unknown,
        sustained: CapabilityEvidence::Unknown,
    }
}

#[test]
fn mali_backend_requests_vulkan_and_native_buffer_sync() {
    let backend = gfxstream_vulkan_mali_backend();

    assert_eq!(backend.profile.id, "gfxstream-vulkan-mali");
    assert_eq!(backend.profile.host_api, HostGraphicsApi::Vulkan);
    assert!(
        backend
            .profile
            .required
            .contains(&GraphicsCapability::Vulkan)
    );
    assert!(
        backend
            .profile
            .required
            .contains(&GraphicsCapability::NativeBuffer)
    );
    assert!(
        backend
            .profile
            .required
            .contains(&GraphicsCapability::ExplicitSync)
    );
    assert!(backend.rutabaga.use_vulkan);
    assert!(backend.rutabaga.use_external_blob);
    assert!(backend.rutabaga.surfaceless);
}

#[test]
fn missing_native_buffer_or_explicit_sync_is_rejected() {
    let backend = gfxstream_vulkan_mali_backend();
    let mut missing_buffer = mali_host();
    missing_buffer.native_buffer = CapabilityEvidence::Unknown;
    assert_eq!(
        backend.validate_host(&missing_buffer),
        Err(RendererAdmissionError::MissingCapability(
            GraphicsCapability::NativeBuffer
        ))
    );

    let mut missing_sync = mali_host();
    missing_sync.explicit_sync = CapabilityEvidence::Observed(false);
    assert_eq!(
        backend.validate_host(&missing_sync),
        Err(RendererAdmissionError::MissingCapability(
            GraphicsCapability::ExplicitSync
        ))
    );
}

#[test]
fn renderer_has_one_fixed_executable_and_backend_entry_point() {
    let backend = gfxstream_vulkan_mali_backend();

    assert_eq!(backend.entry.executable, "/system/bin/carbon-renderer");
    assert_eq!(backend.entry.backend, GFXSTREAM_MALI_ENTRY_POINT);
}

#[test]
fn minimal_renderer_sandbox_has_no_generic_network_or_filesystem_access() {
    let backend = gfxstream_vulkan_mali_backend();

    assert_eq!(backend.profile.sandbox_profile, "renderer-minimal");
    assert!(!backend.sandbox.allow_network);
    assert!(!backend.sandbox.allow_unrelated_host_filesystem);
    assert!(backend.sandbox.allow_runtime_directory);
    assert!(backend.sandbox.allow_gpu_device);
}

#[test]
fn mali_identity_is_required_by_backend_admission() {
    let backend = gfxstream_vulkan_mali_backend();
    let mut host = mali_host();
    host.identity.renderer = "Adreno 740".into();

    assert_eq!(
        backend.validate_host(&host),
        Err(RendererAdmissionError::IncompatibleHostRenderer)
    );
}
