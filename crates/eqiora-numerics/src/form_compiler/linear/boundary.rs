use super::data::{Context, Data};
use super::lowering::Terms;

#[cfg(test)]
mod tests;
use crate::canonical::{boundary_parent, relations_on};
use crate::form_compiler::scalar::{require_closed_dag, typed_relation};
use crate::scalar_conservation::{ScalarExteriorLaw, recognize_exterior_law_with_flux};
use eqiora_core::{Diagnostic, RawId, ValueType};
use eqiora_graph::EdgeKind;
use eqiora_schema::kernel::{BoundarySide, DomainKind, ExprNode, KernelNode, SymbolRef};
use eqiora_sem::KernelProgram;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Inventory {
    pub(super) fields: BTreeMap<RawId, BTreeMap<RawId, ScalarExteriorLaw>>,
    pub(super) dependencies: BTreeMap<RawId, BTreeSet<RawId>>,
}

pub(super) fn derive(
    program: &KernelProgram,
    parent: RawId,
    dimension: usize,
    fields: &[(RawId, ValueType)],
    rows: &[Terms],
    coefficients: &BTreeMap<RawId, Data>,
) -> Result<Inventory, Diagnostic> {
    let mut boundaries = BTreeMap::new();
    let mut sides = BTreeSet::new();
    let mut dependencies = BTreeMap::new();
    let geometry_backed = matches!(program.node(parent), Some(KernelNode::Domain(definition)) if matches!(definition.kind(), DomainKind::GeometryRegion { .. }));
    let mut boundary_count = 0;
    for node in program.nodes() {
        let KernelNode::Domain(domain) = node else {
            continue;
        };
        if boundary_parent(program, domain.id().erase()) != Some(parent) {
            continue;
        }
        match domain.kind() {
            DomainKind::CartesianBoundary { axis, side } if !geometry_backed => {
                if *axis >= dimension || !sides.insert((*axis, *side)) {
                    return Err(super::invalid(
                        "duplicate or invalid Cartesian boundary side",
                    ));
                }
            }
            DomainKind::GeometryBoundary { .. } if geometry_backed => {}
            _ => {
                return Err(super::invalid(
                    "linear block boundary has incompatible support",
                ));
            }
        }
        boundary_count += 1;
        let mut covered = BTreeSet::new();
        for relation in relations_on(program, domain.id().erase()) {
            let typed = typed_relation(program, relation)?;
            let dag = typed.expression();
            require_closed_dag(dag, relation)?;
            let equation_dependencies = dag
                .nodes()
                .iter()
                .filter_map(|node| match node {
                    ExprNode::Symbol(SymbolRef::Field(id) | SymbolRef::Derivative(id)) => {
                        Some(id.erase())
                    }
                    ExprNode::Symbol(SymbolRef::Parameter(id)) => Some(id.erase()),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            let graph = program
                .edges()
                .iter()
                .filter(|edge| edge.from() == relation && edge.kind() == EdgeKind::DependsOn)
                .map(|edge| edge.to())
                .collect();
            if equation_dependencies != graph {
                return Err(super::invalid(
                    "boundary dependency inventory differs from its equation",
                ));
            }
            let matches = fields
                .iter()
                .enumerate()
                .filter(|(_, (field, _))| equation_dependencies.contains(field))
                .collect::<Vec<_>>();
            let [(index, (field, _))] = matches.as_slice() else {
                return Err(super::invalid(
                    "boundary equation requires exactly one unknown Field",
                ));
            };
            let law = recognize_exterior_law_with_flux(
                program,
                relation,
                *field,
                dimension,
                |dag, normal| {
                    let Some(ExprNode::NormalComponent(flux)) = dag.node(normal) else {
                        unreachable!()
                    };
                    let context = Context {
                        program,
                        dag,
                        owner: relation,
                        dimension,
                        coefficients,
                    };
                    let (trial, coefficient) = context.flux(*flux, 0)?;
                    if trial != *field
                        || !coefficient.same_coefficient(&rows[*index].diffusion[field])
                    {
                        return Err(super::invalid(
                            "boundary flux differs from the exact volume constitutive coefficient",
                        ));
                    }
                    Ok(())
                },
            )?;
            if matches!(law, ScalarExteriorLaw::Robin { .. }) {
                return Err(super::invalid(
                    "linear block does not admit Robin boundaries",
                ));
            }
            if !covered.insert(*field) {
                return Err(super::invalid("duplicate Field boundary law"));
            }
            boundaries
                .entry(*field)
                .or_insert_with(BTreeMap::new)
                .insert(domain.id().erase(), law);
            dependencies.insert(relation, equation_dependencies);
        }
        if covered.len() != fields.len() {
            return Err(super::invalid(
                "every unknown Field requires complete boundary law coverage",
            ));
        }
    }
    if boundary_count != 2 * dimension
        || !geometry_backed
            && (0..dimension).any(|axis| {
                [BoundarySide::Lower, BoundarySide::Upper]
                    .iter()
                    .any(|side| !sides.contains(&(axis, *side)))
            })
    {
        return Err(super::invalid(
            "linear block requires every Cartesian boundary side",
        ));
    }
    Ok(Inventory {
        fields: boundaries,
        dependencies,
    })
}
