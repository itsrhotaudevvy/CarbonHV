#![forbid(unsafe_code)]

mod admission;
mod lifecycle;
mod model;
mod storage;

pub use admission::{AdmissionDecision, AdmissionFailure, admit};
pub use lifecycle::{LifecycleAction, LifecycleError, LifecycleMachine};
pub use model::{BundleRequirements, CpuArch, Generation, HostCapabilities, VmState};
pub use storage::{
    DiskLeaseError, DiskLeaseTable, DiskLeaseToken, InstallError, InstallRecovery,
    InstallTransaction,
};
