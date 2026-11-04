#![forbid(unsafe_code)]

#[cfg(target_os = "android")]
mod android_broker_service;
#[cfg(target_os = "android")]
mod android_host_runtime;
mod avf;
mod backend;
mod control_plane;
mod direct_crosvm;
mod graphics;
mod host_resources;
mod host_runtime;
mod performance;
mod runtime_orchestration;
mod starter_bundles;

use std::collections::BTreeMap;
use std::path::PathBuf;

use carbon_core::Generation;
use carbon_protocol::{CallerIdentity, VmStartRequest};

#[cfg(target_os = "android")]
pub use android_broker_service::run_android_broker_service;
#[cfg(target_os = "android")]
pub use android_host_runtime::{
    AndroidHostRuntimeError, AndroidHostRuntimePublisher, LazyAndroidHostRuntimePublisher,
};
pub use avf::{
    AvfBackend, AvfDisplayConfig, AvfGpuConfig, AvfGraphicsConfig, AvfInputConfig, AvfLaunchPlan,
    AvfPerformanceConfig, AvfPlatformVmPlan,
};
pub use backend::{
    BackendError, HostProbe, HostVirtualizationClass, HypervisorKind, HypervisorProperties,
    ProgramInvocation,
};
pub use control_plane::{BrokerControlError, BrokerControlPlane, GraphicalStartIntent};
pub use direct_crosvm::{DirectCrosvmBackend, DirectLaunchPlan, DirectSessionFds};
pub use graphics::{GraphicsHandleLeaseError, GraphicsHandleLeaseTable, GraphicsHandleToken};
pub use host_resources::{
    CAPACITY_SCALE, CoreKind, CpuCore, HostResources, PlacementError, VcpuPin, VcpuPlacement,
};
pub use host_runtime::{
    HostRuntimeDisk, HostRuntimeGraphicalVmRequest, HostRuntimePublisher, HostRuntimeRequestError,
};
pub use performance::{
    CpuControl, CpuFrequencyControl, CpuPlan, GuestPerformancePlan, PerformanceProfile, ThpMode,
};
pub use runtime_orchestration::{GraphicalRuntimeCoordinator, GraphicalRuntimeError};
pub use starter_bundles::{StarterBundleError, resolve_starter_bundle_request};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerPolicy {
    pub allowed_uid: u32,
    pub allowed_signing_digest: String,
    pub managed_root: PathBuf,
    pub max_memory_mib: u64,
    pub max_vcpus: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedVmDisk {
    pub path: PathBuf,
    pub writable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedVmSpec {
    pub request_id: u64,
    pub instance_id: String,
    pub generation: Generation,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub disks: Vec<ValidatedVmDisk>,
    pub memory_mib: u64,
    pub vcpus: u16,
    pub kernel_params: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplaySessionIdentity {
    pub instance_id: String,
    pub generation: Generation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    UnauthorizedCaller,
    InvalidInstanceId,
    UnsafeKernelParameter,
    InvalidResourceRequest,
    MemoryLimitExceeded {
        requested_mib: u64,
        max_mib: u64,
    },
    VcpuLimitExceeded {
        requested: u16,
        max: u16,
    },
    StaleGeneration {
        minimum: Generation,
        got: Generation,
    },
    InstanceAlreadyRunning {
        generation: Generation,
    },
    InstanceNotRunning,
    GenerationMismatch {
        expected: Generation,
        got: Generation,
    },
    DisplaySessionBusy {
        instance_id: String,
        generation: Generation,
    },
    DisplaySessionNotActive,
}

#[derive(Debug, Clone)]
struct InstanceRuntime {
    generation: Generation,
    last_start_request_id: u64,
    spec: ValidatedVmSpec,
}

pub struct Broker {
    policy: BrokerPolicy,
    minimum_generation: BTreeMap<String, Generation>,
    running: BTreeMap<String, InstanceRuntime>,
    completed_stops: BTreeMap<String, (Generation, u64)>,
    active_display_session: Option<DisplaySessionIdentity>,
}

impl Broker {
    pub fn new(policy: BrokerPolicy) -> Self {
        Self {
            policy,
            minimum_generation: BTreeMap::new(),
            running: BTreeMap::new(),
            completed_stops: BTreeMap::new(),
            active_display_session: None,
        }
    }

    pub fn start(
        &mut self,
        caller: &CallerIdentity,
        request: VmStartRequest,
    ) -> Result<ValidatedVmSpec, RuntimeError> {
        self.authorize(caller)?;
        if !valid_instance_id(&request.instance_id) {
            return Err(RuntimeError::InvalidInstanceId);
        }
        if request.resources.memory_mib == 0 || request.resources.vcpus == 0 {
            return Err(RuntimeError::InvalidResourceRequest);
        }
        if request.resources.memory_mib > self.policy.max_memory_mib {
            return Err(RuntimeError::MemoryLimitExceeded {
                requested_mib: request.resources.memory_mib,
                max_mib: self.policy.max_memory_mib,
            });
        }
        if request.resources.vcpus > self.policy.max_vcpus {
            return Err(RuntimeError::VcpuLimitExceeded {
                requested: request.resources.vcpus,
                max: self.policy.max_vcpus,
            });
        }
        if request.kernel_params.len() > 64
            || request.kernel_params.iter().any(|param| {
                param.is_empty() || param.len() > 1024 || param.chars().any(char::is_control)
            })
        {
            return Err(RuntimeError::UnsafeKernelParameter);
        }

        if let Some(existing) = self.running.get(&request.instance_id) {
            if existing.generation == request.generation
                && existing.last_start_request_id == request.request_id
            {
                return Ok(existing.spec.clone());
            }
            return Err(RuntimeError::InstanceAlreadyRunning {
                generation: existing.generation,
            });
        }

        let minimum = self
            .minimum_generation
            .get(&request.instance_id)
            .copied()
            .unwrap_or(Generation(0));
        if request.generation < minimum {
            return Err(RuntimeError::StaleGeneration {
                minimum,
                got: request.generation,
            });
        }

        let spec = ValidatedVmSpec {
            request_id: request.request_id,
            instance_id: request.instance_id.clone(),
            generation: request.generation,
            kernel: self.policy.managed_root.join(request.kernel.as_str()),
            initrd: self.policy.managed_root.join(request.initrd.as_str()),
            disks: request
                .disks
                .iter()
                .map(|disk| ValidatedVmDisk {
                    path: self.policy.managed_root.join(disk.object_id.as_str()),
                    writable: disk.writable,
                })
                .collect(),
            memory_mib: request.resources.memory_mib,
            vcpus: request.resources.vcpus,
            kernel_params: request.kernel_params,
        };
        self.running.insert(
            request.instance_id,
            InstanceRuntime {
                generation: request.generation,
                last_start_request_id: request.request_id,
                spec: spec.clone(),
            },
        );
        Ok(spec)
    }

    pub fn next_generation(
        &self,
        caller: &CallerIdentity,
        instance_id: &str,
    ) -> Result<Generation, RuntimeError> {
        self.authorize(caller)?;
        if !valid_instance_id(instance_id) {
            return Err(RuntimeError::InvalidInstanceId);
        }
        if let Some(existing) = self.running.get(instance_id) {
            return Err(RuntimeError::InstanceAlreadyRunning {
                generation: existing.generation,
            });
        }
        Ok(self
            .minimum_generation
            .get(instance_id)
            .copied()
            .unwrap_or(Generation(0)))
    }

    pub fn generation_for_start(
        &self,
        caller: &CallerIdentity,
        instance_id: &str,
    ) -> Result<Generation, RuntimeError> {
        self.authorize(caller)?;
        if !valid_instance_id(instance_id) {
            return Err(RuntimeError::InvalidInstanceId);
        }
        if let Some(existing) = self.running.get(instance_id) {
            return Ok(existing.generation);
        }
        Ok(self
            .minimum_generation
            .get(instance_id)
            .copied()
            .unwrap_or(Generation(0)))
    }

    pub fn stop(
        &mut self,
        caller: &CallerIdentity,
        instance_id: &str,
        generation: Generation,
        request_id: u64,
    ) -> Result<(), RuntimeError> {
        self.authorize(caller)?;
        let Some(running) = self.running.get(instance_id) else {
            if self.completed_stops.get(instance_id) == Some(&(generation, request_id)) {
                return Ok(());
            }
            return Err(RuntimeError::InstanceNotRunning);
        };
        if running.generation != generation {
            return Err(RuntimeError::GenerationMismatch {
                expected: running.generation,
                got: generation,
            });
        }
        if self.active_display_session.as_ref()
            == Some(&DisplaySessionIdentity {
                instance_id: instance_id.to_string(),
                generation,
            })
        {
            self.active_display_session = None;
        }
        self.running.remove(instance_id);
        self.minimum_generation.insert(
            instance_id.to_string(),
            Generation(generation.0.saturating_add(1)),
        );
        self.completed_stops
            .insert(instance_id.to_string(), (generation, request_id));
        Ok(())
    }

    pub fn claim_display_session(
        &mut self,
        caller: &CallerIdentity,
        instance_id: &str,
        generation: Generation,
    ) -> Result<DisplaySessionIdentity, RuntimeError> {
        self.authorize(caller)?;
        self.ensure_running_generation(instance_id, generation)?;

        if let Some(active) = &self.active_display_session {
            if active.instance_id == instance_id && active.generation == generation {
                return Ok(active.clone());
            }
            return Err(RuntimeError::DisplaySessionBusy {
                instance_id: active.instance_id.clone(),
                generation: active.generation,
            });
        }

        let identity = DisplaySessionIdentity {
            instance_id: instance_id.to_string(),
            generation,
        };
        self.active_display_session = Some(identity.clone());
        Ok(identity)
    }

    pub fn authorize_display_session(
        &self,
        caller: &CallerIdentity,
        instance_id: &str,
        generation: Generation,
    ) -> Result<(), RuntimeError> {
        self.authorize(caller)?;
        let Some(active) = &self.active_display_session else {
            return Err(RuntimeError::DisplaySessionNotActive);
        };
        if active.instance_id != instance_id {
            return Err(RuntimeError::DisplaySessionBusy {
                instance_id: active.instance_id.clone(),
                generation: active.generation,
            });
        }
        if active.generation != generation {
            return Err(RuntimeError::GenerationMismatch {
                expected: active.generation,
                got: generation,
            });
        }
        self.ensure_running_generation(instance_id, generation)
    }

    pub fn active_display_session(&self) -> Option<DisplaySessionIdentity> {
        self.active_display_session.clone()
    }

    pub fn active_display_generation(
        &self,
        caller: &CallerIdentity,
        instance_id: &str,
    ) -> Result<Option<Generation>, RuntimeError> {
        self.authorize(caller)?;
        if !valid_instance_id(instance_id) {
            return Err(RuntimeError::InvalidInstanceId);
        }
        Ok(self
            .active_display_session
            .as_ref()
            .filter(|identity| identity.instance_id == instance_id)
            .map(|identity| identity.generation))
    }

    pub fn reconcile_runtime_died(
        &mut self,
        caller: &CallerIdentity,
        instance_id: &str,
        generation: Generation,
    ) -> Result<bool, RuntimeError> {
        self.authorize(caller)?;
        let Some(running) = self.running.get(instance_id) else {
            return Ok(false);
        };
        if running.generation != generation {
            return Ok(false);
        }

        if self.active_display_session.as_ref()
            == Some(&DisplaySessionIdentity {
                instance_id: instance_id.to_string(),
                generation,
            })
        {
            self.active_display_session = None;
        }
        self.running.remove(instance_id);
        self.minimum_generation.insert(
            instance_id.to_string(),
            Generation(generation.0.saturating_add(1)),
        );
        Ok(true)
    }

    pub(crate) fn prepare_graphical_stop(
        &self,
        caller: &CallerIdentity,
        identity: &DisplaySessionIdentity,
        request_id: u64,
    ) -> Result<runtime_orchestration::GraphicalStopPreparation, RuntimeError> {
        self.authorize(caller)?;
        let Some(running) = self.running.get(&identity.instance_id) else {
            if self.completed_stops.get(&identity.instance_id)
                == Some(&(identity.generation, request_id))
            {
                return Ok(runtime_orchestration::GraphicalStopPreparation::AlreadyCompleted);
            }
            return Err(RuntimeError::InstanceNotRunning);
        };
        if running.generation != identity.generation {
            return Err(RuntimeError::GenerationMismatch {
                expected: running.generation,
                got: identity.generation,
            });
        }
        self.authorize_display_session(caller, &identity.instance_id, identity.generation)?;
        Ok(runtime_orchestration::GraphicalStopPreparation::StopRuntime)
    }

    fn ensure_running_generation(
        &self,
        instance_id: &str,
        generation: Generation,
    ) -> Result<(), RuntimeError> {
        let Some(running) = self.running.get(instance_id) else {
            return Err(RuntimeError::InstanceNotRunning);
        };
        if running.generation != generation {
            return Err(RuntimeError::GenerationMismatch {
                expected: running.generation,
                got: generation,
            });
        }
        Ok(())
    }

    fn authorize(&self, caller: &CallerIdentity) -> Result<(), RuntimeError> {
        if caller.uid != self.policy.allowed_uid
            || caller.signing_digest != self.policy.allowed_signing_digest
        {
            Err(RuntimeError::UnauthorizedCaller)
        } else {
            Ok(())
        }
    }
}

fn valid_instance_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
