use super::GraphicsCapability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuQualificationTuple {
    pub carbonhv_commit: String,
    pub device_model: String,
    pub soc: String,
    pub gpu: String,
    pub host_build_fingerprint: String,
    pub kernel_version: String,
    pub avf_revision: String,
    pub renderer_id: String,
    pub renderer_version: String,
    pub graphics_driver_version: String,
    pub guest_bundle_id: String,
    pub guest_bundle_version: String,
    pub cvd_major: u32,
    pub display_mode: String,
    pub workload_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualificationRecord {
    pub tuple: GpuQualificationTuple,
    pub capabilities: Vec<GraphicsCapability>,
}

pub fn qualification_applies(
    record: &QualificationRecord,
    current: &GpuQualificationTuple,
) -> bool {
    record.tuple == *current
}
