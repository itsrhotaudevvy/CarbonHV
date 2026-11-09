//! The app-side facts an emulated launch plan depends on.
//!
//! These are probe results, not policy: `carbonctl emu plan` reads them from a fixture, and the
//! `:vm` service fills the same struct from `EmulationProbe` on the phone. Unknown keys are
//! rejected so a typo in a fixture fails loudly instead of silently planning with a default.

use crate::json::{self, JsonError, Value};

/// Upper bound for one core index; beyond this the probe is wrong, not the phone.
const MAX_CORE_INDEX: u64 = 4096;
/// Upper bound for a believable `MemAvailable` reading (1 TiB).
const MAX_MEMORY_MIB: u64 = 1 << 20;
const MAX_CORES: usize = 256;

/// How the app may obtain writable-then-executable memory for TCG's translation cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WxMode {
    /// `memfd` double mapping (RW mapping + RX mapping of the same pages). Preferred: no page is
    /// ever writable and executable at the same time.
    SplitWx,
    /// Plain `PROT_READ|PROT_WRITE|PROT_EXEC` anonymous memory. Accepted only when the platform
    /// allows it, and recorded because it is a weaker guarantee than `SplitWx`.
    AnonymousRwx,
    /// Not probed yet. Planning refuses this rather than guessing.
    Unknown,
}

