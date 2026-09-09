//! Whole-Model admission for derived quantities, separate from solve inventory.

use super::*;

impl KernelProgram {
    /// Infer an Observable's retained expression in this exact Model.
    /// # Errors
    /// Rejects foreign identities or incompatible expression/measure types.
    pub fn typed_observable(
        &self,
        observable: Id<kinds::Observable>,
    ) -> Result<TypedResidual<RawId>, Vec<Diagnostic>> {
        let Some(KernelNode::Observable(definition)) = self.nodes.get(&observable.erase()) else {
            return Err(vec![kernel_error(
                observable.erase(),
                "Observable is outside the selected Model",
            )]);
        };
        let typed = self.type_derived_residual(
            definition.expression().clone(),
            observable.erase(),
            definition.reduction().domain().map(Id::erase),
            RootContract::Observable,
        )?;
        let root = typed
            .node_type(definition.expression().roots()[0])
            .expect("typed root exists");
        definition
            .validate_type(
                root,
                definition
                    .reduction()
                    .domain()
                    .and_then(|id| self.spatial_supports.get(&id.erase())),
            )
            .map_err(|error| vec![error])?;
        Ok(typed)
    }
}

pub(super) fn validate(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    spatial_supports: &BTreeMap<RawId, SpatialSupport<RawId>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (&id, node) in nodes {
        let KernelNode::Observable(observable) = node else {
            continue;
        };
        let scope = observable.reduction().domain().map(Id::erase);
        let declared_scopes = edge_targets(edges, id, EdgeKind::AppliesOn);
        if declared_scopes != scope.into_iter().collect() {
            diagnostics.push(kernel_error(
                id,
                "Observable AppliesOn must name exactly its integration Domain",
            ));
        }
        let symbols = validate_expression(
            observable.expression(),
            id,
            TypingEnvironment {
                nodes,
                edges,
                spatial_supports,
            },
            scope,
            RootContract::Observable,
            diagnostics,
        );
        if symbols != edge_targets(edges, id, EdgeKind::DependsOn) {
            diagnostics.push(kernel_error(
                id,
                "Observable dependencies differ from its retained expression symbols",
            ));
        }
        if observable.expression().nodes().iter().any(|node| {
            matches!(
                node,
                ExprNode::Symbol(SymbolRef::Derivative(_) | SymbolRef::Pre(_) | SymbolRef::Next(_))
            )
        }) {
            diagnostics.push(kernel_error(
                id,
                "instantaneous Observable cannot read temporal derivative or event-side symbols",
            ));
        }
        let support = scope.and_then(|id| spatial_supports.get(&id));
        if let Ok(typed) = TypedResidual::infer(
            observable.expression().clone(),
            support.cloned(),
            RootContract::Observable,
            |symbol| symbol_type(symbol, nodes, edges, spatial_supports),
        ) {
            let root = typed
                .node_type(observable.expression().roots()[0])
                .expect("typed root exists");
            if let Err(error) = observable.validate_type(root, support) {
                diagnostics.push(error);
            }
        }
    }
}
