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
    rollback_version: Option<String>,
    rollback_data_epoch: Option<u64>,
    candidate: Option<CandidateUpdate>,
    committed: bool,
}

impl UpdateTransaction {
    pub fn begin(current_version: impl Into<String>, current_data_epoch: u64) -> Self {
        Self {
            current_version: current_version.into(),
            current_data_epoch,
            rollback_version: None,
            rollback_data_epoch: None,
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
        self.rollback_version = Some(self.current_version.clone());
        self.rollback_data_epoch = Some(self.current_data_epoch);
        self.current_version = candidate.version.clone();
        self.current_data_epoch = candidate.data_epoch;
        self.committed = true;
        Ok(())
    }

    pub fn rollback_decision(&self) -> Result<UpdateDecision, UpdateError> {
        let candidate = self.candidate.as_ref().ok_or(UpdateError::NoCandidate)?;
        let (rollback_version, rollback_epoch) = if self.committed {
            (
                self.rollback_version
                    .as_ref()
                    .expect("committed update must retain rollback version"),
                self.rollback_data_epoch
                    .expect("committed update must retain rollback data epoch"),
            )
        } else {
            (&self.current_version, self.current_data_epoch)
        };
        if candidate.data_epoch > rollback_epoch {
            return Err(UpdateError::DataMigrationPreventsRollback {
                current_epoch: rollback_epoch,
                candidate_epoch: candidate.data_epoch,
            });
        }
        Ok(UpdateDecision::RollbackTo(rollback_version.clone()))
    }

    pub fn recover_after_interrupt(&self) -> UpdateDecision {
        UpdateDecision::BootCurrent(self.current_version.clone())
    }
}
