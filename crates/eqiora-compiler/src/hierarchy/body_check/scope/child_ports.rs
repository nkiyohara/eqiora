//! Substitute child interface clocks and supports through exact named bindings.
use super::*;

impl DefinitionScope<'_, '_> {
    pub(super) fn specialize_child_port(
        &self,
        instance: &str,
        mut contract: PortContract,
    ) -> PortContract {
        let Some(occurrence) = self.child_instances.get(instance) else {
            return contract;
        };
        let target = |name: &str| {
            occurrence.bindings().iter().find_map(|binding| {
                if binding.name() != name {
                    return None;
                }
                match binding.value().kind() {
                    eqiora_lang::ExprKind::Name(target) => Some(target.as_str()),
                    _ => None,
                }
            })
        };
        if let PortContract::Signal {
            support,
            activation,
            ..
        } = &mut contract
        {
            if let Some(current) = support {
                if let Some(bound) =
                    target(current.domain()).and_then(|name| self.spatial_support(name))
                {
                    *current = bound;
                }
            }
            if let eqiora_lang::ActivationSyntax::Periodic(clock) = activation {
                *clock = target(clock)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{instance}.{clock}"));
            }
        }
        contract
    }
}
