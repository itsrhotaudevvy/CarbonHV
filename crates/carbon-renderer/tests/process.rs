use std::path::PathBuf;

use carbon_renderer::{
    PRESENTATION_FD, RendererProcessConfig, RendererProcessConfigError, VIRTIO_GPU_FD,
};

fn args() -> Vec<String> {
    vec![
        "--profile".into(),
        "gfxstream-vulkan-mali".into(),
        "--runtime-dir".into(),
        "/data/misc/carbonhv/runtime/vm-1".into(),
        "--virtio-gpu-fd".into(),
        VIRTIO_GPU_FD.to_string(),
        "--presentation-fd".into(),
        PRESENTATION_FD.to_string(),
    ]
}

#[test]
fn parses_fixed_native_renderer_process_contract() {
    let config = RendererProcessConfig::parse(&args()).expect("valid renderer argv");

    assert_eq!(config.profile_id, "gfxstream-vulkan-mali");
    assert_eq!(
        config.runtime_dir,
        PathBuf::from("/data/misc/carbonhv/runtime/vm-1"),
    );
    assert_eq!(config.virtio_gpu_fd, VIRTIO_GPU_FD);
    assert_eq!(config.presentation_fd, PRESENTATION_FD);
}

#[test]
fn rejects_runtime_directory_outside_managed_renderer_root() {
    let mut input = args();
    input[3] = "/data/local/tmp/carbonhv".into();

    assert_eq!(
        RendererProcessConfig::parse(&input),
        Err(RendererProcessConfigError::UnsafeRuntimeDirectory),
    );
}

#[test]
fn rejects_unknown_profile_and_unsafe_descriptor_layout() {
    let mut unknown = args();
    unknown[1] = "unknown-renderer".into();
    assert_eq!(
        RendererProcessConfig::parse(&unknown),
        Err(RendererProcessConfigError::UnsupportedProfile),
    );

    let mut stdio = args();
    stdio[5] = "2".into();
    assert_eq!(
        RendererProcessConfig::parse(&stdio),
        Err(RendererProcessConfigError::InvalidFileDescriptor),
    );

    let mut collision = args();
    collision[7] = VIRTIO_GPU_FD.to_string();
    assert_eq!(
        RendererProcessConfig::parse(&collision),
        Err(RendererProcessConfigError::FileDescriptorCollision),
    );
}

#[test]
fn rejects_duplicate_and_unknown_arguments() {
    let mut duplicate = args();
    duplicate.extend(["--profile".into(), "gfxstream-vulkan-mali".into()]);
    assert_eq!(
        RendererProcessConfig::parse(&duplicate),
        Err(RendererProcessConfigError::DuplicateArgument),
    );

    let mut unknown = args();
    unknown.extend(["--shell".into(), "/system/bin/sh".into()]);
    assert_eq!(
        RendererProcessConfig::parse(&unknown),
        Err(RendererProcessConfigError::UnknownArgument),
    );
}
