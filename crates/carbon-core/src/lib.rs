#![forbid(unsafe_code)]

mod admission;
mod backup;
mod bundle;
mod guest;
mod lifecycle;
mod model;
mod storage;
mod update;

pub use admission::{AdmissionDecision, AdmissionFailure, admit};
pub use backup::{BackupError, BackupManifest, CloneIdentityPlan, RestoreMode};
pub use bundle::{
    BundleArtifact, BundleError, BundleManifest, IntegrationProfile, ReleaseStatus,
};
pub use guest::{GuestContract, GuestContractError, GuestOsFamily};
pub use lifecycle::{LifecycleAction, LifecycleError, LifecycleMachine};
pub use model::{BundleRequirements, CpuArch, Generation, HostCapabilities, VmState};
pub use storage::{
    DiskLeaseError, DiskLeaseTable, DiskLeaseToken, InstallError, InstallRecovery,
    InstallTransaction,
};
pub use update::{UpdateDecision, UpdateError, UpdateTransaction};
