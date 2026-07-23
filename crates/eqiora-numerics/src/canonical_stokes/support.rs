use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, GraphPath, RawId};
use eqiora_graph::EdgeKind;
use eqiora_schema::kernel::typing::TypedResidual;
use eqiora_schema::kernel::{
    ActivationKind, ExprDag, ExprId, ExprNode, KernelNode, RepresentationKind, SymbolRef,
};
use eqiora_sem::KernelProgram;

pub(super) fn require_continuous_relation(
    program: &KernelProgram,
    relation: RawId,
) -> Result<(), Diagnostic> {
    let activations = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Activates && edge.to() == relation)
        .filter_map(|edge| match program.node(edge.from()) {
            Some(KernelNode::Activation(activation)) => Some(activation),
            _ => None,
        })
        .collect::<Vec<_>>();
    if activations.len() == 1 && matches!(activations[0].kind(), ActivationKind::Continuous) {
        Ok(())
    } else {
        Err(lowering_error(
            relation,
            "canonical fluid Relations require exactly one continuous Activation",
        ))
    }
}

pub(super) fn continuum_representation(program: &KernelProgram, field: RawId) -> Option<RawId> {
    let representations = program
        .edges()
        .iter()
        .filter(|edge| edge.from() == field && edge.kind() == EdgeKind::DefinedOn)
        .filter_map(|edge| match program.node(edge.to()) {
            Some(KernelNode::Representation(representation))
                if representation.kind() == RepresentationKind::Continuum =>
            {
                Some(edge.to())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    (representations.len() == 1).then(|| representations[0])
}

pub(super) fn relations_on(program: &KernelProgram, domain: RawId) -> Vec<RawId> {
    program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::AppliesOn && edge.to() == domain)
        .map(|edge| edge.from())
        .collect()
}

pub(super) fn relation_expression(
    program: &KernelProgram,
    relation: RawId,
) -> Result<&ExprDag, Diagnostic> {
    match program.node(relation) {
        Some(KernelNode::Relation(relation)) => Ok(relation.residuals()),
        _ => Err(lowering_error(
            relation,
            "AppliesOn source has no Relation definition",
        )),
    }
}

pub(super) fn typed_relation(
    program: &KernelProgram,
    relation: RawId,
) -> Result<TypedResidual<RawId>, Diagnostic> {
    let relation_id = relation
        .downcast::<kinds::Relation>()
        .ok_or_else(|| lowering_error(relation, "calculus typing owner is not a Relation"))?;
    program
        .typed_relation_residual(relation_id)
        .map_err(|diagnostics| {
            diagnostics.into_iter().next().unwrap_or_else(|| {
                lowering_error(relation, "calculus typing failed without a diagnostic")
            })
        })
}

pub(super) fn unique_root(expression: &ExprDag, owner: RawId) -> Result<ExprId, Diagnostic> {
    if expression.roots().len() == 1 {
        Ok(expression.roots()[0])
    } else {
        Err(lowering_error(
            owner,
            "canonical fluid Relation requires exactly one residual root",
        ))
    }
}

pub(super) fn is_field(expression: &ExprDag, value: ExprId, field: RawId) -> bool {
    matches!(
        expression.node(value),
        Some(ExprNode::Symbol(SymbolRef::Field(id))) if id.erase() == field
    )
}

pub(super) fn has_edge(program: &KernelProgram, from: RawId, to: RawId, kind: EdgeKind) -> bool {
    program
        .edges()
        .iter()
        .any(|edge| edge.from() == from && edge.to() == to && edge.kind() == kind)
}

pub(super) fn lowering_error(owner: RawId, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_SPATIAL_LOWERING, message).with_graph_path(GraphPath::new([
        owner.kind().graph().name().to_owned(),
        format!("{:?}", owner.kind()),
        owner.to_string(),
    ]))
}

pub(super) fn model_lowering_error(
    program: &KernelProgram,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(codes::INVALID_SPATIAL_LOWERING, message).with_graph_path(GraphPath::new([
        "ontology-view".to_owned(),
        "eqiora.model/v1".to_owned(),
        program.model().to_string(),
    ]))
}
