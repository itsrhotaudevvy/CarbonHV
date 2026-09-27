use carbon_core::{GpuHostCapabilities, GpuRendererProfile, GraphicsCapability};

use crate::RutabagaConfig;

pub const GFXSTREAM_MALI_ENTRY_POINT: &str = "rutabaga-gfxstream";
pub const AVF_CROSVM_OWNER: &str = "avf-crosvm";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererIntegrationPoint {
    pub owner: &'static str,
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
    pub integration: RendererIntegrationPoint,
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
        integration: RendererIntegrationPoint {
            owner: AVF_CROSVM_OWNER,
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
