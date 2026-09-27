use super::{GpuHostCapabilities, GraphicsCapability, HostGraphicsApi, RendererFamily};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuRendererProfile {
    pub id: String,
    pub family: RendererFamily,
    pub host_api: HostGraphicsApi,
    pub required: Vec<GraphicsCapability>,
    pub required_host_extensions: Vec<String>,
    pub supported_guest_apis: Vec<String>,
    pub buffer_transport: String,
    pub synchronization: String,
    pub sandbox_profile: String,
    pub quirks: Vec<String>,
    pub host_renderer_prefix: Option<String>,
}

impl GpuRendererProfile {
    pub fn gfxstream_vulkan_mali() -> Self {
        Self {
            id: "gfxstream-vulkan-mali".into(),
            family: RendererFamily::Gfxstream,
            host_api: HostGraphicsApi::Vulkan,
            required: vec![
                GraphicsCapability::Vulkan,
                GraphicsCapability::NativeBuffer,
                GraphicsCapability::ExplicitSync,
            ],
            required_host_extensions: Vec::new(),
            supported_guest_apis: vec!["gles".into(), "vulkan".into()],
            buffer_transport: "ahardwarebuffer".into(),
            synchronization: "explicit".into(),
            sandbox_profile: "renderer-minimal".into(),
            quirks: Vec::new(),
            host_renderer_prefix: Some("Mali-".into()),
        }
    }

    fn host_matches(&self, host: &GpuHostCapabilities) -> bool {
        self.host_renderer_prefix
            .as_ref()
            .is_none_or(|prefix| host.identity.renderer.starts_with(prefix))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererSelectionMode {
    Release,
    DeveloperExperimental,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererSelection {
    Selected(GpuRendererProfile),
    Experimental(GpuRendererProfile),
    Unavailable,
}

pub fn select_renderer(
    host: &GpuHostCapabilities,
    profiles: &[GpuRendererProfile],
    mode: RendererSelectionMode,
) -> RendererSelection {
    for profile in profiles {
        if !profile.host_matches(host)
            || !profile
                .required
                .iter()
                .all(|capability| host.supports(*capability))
        {
            continue;
        }

        let qualified = profile
            .required
            .iter()
            .all(|capability| host.is_qualified(*capability));

        match (mode, qualified) {
            (_, true) => return RendererSelection::Selected(profile.clone()),
            (RendererSelectionMode::DeveloperExperimental, false) => {
                return RendererSelection::Experimental(profile.clone());
            }
            (RendererSelectionMode::Release, false) => {}
        }
    }

    RendererSelection::Unavailable
}
