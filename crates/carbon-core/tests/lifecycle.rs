use carbon_core::{
    DiskLeaseError, DiskLeaseTable, Generation, InstallRecovery, InstallTransaction,
    LifecycleAction, LifecycleError, LifecycleMachine, VmState,
};

#[test]
fn follows_legal_start_pause_resume_stop_cycle() {
    let mut vm = LifecycleMachine::new(Generation(7), VmState::Stopped);

    assert_eq!(
        vm.apply(Generation(7), 1, LifecycleAction::Start).unwrap(),
        VmState::Starting
    );
    assert_eq!(
        vm.apply(Generation(7), 2, LifecycleAction::Started)
            .unwrap(),
        VmState::Running
    );
    assert_eq!(
        vm.apply(Generation(7), 3, LifecycleAction::Pause).unwrap(),
        VmState::Paused
    );
    assert_eq!(
        vm.apply(Generation(7), 4, LifecycleAction::Resume).unwrap(),
        VmState::Running
    );
    assert_eq!(
        vm.apply(Generation(7), 5, LifecycleAction::Stop).unwrap(),
        VmState::Stopping
    );
    assert_eq!(
        vm.apply(Generation(7), 6, LifecycleAction::Stopped)
            .unwrap(),
        VmState::Stopped
    );
}

#[test]
fn rejects_invalid_transition_and_stale_generation() {
    let mut vm = LifecycleMachine::new(Generation(8), VmState::Stopped);

    assert_eq!(
        vm.apply(Generation(7), 1, LifecycleAction::Start),
        Err(LifecycleError::StaleGeneration {
            expected: Generation(8),
            got: Generation(7),
        })
    );
    assert_eq!(
        vm.apply(Generation(8), 2, LifecycleAction::Pause),
        Err(LifecycleError::InvalidTransition {
            from: VmState::Stopped,
            action: LifecycleAction::Pause,
        })
    );
}

#[test]
fn duplicate_request_is_idempotent() {
    let mut vm = LifecycleMachine::new(Generation(2), VmState::Stopped);
    let first = vm.apply(Generation(2), 44, LifecycleAction::Start).unwrap();
    let duplicate = vm.apply(Generation(2), 44, LifecycleAction::Start).unwrap();

    assert_eq!(first, VmState::Starting);
    assert_eq!(duplicate, VmState::Starting);
}

#[test]
fn disk_lease_allows_only_one_writer_per_disk() {
    let mut leases = DiskLeaseTable::default();
    let first = leases.acquire("disk-a", "vm-1", Generation(3)).unwrap();
    let duplicate = leases.acquire("disk-a", "vm-1", Generation(3)).unwrap();
    assert_eq!(first, duplicate);

    assert_eq!(
        leases.acquire("disk-a", "vm-2", Generation(1)),
        Err(DiskLeaseError::AlreadyLeased {
            disk_id: "disk-a".into(),
            owner_instance: "vm-1".into(),
        })
    );

    leases.release(&first).unwrap();
    assert!(leases.acquire("disk-a", "vm-2", Generation(1)).is_ok());
}

#[test]
fn stale_lease_token_cannot_release_new_owner() {
    let mut leases = DiskLeaseTable::default();
    let old = leases.acquire("disk-a", "vm-1", Generation(1)).unwrap();
    leases.release(&old).unwrap();
    let _new = leases.acquire("disk-a", "vm-1", Generation(2)).unwrap();

    assert_eq!(leases.release(&old), Err(DiskLeaseError::StaleToken));
}

#[test]
fn interrupted_install_preserves_previous_published_version() {
    let mut tx = InstallTransaction::begin("grapheneos", Some("2026.09.01"));
    tx.stage_verified("2026.09.27").unwrap();

    assert_eq!(
        tx.recover_after_interrupt(),
        InstallRecovery::UsePrevious("2026.09.01".into())
    );
}

#[test]
fn completed_publish_recovers_to_new_version() {
    let mut tx = InstallTransaction::begin("ubuntu-touch", None::<&str>);
    tx.stage_verified("24.04-1").unwrap();
    tx.publish().unwrap();

    assert_eq!(
        tx.recover_after_interrupt(),
        InstallRecovery::UsePublished("24.04-1".into())
    );
}
