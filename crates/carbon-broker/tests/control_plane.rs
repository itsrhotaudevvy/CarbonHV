use std::sync::Mutex;

use carbon_broker::{
    AvfBackend, BrokerControlError, BrokerControlPlane, BrokerPolicy, DisplaySessionIdentity,
    GraphicalStartIntent, HostRuntimeGraphicalVmRequest, HostRuntimePublisher, StarterBundleError,
};
use carbon_core::Generation;
use carbon_protocol::CallerIdentity;

#[derive(Default)]
struct FakePublisher {
    starts: Mutex<Vec<HostRuntimeGraphicalVmRequest>>,
    stops: Mutex<Vec<DisplaySessionIdentity>>,
}

impl HostRuntimePublisher for FakePublisher {
    type Error = ();

    fn start_graphical_vm(
        &self,
        request: &HostRuntimeGraphicalVmRequest,
    ) -> Result<(), Self::Error> {
        self.starts.lock().unwrap().push(request.clone());
        Ok(())
    }

    fn stop_graphical_vm(&self, identity: &DisplaySessionIdentity) -> Result<(), Self::Error> {
        self.stops.lock().unwrap().push(identity.clone());
        Ok(())
    }
}

fn control() -> BrokerControlPlane<FakePublisher> {
    BrokerControlPlane::new(
        BrokerPolicy {
            allowed_uid: 1000,
            allowed_signing_digest: "host-bridge".into(),
            managed_root: "/data/misc/carbonhv/objects".into(),
            max_memory_mib: 8192,
            max_vcpus: 8,
        },
        AvfBackend::new("/apex/com.android.virt/bin/vm".into(), "/dev/kvm".into()),
        FakePublisher::default(),
        CallerIdentity {
            uid: 1000,
            signing_digest: "host-bridge".into(),
        },
    )
}

fn graphene(request_id: u64) -> GraphicalStartIntent {
    GraphicalStartIntent {
        request_id,
        instance_id: "daily-driver".into(),
        bundle_id: "grapheneos".into(),
        memory_mib: 4096,
        vcpus: 4,
        width_pixels: 1080,
        height_pixels: 2400,
        density_dpi: 420,
        refresh_rate: 90,
    }
}

#[test]
fn start_is_broker_selected_and_retry_with_same_request_id_is_idempotent() {
    let control = control();

    let first = control.start_graphical_vm(graphene(700)).unwrap();
    let retry = control.start_graphical_vm(graphene(700)).unwrap();

    assert_eq!(first, Generation(0));
    assert_eq!(retry, Generation(0));
    assert_eq!(
        control.active_display_generation("daily-driver").unwrap(),
        Some(Generation(0))
    );
    assert_eq!(control.publisher().starts.lock().unwrap().len(), 1);
}

#[test]
fn unsupported_bundle_is_rejected_before_runtime_publication() {
    let control = control();
    let mut request = graphene(701);
    request.bundle_id = "../../tmp/guest".into();

    let error = control.start_graphical_vm(request).unwrap_err();

    assert_eq!(
        error,
        BrokerControlError::StarterBundle(StarterBundleError::UnsupportedBundle)
    );
    assert!(control.publisher().starts.lock().unwrap().is_empty());
}

#[test]
fn exact_runtime_death_clears_active_generation_but_stale_death_does_not() {
    let control = control();
    let generation = control.start_graphical_vm(graphene(702)).unwrap();

    assert!(
        !control
            .notify_runtime_died("daily-driver", Generation(generation.0 + 1))
            .unwrap()
    );
    assert_eq!(
        control.active_display_generation("daily-driver").unwrap(),
        Some(generation)
    );

    assert!(
        control
            .notify_runtime_died("daily-driver", generation)
            .unwrap()
    );
    assert_eq!(
        control.active_display_generation("daily-driver").unwrap(),
        None
    );
}

#[test]
fn stop_publishes_exact_generation_then_clears_authoritative_state() {
    let control = control();
    let generation = control.start_graphical_vm(graphene(703)).unwrap();

    control.stop_vm("daily-driver", generation, 704).unwrap();

    assert_eq!(
        control.publisher().stops.lock().unwrap().as_slice(),
        &[DisplaySessionIdentity {
            instance_id: "daily-driver".into(),
            generation,
        }]
    );
    assert_eq!(
        control.active_display_generation("daily-driver").unwrap(),
        None
    );
}
