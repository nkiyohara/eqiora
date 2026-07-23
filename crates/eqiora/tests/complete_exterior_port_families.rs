//! Conformance root for RFC 0041 complete-exterior family elaboration.

#[path = "complete_exterior_port_families/aliasing.rs"]
mod aliasing;
#[path = "complete_exterior_port_families/fail_closed.rs"]
mod fail_closed;
#[path = "complete_exterior_port_families/forwarding.rs"]
mod forwarding;

use eqiora::Entity;
use eqiora::RawId;
use eqiora::compiler::{ModelSymbols, compile};
use eqiora::entity::kinds;
use eqiora::kernel::{ExprDag, ExprNode, KernelNode, SymbolRef};

const COMPLETE: &str =
    include_str!("../../../verify/packages/complete-exterior-port-families/models/complete.eqi");

fn count_symbols<I: Entity>(symbols: &ModelSymbols) -> usize {
    symbols
        .iter()
        .filter(|(_, id)| id.downcast::<I>().is_some())
        .count()
}

fn relation_shape(dag: &ExprDag, selected_port: RawId) -> Vec<&'static str> {
    dag.nodes()
        .iter()
        .map(|node| match node {
            ExprNode::Symbol(SymbolRef::PortTrace(port)) if port.erase() == selected_port => {
                "selected-port-trace"
            }
            ExprNode::Symbol(SymbolRef::PortFlux(port)) if port.erase() == selected_port => {
                "selected-port-flux"
            }
            ExprNode::Sub(_, _) => "sub",
            _ => "other",
        })
        .collect()
}

#[test]
fn complete_exterior_families_flatten_to_ordinary_boundary_entities() {
    let mut compiled = compile("complete.eqi", COMPLETE).expect("complete exterior compiles");
    assert_eq!(compiled.len(), 1);
    let compiled = compiled.pop().expect("one Model");

    assert_eq!(count_symbols::<kinds::Port>(compiled.symbols()), 8);
    assert_eq!(count_symbols::<kinds::Relation>(compiled.symbols()), 8);
    assert!(
        compiled
            .symbols()
            .iter()
            .any(|(name, _)| name == "solid.mechanical[axis=0,side=lower]")
    );
    assert!(
        !compiled
            .symbols()
            .iter()
            .any(|(name, _)| name.contains("exterior"))
    );

    let symbols = compiled.symbols().clone();
    let program = forwarding::admit(compiled);
    for (side, terminal) in [
        ("axis=0,side=lower", "x_lower"),
        ("axis=0,side=upper", "x_upper"),
        ("axis=1,side=lower", "y_lower"),
        ("axis=1,side=upper", "y_upper"),
    ] {
        let family_port = symbols
            .get(&format!("solid.mechanical[{side}]"))
            .expect("generated family Port");
        let flat_port = symbols
            .get(&format!("{terminal}_terminal.mechanical"))
            .expect("explicit singular Port");
        let (KernelNode::Port(family_port_definition), KernelNode::Port(flat_port_definition)) = (
            program.node(family_port).expect("family Port node"),
            program.node(flat_port).expect("singular Port node"),
        ) else {
            panic!("verification bijection selects ordinary Port nodes");
        };
        assert_eq!(
            family_port_definition.payload(),
            flat_port_definition.payload(),
            "family elaboration and explicit singular declaration have one exact connector/support contract"
        );

        let family_relation = symbols
            .get(&format!("solid.boundary_law[{side}]"))
            .and_then(RawId::downcast::<kinds::Relation>)
            .expect("generated family Relation");
        let flat_relation = symbols
            .get(&format!("{terminal}_terminal.terminal_law"))
            .and_then(RawId::downcast::<kinds::Relation>)
            .expect("explicit singular Relation");
        let (KernelNode::Relation(family_definition), KernelNode::Relation(flat_definition)) = (
            program
                .node(family_relation.erase())
                .expect("family Relation node"),
            program
                .node(flat_relation.erase())
                .expect("singular Relation node"),
        ) else {
            panic!("verification bijection selects ordinary Relation nodes");
        };
        assert_eq!(
            relation_shape(family_definition.residuals(), family_port),
            relation_shape(flat_definition.residuals(), flat_port),
            "family elaboration preserves the explicit singular residual structure"
        );
        assert_eq!(
            family_definition.residuals().roots(),
            flat_definition.residuals().roots()
        );
    }
}
