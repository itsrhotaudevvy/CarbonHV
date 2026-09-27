use std::path::PathBuf;

use carbon_broker::{Broker, BrokerPolicy, RuntimeError};
use carbon_core::Generation;
use carbon_protocol::{CallerIdentity, ManagedObjectId, VmResources, VmStartRequest};

fn policy() -> BrokerPolicy {
    BrokerPolicy {
        allowed_uid: 10_123,
        allowed_signing_digest: "sha256:carbonhv-test".into(),
        managed_root: PathBuf::from("/data/misc/carbonhv/objects"),
        max_memory_mib: 6144,
        max_vcpus: 4,
    }
}

fn caller() -> CallerIdentity {
    CallerIdentity {
        uid: 10_123,
        signing_digest: "sha256:carbonhv-test".into(),
    }
}

fn request(generation: u64) -> VmStartRequest {
    VmStartRequest {
        request_id: 50,
        instance_id: "vm-demo".into(),
        generation: Generation(generation),
        kernel: ManagedObjectId::new("kernel-a").unwrap(),
        initrd: ManagedObjectId::new("initrd-a").unwrap(),
        disks: vec![ManagedObjectId::new("disk-a").unwrap()],
        resources: VmResources {
            memory_mib: 4096,
            vcpus: 4,
        },
        kernel_params: vec!["console=hvc0".into(), "rdinit=/bin/init".into()],
    }
}

#[test]
fn rejects_wrong_caller_identity() {
    let mut broker = Broker::new(policy());
    let mut wrong = caller();
    wrong.uid = 20_000;

    assert_eq!(
        broker.start(&wrong, request(1)),
        Err(RuntimeError::UnauthorizedCaller)
    );
}

#[test]
fn managed_object_ids_reject_paths_and_shell_metacharacters() {
    assert!(ManagedObjectId::new("../etc/passwd").is_err());
    assert!(ManagedObjectId::new("disk;reboot").is_err());
    assert!(ManagedObjectId::new("disk/a").is_err());
    assert!(ManagedObjectId::new("disk_a-01.img").is_ok());
}

#[test]
fn rejects_kernel_parameter_with_control_characters() {
    let mut broker = Broker::new(policy());
    let mut bad = request(1);
    bad.kernel_params.push("ok\nreboot".into());

    assert_eq!(
        broker.start(&caller(), bad),
        Err(RuntimeError::UnsafeKernelParameter)
    );
}

#[test]
fn rejects_resource_caps_before_runtime_start() {
    let mut broker = Broker::new(policy());
    let mut too_large = request(1);
    too_large.resources.memory_mib = 8192;

    assert_eq!(
        broker.start(&caller(), too_large),
        Err(RuntimeError::MemoryLimitExceeded {
            requested_mib: 8192,
            max_mib: 6144
        })
    );
}

#[test]
fn rejects_generation_replay_after_instance_advances() {
    let mut broker = Broker::new(policy());
    let first = broker.start(&caller(), request(4)).unwrap();
    assert_eq!(first.generation, Generation(4));
    broker
        .stop(&caller(), "vm-demo", Generation(4), 51)
        .unwrap();

    assert_eq!(
        broker.start(&caller(), request(4)),
        Err(RuntimeError::StaleGeneration {
            minimum: Generation(5),
            got: Generation(4),
        })
    );
}

#[test]
fn resolves_only_managed_object_ids_under_policy_root() {
    let mut broker = Broker::new(policy());
    let spec = broker.start(&caller(), request(2)).unwrap();

    assert_eq!(
        spec.kernel,
        PathBuf::from("/data/misc/carbonhv/objects/kernel-a")
    );
    assert_eq!(
        spec.disks,
        vec![PathBuf::from("/data/misc/carbonhv/objects/disk-a")]
    );
}

#[test]
fn rejects_second_start_while_instance_is_already_running() {
    let mut broker = Broker::new(policy());
    broker.start(&caller(), request(7)).unwrap();

    let mut replacement = request(8);
    replacement.request_id = 51;

    assert_eq!(
        broker.start(&caller(), replacement),
        Err(RuntimeError::InstanceAlreadyRunning {
            generation: Generation(7),
        })
    );
}
