use carbon_core::{
    GpuQualificationTuple, GraphicsCapability, QualificationRecord, qualification_applies,
};

fn tuple() -> GpuQualificationTuple {
    GpuQualificationTuple {
        carbonhv_commit: "abc123".into(),
        device_model: "Pixel 7a".into(),
        soc: "GS201".into(),
        gpu: "Mali-G710".into(),
        host_build_fingerprint: "google/lynx/test".into(),
        kernel_version: "6.1-test".into(),
        avf_revision: "avf-17-r1".into(),
        renderer_id: "gfxstream-vulkan-mali".into(),
        renderer_version: "renderer-a".into(),
        graphics_driver_version: "226504704".into(),
        guest_bundle_id: "grapheneos-derived".into(),
        guest_bundle_version: "guest-a".into(),
        cvd_major: 1,
        display_mode: "1080x2400@90".into(),
        workload_version: "gpu-workload-v1".into(),
    }
}

fn record() -> QualificationRecord {
    QualificationRecord {
        tuple: tuple(),
        capabilities: vec![GraphicsCapability::Vulkan, GraphicsCapability::NativeBuffer],
    }
}

#[test]
fn exact_tuple_applies() {
    assert!(qualification_applies(&record(), &tuple()));
}

#[test]
fn material_tuple_changes_invalidate_qualification() {
    let cases: Vec<(&str, Box<dyn Fn(&mut GpuQualificationTuple)>)> = vec![
        (
            "host fingerprint",
            Box::new(|v| v.host_build_fingerprint = "different".into()),
        ),
        (
            "kernel",
            Box::new(|v| v.kernel_version = "different".into()),
        ),
        ("AVF", Box::new(|v| v.avf_revision = "different".into())),
        (
            "GPU driver",
            Box::new(|v| v.graphics_driver_version = "different".into()),
        ),
        (
            "renderer",
            Box::new(|v| v.renderer_version = "different".into()),
        ),
        (
            "guest",
            Box::new(|v| v.guest_bundle_version = "different".into()),
        ),
        ("CVD", Box::new(|v| v.cvd_major = 2)),
        (
            "display",
            Box::new(|v| v.display_mode = "1080x2400@60".into()),
        ),
        (
            "workload",
            Box::new(|v| v.workload_version = "gpu-workload-v2".into()),
        ),
    ];

    for (name, mutate) in cases {
        let mut current = tuple();
        mutate(&mut current);
        assert!(
            !qualification_applies(&record(), &current),
            "{name} change must invalidate qualification",
        );
    }
}
