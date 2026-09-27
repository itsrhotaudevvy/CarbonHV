use carbon_core::{GpuHostCapabilities, GpuRendererProfile, GraphicsCapability};

use crate::RutabagaConfig;

pub const RENDERER_EXECUTABLE: &str = "/system/bin/carbon-renderer";
pub const GFXSTREAM_MALI_ENTRY_POINT: &str = "rutabaga-gfxstream";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererEntryPoint {
    pub executable: &'static str,
    pub backend: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererSandboxPolicy {
    pub allow_network: bool,
    pub allow_unrelated_host_filesystem: bool,
    pub allow_runtime_directory: bool,
    pub allow_gpu_device: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererBackendProfile {
    pub profile: GpuRendererProfile,
    pub entry: RendererEntryPoint,
    pub sandbox: RendererSandboxPolicy,
    pub rutabaga: RutabagaConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererAdmissionError {
    IncompatibleHostRenderer,
    MissingCapability(GraphicsCapability),
}

impl RendererBackendProfile {
    pub fn validate_host(&self, host: &GpuHostCapabilities) -> Result<(), RendererAdmissionError> {
        if self
            .profile
            .host_renderer_prefix
            .as_ref()
            .is_some_and(|prefix| !host.identity.renderer.starts_with(prefix))
        {
            return Err(RendererAdmissionError::IncompatibleHostRenderer);
        }

        for capability in &self.profile.required {
            if !host.supports(*capability) {
                return Err(RendererAdmissionError::MissingCapability(*capability));
            }
        }

        Ok(())
    }
}

pub fn gfxstream_vulkan_mali_backend() -> RendererBackendProfile {
    RendererBackendProfile {
        profile: GpuRendererProfile::gfxstream_vulkan_mali(),
        entry: RendererEntryPoint {
            executable: RENDERER_EXECUTABLE,
            backend: GFXSTREAM_MALI_ENTRY_POINT,
        },
        sandbox: RendererSandboxPolicy {
            allow_network: false,
            allow_unrelated_host_filesystem: false,
            allow_runtime_directory: true,
            allow_gpu_device: true,
        },
        rutabaga: RutabagaConfig::gfxstream_vulkan(),
    }
}

pub fn renderer_entry_point(profile_id: &str) -> Option<RendererEntryPoint> {
    match profile_id {
        "gfxstream-vulkan-mali" => Some(RendererEntryPoint {
            executable: RENDERER_EXECUTABLE,
            backend: GFXSTREAM_MALI_ENTRY_POINT,
        }),
        _ => None,
    }
}
