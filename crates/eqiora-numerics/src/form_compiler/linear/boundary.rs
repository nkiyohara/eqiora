use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::{Diagnostic, RawId, ValueType};
use eqiora_schema::kernel::{BoundarySide, DomainKind, KernelNode};
use eqiora_sem::KernelProgram;

use crate::canonical::{boundary_parent, relations_on};
use crate::form_compiler::scalar::{is_homogeneous_trace, require_closed_dag, typed_relation};

pub(super) struct Inventory {
    pub(super) fields: BTreeMap<RawId, Vec<RawId>>,
    pub(super) relations: BTreeSet<RawId>,
}

pub(super) fn derive(
    program: &KernelProgram,
    parent: RawId,
    dimension: usize,
    fields: &[(RawId, ValueType)],
) -> Result<Inventory, Diagnostic> {
    let mut boundaries = BTreeMap::new();
    let mut sides = BTreeSet::new();
    let mut boundary_relations = BTreeSet::new();
    let geometry_backed = matches!(program.node(parent), Some(KernelNode::Domain(definition)) if matches!(definition.kind(), DomainKind::GeometryRegion { .. }));
    let mut geometry_sides = 0;
    for node in program.nodes() {
        let KernelNode::Domain(domain) = node else {
            continue;
        };
        if boundary_parent(program, domain.id().erase()) != Some(parent) {
            continue;
        }
        let (axis, side) = match domain.kind() {
            DomainKind::CartesianBoundary { axis, side } => (*axis, *side),
            DomainKind::GeometryBoundary { .. } if geometry_backed => {
                let index = geometry_sides;
                geometry_sides += 1;
                (
                    index / 2,
                    if index % 2 == 0 {
                        BoundarySide::Lower
                    } else {
                        BoundarySide::Upper
                    },
                )
            }
            _ => {
                return Err(super::invalid(
                    "linear block boundary requires exact Cartesian or GeometryRegion support",
                ));
            }
        };
        if axis >= dimension || !sides.insert((axis, side)) {
            return Err(super::invalid(
                "duplicate or invalid Cartesian boundary side",
            ));
        }
        let mut covered = BTreeSet::new();
        for relation in relations_on(program, domain.id().erase()) {
            let typed = typed_relation(program, relation)?;
            require_closed_dag(typed.expression(), relation)?;
            boundary_relations.insert(relation);
            let mut matched = None;
            for (field, _) in fields {
                if is_homogeneous_trace(typed.expression(), relation, *field)?
                    && matched.replace(*field).is_some()
                {
                    return Err(super::invalid("ambiguous essential Field boundary"));
                }
            }
            let field = matched.ok_or_else(|| {
                super::invalid(
                    "every boundary equation must prescribe one homogeneous unknown trace",
                )
            })?;
            if !covered.insert(field) {
                return Err(super::invalid("duplicate essential Field boundary"));
            }
            boundaries
                .entry(field)
                .or_insert_with(Vec::new)
                .push(domain.id().erase());
        }
        if covered.len() != fields.len() {
            return Err(super::invalid(
                "every unknown Field requires homogeneous essential coverage on every side",
            ));
        }
    }
    if sides.len() != 2 * dimension
        || (0..dimension).any(|axis| {
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
        relations: boundary_relations,
    })
}
