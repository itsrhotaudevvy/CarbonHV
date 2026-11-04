use carbon_broker::{StarterBundleError, resolve_starter_bundle_request};
use carbon_core::Generation;

#[test]
fn grapheneos_profile_resolves_only_broker_owned_managed_objects() {
    let request =
        resolve_starter_bundle_request("grapheneos", "daily-driver", Generation(7), 1001, 4096, 4)
            .unwrap();

    assert_eq!(request.request_id, 1001);
    assert_eq!(request.instance_id, "daily-driver");
    assert_eq!(request.generation, Generation(7));
    assert_eq!(request.kernel.as_str(), "grapheneos-kernel.img");
    assert_eq!(request.initrd.as_str(), "grapheneos-initrd.img");
    assert_eq!(request.disks.len(), 2);
    assert_eq!(request.disks[0].object_id.as_str(), "grapheneos-system.img");
    assert!(!request.disks[0].writable);
    assert_eq!(request.disks[1].object_id.as_str(), "daily-driver-data.img");
    assert!(request.disks[1].writable);
    assert_eq!(request.resources.memory_mib, 4096);
    assert_eq!(request.resources.vcpus, 4);
}

#[test]
fn ubuntu_touch_profile_uses_its_own_rootfs_and_same_instance_scoped_data_rule() {
    let request =
        resolve_starter_bundle_request("ubuntu-touch", "lomiri", Generation(2), 1002, 3072, 3)
            .unwrap();

    assert_eq!(request.kernel.as_str(), "ubuntu-touch-kernel.img");
    assert_eq!(request.initrd.as_str(), "ubuntu-touch-initrd.img");
    assert_eq!(
        request.disks[0].object_id.as_str(),
        "ubuntu-touch-rootfs.img"
    );
    assert_eq!(request.disks[1].object_id.as_str(), "lomiri-data.img");
}

#[test]
fn unsupported_bundle_id_is_rejected_instead_of_becoming_an_object_name() {
    let error = resolve_starter_bundle_request(
        "../../data/local/tmp/guest",
        "vm-a",
        Generation(0),
        1003,
        2048,
        2,
    )
    .unwrap_err();

    assert_eq!(error, StarterBundleError::UnsupportedBundle);
}

#[test]
fn custom_rom_ports_boot_their_own_read_only_super_image() {
    for (bundle, prefix) in [("lineageos", "lineageos"), ("pixelos", "pixelos")] {
        let request =
            resolve_starter_bundle_request(bundle, "rom-a", Generation(1), 1004, 4096, 4).unwrap();
        assert_eq!(request.kernel.as_str(), format!("{prefix}-kernel.img"));
        assert_eq!(request.initrd.as_str(), format!("{prefix}-initrd.img"));
        assert_eq!(
            request.disks[0].object_id.as_str(),
            format!("{prefix}-super.img")
        );
        assert!(!request.disks[0].writable);
        assert_eq!(request.disks[1].object_id.as_str(), "rom-a-data.img");
        assert!(request.disks[1].writable);
    }
}

#[test]
fn every_shipped_bundle_manifest_has_a_broker_starter_profile() {
    let bundles = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bundles");
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(bundles).unwrap() {
        let manifest = entry.unwrap().path().join("bundle.toml");
        if let Ok(text) = std::fs::read_to_string(&manifest) {
            let id = carbon_core::BundleManifest::parse(&text).unwrap().id;
            assert!(
                resolve_starter_bundle_request(&id, "vm-a", Generation(0), 1, 2048, 2).is_ok(),
                "{id} has a manifest but no broker starter profile"
            );
            ids.push(id);
        }
    }
    ids.sort();
    assert_eq!(ids, ["grapheneos", "lineageos", "pixelos", "ubuntu-touch"]);
}
