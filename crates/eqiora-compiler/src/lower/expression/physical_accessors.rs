//! Lower accessors on existing physical Port contracts.
use super::*;

impl ExpressionLowerer<'_> {
    pub(super) fn lower_physical_accessor(
        &mut self,
        expression: &LoweringExpression,
        callee: &str,
        argument: &LoweringExpression,
    ) -> Result<TypedExpression, Diagnostic> {
        let LoweringExpressionNode::Name(name) = argument.node.as_ref() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                argument.range(),
                if matches!(callee, "across" | "through") {
                    format!("`{callee}(...)` requires one bare scalar physical Port name")
                } else {
                    format!("`{callee}(...)` requires one bare field-physical Port name")
                },
            ));
        };
        let Some(binding) = self.bindings.get(name) else {
            return Err(unresolved(
                self.file,
                argument.range(),
                name,
                "scalar physical Port",
            ));
        };
        let Binding::Port(port, contract) = binding else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                argument.range(),
                format!("`{name}` is not a scalar physical Port"),
            ));
        };
        let contract = resolve_port_contract(self.file, argument.range(), contract, self.bindings)?;
        self.dependencies.insert(port.erase());
        self.ports.insert(port.erase());
        let (symbol, dimension) = match (callee, contract) {
            ("across", ResolvedPortContract::ScalarPhysical { across_type, .. }) => {
                (SymbolRef::Across(*port), across_type.dimension())
            }
            ("through", ResolvedPortContract::ScalarPhysical { through_type, .. }) => {
                (SymbolRef::Through(*port), through_type.dimension())
            }
            ("trace", ResolvedPortContract::BoundaryPhysical { trace_type, .. }) => {
                (SymbolRef::PortTrace(*port), trace_type.dimension())
            }
            ("flux", ResolvedPortContract::BoundaryPhysical { flux_type, .. }) => {
                (SymbolRef::PortFlux(*port), flux_type.dimension())
            }
            _ => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    argument.range(),
                    if matches!(callee, "across" | "through") {
                        format!("`{name}` is not a scalar physical Port")
                    } else {
                        format!("`{name}` is not a field-physical Port")
                    },
                ));
            }
        };
        self.builder
            .symbol(symbol)
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }
}
