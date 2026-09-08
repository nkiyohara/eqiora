//! On-demand numerical projection of admitted equation sides; never persisted.
use super::*;
use eqiora_core::ScalarDomain;
use eqiora_schema::kernel::ExprDagBuilder;

impl KernelProgram {
    /// Derive numerical residuals while retaining original node indices and sharing.
    /// Discrete equations have no subtraction projection and must use typed execution.
    ///
    /// # Errors
    /// Rejects absent Relations, non-numerical equation sides, or failed typed projection.
    pub fn numerical_residuals(&self, relation: RawId) -> Result<ExprDag, Diagnostic> {
        let Some(KernelNode::Relation(definition)) = self.node(relation) else {
            return Err(kernel_error(
                relation,
                "numerical projection requires a Relation",
            ));
        };
        let scope = edge_targets(&self.edges, relation, EdgeKind::AppliesOn)
            .first()
            .copied();
        let typed = self
            .type_derived_residual(
                definition.expression().clone(),
                relation,
                scope,
                if definition.is_initial() {
                    RootContract::InitialConditions
                } else {
                    RootContract::EquationSides
                },
            )
            .map_err(|errors| {
                errors
                    .into_iter()
                    .next()
                    .expect("typing failure has diagnostics")
            })?;
        for (left, right) in definition.equation_sides() {
            for id in [left, right] {
                let value = &typed
                    .node_type(id)
                    .expect("validated equation side")
                    .value_type;
                if !matches!(
                    value.scalar_domain(),
                    ScalarDomain::Real | ScalarDomain::Complex
                ) {
                    return Err(kernel_error(
                        relation,
                        "discrete equation sides have no numerical residual projection",
                    ));
                }
            }
        }
        let mut builder = ExprDagBuilder::new();
        for node in definition.expression().nodes() {
            match node {
                ExprNode::PureOperatorApplication(application) => {
                    let pure = definition
                        .expression()
                        .definition(application.definition())
                        .expect("validated pure definition");
                    builder.pure_operator(pure, application.arguments().iter().copied())?;
                }
                _ => {
                    builder.push(node.clone())?;
                }
            }
        }
        let mut roots = Vec::with_capacity(definition.equation_sides().len());
        for (left, right) in definition.equation_sides() {
            let left_type = typed.node_type(left).expect("validated side");
            let right_type = typed.node_type(right).expect("validated side");
            let same =
                typing::additive(left_type, right_type).is_ok_and(|result| result == *left_type);
            let zero = matches!(definition.expression().nodes().get(right.index() as usize),Some(ExprNode::Constant(value)) if value.is_zero());
            roots.push(if same && zero {
                left
            } else {
                builder.sub(left, right)?
            });
        }
        builder.finish(roots)
    }
}
