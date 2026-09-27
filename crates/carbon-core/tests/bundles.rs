use carbon_core::{BundleError, BundleManifest, IntegrationProfile, ReleaseStatus};

const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn valid_manifest() -> String {
    format!(
        r#"
id = "grapheneos"
display_name = "GrapheneOS (CarbonHV virtual-device build)"
version = "2026.09.27-dev"
publisher = "CarbonHV"
release_status = "development"
guest_arch = "arm64"
cvd_major = 1
memory_mib = 4096
storage_mib = 16384
requires_hardware_virtualization = true
requires_gpu_acceleration = true
backend = "avf"
integration_profile = "private"
artifact = "boot|out/grapheneos/boot.img|{SHA}"
artifact = "system|out/grapheneos/system.img|{SHA}"
"#
    )
}

#[test]
fn parses_valid_arm64_manifest() {
    let manifest = BundleManifest::parse(&valid_manifest()).unwrap();
    assert_eq!(manifest.id, "grapheneos");
    assert_eq!(manifest.release_status, ReleaseStatus::Development);
    assert_eq!(manifest.integration_profile, IntegrationProfile::Private);
    assert_eq!(manifest.artifacts.len(), 2);
    assert_eq!(manifest.requirements.memory_mib, 4096);
}

#[test]
fn rejects_artifact_path_traversal() {
    let bad = valid_manifest().replace("out/grapheneos/boot.img", "../../data/local/tmp/boot.img");
    assert_eq!(
        BundleManifest::parse(&bad),
        Err(BundleError::UnsafeArtifactPath)
    );
}

#[test]
fn rejects_duplicate_artifact_role() {
    let bad = valid_manifest().replace(
        "system|out/grapheneos/system.img",
        "boot|out/grapheneos/system.img",
    );
    assert_eq!(
        BundleManifest::parse(&bad),
        Err(BundleError::DuplicateArtifactRole("boot".into()))
    );
}

#[test]
fn rejects_resource_overflow() {
    let bad = valid_manifest().replace("memory_mib = 4096", "memory_mib = 18446744073709551616");
    assert_eq!(
        BundleManifest::parse(&bad),
        Err(BundleError::InvalidInteger("memory_mib".into()))
    );
}

#[test]
fn rejects_non_arm64_guest() {
    let bad = valid_manifest().replace("guest_arch = \"arm64\"", "guest_arch = \"x86_64\"");
    assert_eq!(
        BundleManifest::parse(&bad),
        Err(BundleError::UnsupportedArchitecture("x86_64".into()))
    );
}

#[test]
fn rejects_runtime_that_allows_cpu_emulation() {
    let bad = valid_manifest().replace("backend = \"avf\"", "backend = \"qemu-tcg\"");
    assert_eq!(
        BundleManifest::parse(&bad),
        Err(BundleError::UnsafeBackend("qemu-tcg".into()))
    );
}

#[test]
fn rejects_disabled_hardware_virtualization_requirement() {
    let bad = valid_manifest().replace(
        "requires_hardware_virtualization = true",
        "requires_hardware_virtualization = false",
    );
    assert_eq!(
        BundleManifest::parse(&bad),
        Err(BundleError::HardwareVirtualizationRequired)
    );
}

#[test]
fn rejects_unknown_critical_key() {
    let bad = format!("{}\nroot_shell = true\n", valid_manifest());
    assert_eq!(
        BundleManifest::parse(&bad),
        Err(BundleError::UnknownKey("root_shell".into()))
    );
}
