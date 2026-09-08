//! Resolve declared alias activation against exact occurrence symbols.
use super::*;
use crate::hierarchy::body_check::DependencyActivation;

pub(in crate::hierarchy) fn port_activation(
    file: &str,
    syntax: &PortSyntax,
    range: TextRange,
    scope: &Scope,
) -> Result<ActivationSyntax, Diagnostic> {
    match syntax {
        PortSyntax::Signal { activation, .. } => rewrite_activation(file, activation, range, scope),
        _ => Ok(ActivationSyntax::Continuous),
    }
}

impl Scope {
    pub(in crate::hierarchy) fn alias_activation(
        &self,
        file: &str,
        expression: &Expr,
    ) -> Result<DependencyActivation, Diagnostic> {
        let declared = |activation: &ActivationSyntax| match activation {
            ActivationSyntax::Periodic(clock) => DependencyActivation::Clock(clock.clone()),
            _ => DependencyActivation::Continuous,
        };
        let mut invalid_selection = None;
        let profile = DependencyActivation::infer_with(
            expression,
            |expression| {
                let symbol = match expression.kind() {
                    ExprKind::Name(name) => {
                        if let Some(value) = self.value_activation(name) {
                            return Some(value);
                        }
                        if let Some((_, activation)) = self.field_evolution.get(name) {
                            return Some(declared(activation));
                        }
                        self.symbol(name)
                    }
                    ExprKind::Path(path) => self.resolve_symbol(path),
                    ExprKind::Member { .. } => match self.indexed_port(file, expression) {
                        Ok(symbol) => Some(symbol),
                        Err(error) => {
                            invalid_selection = Some(error);
                            None
                        }
                    },
                    _ => None,
                }?;
                Some(match &symbol.kind {
                    SymbolKind::Port(activation) => declared(activation),
                    SymbolKind::Field => DependencyActivation::Continuous,
                    _ => DependencyActivation::Static,
                })
            },
            |name| {
                self.symbol(name)
                    .map_or_else(|| name.to_owned(), |symbol| symbol.internal_name.clone())
            },
        );
        match invalid_selection {
            Some(error) => Err(error),
            None => Ok(profile),
        }
    }
}
