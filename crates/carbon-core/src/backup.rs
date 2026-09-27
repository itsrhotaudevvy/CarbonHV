use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupManifest {
    pub instance_id: String,
    pub generation: u64,
    pub machine_identity: String,
    pub data_object: String,
    pub system_hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupError {
    InvalidLine(usize),
    UnknownKey(String),
    MissingKey(String),
    DuplicateKey(String),
    InvalidGeneration,
    UnsafeObjectId(String),
    InvalidSystemHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreMode {
    Restore { target_instance_id: String },
    Clone { target_instance_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloneIdentityPlan {
    Retain(String),
    Regenerate {
        target_instance_id: String,
        regenerate_machine_id: bool,
        regenerate_network_identity: bool,
    },
}

impl BackupManifest {
    pub fn parse(text: &str) -> Result<Self, BackupError> {
        let mut singleton = BTreeMap::<String, String>::new();
        let mut hashes = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or(BackupError::InvalidLine(index + 1))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "system_hash" => hashes.push(value.to_ascii_lowercase()),
                "instance_id" | "generation" | "machine_identity" | "data_object" => {
                    if singleton.insert(key.into(), value.into()).is_some() {
                        return Err(BackupError::DuplicateKey(key.into()));
                    }
                }
                other => return Err(BackupError::UnknownKey(other.into())),
            }
        }

        let instance_id = required(&singleton, "instance_id")?;
        let generation = required(&singleton, "generation")?
            .parse::<u64>()
            .map_err(|_| BackupError::InvalidGeneration)?;
        let machine_identity = required(&singleton, "machine_identity")?;
        let data_object = required(&singleton, "data_object")?;
        for value in [&instance_id, &machine_identity, &data_object] {
            if !safe_id(value) {
                return Err(BackupError::UnsafeObjectId(value.clone()));
            }
        }
        if hashes.is_empty() || hashes.iter().any(|hash| !valid_sha256(hash)) {
            return Err(BackupError::InvalidSystemHash);
        }

        Ok(Self {
            instance_id,
            generation,
            machine_identity,
            data_object,
            system_hashes: hashes,
        })
    }

    pub fn encode(&self) -> String {
        let mut output = format!(
            "instance_id={}\ngeneration={}\nmachine_identity={}\ndata_object={}\n",
            self.instance_id, self.generation, self.machine_identity, self.data_object
        );
        for hash in &self.system_hashes {
            output.push_str("system_hash=");
            output.push_str(hash);
            output.push('\n');
        }
        output
    }

    pub fn identity_plan(&self, mode: RestoreMode) -> CloneIdentityPlan {
        match mode {
            RestoreMode::Restore { .. } => CloneIdentityPlan::Retain(self.machine_identity.clone()),
            RestoreMode::Clone { target_instance_id } => CloneIdentityPlan::Regenerate {
                target_instance_id,
                regenerate_machine_id: true,
                regenerate_network_identity: true,
            },
        }
    }
}

fn required(values: &BTreeMap<String, String>, key: &str) -> Result<String, BackupError> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| BackupError::MissingKey(key.into()))
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
