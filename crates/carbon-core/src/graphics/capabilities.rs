#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityEvidence<T> {
    Unknown,
    Observed(T),
    Qualified(T),
}

impl CapabilityEvidence<bool> {
    fn value(&self) -> bool {
        matches!(self, Self::Observed(true) | Self::Qualified(true))
    }

    fn qualified_value(&self) -> bool {
        matches!(self, Self::Qualified(true))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RendererFamily {
    Gfxstream,
    Venus,
    Virgl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostGraphicsApi {
    Vulkan,
    Gles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuIdentity {
    pub vendor: String,
    pub renderer: String,
    pub driver_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GraphicsCapability {
    Basic,
    Gles,
    Vulkan,
    NativeBuffer,
    ExplicitSync,
    ZeroCopy,
    Sustained,
    Qualified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuHostCapabilities {
    pub identity: GpuIdentity,
    pub vulkan: CapabilityEvidence<bool>,
    pub gles: CapabilityEvidence<bool>,
    pub native_buffer: CapabilityEvidence<bool>,
    pub explicit_sync: CapabilityEvidence<bool>,
    pub zero_copy: CapabilityEvidence<bool>,
    pub sustained: CapabilityEvidence<bool>,
}

impl GpuHostCapabilities {
    pub fn supports(&self, capability: GraphicsCapability) -> bool {
        self.evidence(capability)
            .is_some_and(CapabilityEvidence::value)
    }

    pub fn is_qualified(&self, capability: GraphicsCapability) -> bool {
        self.evidence(capability)
            .is_some_and(CapabilityEvidence::qualified_value)
    }

    pub fn available_capabilities(&self) -> Vec<GraphicsCapability> {
        [
            GraphicsCapability::Gles,
            GraphicsCapability::Vulkan,
            GraphicsCapability::NativeBuffer,
            GraphicsCapability::ExplicitSync,
            GraphicsCapability::ZeroCopy,
            GraphicsCapability::Sustained,
        ]
        .into_iter()
        .filter(|capability| self.supports(*capability))
        .collect()
    }

    fn evidence(&self, capability: GraphicsCapability) -> Option<&CapabilityEvidence<bool>> {
        match capability {
            GraphicsCapability::Gles => Some(&self.gles),
            GraphicsCapability::Vulkan => Some(&self.vulkan),
            GraphicsCapability::NativeBuffer => Some(&self.native_buffer),
            GraphicsCapability::ExplicitSync => Some(&self.explicit_sync),
            GraphicsCapability::ZeroCopy => Some(&self.zero_copy),
            GraphicsCapability::Sustained => Some(&self.sustained),
            GraphicsCapability::Basic | GraphicsCapability::Qualified => None,
        }
    }
}
