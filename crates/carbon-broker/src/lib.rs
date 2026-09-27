#![forbid(unsafe_code)]

mod avf;
mod backend;

use std::collections::BTreeMap;
use std::path::PathBuf;

use carbon_core::Generation;
use carbon_protocol::{CallerIdentity, VmStartRequest};

pub use avf::{AvfBackend, AvfLaunchPlan};
pub use backend::{BackendError, HostProbe, HostVirtualizationClass, ProgramInvocation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerPolicy {
    pub allowed_uid: u32,
    pub allowed_signing_digest: String,
    pub managed_root: PathBuf,
    pub max_memory_mib: u64,
    pub max_vcpus: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedVmSpec {
    pub request_id: u64,
    pub instance_id: String,
    pub generation: Generation,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub disks: Vec<PathBuf>,
    pub memory_mib: u64,
    pub vcpus: u16,
    pub kernel_params: Vec<String>,
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
    InstanceNotRunning,
    GenerationMismatch {
        expected: Generation,
        got: Generation,
    },
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
}

impl Broker {
    pub fn new(policy: BrokerPolicy) -> Self {
        Self {
            policy,
            minimum_generation: BTreeMap::new(),
            running: BTreeMap::new(),
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
                .map(|disk| self.policy.managed_root.join(disk.as_str()))
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

    pub fn stop(
        &mut self,
        caller: &CallerIdentity,
        instance_id: &str,
        generation: Generation,
        _request_id: u64,
    ) -> Result<(), RuntimeError> {
        self.authorize(caller)?;
        let running = self
            .running
            .get(instance_id)
            .ok_or(RuntimeError::InstanceNotRunning)?;
        if running.generation != generation {
            return Err(RuntimeError::GenerationMismatch {
                expected: running.generation,
                got: generation,
            });
        }
        self.running.remove(instance_id);
        self.minimum_generation.insert(
            instance_id.to_string(),
            Generation(generation.0.saturating_add(1)),
        );
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