impl WxMode {
    pub fn as_str(self) -> &'static str {
        match self {
            WxMode::SplitWx => "split-wx",
            WxMode::AnonymousRwx => "anonymous-rwx",
            WxMode::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "split-wx" => Some(WxMode::SplitWx),
            "anonymous-rwx" => Some(WxMode::AnonymousRwx),
            "unknown" => Some(WxMode::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VulkanFacts {
    pub available: bool,
    /// Whether a `VkImage` can be exported as an `AHardwareBuffer` (zero-copy presentation).
    pub hardware_buffer_export: bool,
    pub device_name: Option<String>,
}

impl Default for VulkanFacts {
    fn default() -> Self {
        Self {
            available: false,
            hardware_buffer_export: false,
            device_name: None,
        }
    }
}

/// Everything the emulated engine needs to know about *this* install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmuHostFacts {
    /// Live `MemAvailable`, not `MemTotal`.
    pub mem_available_mib: u64,
    /// Host CPUs that may run a vCPU thread, fastest first. Efficiency cores are excluded by
    /// `MIDR_EL1` in the broker's probe, not by this crate.
    pub big_cores: Vec<u32>,
    pub w_xor_x: WxMode,
    pub o_direct: bool,
    pub vulkan: VulkanFacts,
    pub affinity_allowed: bool,
    pub uclamp_allowed: bool,
    /// Optional provenance note from the probe (`adb:...`, `fixture:...`). Diagnostics only.
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactsError {
    Json(JsonError),
    MissingField(&'static str),
    UnknownField(String),
    InvalidField(&'static str),
}

impl From<JsonError> for FactsError {
    fn from(error: JsonError) -> Self {
        FactsError::Json(error)
    }
}

impl EmuHostFacts {
    /// Parses the `carbonctl emu plan --facts` fixture format.
    pub fn parse_json(document: &str) -> Result<Self, FactsError> {
        let root = json::parse(document)?;
        let Value::Object(entries) = &root else {
            return Err(FactsError::InvalidField("root"));
        };
        for (key, _) in entries {
            match key.as_str() {
                "source" | "mem_available_mib" | "big_cores" | "w_xor_x" | "o_direct"
                | "vulkan" | "affinity_allowed" | "uclamp_allowed" => {}
                other => return Err(FactsError::UnknownField(other.to_owned())),
            }
        }
        let mem_available_mib = required(&root, "mem_available_mib")?
            .as_u64()
            .filter(|value| *value > 0 && *value <= MAX_MEMORY_MIB)
            .ok_or(FactsError::InvalidField("mem_available_mib"))?;
        let big_cores = required(&root, "big_cores")?
            .as_array()
            .ok_or(FactsError::InvalidField("big_cores"))?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .filter(|core| *core < MAX_CORE_INDEX)
                    .map(|core| core as u32)
                    .ok_or(FactsError::InvalidField("big_cores"))
            })
            .collect::<Result<Vec<u32>, FactsError>>()?;
        if big_cores.is_empty() || big_cores.len() > MAX_CORES {
            return Err(FactsError::InvalidField("big_cores"));
        }
        if big_cores.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(FactsError::InvalidField("big_cores"));
        }
        let w_xor_x = required(&root, "w_xor_x")?
            .as_str()
            .and_then(WxMode::parse)
            .ok_or(FactsError::InvalidField("w_xor_x"))?;
        let o_direct = required(&root, "o_direct")?
            .as_bool()
            .ok_or(FactsError::InvalidField("o_direct"))?;
        let affinity_allowed = required(&root, "affinity_allowed")?
            .as_bool()
            .ok_or(FactsError::InvalidField("affinity_allowed"))?;
        let uclamp_allowed = required(&root, "uclamp_allowed")?
            .as_bool()
            .ok_or(FactsError::InvalidField("uclamp_allowed"))?;
        let vulkan = parse_vulkan(required(&root, "vulkan")?)?;
        let source = match root.get("source") {
            None => None,
            Some(value) => Some(
                value
                    .as_str()
                    .ok_or(FactsError::InvalidField("source"))?
                    .to_owned(),
            ),
        };
        Ok(Self {
            mem_available_mib,
            big_cores,
            w_xor_x,
            o_direct,
            vulkan,
            affinity_allowed,
            uclamp_allowed,
            source,
        })
    }

    /// Human-readable provenance for `carbonctl emu plan` headers.
    pub fn describe(&self) -> String {
        let mut text = format!(
            "{} MiB available, {} performance core(s) [{}], w^x={}, O_DIRECT={}, affinity={}, uclamp={}, Vulkan={}",
            self.mem_available_mib,
            self.big_cores.len(),
            self.big_cores
                .iter()
                .map(|core| core.to_string())
                .collect::<Vec<_>>()
                .join(","),
            self.w_xor_x.as_str(),
            if self.o_direct { "yes" } else { "no" },
            if self.affinity_allowed { "yes" } else { "no" },
            if self.uclamp_allowed { "yes" } else { "no" },
            match (&self.vulkan.device_name, self.vulkan.available) {
                (_, false) => "absent",
                (Some(name), true) => name.as_str(),
                (None, true) => "present",
            }
        );
        if let Some(source) = &self.source {
            text.push_str(&format!(" ({source})"));
        }
        text
    }
}

fn required<'a>(root: &'a Value, key: &'static str) -> Result<&'a Value, FactsError> {
    root.get(key).ok_or(FactsError::MissingField(key))
}

fn parse_vulkan(value: &Value) -> Result<VulkanFacts, FactsError> {
    let Value::Object(entries) = value else {
        return Err(FactsError::InvalidField("vulkan"));
    };
    for (key, _) in entries {
        match key.as_str() {
            "available" | "hardware_buffer_export" | "device_name" => {}
            other => return Err(FactsError::UnknownField(format!("vulkan.{other}"))),
        }
    }
    let available = value
        .get("available")
        .and_then(Value::as_bool)
        .ok_or(FactsError::MissingField("vulkan.available"))?;
    let hardware_buffer_export = value
        .get("hardware_buffer_export")
        .and_then(Value::as_bool)
        .ok_or(FactsError::MissingField("vulkan.hardware_buffer_export"))?;
    if hardware_buffer_export && !available {
        return Err(FactsError::InvalidField("vulkan.hardware_buffer_export"));
    }
    let device_name = match value.get("device_name") {
        None | Some(Value::Null) => None,
        Some(Value::String(name)) if !name.is_empty() && name.len() <= 256 => Some(name.clone()),
        Some(_) => return Err(FactsError::InvalidField("vulkan.device_name")),
    };
    Ok(VulkanFacts {
        available,
        hardware_buffer_export,
        device_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KUNZITE: &str = include_str!("../../../qualification/emu/fixtures/kunzite.json");

    #[test]
    fn parses_the_kunzite_fixture() {
        let facts = EmuHostFacts::parse_json(KUNZITE).unwrap();
        assert_eq!(facts.mem_available_mib, 2750);
        assert_eq!(facts.big_cores, vec![4, 5, 6, 7]);
        assert_eq!(facts.w_xor_x, WxMode::SplitWx);
        assert!(facts.vulkan.available);
        assert!(facts.vulkan.hardware_buffer_export);
        assert_eq!(facts.vulkan.device_name.as_deref(), Some("Adreno (TM) 710"));
    }

    #[test]
    fn rejects_unknown_and_missing_fields() {
        let document = KUNZITE.replace("\"o_direct\"", "\"o_direkt\"");
        assert_eq!(
            EmuHostFacts::parse_json(&document),
            Err(FactsError::UnknownField("o_direkt".into()))
        );
        let document = KUNZITE.replace("\"uclamp_allowed\"", "\"unrelated\"");
        assert!(matches!(
            EmuHostFacts::parse_json(&document),
            Err(FactsError::UnknownField(_)) | Err(FactsError::MissingField("uclamp_allowed"))
        ));
    }

    #[test]
    fn rejects_unordered_cores_and_export_without_vulkan() {
        let document = KUNZITE.replace("[4, 5, 6, 7]", "[4, 6, 5, 7]");
        assert_eq!(
            EmuHostFacts::parse_json(&document),
            Err(FactsError::InvalidField("big_cores"))
        );
        let document = KUNZITE.replace("\"available\": true", "\"available\": false");
        assert_eq!(
            EmuHostFacts::parse_json(&document),
            Err(FactsError::InvalidField("vulkan.hardware_buffer_export"))
        );
    }
}
