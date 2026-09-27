use std::path::PathBuf;

use carbon_broker::{
    GraphicsHandleLeaseError, GraphicsHandleLeaseTable, ProgramInvocation, RendererLaunchError,
    build_renderer_launch_plan,
};
use carbon_core::{Generation, GpuRendererProfile};

#[test]
fn renderer_launch_uses_fixed_program_and_argv_without_shell() {
    let profile = GpuRendererProfile::gfxstream_vulkan_mali();
    let plan = build_renderer_launch_plan(&profile, PathBuf::from("/data/misc/carbonhv/runtime"))
        .expect("safe renderer plan");

    assert_eq!(
        plan.invocation,
        ProgramInvocation {
            program: PathBuf::from("/system/bin/carbon-renderer"),
            args: vec![
                "--profile".into(),
                "gfxstream-vulkan-mali".into(),
                "--runtime-dir".into(),
                "/data/misc/carbonhv/runtime".into(),
            ],
        },
    );
    assert_eq!(plan.profile_id, "gfxstream-vulkan-mali");
    assert!(
        plan.invocation
            .args
            .iter()
            .all(|arg| !arg.contains(';') && !arg.contains("&&"))
    );
}

#[test]
fn renderer_launch_rejects_unsafe_runtime_path() {
    let err = build_renderer_launch_plan(
        &GpuRendererProfile::gfxstream_vulkan_mali(),
        PathBuf::from("/data/misc/carbonhv/../escape"),
    )
    .unwrap_err();

    assert_eq!(err, RendererLaunchError::UnsafeRuntimeDirectory);
}

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
