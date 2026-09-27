#![forbid(unsafe_code)]

use carbon_core::Generation;

pub const PROTOCOL_MAJOR: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerIdentity {
    pub uid: u32,
    pub signing_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManagedObjectId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectIdError {
    Empty,
    TooLong,
    InvalidCharacter,
    ReservedPathComponent,
}

impl ManagedObjectId {
    pub fn new(value: impl Into<String>) -> Result<Self, ObjectIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ObjectIdError::Empty);
        }
        if value.len() > 128 {
            return Err(ObjectIdError::TooLong);
        }
        if value == "." || value == ".." {
            return Err(ObjectIdError::ReservedPathComponent);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(ObjectIdError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmResources {
    pub memory_mib: u64,
    pub vcpus: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmDiskRequest {
    pub object_id: ManagedObjectId,
    pub writable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmStartRequest {
    pub request_id: u64,
    pub instance_id: String,
    pub generation: Generation,
    pub kernel: ManagedObjectId,
    pub initrd: ManagedObjectId,
    pub disks: Vec<VmDiskRequest>,
    pub resources: VmResources,
    pub kernel_params: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmStopRequest {
    pub request_id: u64,
    pub instance_id: String,
    pub generation: Generation,
}
