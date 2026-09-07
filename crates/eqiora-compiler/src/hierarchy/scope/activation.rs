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
    pub(in crate::hierarchy) fn alias_activation(&self, expression: &Expr) -> DependencyActivation {
        let declared = |activation: &ActivationSyntax| match activation {
            ActivationSyntax::Periodic(clock) => DependencyActivation::Clock(clock.clone()),
            _ => DependencyActivation::Continuous,
        };
        DependencyActivation::infer_with(
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
        )
    }
}
