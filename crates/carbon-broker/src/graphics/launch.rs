use std::path::{Component, PathBuf};

use carbon_core::GpuRendererProfile;

use crate::ProgramInvocation;

const RENDERER_PROGRAM: &str = "/system/bin/carbon-renderer";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererLaunchPlan {
    pub invocation: ProgramInvocation,
    pub runtime_dir: PathBuf,
    pub profile_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererLaunchError {
    UnsafeRuntimeDirectory,
    InvalidProfileId,
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

    Ok(RendererLaunchPlan {
        invocation: ProgramInvocation {
            program: PathBuf::from(RENDERER_PROGRAM),
            args: vec![
                "--profile".into(),
                profile.id.clone(),
                "--runtime-dir".into(),
                runtime_dir.to_string_lossy().into_owned(),
            ],
        },
        runtime_dir,
        profile_id: profile.id.clone(),
    })
}
