use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestOsFamily {
    Android,
    UbuntuTouch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestContract {
    pub id: String,
    pub os_family: GuestOsFamily,
    pub cvd_major: u32,
    pub agent_protocol_major: u32,
    pub display: String,
    pub input: String,
    pub network: String,
    pub block: String,
    pub audio: String,
    pub vsock: bool,
    pub required_capabilities: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestContractError {
    InvalidLine(usize),
    MissingKey(String),
    DuplicateKey(String),
    UnknownKey(String),
    InvalidInteger(String),
    InvalidBoolean(String),
    InvalidFamily(String),
    UnsupportedArchitecture(String),
    UnsupportedDevice { key: String, value: String },
    CvdMajorMismatch { expected: u32, got: u32 },
    VsockRequired,
}

impl GuestContract {
    pub fn parse(text: &str, expected_cvd_major: u32) -> Result<Self, GuestContractError> {
        let allowed = BTreeSet::from([
            "id",
            "os_family",
            "arch",
            "cvd_major",
            "agent_protocol_major",
            "display",
            "input",
            "network",
            "block",
            "audio",
            "vsock",
            "required_capability",
        ]);
        let mut values = BTreeMap::<String, String>::new();
        let mut required_capabilities = BTreeSet::new();

        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, raw_value) = line
                .split_once('=')
                .ok_or(GuestContractError::InvalidLine(index + 1))?;
            let key = key.trim();
            if !allowed.contains(key) {
                return Err(GuestContractError::UnknownKey(key.to_string()));
            }
            let value = parse_value(raw_value.trim())
                .ok_or(GuestContractError::InvalidLine(index + 1))?;
            if key == "required_capability" {
                required_capabilities.insert(value);
            } else if values.insert(key.to_string(), value).is_some() {
                return Err(GuestContractError::DuplicateKey(key.to_string()));
            }
        }

        let id = required(&values, "id")?;
        let os_family = match required(&values, "os_family")?.as_str() {
            "android" => GuestOsFamily::Android,
            "ubuntu-touch" => GuestOsFamily::UbuntuTouch,
            other => return Err(GuestContractError::InvalidFamily(other.to_string())),
        };
        let arch = required(&values, "arch")?;
        if arch != "arm64" {
            return Err(GuestContractError::UnsupportedArchitecture(arch));
        }
        let cvd_major = parse_u32(&values, "cvd_major")?;
        if cvd_major != expected_cvd_major {
            return Err(GuestContractError::CvdMajorMismatch {
                expected: expected_cvd_major,
                got: cvd_major,
            });
        }
        let agent_protocol_major = parse_u32(&values, "agent_protocol_major")?;
        let display = required(&values, "display")?;
        let input = required(&values, "input")?;
        let network = required(&values, "network")?;
        let block = required(&values, "block")?;
        let audio = required(&values, "audio")?;
        let vsock = parse_bool(&values, "vsock")?;

        require_device("display", &display, "virtio-gpu")?;
        require_device("input", &input, "virtio-input")?;
        require_device("network", &network, "virtio-net")?;
        require_device("block", &block, "virtio-blk")?;
        require_device("audio", &audio, "virtio-snd")?;
        if !vsock {
            return Err(GuestContractError::VsockRequired);
        }

        Ok(Self {
            id,
            os_family,
            cvd_major,
            agent_protocol_major,
            display,
            input,
            network,
            block,
            audio,
            vsock,
            required_capabilities,
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

fn required(values: &BTreeMap<String, String>, key: &str) -> Result<String, GuestContractError> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| GuestContractError::MissingKey(key.to_string()))
}

fn parse_u32(values: &BTreeMap<String, String>, key: &str) -> Result<u32, GuestContractError> {
    required(values, key)?
        .parse::<u32>()
        .map_err(|_| GuestContractError::InvalidInteger(key.to_string()))
}

fn parse_bool(values: &BTreeMap<String, String>, key: &str) -> Result<bool, GuestContractError> {
    match required(values, key)?.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(GuestContractError::InvalidBoolean(key.to_string())),
    }
}

fn require_device(key: &str, value: &str, expected: &str) -> Result<(), GuestContractError> {
    if value == expected {
        Ok(())
    } else {
        Err(GuestContractError::UnsupportedDevice {
            key: key.to_string(),
            value: value.to_string(),
        })
    }
}

