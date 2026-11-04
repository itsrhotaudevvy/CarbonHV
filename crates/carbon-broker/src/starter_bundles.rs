use carbon_core::Generation;
use carbon_protocol::{ManagedObjectId, VmDiskRequest, VmResources, VmStartRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarterBundleError {
    UnsupportedBundle,
    InvalidInstanceId,
}

pub fn resolve_starter_bundle_request(
    bundle_id: &str,
    instance_id: &str,
    generation: Generation,
    request_id: u64,
    memory_mib: u64,
    vcpus: u16,
) -> Result<VmStartRequest, StarterBundleError> {
    let (kernel, initrd, system_disk) = match bundle_id {
        "grapheneos" => (
            "grapheneos-kernel.img",
            "grapheneos-initrd.img",
            "grapheneos-system.img",
        ),
        // AOSP-derived ROM ports boot a dynamic-partition super image read-only.
        "lineageos" => (
            "lineageos-kernel.img",
            "lineageos-initrd.img",
            "lineageos-super.img",
        ),
        "pixelos" => (
            "pixelos-kernel.img",
            "pixelos-initrd.img",
            "pixelos-super.img",
        ),
        "ubuntu-touch" => (
            "ubuntu-touch-kernel.img",
            "ubuntu-touch-initrd.img",
            "ubuntu-touch-rootfs.img",
        ),
        _ => return Err(StarterBundleError::UnsupportedBundle),
    };

    let data = ManagedObjectId::new(format!("{instance_id}-data.img"))
        .map_err(|_| StarterBundleError::InvalidInstanceId)?;

    Ok(VmStartRequest {
        request_id,
        instance_id: instance_id.to_string(),
        generation,
        kernel: ManagedObjectId::new(kernel).expect("static kernel object id is valid"),
        initrd: ManagedObjectId::new(initrd).expect("static initrd object id is valid"),
        disks: vec![
            VmDiskRequest {
                object_id: ManagedObjectId::new(system_disk)
                    .expect("static system object id is valid"),
                writable: false,
            },
            VmDiskRequest {
                object_id: data,
                writable: true,
            },
        ],
        resources: VmResources { memory_mib, vcpus },
        kernel_params: vec!["console=hvc0".to_string()],
    })
}
