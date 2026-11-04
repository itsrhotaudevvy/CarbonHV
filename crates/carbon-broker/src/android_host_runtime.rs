use std::sync::Mutex;

use binder::{Strong, wait_for_interface};
use dev_carbonhv_host_runtime::aidl::dev::carbonhv::host::{
    GraphicalVmDisk::GraphicalVmDisk, GraphicalVmLaunchRequest::GraphicalVmLaunchRequest,
    ICarbonHvHostRuntime::ICarbonHvHostRuntime,
};

use crate::{DisplaySessionIdentity, HostRuntimeGraphicalVmRequest, HostRuntimePublisher};

const HOST_RUNTIME_SERVICE: &str = "dev.carbonhv.host.runtime";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AndroidHostRuntimeError {
    Binder(String),
    GenerationOutOfRange,
    LockPoisoned,
}

pub struct AndroidHostRuntimePublisher {
    service: Strong<dyn ICarbonHvHostRuntime>,
}

impl AndroidHostRuntimePublisher {
    pub fn connect() -> Result<Self, AndroidHostRuntimeError> {
        let service = wait_for_interface::<dyn ICarbonHvHostRuntime>(HOST_RUNTIME_SERVICE)
            .map_err(binder_error)?;
        Ok(Self { service })
    }
}

#[derive(Default)]
pub struct LazyAndroidHostRuntimePublisher {
    service: Mutex<Option<AndroidHostRuntimePublisher>>,
}

impl HostRuntimePublisher for LazyAndroidHostRuntimePublisher {
    type Error = AndroidHostRuntimeError;

    fn start_graphical_vm(
        &self,
        request: &HostRuntimeGraphicalVmRequest,
    ) -> Result<(), Self::Error> {
        let mut service = self
            .service
            .lock()
            .map_err(|_| AndroidHostRuntimeError::LockPoisoned)?;
        if service.is_none() {
            *service = Some(AndroidHostRuntimePublisher::connect()?);
        }
        let result = service
            .as_ref()
            .expect("host runtime publisher initialized")
            .start_graphical_vm(request);
        if result.is_err() {
            *service = None;
        }
        result
    }

    fn stop_graphical_vm(&self, identity: &DisplaySessionIdentity) -> Result<(), Self::Error> {
        let mut service = self
            .service
            .lock()
            .map_err(|_| AndroidHostRuntimeError::LockPoisoned)?;
        if service.is_none() {
            *service = Some(AndroidHostRuntimePublisher::connect()?);
        }
        let result = service
            .as_ref()
            .expect("host runtime publisher initialized")
            .stop_graphical_vm(identity);
        if result.is_err() {
            *service = None;
        }
        result
    }
}

impl HostRuntimePublisher for AndroidHostRuntimePublisher {
    type Error = AndroidHostRuntimeError;

    fn start_graphical_vm(
        &self,
        request: &HostRuntimeGraphicalVmRequest,
    ) -> Result<(), Self::Error> {
        let disks = request
            .disks
            .iter()
            .map(|disk| GraphicalVmDisk {
                path: disk.path.clone(),
                writable: disk.writable,
            })
            .collect();
        let request = GraphicalVmLaunchRequest {
            instanceId: request.instance_id.clone(),
            generation: request.generation,
            kernelPath: request.kernel_path.clone(),
            initrdPath: request.initrd_path.clone(),
            disks,
            memoryMib: request.memory_mib,
            vcpus: request.vcpus,
            kernelParams: request.kernel_params.clone(),
            widthPixels: request.width_pixels,
            heightPixels: request.height_pixels,
            densityDpi: request.density_dpi,
            refreshRate: request.refresh_rate,
        };
        self.service
            .r#startGraphicalVm(&request)
            .map_err(binder_error)
    }

    fn stop_graphical_vm(&self, identity: &DisplaySessionIdentity) -> Result<(), Self::Error> {
        let generation = i64::try_from(identity.generation.0)
            .map_err(|_| AndroidHostRuntimeError::GenerationOutOfRange)?;
        self.service
            .r#stopGraphicalVm(&identity.instance_id, generation)
            .map_err(binder_error)
    }
}

fn binder_error(error: impl std::fmt::Debug) -> AndroidHostRuntimeError {
    AndroidHostRuntimeError::Binder(format!("{error:?}"))
}
