//! CVD1 guest kernel configuration checks.
//!
//! The host's near-native plan only pays off when the guest kernel has the matching drivers:
//! pinned vCPUs with mirrored capacities need guest DVFS (`CONFIG_CPUFREQ_VIRT`, which binds the
//! `qemu,virtual-cpufreq` device crosvm creates for `--virt-cpufreq-upstream`), gfxstream needs
//! virtio-gpu plus explicit sync, and free-page reporting needs the balloon driver. A guest that
//! silently lacks these would boot but run far from native, so release builds verify the config.

use std::collections::BTreeMap;

use crate::GuestOsFamily;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelOptionValue {
    BuiltIn,
    Module,
    NotSet,
    /// String or numeric values; only presence is meaningful to CVD1.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelRequirement {
    /// Must be `=y` (needed before any module can load, or a security LSM).
    BuiltIn,
    /// `=y` or `=m`.
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelRequirementClass {
    /// The CVD1 device contract cannot be realized without it.
    Contract,
    /// The guest boots without it, but not at near-native performance.
    Performance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelConfigGap {
    pub option: &'static str,
    pub requirement: KernelRequirement,
    pub class: KernelRequirementClass,
    pub actual: KernelOptionValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelConfigError {
    InvalidLine(usize),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KernelConfig {
    options: BTreeMap<String, KernelOptionValue>,
}

const COMMON: &[(&str, KernelRequirement, KernelRequirementClass)] = {
    use KernelRequirement::{BuiltIn, Enabled};
    use KernelRequirementClass::{Contract, Performance};
    &[
        ("CONFIG_ARM64", BuiltIn, Contract),
        ("CONFIG_PCI_HOST_GENERIC", BuiltIn, Contract),
        ("CONFIG_VIRTIO_PCI", Enabled, Contract),
        ("CONFIG_VIRTIO_BLK", Enabled, Contract),
        ("CONFIG_VIRTIO_CONSOLE", Enabled, Contract),
        ("CONFIG_VIRTIO_INPUT", Enabled, Contract),
        ("CONFIG_VIRTIO_NET", Enabled, Contract),
        ("CONFIG_VIRTIO_VSOCKETS", Enabled, Contract),
        ("CONFIG_SND_VIRTIO", Enabled, Contract),
        ("CONFIG_DRM_VIRTIO_GPU", Enabled, Contract),
        ("CONFIG_SYNC_FILE", BuiltIn, Contract),
        ("CONFIG_DMA_SHARED_BUFFER", BuiltIn, Contract),
        ("CONFIG_CPU_FREQ", BuiltIn, Performance),
        ("CONFIG_CPU_FREQ_GOV_SCHEDUTIL", BuiltIn, Performance),
        ("CONFIG_CPUFREQ_VIRT", Enabled, Performance),
        ("CONFIG_TRANSPARENT_HUGEPAGE", BuiltIn, Performance),
        ("CONFIG_VIRTIO_BALLOON", Enabled, Performance),
        ("CONFIG_PAGE_REPORTING", BuiltIn, Performance),
    ]
};

const ANDROID: &[(&str, KernelRequirement, KernelRequirementClass)] = &[(
    "CONFIG_SECURITY_SELINUX",
    KernelRequirement::BuiltIn,
    KernelRequirementClass::Contract,
)];

const UBUNTU_TOUCH: &[(&str, KernelRequirement, KernelRequirementClass)] = &[(
    "CONFIG_SECURITY_APPARMOR",
    KernelRequirement::BuiltIn,
    KernelRequirementClass::Contract,
)];

impl KernelConfig {
    /// Parses `.config` / defconfig-fragment syntax. Later lines override earlier ones, so
    /// fragments can be concatenated exactly like `merge_config.sh` applies them.
    pub fn parse(text: &str) -> Result<Self, KernelConfigError> {
        let mut config = Self::default();
        config.merge(text)?;
        Ok(config)
    }

    pub fn merge(&mut self, text: &str) -> Result<(), KernelConfigError> {
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("# ") {
                if let Some(name) = rest.strip_suffix(" is not set") {
                    if valid_name(name) {
                        self.options
                            .insert(name.to_string(), KernelOptionValue::NotSet);
                    }
                }
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            let Some((name, value)) = line.split_once('=') else {
                return Err(KernelConfigError::InvalidLine(index + 1));
            };
            if !valid_name(name) || value.is_empty() {
                return Err(KernelConfigError::InvalidLine(index + 1));
            }
            let value = match value {
                "y" => KernelOptionValue::BuiltIn,
                "m" => KernelOptionValue::Module,
                "n" => KernelOptionValue::NotSet,
                _ => KernelOptionValue::Other,
            };
            self.options.insert(name.to_string(), value);
        }
        Ok(())
    }

    pub fn value(&self, option: &str) -> KernelOptionValue {
        self.options
            .get(option)
            .copied()
            .unwrap_or(KernelOptionValue::NotSet)
    }

    /// Every CVD1 requirement this config does not meet, contract gaps first.
    pub fn cvd1_gaps(&self, family: &GuestOsFamily) -> Vec<KernelConfigGap> {
        let family_specific = match family {
            GuestOsFamily::Android => ANDROID,
            GuestOsFamily::UbuntuTouch => UBUNTU_TOUCH,
        };
        let mut gaps: Vec<KernelConfigGap> = COMMON
            .iter()
            .chain(family_specific)
            .filter_map(|(option, requirement, class)| {
                let actual = self.value(option);
                let met = match requirement {
                    KernelRequirement::BuiltIn => actual == KernelOptionValue::BuiltIn,
                    KernelRequirement::Enabled => matches!(
                        actual,
                        KernelOptionValue::BuiltIn | KernelOptionValue::Module
                    ),
                };
                (!met).then_some(KernelConfigGap {
                    option,
                    requirement: *requirement,
                    class: *class,
                    actual,
                })
            })
            .collect();
        gaps.sort_by_key(|gap| gap.class != KernelRequirementClass::Contract);
        gaps
    }
}

fn valid_name(name: &str) -> bool {
    name.strip_prefix("CONFIG_").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}
