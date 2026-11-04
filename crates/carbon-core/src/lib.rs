#![forbid(unsafe_code)]

mod admission;
mod backup;
mod bundle;
mod graphics;
mod guest;
mod kernel_config;
mod lifecycle;
mod model;
mod storage;
mod update;

pub use admission::{AdmissionDecision, AdmissionFailure, admit};
pub use backup::{BackupError, BackupManifest, CloneIdentityPlan, RestoreMode};
pub use bundle::{BundleArtifact, BundleError, BundleManifest, IntegrationProfile, ReleaseStatus};
pub use graphics::{
    CapabilityEvidence, GpuHostCapabilities, GpuIdentity, GpuQualificationTuple,
    GpuRendererProfile, GraphicsCapability, HostGraphicsApi, QualificationRecord, RendererFamily,
    RendererQualification, RendererSelection, RendererSelectionMode, qualification_applies,
    select_renderer,
};
pub use guest::{GuestContract, GuestContractError, GuestOsFamily};
pub use kernel_config::{
    KernelConfig, KernelConfigError, KernelConfigGap, KernelOptionValue, KernelRequirement,
    KernelRequirementClass,
};
pub use lifecycle::{LifecycleAction, LifecycleError, LifecycleMachine};
pub use model::{BundleRequirements, CpuArch, Generation, HostCapabilities, VmState};
pub use storage::{
    DiskLeaseError, DiskLeaseTable, DiskLeaseToken, InstallError, InstallRecovery,
    InstallTransaction,
};
pub use update::{UpdateDecision, UpdateError, UpdateTransaction};
