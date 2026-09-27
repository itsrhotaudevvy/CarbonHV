use carbon_core::{GuestContract, GuestContractError, GuestOsFamily};

fn valid_android_contract() -> &'static str {
    r#"
id = "grapheneos-cvd1"
os_family = "android"
arch = "arm64"
cvd_major = 1
agent_protocol_major = 1
display = "virtio-gpu"
input = "virtio-input"
network = "virtio-net"
block = "virtio-blk"
audio = "virtio-snd"
vsock = true
required_capability = "gpu"
required_capability = "audio-output"
"#
}

#[test]
fn parses_cvd1_android_contract() {
    let contract = GuestContract::parse(valid_android_contract(), 1).unwrap();
    assert_eq!(contract.os_family, GuestOsFamily::Android);
    assert_eq!(contract.cvd_major, 1);
    assert!(contract.required_capabilities.contains("gpu"));
}

#[test]
fn rejects_incompatible_cvd_major() {
    let bad = valid_android_contract().replace("cvd_major = 1", "cvd_major = 2");
    assert_eq!(
        GuestContract::parse(&bad, 1),
        Err(GuestContractError::CvdMajorMismatch { expected: 1, got: 2 }),
    );
}

#[test]
fn rejects_missing_required_virtual_gpu() {
    let bad = valid_android_contract().replace("display = \"virtio-gpu\"\n", "");
    assert_eq!(
        GuestContract::parse(&bad, 1),
        Err(GuestContractError::MissingKey("display".into())),
    );
}

#[test]
fn rejects_non_arm64_contract() {
    let bad = valid_android_contract().replace("arch = \"arm64\"", "arch = \"x86_64\"");
    assert_eq!(
        GuestContract::parse(&bad, 1),
        Err(GuestContractError::UnsupportedArchitecture("x86_64".into())),
    );
}

#[test]
fn rejects_unknown_virtual_device_backend() {
    let bad = valid_android_contract().replace("display = \"virtio-gpu\"", "display = \"physical-gpu\"");
    assert_eq!(
        GuestContract::parse(&bad, 1),
        Err(GuestContractError::UnsupportedDevice {
            key: "display".into(),
            value: "physical-gpu".into(),
        }),
    );
}

#[test]
fn ubuntu_touch_contract_is_distinct_family() {
    let ubuntu = valid_android_contract()
        .replace("grapheneos-cvd1", "ubuntu-touch-cvd1")
        .replace("os_family = \"android\"", "os_family = \"ubuntu-touch\"");
    let contract = GuestContract::parse(&ubuntu, 1).unwrap();
    assert_eq!(contract.os_family, GuestOsFamily::UbuntuTouch);
}
