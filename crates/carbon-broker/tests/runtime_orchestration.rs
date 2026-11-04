use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use carbon_broker::{
    AvfBackend, AvfGraphicsConfig, Broker, BrokerPolicy, DisplaySessionIdentity,
    GraphicalRuntimeCoordinator, GraphicalRuntimeError, HostRuntimeGraphicalVmRequest,
    HostRuntimePublisher,
};
use carbon_core::Generation;
use carbon_protocol::{
    CallerIdentity, ManagedObjectId, VmDiskRequest, VmResources, VmStartRequest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FakePublisherError {
    Start,
    Stop,
}

#[derive(Default)]
struct FakePublisher {
    starts: RefCell<Vec<HostRuntimeGraphicalVmRequest>>,
    stops: RefCell<Vec<DisplaySessionIdentity>>,
    fail_start: Cell<bool>,
    fail_stop: Cell<bool>,
}

impl HostRuntimePublisher for FakePublisher {
    type Error = FakePublisherError;

    fn start_graphical_vm(
        &self,
        request: &HostRuntimeGraphicalVmRequest,
    ) -> Result<(), Self::Error> {
        self.starts.borrow_mut().push(request.clone());
        if self.fail_start.get() {
            Err(FakePublisherError::Start)
        } else {
            Ok(())
        }
    }

    fn stop_graphical_vm(&self, identity: &DisplaySessionIdentity) -> Result<(), Self::Error> {
        self.stops.borrow_mut().push(identity.clone());
        if self.fail_stop.get() {
            Err(FakePublisherError::Stop)
        } else {
            Ok(())
        }
    }
}

fn caller() -> CallerIdentity {
    CallerIdentity {
        uid: 1000,
        signing_digest: "host-bridge".into(),
    }
}

fn broker() -> Broker {
    Broker::new(BrokerPolicy {
        allowed_uid: 1000,
        allowed_signing_digest: "host-bridge".into(),
        managed_root: PathBuf::from("/data/misc/carbonhv/objects"),
        max_memory_mib: 8192,
        max_vcpus: 8,
    })
}

fn start_request(instance_id: &str, generation: Generation, request_id: u64) -> VmStartRequest {
    VmStartRequest {
        request_id,
        instance_id: instance_id.into(),
        generation,
        kernel: ManagedObjectId::new("grapheneos-kernel.img").unwrap(),
        initrd: ManagedObjectId::new("grapheneos-initrd.img").unwrap(),
        disks: vec![
            VmDiskRequest {
                object_id: ManagedObjectId::new("grapheneos-system.img").unwrap(),
                writable: false,
            },
            VmDiskRequest {
                object_id: ManagedObjectId::new(format!("{instance_id}-data.img")).unwrap(),
                writable: true,
            },
        ],
        resources: VmResources {
            memory_mib: 4096,
            vcpus: 4,
        },
        kernel_params: vec!["console=hvc0".into()],
    }
}

fn coordinator(publisher: FakePublisher) -> GraphicalRuntimeCoordinator<FakePublisher> {
    GraphicalRuntimeCoordinator::new(
        AvfBackend::new(
            PathBuf::from("/apex/com.android.virt/bin/vm"),
            PathBuf::from("/dev/kvm"),
        ),
        publisher,
    )
}

fn graphics() -> AvfGraphicsConfig {
    AvfGraphicsConfig::gfxstream_vulkan(1080, 2400, 420, 90).unwrap()
}

#[test]
fn broker_selects_generation_and_failed_publication_invalidates_it() {
    let mut broker = broker();
    let publisher = FakePublisher::default();
    publisher.fail_start.set(true);
    let coordinator = coordinator(publisher);

    let generation = broker.next_generation(&caller(), "vm-demo").unwrap();
    assert_eq!(generation, Generation(0));

    let error = coordinator
        .start_graphical_vm(
            &mut broker,
            &caller(),
            start_request("vm-demo", generation, 41),
            graphics(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        GraphicalRuntimeError::Publisher(FakePublisherError::Start)
    );
    assert_eq!(broker.active_display_session(), None);
    assert_eq!(
        broker.next_generation(&caller(), "vm-demo"),
        Ok(Generation(1))
    );
}

#[test]
fn successful_start_publishes_once_and_duplicate_retry_is_idempotent() {
    let mut broker = broker();
    let coordinator = coordinator(FakePublisher::default());
    let generation = broker.next_generation(&caller(), "vm-demo").unwrap();
    let request = start_request("vm-demo", generation, 42);

    let first = coordinator
        .start_graphical_vm(&mut broker, &caller(), request.clone(), graphics())
        .unwrap();
    let retry = coordinator
        .start_graphical_vm(&mut broker, &caller(), request, graphics())
        .unwrap();

    assert_eq!(first, retry);
    assert_eq!(first.generation, Generation(0));
    assert_eq!(coordinator.publisher().starts.borrow().len(), 1);
    let published = &coordinator.publisher().starts.borrow()[0];
    assert_eq!(published.instance_id, "vm-demo");
    assert_eq!(published.generation, 0);
    assert_eq!(published.vcpus, 4);
    assert_eq!(published.width_pixels, 1080);
}

#[test]
fn failed_runtime_stop_keeps_broker_generation_live_until_stop_succeeds() {
    let mut broker = broker();
    let publisher = FakePublisher::default();
    publisher.fail_stop.set(true);
    let coordinator = coordinator(publisher);
    let generation = broker.next_generation(&caller(), "vm-demo").unwrap();
    let identity = coordinator
        .start_graphical_vm(
            &mut broker,
            &caller(),
            start_request("vm-demo", generation, 43),
            graphics(),
        )
        .unwrap();

    assert_eq!(
        coordinator.stop_graphical_vm(&mut broker, &caller(), &identity, 44),
        Err(GraphicalRuntimeError::Publisher(FakePublisherError::Stop)),
    );
    assert_eq!(broker.active_display_session(), Some(identity.clone()));

    coordinator.publisher().fail_stop.set(false);
    coordinator
        .stop_graphical_vm(&mut broker, &caller(), &identity, 44)
        .unwrap();
    assert_eq!(broker.active_display_session(), None);
    assert_eq!(
        broker.next_generation(&caller(), "vm-demo"),
        Ok(Generation(1))
    );
}

#[test]
fn duplicate_completed_stop_does_not_republish_runtime_stop() {
    let mut broker = broker();
    let coordinator = coordinator(FakePublisher::default());
    let generation = broker.next_generation(&caller(), "vm-demo").unwrap();
    let identity = coordinator
        .start_graphical_vm(
            &mut broker,
            &caller(),
            start_request("vm-demo", generation, 45),
            graphics(),
        )
        .unwrap();

    coordinator
        .stop_graphical_vm(&mut broker, &caller(), &identity, 46)
        .unwrap();
    coordinator
        .stop_graphical_vm(&mut broker, &caller(), &identity, 46)
        .unwrap();
    assert_eq!(coordinator.publisher().stops.borrow().len(), 1);
}

#[test]
fn stale_runtime_death_is_ignored_and_exact_death_advances_generation() {
    let mut broker = broker();
    let coordinator = coordinator(FakePublisher::default());
    let generation = broker.next_generation(&caller(), "vm-demo").unwrap();
    let identity = coordinator
        .start_graphical_vm(
            &mut broker,
            &caller(),
            start_request("vm-demo", generation, 47),
            graphics(),
        )
        .unwrap();

    assert_eq!(
        broker.reconcile_runtime_died(&caller(), "vm-demo", Generation(9)),
        Ok(false),
    );
    assert_eq!(broker.active_display_session(), Some(identity.clone()));

    assert_eq!(
        broker.reconcile_runtime_died(&caller(), "vm-demo", identity.generation),
        Ok(true),
    );
    assert_eq!(broker.active_display_session(), None);
    assert_eq!(
        broker.next_generation(&caller(), "vm-demo"),
        Ok(Generation(1))
    );
}
