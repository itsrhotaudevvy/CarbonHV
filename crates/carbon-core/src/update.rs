#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    BootCurrent(String),
    RollbackTo(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    NoCandidate,
    CandidateNotHealthy,
    AlreadyCommitted,
    DataMigrationPreventsRollback {
        current_epoch: u64,
        candidate_epoch: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CandidateUpdate {
    version: String,
    data_epoch: u64,
    healthy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateTransaction {
    current_version: String,
    current_data_epoch: u64,
    candidate: Option<CandidateUpdate>,
    committed: bool,
}

impl UpdateTransaction {
    pub fn begin(current_version: impl Into<String>, current_data_epoch: u64) -> Self {
        Self {
            current_version: current_version.into(),
            current_data_epoch,
            candidate: None,
            committed: false,
        }
    }

    pub fn stage_verified(
        &mut self,
        version: impl Into<String>,
        candidate_data_epoch: u64,
    ) -> Result<(), UpdateError> {
        if self.committed {
            return Err(UpdateError::AlreadyCommitted);
        }
        self.candidate = Some(CandidateUpdate {
            version: version.into(),
            data_epoch: candidate_data_epoch,
            healthy: false,
        });
        Ok(())
    }

    pub fn mark_candidate_healthy(&mut self) -> Result<(), UpdateError> {
        let candidate = self.candidate.as_mut().ok_or(UpdateError::NoCandidate)?;
        candidate.healthy = true;
        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), UpdateError> {
        if self.committed {
            return Err(UpdateError::AlreadyCommitted);
        }
        let candidate = self.candidate.as_ref().ok_or(UpdateError::NoCandidate)?;
        if !candidate.healthy {
            return Err(UpdateError::CandidateNotHealthy);
        }
        self.current_version = candidate.version.clone();
        self.current_data_epoch = candidate.data_epoch;
        self.committed = true;
        Ok(())
    }

    pub fn rollback_decision(&self) -> Result<UpdateDecision, UpdateError> {
        let candidate = self.candidate.as_ref().ok_or(UpdateError::NoCandidate)?;
        if candidate.data_epoch > self.current_data_epoch {
            return Err(UpdateError::DataMigrationPreventsRollback {
                current_epoch: self.current_data_epoch,
                candidate_epoch: candidate.data_epoch,
            });
        }
        Ok(UpdateDecision::RollbackTo(self.current_version.clone()))
    }

    pub fn recover_after_interrupt(&self) -> UpdateDecision {
        UpdateDecision::BootCurrent(self.current_version.clone())
    }
}
