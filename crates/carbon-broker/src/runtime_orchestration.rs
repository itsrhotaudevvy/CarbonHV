use carbon_protocol::{CallerIdentity, VmStartRequest};

use crate::{
    AvfBackend, AvfGraphicsConfig, BackendError, Broker, DisplaySessionIdentity,
    HostRuntimeGraphicalVmRequest, HostRuntimePublisher, HostRuntimeRequestError, RuntimeError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicalRuntimeError<E> {
    Broker(RuntimeError),
    Backend(BackendError),
    HostRequest(HostRuntimeRequestError),
    Publisher(E),
}

pub struct GraphicalRuntimeCoordinator<P> {
    backend: AvfBackend,
    publisher: P,
}

impl<P> GraphicalRuntimeCoordinator<P>
where
    P: HostRuntimePublisher,
{
    pub fn new(backend: AvfBackend, publisher: P) -> Self {
        Self { backend, publisher }
    }

    pub fn publisher(&self) -> &P {
        &self.publisher
    }

    pub fn start_graphical_vm(
        &self,
        broker: &mut Broker,
        caller: &CallerIdentity,
        request: VmStartRequest,
        graphics: AvfGraphicsConfig,
    ) -> Result<DisplaySessionIdentity, GraphicalRuntimeError<P::Error>> {
        let requested_identity = DisplaySessionIdentity {
            instance_id: request.instance_id.clone(),
            generation: request.generation,
        };
        let was_active = broker.active_display_session().as_ref() == Some(&requested_identity);

        let spec = broker
            .start(caller, request)
            .map_err(GraphicalRuntimeError::Broker)?;
        let identity =
            match broker.claim_display_session(caller, &spec.instance_id, spec.generation) {
                Ok(identity) => identity,
                Err(error) => {
                    let _ =
                        broker.reconcile_runtime_died(caller, &spec.instance_id, spec.generation);
                    return Err(GraphicalRuntimeError::Broker(error));
                }
            };

        // A retried request with the same request id and active generation is already committed.
        // The platform host runtime is generation-idempotent too, but avoiding a second Binder
        // call removes ambiguity if the retry was caused by a client-side timeout.
        if was_active {
            return Ok(identity);
        }

        let plan = match self.backend.build_graphics_platform_plan(&spec, graphics) {
            Ok(plan) => plan,
            Err(error) => {
                let _ = broker.reconcile_runtime_died(caller, &spec.instance_id, spec.generation);
                return Err(GraphicalRuntimeError::Backend(error));
            }
        };
        let runtime_request = match HostRuntimeGraphicalVmRequest::try_from_plan(&identity, &plan) {
            Ok(request) => request,
            Err(error) => {
                let _ = broker.reconcile_runtime_died(caller, &spec.instance_id, spec.generation);
                return Err(GraphicalRuntimeError::HostRequest(error));
            }
        };

        if let Err(error) = self.publisher.start_graphical_vm(&runtime_request) {
            let _ = broker.reconcile_runtime_died(caller, &spec.instance_id, spec.generation);
            return Err(GraphicalRuntimeError::Publisher(error));
        }
        Ok(identity)
    }

    pub fn stop_graphical_vm(
        &self,
        broker: &mut Broker,
        caller: &CallerIdentity,
        identity: &DisplaySessionIdentity,
        request_id: u64,
    ) -> Result<(), GraphicalRuntimeError<P::Error>> {
        match broker
            .prepare_graphical_stop(caller, identity, request_id)
            .map_err(GraphicalRuntimeError::Broker)?
        {
            GraphicalStopPreparation::AlreadyCompleted => Ok(()),
            GraphicalStopPreparation::StopRuntime => {
                self.publisher
                    .stop_graphical_vm(identity)
                    .map_err(GraphicalRuntimeError::Publisher)?;
                broker
                    .stop(
                        caller,
                        &identity.instance_id,
                        identity.generation,
                        request_id,
                    )
                    .map_err(GraphicalRuntimeError::Broker)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphicalStopPreparation {
    AlreadyCompleted,
    StopRuntime,
}
