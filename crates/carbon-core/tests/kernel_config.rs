use std::fs;
use std::path::Path;

use carbon_core::{
    GuestOsFamily, KernelConfig, KernelConfigError, KernelOptionValue, KernelRequirementClass,
};

fn repo_file(relative: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative),
    )
    .unwrap()
}

/// What any arm64 defconfig already provides; the CVD1 fragments are applied on top.
const ARM64_BASE: &str = "CONFIG_ARM64=y\n";

fn shipped(family_fragment: &str) -> KernelConfig {
    let mut config = KernelConfig::parse(ARM64_BASE).unwrap();
    config
        .merge(&repo_file("guest/common/kernel/cvd1-near-native.fragment"))
        .unwrap();
    config.merge(&repo_file(family_fragment)).unwrap();
    config
}

#[test]
fn shipped_fragments_satisfy_both_starter_families() {
    let android = shipped("guest/android/device/kernel.fragment");
    assert_eq!(android.cvd1_gaps(&GuestOsFamily::Android), vec![]);
    let ubuntu = shipped("guest/ubuntu-touch/device/kernel.fragment");
    assert_eq!(ubuntu.cvd1_gaps(&GuestOsFamily::UbuntuTouch), vec![]);
}

#[test]
fn family_security_modules_are_not_interchangeable() {
    let android = shipped("guest/android/device/kernel.fragment");
    let gaps = android.cvd1_gaps(&GuestOsFamily::UbuntuTouch);
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].option, "CONFIG_SECURITY_APPARMOR");
    assert_eq!(gaps[0].class, KernelRequirementClass::Contract);
}

#[test]
fn later_fragments_override_earlier_values_like_merge_config() {
    let mut config = KernelConfig::parse("CONFIG_CPUFREQ_VIRT=y\n").unwrap();
    config.merge("# CONFIG_CPUFREQ_VIRT is not set\n").unwrap();
    assert_eq!(
        config.value("CONFIG_CPUFREQ_VIRT"),
        KernelOptionValue::NotSet
    );
    config.merge("CONFIG_CPUFREQ_VIRT=m\n").unwrap();
    assert_eq!(
        config.value("CONFIG_CPUFREQ_VIRT"),
        KernelOptionValue::Module
    );
}

#[test]
fn missing_virtual_cpufreq_is_a_performance_gap_not_a_contract_gap() {
    let mut config = shipped("guest/ubuntu-touch/device/kernel.fragment");
    config.merge("# CONFIG_CPUFREQ_VIRT is not set\n").unwrap();
    let gaps = config.cvd1_gaps(&GuestOsFamily::UbuntuTouch);
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].option, "CONFIG_CPUFREQ_VIRT");
    assert_eq!(gaps[0].class, KernelRequirementClass::Performance);
}

#[test]
fn modules_cannot_stand_in_for_built_in_requirements() {
    let mut config = shipped("guest/android/device/kernel.fragment");
    config.merge("CONFIG_SYNC_FILE=m\n").unwrap();
    let gaps = config.cvd1_gaps(&GuestOsFamily::Android);
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].option, "CONFIG_SYNC_FILE");
    assert_eq!(gaps[0].actual, KernelOptionValue::Module);
}

#[test]
fn contract_gaps_are_reported_before_performance_gaps() {
    let gaps = KernelConfig::parse(ARM64_BASE)
        .unwrap()
        .cvd1_gaps(&GuestOsFamily::Android);
    let first_performance = gaps
        .iter()
        .position(|gap| gap.class == KernelRequirementClass::Performance)
        .unwrap();
    assert!(
        gaps[first_performance..]
            .iter()
            .all(|gap| gap.class == KernelRequirementClass::Performance)
    );
    assert!(first_performance > 0);
}

#[test]
fn malformed_lines_are_rejected_with_their_line_number() {
    assert_eq!(
        KernelConfig::parse("CONFIG_ARM64=y\nrm -rf /\n"),
        Err(KernelConfigError::InvalidLine(2))
    );
    assert_eq!(
        KernelConfig::parse("NOT_A_CONFIG=y\n"),
        Err(KernelConfigError::InvalidLine(1))
    );
    // Comments, blanks and string values are accepted.
    let config =
        KernelConfig::parse("# comment\n\nCONFIG_CMDLINE=\"console=hvc0\"\nCONFIG_HZ=250\n")
            .unwrap();
    assert_eq!(config.value("CONFIG_HZ"), KernelOptionValue::Other);
}
