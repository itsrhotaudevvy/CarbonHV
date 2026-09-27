use carbon_broker::{GraphicsHandleLeaseError, GraphicsHandleLeaseTable};
use carbon_core::Generation;

#[test]
fn handle_is_scoped_to_instance_vm_and_presentation_generations() {
    let mut table = GraphicsHandleLeaseTable::new();
    let token = table.lease("vm-1", Generation(7), 12);

    assert_eq!(table.validate(token, "vm-1", Generation(7), 12), Ok(()),);
    assert_eq!(
        table.validate(token, "vm-2", Generation(7), 12),
        Err(GraphicsHandleLeaseError::ScopeMismatch),
    );
    assert_eq!(
        table.validate(token, "vm-1", Generation(6), 12),
        Err(GraphicsHandleLeaseError::ScopeMismatch),
    );
    assert_eq!(
        table.validate(token, "vm-1", Generation(7), 11),
        Err(GraphicsHandleLeaseError::ScopeMismatch),
    );
}

#[test]
fn renderer_crash_cleanup_invalidates_all_matching_generation_leases() {
    let mut table = GraphicsHandleLeaseTable::new();
    let first = table.lease("vm-1", Generation(7), 10);
    let second = table.lease("vm-1", Generation(7), 11);
    let other_vm = table.lease("vm-2", Generation(7), 10);

    assert_eq!(table.invalidate_renderer("vm-1", Generation(7)), 2);
    assert_eq!(
        table.validate(first, "vm-1", Generation(7), 10),
        Err(GraphicsHandleLeaseError::UnknownHandle),
    );
    assert_eq!(
        table.validate(second, "vm-1", Generation(7), 11),
        Err(GraphicsHandleLeaseError::UnknownHandle),
    );
    assert_eq!(table.validate(other_vm, "vm-2", Generation(7), 10), Ok(()),);
}
