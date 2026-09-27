use std::collections::BTreeMap;

use carbon_core::Generation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GraphicsHandleToken(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphicsHandleScope {
    instance_id: String,
    vm_generation: Generation,
    presentation_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsHandleLeaseError {
    UnknownHandle,
    ScopeMismatch,
}

#[derive(Debug, Default)]
pub struct GraphicsHandleLeaseTable {
    next_token: u64,
    leases: BTreeMap<GraphicsHandleToken, GraphicsHandleScope>,
}

impl GraphicsHandleLeaseTable {
    pub fn new() -> Self {
        Self {
            next_token: 1,
            leases: BTreeMap::new(),
        }
    }

    pub fn lease(
        &mut self,
        instance_id: &str,
        vm_generation: Generation,
        presentation_generation: u64,
    ) -> GraphicsHandleToken {
        let token = GraphicsHandleToken(self.next_token);
        self.next_token = self.next_token.saturating_add(1);
        self.leases.insert(
            token,
            GraphicsHandleScope {
                instance_id: instance_id.to_string(),
                vm_generation,
                presentation_generation,
            },
        );
        token
    }

    pub fn validate(
        &self,
        token: GraphicsHandleToken,
        instance_id: &str,
        vm_generation: Generation,
        presentation_generation: u64,
    ) -> Result<(), GraphicsHandleLeaseError> {
        let Some(scope) = self.leases.get(&token) else {
            return Err(GraphicsHandleLeaseError::UnknownHandle);
        };
        if scope.instance_id != instance_id
            || scope.vm_generation != vm_generation
            || scope.presentation_generation != presentation_generation
        {
            return Err(GraphicsHandleLeaseError::ScopeMismatch);
        }
        Ok(())
    }

    pub fn invalidate_renderer(&mut self, instance_id: &str, vm_generation: Generation) -> usize {
        let before = self.leases.len();
        self.leases.retain(|_, scope| {
            scope.instance_id != instance_id || scope.vm_generation != vm_generation
        });
        before - self.leases.len()
    }
}
