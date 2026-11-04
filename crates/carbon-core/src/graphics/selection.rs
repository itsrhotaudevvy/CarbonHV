use super::{
    GpuHostCapabilities, GpuQualificationTuple, GraphicsCapability, HostGraphicsApi,
    QualificationRecord, RendererFamily, qualification_applies,
};

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

    /// Adreno is a host provider/profile only: the guest still sees CVD1 `virtio-gpu`.
    /// Starts from the same gfxstream/Vulkan transport as Mali; Venus remains a candidate to be
    /// compared with the shared workload harness, and no Adreno quirk is added without evidence.
    pub fn gfxstream_vulkan_adreno() -> Self {
        Self {
            id: "gfxstream-vulkan-adreno".into(),
            host_renderer_prefix: Some("Adreno".into()),
            ..Self::gfxstream_vulkan_mali()
        }
    }

    /// Arm Immortalis (MediaTek Dimensity 9200+). Same Arm driver family as Mali; some drivers
    /// report `Mali-G720-Immortalis`, which the Mali profile already matches.
    pub fn gfxstream_vulkan_immortalis() -> Self {
        Self {
            id: "gfxstream-vulkan-immortalis".into(),
            host_renderer_prefix: Some("Immortalis-".into()),
            ..Self::gfxstream_vulkan_mali()
        }
    }

    /// Imagination PowerVR (Google Tensor G5 `DXT-48-1536`, some MediaTek Helio/Dimensity).
    pub fn gfxstream_vulkan_powervr() -> Self {
        Self {
            id: "gfxstream-vulkan-powervr".into(),
            host_renderer_prefix: Some("PowerVR".into()),
            ..Self::gfxstream_vulkan_mali()
        }
    }

    /// Samsung Xclipse (AMD RDNA-derived, Exynos 2200 and later). Older Exynos parts use Mali.
    pub fn gfxstream_vulkan_xclipse() -> Self {
        Self {
            id: "gfxstream-vulkan-xclipse".into(),
            host_renderer_prefix: Some("Samsung Xclipse".into()),
            ..Self::gfxstream_vulkan_mali()
        }
    }

    /// Every built-in host GPU profile, in selection order. All share one gfxstream/Vulkan
    /// transport, buffer, sync and sandbox contract; the guest always sees CVD1 `virtio-gpu`.
    /// No vendor quirk is carried without physical evidence.
    pub fn builtin_profiles() -> Vec<Self> {
        vec![
            Self::gfxstream_vulkan_mali(),
            Self::gfxstream_vulkan_immortalis(),
            Self::gfxstream_vulkan_adreno(),
            Self::gfxstream_vulkan_powervr(),
            Self::gfxstream_vulkan_xclipse(),
        ]
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

#[derive(Debug, Clone, Copy)]
pub struct RendererQualification<'a> {
    pub record: &'a QualificationRecord,
    pub current: &'a GpuQualificationTuple,
}

impl RendererQualification<'_> {
    fn qualifies(&self, profile: &GpuRendererProfile) -> bool {
        qualification_applies(self.record, self.current)
            && self.current.renderer_id == profile.id
            && self
                .record
                .capabilities
                .contains(&GraphicsCapability::Qualified)
            && profile
                .required
                .iter()
                .all(|capability| self.record.capabilities.contains(capability))
    }
}

pub fn select_renderer(
    host: &GpuHostCapabilities,
    profiles: &[GpuRendererProfile],
    mode: RendererSelectionMode,
    qualification: Option<RendererQualification<'_>>,
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

        let qualified = qualification.is_some_and(|evidence| evidence.qualifies(profile));

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
