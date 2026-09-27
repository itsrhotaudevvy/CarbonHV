use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use crate::{BundleRequirements, CpuArch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseStatus {
    Development,
    Experimental,
    Beta,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationProfile {
    Private,
    Balanced,
    Integrated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleArtifact {
    pub role: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleManifest {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub publisher: String,
    pub release_status: ReleaseStatus,
    pub cvd_major: u32,
    pub backend: String,
    pub integration_profile: IntegrationProfile,
    pub requirements: BundleRequirements,
    pub artifacts: Vec<BundleArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    InvalidLine(usize),
    UnknownKey(String),
    MissingKey(String),
    DuplicateKey(String),
    InvalidInteger(String),
    InvalidBoolean(String),
    InvalidEnum { key: String, value: String },
    UnsupportedArchitecture(String),
    UnsafeBackend(String),
    HardwareVirtualizationRequired,
    UnsafeArtifactPath,
    InvalidArtifact,
    DuplicateArtifactRole(String),
    InvalidSha256,
}

impl BundleManifest {
    pub fn parse(text: &str) -> Result<Self, BundleError> {
        let mut values = BTreeMap::<String, String>::new();
        let mut artifacts = Vec::<BundleArtifact>::new();
        let allowed = BTreeSet::from([
            "id",
            "display_name",
            "version",
            "publisher",
            "release_status",
            "guest_arch",
            "cvd_major",
            "memory_mib",
            "storage_mib",
            "requires_hardware_virtualization",
            "requires_gpu_acceleration",
            "backend",
            "integration_profile",
            "artifact",
        ]);

        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, raw_value) = line
                .split_once('=')
                .ok_or(BundleError::InvalidLine(index + 1))?;
            let key = key.trim();
            if !allowed.contains(key) {
                return Err(BundleError::UnknownKey(key.to_string()));
            }
            let value = parse_value(raw_value.trim()).ok_or(BundleError::InvalidLine(index + 1))?;
            if key == "artifact" {
                artifacts.push(parse_artifact(&value)?);
            } else if values.insert(key.to_string(), value).is_some() {
                return Err(BundleError::DuplicateKey(key.to_string()));
            }
        }

        let id = required(&values, "id")?;
        let display_name = required(&values, "display_name")?;
        let version = required(&values, "version")?;
        let publisher = required(&values, "publisher")?;
        let release_status = match required(&values, "release_status")?.as_str() {
            "development" => ReleaseStatus::Development,
            "experimental" => ReleaseStatus::Experimental,
            "beta" => ReleaseStatus::Beta,
            "stable" => ReleaseStatus::Stable,
            other => {
                return Err(BundleError::InvalidEnum {
                    key: "release_status".into(),
                    value: other.into(),
                });
            }
        };

        let arch_value = required(&values, "guest_arch")?;
        let arch = match arch_value.as_str() {
            "arm64" => CpuArch::Arm64,
            other => return Err(BundleError::UnsupportedArchitecture(other.into())),
        };

        let cvd_major = parse_u32(&values, "cvd_major")?;
        let memory_mib = parse_u64(&values, "memory_mib")?;
        let storage_mib = parse_u64(&values, "storage_mib")?;
        let requires_hardware_virtualization =
            parse_bool(&values, "requires_hardware_virtualization")?;
        let requires_gpu_acceleration = parse_bool(&values, "requires_gpu_acceleration")?;
        if !requires_hardware_virtualization {
            return Err(BundleError::HardwareVirtualizationRequired);
        }

        let backend = required(&values, "backend")?;
        if backend != "avf" && backend != "kvm" {
            return Err(BundleError::UnsafeBackend(backend));
        }

        let integration_profile = match required(&values, "integration_profile")?.as_str() {
            "private" => IntegrationProfile::Private,
            "balanced" => IntegrationProfile::Balanced,
            "integrated" => IntegrationProfile::Integrated,
            other => {
                return Err(BundleError::InvalidEnum {
                    key: "integration_profile".into(),
                    value: other.into(),
                });
            }
        };

        let mut roles = BTreeSet::new();
        for artifact in &artifacts {
            if !roles.insert(artifact.role.clone()) {
                return Err(BundleError::DuplicateArtifactRole(artifact.role.clone()));
            }
        }

        Ok(Self {
            id,
            display_name,
            version,
            publisher,
            release_status,
            cvd_major,
            backend,
            integration_profile,
            requirements: BundleRequirements {
                arch,
                requires_hardware_virtualization,
                requires_gpu_acceleration,
                memory_mib,
                storage_mib,
            },
            artifacts,
        })
    }
}

fn parse_value(raw: &str) -> Option<String> {
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        Some(raw[1..raw.len() - 1].to_string())
    } else if !raw.contains(char::is_whitespace) {
        Some(raw.to_string())
    } else {
        None
    }
}

fn required(values: &BTreeMap<String, String>, key: &str) -> Result<String, BundleError> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| BundleError::MissingKey(key.to_string()))
}

fn parse_u64(values: &BTreeMap<String, String>, key: &str) -> Result<u64, BundleError> {
    required(values, key)?
        .parse::<u64>()
        .map_err(|_| BundleError::InvalidInteger(key.to_string()))
}

fn parse_u32(values: &BTreeMap<String, String>, key: &str) -> Result<u32, BundleError> {
    required(values, key)?
        .parse::<u32>()
        .map_err(|_| BundleError::InvalidInteger(key.to_string()))
}

fn parse_bool(values: &BTreeMap<String, String>, key: &str) -> Result<bool, BundleError> {
    match required(values, key)?.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(BundleError::InvalidBoolean(key.to_string())),
    }
}

fn parse_artifact(value: &str) -> Result<BundleArtifact, BundleError> {
    let mut parts = value.split('|');
    let role = parts.next().unwrap_or_default().trim();
    let path = parts.next().unwrap_or_default().trim();
    let sha256 = parts.next().unwrap_or_default().trim();
    if role.is_empty() || path.is_empty() || sha256.is_empty() || parts.next().is_some() {
        return Err(BundleError::InvalidArtifact);
    }
    let p = Path::new(path);
    if p.is_absolute()
        || p.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(BundleError::UnsafeArtifactPath);
    }
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(BundleError::InvalidSha256);
    }
    Ok(BundleArtifact {
        role: role.to_string(),
        path: path.to_string(),
        sha256: sha256.to_ascii_lowercase(),
    })
}
