use std::path::{Component, PathBuf};

use carbon_core::GpuRendererProfile;
use carbon_renderer::renderer_entry_point;

use crate::ProgramInvocation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererLaunchPlan {
    pub invocation: ProgramInvocation,
    pub runtime_dir: PathBuf,
    pub profile_id: String,
    pub backend_entry_point: String,
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
    if runtime_dir
        .components()
        .any(|component| matches!(component, Component::ParentDir))
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
            ],
        },
        runtime_dir,
        profile_id: profile.id.clone(),
        backend_entry_point: entry.backend.into(),
    })
}
