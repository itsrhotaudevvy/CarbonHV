use std::path::{Component, Path, PathBuf};

use carbon_core::GpuRendererProfile;
use carbon_renderer::{
    PRESENTATION_FD, RENDERER_RUNTIME_ROOT, VIRTIO_GPU_FD, renderer_entry_point,
};

use crate::ProgramInvocation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererLaunchPlan {
    pub invocation: ProgramInvocation,
    pub runtime_dir: PathBuf,
    pub profile_id: String,
    pub backend_entry_point: String,
    pub virtio_gpu_fd: i32,
    pub presentation_fd: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererLaunchError {
    UnsafeRuntimeDirectory,
    InvalidProfileId,
    UnsupportedProfile,
}

pub fn build_renderer_launch_plan(
    profile: &GpuRendererProfile,
    runtime_dir: PathBuf,
) -> Result<RendererLaunchPlan, RendererLaunchError> {
    if !runtime_dir.is_absolute()
        || !runtime_dir.starts_with(Path::new(RENDERER_RUNTIME_ROOT))
        || runtime_dir
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(RendererLaunchError::UnsafeRuntimeDirectory);
    }
    if profile.id.is_empty()
        || !profile
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(RendererLaunchError::InvalidProfileId);
    }
    let entry = renderer_entry_point(&profile.id).ok_or(RendererLaunchError::UnsupportedProfile)?;

    Ok(RendererLaunchPlan {
        invocation: ProgramInvocation {
            program: PathBuf::from(entry.executable),
            args: vec![
                "--profile".into(),
                profile.id.clone(),
                "--runtime-dir".into(),
                runtime_dir.to_string_lossy().into_owned(),
                "--virtio-gpu-fd".into(),
                VIRTIO_GPU_FD.to_string(),
                "--presentation-fd".into(),
                PRESENTATION_FD.to_string(),
            ],
        },
        runtime_dir,
        profile_id: profile.id.clone(),
        backend_entry_point: entry.backend.into(),
        virtio_gpu_fd: VIRTIO_GPU_FD,
        presentation_fd: PRESENTATION_FD,
    })
}
