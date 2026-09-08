//! Accepted numerical and exact discrete storage for one reference execution.
use super::*;

#[derive(Debug, Clone)]
pub(super) struct RuntimeState {
    pub(super) discrete_fields: BTreeMap<RawId, eqiora_core::ValueLiteral>,
    pub(super) discrete_ports: BTreeMap<RawId, eqiora_core::ValueLiteral>,
    pub(super) discrete_next: BTreeMap<RawId, eqiora_core::ValueLiteral>,
    pub(super) fields: BTreeMap<RawId, f64>,
    pub(super) derivatives: BTreeMap<RawId, f64>,
    pub(super) ports: BTreeMap<RawId, f64>,
    pub(super) physical: BTreeMap<PhysicalUnknown, f64>,
}

impl RuntimeState {
    pub(super) fn new(program: &KernelProgram, plan: &ExecutionPlan) -> Result<Self, Diagnostic> {
        let mut fields = BTreeMap::new();
        let mut ports = BTreeMap::new();
        for node in program.nodes() {
            match node {
                KernelNode::Field(field)
                    if !is_clocked_variable(program, field.id().erase())
                        && !discrete::is_discrete(program, SymbolRef::Field(field.id())) =>
                {
                    let id = field.id().erase();
                    fields.insert(id, 0.0);
                }
                KernelNode::Port(port)
                    if matches!(port.signal_contract(), Some((SignalDirection::Output, _)))
                        && !discrete::is_discrete(program, SymbolRef::Port(port.id())) =>
                {
                    ports.insert(port.id().erase(), 0.0);
                }
                _ => {}
            }
        }
        Ok(Self {
            discrete_fields: BTreeMap::new(),
            discrete_ports: BTreeMap::new(),
            discrete_next: BTreeMap::new(),
            fields,
            derivatives: BTreeMap::new(),
            ports,
            physical: plan
                .physical_unknowns
                .iter()
                .copied()
                .map(|unknown| (unknown, 0.0))
                .collect(),
        })
    }
}
