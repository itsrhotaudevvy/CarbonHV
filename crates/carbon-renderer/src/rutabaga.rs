#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RutabagaConfig {
    pub use_vulkan: bool,
    pub use_gles: bool,
    pub use_egl: bool,
    pub surfaceless: bool,
    pub use_external_blob: bool,
    pub use_system_blob: bool,
}

impl RutabagaConfig {
    pub const fn gfxstream_vulkan() -> Self {
        Self {
            use_vulkan: true,
            use_gles: false,
            use_egl: false,
            surfaceless: true,
            use_external_blob: true,
            use_system_blob: false,
        }
    }
}

#[cfg(feature = "native-gfxstream")]
pub fn build_native_rutabaga(
    config: RutabagaConfig,
    display_width: u32,
    display_height: u32,
    fence_handler: rutabaga_gfx::RutabagaFenceHandler,
) -> rutabaga_gfx::RutabagaResult<rutabaga_gfx::Rutabaga> {
    use rutabaga_gfx::{
        RUTABAGA_CAPSET_GFXSTREAM_COMPOSER, RUTABAGA_CAPSET_GFXSTREAM_GLES,
        RUTABAGA_CAPSET_GFXSTREAM_VULKAN, RutabagaBuilder, RutabagaComponentType, RutabagaWsi,
    };

    let capset_mask = (1_u64 << RUTABAGA_CAPSET_GFXSTREAM_VULKAN)
        | (1_u64 << RUTABAGA_CAPSET_GFXSTREAM_GLES)
        | (1_u64 << RUTABAGA_CAPSET_GFXSTREAM_COMPOSER);

    RutabagaBuilder::new(capset_mask, fence_handler)
        .set_default_component(RutabagaComponentType::Gfxstream)
        .set_display_width(display_width)
        .set_display_height(display_height)
        .set_use_egl(config.use_egl)
        .set_use_gles(config.use_gles)
        .set_use_surfaceless(config.surfaceless)
        .set_use_vulkan(config.use_vulkan)
        .set_wsi(RutabagaWsi::Surfaceless)
        .set_use_external_blob(config.use_external_blob)
        .set_use_system_blob(config.use_system_blob)
        .build()
}
