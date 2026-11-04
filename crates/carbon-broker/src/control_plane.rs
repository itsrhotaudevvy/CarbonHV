use std::sync::Mutex;

use carbon_core::Generation;
use carbon_protocol::CallerIdentity;

use crate::{
    AvfBackend, AvfGraphicsConfig, BackendError, Broker, BrokerPolicy, DisplaySessionIdentity,
    GraphicalRuntimeCoordinator, GraphicalRuntimeError, HostRuntimePublisher, RuntimeError,
    StarterBundleError, resolve_starter_bundle_request,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicalStartIntent {
    pub request_id: u64,
    pub instance_id: String,
    pub bundle_id: String,
    pub memory_mib: u64,
    pub vcpus: u16,
    pub width_pixels: u32,
    pub height_pixels: u32,
    pub density_dpi: u32,
    pub refresh_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerControlError<E> {
    StatePoisoned,
    Broker(RuntimeError),
    StarterBundle(StarterBundleError),
    Graphics(BackendError),
    Runtime(GraphicalRuntimeError<E>),
}

pub struct BrokerControlPlane<P> {
    broker: Mutex<Broker>,
    coordinator: GraphicalRuntimeCoordinator<P>,
    caller: CallerIdentity,
}

impl<P> BrokerControlPlane<P>
where
    P: HostRuntimePublisher,
{
    pub fn new(
        policy: BrokerPolicy,
        backend: AvfBackend,
        publisher: P,
        caller: CallerIdentity,
    ) -> Self {
        Self {
            broker: Mutex::new(Broker::new(policy)),
            coordinator: GraphicalRuntimeCoordinator::new(backend, publisher),
            caller,
        }
    }

    pub fn publisher(&self) -> &P {
        self.coordinator.publisher()
    }

    pub fn start_graphical_vm(
        &self,
        intent: GraphicalStartIntent,
    ) -> Result<Generation, BrokerControlError<P::Error>> {
        let graphics = AvfGraphicsConfig::gfxstream_vulkan(
            intent.width_pixels,
            intent.height_pixels,
            intent.density_dpi,
            intent.refresh_rate,
        )
        .map_err(BrokerControlError::Graphics)?;

        let mut broker = self
            .broker
            .lock()
            .map_err(|_| BrokerControlError::StatePoisoned)?;
        let generation = broker
            .generation_for_start(&self.caller, &intent.instance_id)
            .map_err(BrokerControlError::Broker)?;
        let request = resolve_starter_bundle_request(
            &intent.bundle_id,
            &intent.instance_id,
            generation,
            intent.request_id,
            intent.memory_mib,
            intent.vcpus,
        )
        .map_err(BrokerControlError::StarterBundle)?;

        let identity = self
            .coordinator
            .start_graphical_vm(&mut broker, &self.caller, request, graphics)
            .map_err(BrokerControlError::Runtime)?;
        Ok(identity.generation)
    }

    pub fn stop_vm(
        &self,
        instance_id: &str,
        generation: Generation,
        request_id: u64,
    ) -> Result<(), BrokerControlError<P::Error>> {
        let identity = DisplaySessionIdentity {
            instance_id: instance_id.to_string(),
            generation,
        };
        let mut broker = self
            .broker
            .lock()
            .map_err(|_| BrokerControlError::StatePoisoned)?;
        self.coordinator
            .stop_graphical_vm(&mut broker, &self.caller, &identity, request_id)
            .map_err(BrokerControlError::Runtime)
    }

    pub fn active_display_generation(
        &self,
        instance_id: &str,
    ) -> Result<Option<Generation>, BrokerControlError<P::Error>> {
        let broker = self
            .broker
            .lock()
            .map_err(|_| BrokerControlError::StatePoisoned)?;
        broker
            .active_display_generation(&self.caller, instance_id)
            .map_err(BrokerControlError::Broker)
    }

    pub fn notify_runtime_died(
        &self,
        instance_id: &str,
        generation: Generation,
    ) -> Result<bool, BrokerControlError<P::Error>> {
        let mut broker = self
            .broker
            .lock()
            .map_err(|_| BrokerControlError::StatePoisoned)?;
        broker
            .reconcile_runtime_died(&self.caller, instance_id, generation)
            .map_err(BrokerControlError::Broker)
    }
}
