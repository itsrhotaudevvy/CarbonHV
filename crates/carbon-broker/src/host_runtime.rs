use std::path::Path;

use crate::{AvfPlatformVmPlan, DisplaySessionIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRuntimeDisk {
    pub path: String,
    pub writable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRuntimeGraphicalVmRequest {
    pub instance_id: String,
    pub generation: i64,
    pub kernel_path: String,
    pub initrd_path: String,
    pub disks: Vec<HostRuntimeDisk>,
    pub memory_mib: i64,
    pub vcpus: i32,
    pub kernel_params: Vec<String>,
    pub width_pixels: i32,
    pub height_pixels: i32,
    pub density_dpi: i32,
    pub refresh_rate: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRuntimeRequestError {
    GenerationOutOfRange,
    MemoryOutOfRange,
    DisplayValueOutOfRange,
    NonUtf8Path,
}

impl HostRuntimeGraphicalVmRequest {
    pub fn try_from_plan(
        identity: &DisplaySessionIdentity,
        plan: &AvfPlatformVmPlan,
    ) -> Result<Self, HostRuntimeRequestError> {
        let generation = i64::try_from(identity.generation.0)
            .map_err(|_| HostRuntimeRequestError::GenerationOutOfRange)?;
        let memory_mib = i64::try_from(plan.memory_mib)
            .map_err(|_| HostRuntimeRequestError::MemoryOutOfRange)?;
        let width_pixels = i32::try_from(plan.display.width_pixels)
            .map_err(|_| HostRuntimeRequestError::DisplayValueOutOfRange)?;
        let height_pixels = i32::try_from(plan.display.height_pixels)
            .map_err(|_| HostRuntimeRequestError::DisplayValueOutOfRange)?;
        let density_dpi = i32::try_from(plan.display.density_dpi)
            .map_err(|_| HostRuntimeRequestError::DisplayValueOutOfRange)?;
        let refresh_rate = i32::try_from(plan.display.refresh_rate)
            .map_err(|_| HostRuntimeRequestError::DisplayValueOutOfRange)?;

        let disks = plan
            .disks
            .iter()
            .map(|disk| {
                Ok(HostRuntimeDisk {
                    path: path_to_utf8(&disk.path)?,
                    writable: disk.writable,
                })
            })
            .collect::<Result<Vec<_>, HostRuntimeRequestError>>()?;

        Ok(Self {
            instance_id: identity.instance_id.clone(),
            generation,
            kernel_path: path_to_utf8(&plan.kernel)?,
            initrd_path: path_to_utf8(&plan.initrd)?,
            disks,
            memory_mib,
            vcpus: i32::from(plan.vcpus),
            kernel_params: plan.kernel_params.clone(),
            width_pixels,
            height_pixels,
            density_dpi,
            refresh_rate,
        })
    }
}

pub trait HostRuntimePublisher {
    type Error;

    fn start_graphical_vm(
        &self,
        request: &HostRuntimeGraphicalVmRequest,
    ) -> Result<(), Self::Error>;

    fn stop_graphical_vm(&self, identity: &DisplaySessionIdentity) -> Result<(), Self::Error>;
}

fn path_to_utf8(path: &Path) -> Result<String, HostRuntimeRequestError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or(HostRuntimeRequestError::NonUtf8Path)
}
