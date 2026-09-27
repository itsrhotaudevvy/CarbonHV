#![forbid(unsafe_code)]

mod admission;
mod model;

pub use admission::{AdmissionDecision, AdmissionFailure, admit};
pub use model::{BundleRequirements, CpuArch, Generation, HostCapabilities, VmState};
