use std::collections::BTreeMap;

use crate::Generation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskLeaseToken {
    pub disk_id: String,
    pub owner_instance: String,
    pub generation: Generation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskLeaseError {
    AlreadyLeased {
        disk_id: String,
        owner_instance: String,
    },
    StaleToken,
}

#[derive(Debug, Default)]
pub struct DiskLeaseTable {
    leases: BTreeMap<String, DiskLeaseToken>,
}

impl DiskLeaseTable {
    pub fn acquire(
        &mut self,
        disk_id: impl Into<String>,
        owner_instance: impl Into<String>,
        generation: Generation,
    ) -> Result<DiskLeaseToken, DiskLeaseError> {
        let disk_id = disk_id.into();
        let owner_instance = owner_instance.into();
        if let Some(existing) = self.leases.get(&disk_id) {
            if existing.owner_instance == owner_instance && existing.generation == generation {
                return Ok(existing.clone());
            }
            return Err(DiskLeaseError::AlreadyLeased {
                disk_id,
                owner_instance: existing.owner_instance.clone(),
            });
        }

        let token = DiskLeaseToken {
            disk_id: disk_id.clone(),
            owner_instance,
            generation,
        };
        self.leases.insert(disk_id, token.clone());
        Ok(token)
    }

    pub fn release(&mut self, token: &DiskLeaseToken) -> Result<(), DiskLeaseError> {
        match self.leases.get(&token.disk_id) {
            Some(current) if current == token => {
                self.leases.remove(&token.disk_id);
                Ok(())
            }
            _ => Err(DiskLeaseError::StaleToken),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallRecovery {
    UsePrevious(String),
    UsePublished(String),
    NoUsableVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    NothingVerified,
    AlreadyPublished,
}

#[derive(Debug, Clone)]
pub struct InstallTransaction {
    bundle_id: String,
    previous_version: Option<String>,
    staged_version: Option<String>,
    published: bool,
}

impl InstallTransaction {
    pub fn begin(bundle_id: impl Into<String>, previous_version: Option<impl Into<String>>) -> Self {
        Self {
            bundle_id: bundle_id.into(),
            previous_version: previous_version.map(Into::into),
            staged_version: None,
            published: false,
        }
    }

    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }

    pub fn stage_verified(&mut self, version: impl Into<String>) -> Result<(), InstallError> {
        if self.published {
            return Err(InstallError::AlreadyPublished);
        }
        self.staged_version = Some(version.into());
        Ok(())
    }

    pub fn publish(&mut self) -> Result<(), InstallError> {
        if self.staged_version.is_none() {
            return Err(InstallError::NothingVerified);
        }
        self.published = true;
        Ok(())
    }

    pub fn recover_after_interrupt(&self) -> InstallRecovery {
        if self.published {
            if let Some(version) = &self.staged_version {
                return InstallRecovery::UsePublished(version.clone());
            }
        }
        if let Some(version) = &self.previous_version {
            return InstallRecovery::UsePrevious(version.clone());
        }
        InstallRecovery::NoUsableVersion
    }
}

