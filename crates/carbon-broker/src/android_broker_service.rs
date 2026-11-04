use std::path::PathBuf;

use binder::{BinderFeatures, Interface as _, ProcessState, ThreadState};
use carbon_core::Generation;
use carbon_protocol::CallerIdentity;
use dev_carbonhv_broker::aidl::dev::carbonhv::broker::{
    BrokerStartRequest::BrokerStartRequest,
    ICarbonHvBroker::{BnCarbonHvBroker, ICarbonHvBroker},
};

use crate::{
    AndroidHostRuntimeError, AvfBackend, BackendError, BrokerControlError, BrokerControlPlane,
    BrokerPolicy, GraphicalRuntimeError, GraphicalStartIntent, HostResources,
    HostRuntimeRequestError, LazyAndroidHostRuntimePublisher, RuntimeError, StarterBundleError,
};

const BROKER_SERVICE: &str = "dev.carbonhv.broker";
const SYSTEM_UID: u32 = 1000;
const INTERNAL_CALLER_DIGEST: &str = "carbonhv-host-bridge";
const MANAGED_ROOT: &str = "/data/misc/carbonhv/objects";
const GUNYAH_DEVICE: &str = "/dev/gunyah";

pub fn run_android_broker_service() -> Result<(), String> {
    ProcessState::set_thread_pool_max_thread_count(4);
    ProcessState::start_thread_pool();

    let mut policy = BrokerPolicy {
        allowed_uid: SYSTEM_UID,
        allowed_signing_digest: INTERNAL_CALLER_DIGEST.to_string(),
        managed_root: PathBuf::from(MANAGED_ROOT),
        max_memory_mib: 16 * 1024,
        max_vcpus: 8,
    };
    // Tighten the static ceiling to what this phone can give a guest. If the host cannot be
    // read, the conservative static policy stays in force rather than a guess.
    match HostResources::read(
        &PathBuf::from("/proc/meminfo"),
        &PathBuf::from("/sys/devices/system/cpu"),
    ) {
        Ok(host) => policy = policy.constrained_to(&host),
        Err(error) => {
            eprintln!("carbonhvd: host resource probe failed, keeping static limits: {error}")
        }
    }

    let service = CarbonHvBrokerService {
        control: BrokerControlPlane::new(
            policy,
            AvfBackend::new(
                PathBuf::from("/apex/com.android.virt/bin/vm"),
                PathBuf::from("/dev/kvm"),
            )
            .with_gunyah_device(PathBuf::from(GUNYAH_DEVICE)),
            LazyAndroidHostRuntimePublisher::default(),
            CallerIdentity {
                uid: SYSTEM_UID,
                signing_digest: INTERNAL_CALLER_DIGEST.to_string(),
            },
        ),
    };
    let service = BnCarbonHvBroker::new_binder(service, BinderFeatures::default());
    binder::add_service(BROKER_SERVICE, service.as_binder())
        .map_err(|error| format!("failed to publish {BROKER_SERVICE}: {error:?}"))?;

    ProcessState::join_thread_pool();
    Ok(())
}

struct CarbonHvBrokerService {
    control: BrokerControlPlane<LazyAndroidHostRuntimePublisher>,
}

impl binder::Interface for CarbonHvBrokerService {}

impl ICarbonHvBroker for CarbonHvBrokerService {
    fn r#startGraphicalVm(&self, request: &BrokerStartRequest) -> binder::Result<i64> {
        enforce_system_uid()?;
        let intent = GraphicalStartIntent {
            request_id: nonnegative_u64(request.requestId)?,
            instance_id: request.instanceId.clone(),
            bundle_id: request.bundleId.clone(),
            memory_mib: positive_u64(request.memoryMib)?,
            vcpus: positive_u16(request.vcpus)?,
            width_pixels: positive_u32(request.widthPixels)?,
            height_pixels: positive_u32(request.heightPixels)?,
            density_dpi: positive_u32(request.densityDpi)?,
            refresh_rate: positive_u32(request.refreshRate)?,
        };
        let generation = self
            .control
            .start_graphical_vm(intent)
            .map_err(control_error_status)?;
        i64::try_from(generation.0).map_err(|_| bad_value())
    }

    fn r#stopVm(&self, instance_id: &str, generation: i64, request_id: i64) -> binder::Result<()> {
        enforce_system_uid()?;
        self.control
            .stop_vm(
                instance_id,
                Generation(nonnegative_u64(generation)?),
                nonnegative_u64(request_id)?,
            )
            .map_err(control_error_status)
    }

    fn r#activeDisplayGeneration(&self, instance_id: &str) -> binder::Result<i64> {
        enforce_system_uid()?;
        match self
            .control
            .active_display_generation(instance_id)
            .map_err(control_error_status)?
        {
            Some(generation) => i64::try_from(generation.0).map_err(|_| bad_value()),
            None => Ok(-1),
        }
    }

    fn r#notifyRuntimeDied(&self, instance_id: &str, generation: i64) -> binder::Result<()> {
        enforce_system_uid()?;
        self.control
            .notify_runtime_died(instance_id, Generation(nonnegative_u64(generation)?))
            .map(|_| ())
            .map_err(control_error_status)
    }
}

