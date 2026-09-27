use carbon_core::{
    BackupError, BackupManifest, CloneIdentityPlan, RestoreMode, UpdateDecision, UpdateError,
    UpdateTransaction,
};

#[test]
fn interrupted_update_keeps_current_system_before_candidate_health() {
    let mut tx = UpdateTransaction::begin("2026.09.01", 4);
    tx.stage_verified("2026.09.27", 4).unwrap();

    assert_eq!(tx.recover_after_interrupt(), UpdateDecision::BootCurrent("2026.09.01".into()));
}

#[test]
fn healthy_candidate_can_commit_and_become_current() {
    let mut tx = UpdateTransaction::begin("2026.09.01", 4);
    tx.stage_verified("2026.09.27", 4).unwrap();
    tx.mark_candidate_healthy().unwrap();
    tx.commit().unwrap();

    assert_eq!(tx.recover_after_interrupt(), UpdateDecision::BootCurrent("2026.09.27".into()));
}

#[test]
fn rollback_is_blocked_after_incompatible_data_migration() {
    let mut tx = UpdateTransaction::begin("2026.09.01", 4);
    tx.stage_verified("2026.09.27", 5).unwrap();
    tx.mark_candidate_healthy().unwrap();

    assert_eq!(
        tx.rollback_decision(),
        Err(UpdateError::DataMigrationPreventsRollback {
            current_epoch: 4,
            candidate_epoch: 5,
        })
    );
}

#[test]
fn backup_manifest_rejects_invalid_hash_and_unsafe_object_id() {
    let invalid_hash = BackupManifest::parse(
        "instance_id=vm-1\ngeneration=8\nmachine_identity=id-a\ndata_object=data-a\nsystem_hash=not-a-hash\n",
    );
    assert_eq!(invalid_hash, Err(BackupError::InvalidSystemHash));

    let unsafe_object = BackupManifest::parse(
        "instance_id=vm-1\ngeneration=8\nmachine_identity=id-a\ndata_object=../data\nsystem_hash=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n",
    );
    assert_eq!(unsafe_object, Err(BackupError::UnsafeObjectId("../data".into())));
}

#[test]
fn restore_retains_machine_identity_but_clone_regenerates_it() {
    let manifest = BackupManifest::parse(
        "instance_id=vm-1\ngeneration=8\nmachine_identity=id-a\ndata_object=data-a\nsystem_hash=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n",
    )
    .unwrap();

    assert_eq!(
        manifest.identity_plan(RestoreMode::Restore { target_instance_id: "vm-1".into() }),
        CloneIdentityPlan::Retain("id-a".into())
    );
    assert_eq!(
        manifest.identity_plan(RestoreMode::Clone { target_instance_id: "vm-2".into() }),
        CloneIdentityPlan::Regenerate {
            target_instance_id: "vm-2".into(),
            regenerate_machine_id: true,
            regenerate_network_identity: true,
        }
    );
}

#[test]
fn backup_round_trip_preserves_integrity_fields() {
    let manifest = BackupManifest {
        instance_id: "vm-1".into(),
        generation: 8,
        machine_identity: "id-a".into(),
        data_object: "data-a".into(),
        system_hashes: vec![
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into(),
        ],
    };
    let encoded = manifest.encode();

    assert_eq!(BackupManifest::parse(&encoded).unwrap(), manifest);
}