fn enforce_system_uid() -> binder::Result<()> {
    if ThreadState::get_calling_uid() == SYSTEM_UID {
        Ok(())
    } else {
        Err(binder::StatusCode::PERMISSION_DENIED.into())
    }
}

fn nonnegative_u64(value: i64) -> binder::Result<u64> {
    u64::try_from(value).map_err(|_| bad_value())
}

fn positive_u64(value: i32) -> binder::Result<u64> {
    let value = u64::try_from(value).map_err(|_| bad_value())?;
    if value == 0 {
        Err(bad_value())
    } else {
        Ok(value)
    }
}

fn positive_u32(value: i32) -> binder::Result<u32> {
    let value = u32::try_from(value).map_err(|_| bad_value())?;
    if value == 0 {
        Err(bad_value())
    } else {
        Ok(value)
    }
}

fn positive_u16(value: i32) -> binder::Result<u16> {
    let value = u16::try_from(value).map_err(|_| bad_value())?;
    if value == 0 {
        Err(bad_value())
    } else {
        Ok(value)
    }
}

fn bad_value() -> binder::Status {
    binder::StatusCode::BAD_VALUE.into()
}

fn runtime_error_status(error: RuntimeError) -> binder::Status {
    match error {
        RuntimeError::UnauthorizedCaller => binder::StatusCode::PERMISSION_DENIED.into(),
        RuntimeError::InstanceAlreadyRunning { .. }
        | RuntimeError::InstanceNotRunning
        | RuntimeError::DisplaySessionBusy { .. }
        | RuntimeError::DisplaySessionNotActive => binder::StatusCode::INVALID_OPERATION.into(),
        RuntimeError::InvalidInstanceId
        | RuntimeError::UnsafeKernelParameter
        | RuntimeError::InvalidResourceRequest
        | RuntimeError::MemoryLimitExceeded { .. }
        | RuntimeError::VcpuLimitExceeded { .. }
        | RuntimeError::StaleGeneration { .. }
        | RuntimeError::GenerationMismatch { .. } => bad_value(),
    }
}

fn control_error_status(error: BrokerControlError<AndroidHostRuntimeError>) -> binder::Status {
    match error {
        BrokerControlError::StatePoisoned => binder::StatusCode::FAILED_TRANSACTION.into(),
        BrokerControlError::Broker(error) => runtime_error_status(error),
        BrokerControlError::StarterBundle(
            StarterBundleError::UnsupportedBundle | StarterBundleError::InvalidInstanceId,
        )
        | BrokerControlError::Graphics(BackendError::InvalidGraphicsConfig) => bad_value(),
        BrokerControlError::Graphics(_) => binder::StatusCode::INVALID_OPERATION.into(),
        BrokerControlError::Runtime(GraphicalRuntimeError::Broker(error)) => {
            runtime_error_status(error)
        }
        BrokerControlError::Runtime(GraphicalRuntimeError::Backend(
            BackendError::InvalidGraphicsConfig,
        ))
        | BrokerControlError::Runtime(GraphicalRuntimeError::HostRequest(
            HostRuntimeRequestError::GenerationOutOfRange
            | HostRuntimeRequestError::MemoryOutOfRange
            | HostRuntimeRequestError::DisplayValueOutOfRange,
        )) => bad_value(),
        BrokerControlError::Runtime(GraphicalRuntimeError::Backend(_))
        | BrokerControlError::Runtime(GraphicalRuntimeError::HostRequest(_)) => {
            binder::StatusCode::INVALID_OPERATION.into()
        }
        BrokerControlError::Runtime(GraphicalRuntimeError::Publisher(_)) => {
            binder::StatusCode::FAILED_TRANSACTION.into()
        }
    }
}
