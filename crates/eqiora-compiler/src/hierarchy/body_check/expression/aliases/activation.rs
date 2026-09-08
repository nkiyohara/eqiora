//! Declared dependency clocks, independent of evolution-use obligations.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::hierarchy) enum DependencyActivation {
    Static,
    Continuous,
    Clock(String),
    Clocks(BTreeSet<String>),
    Mixed,
}

impl DependencyActivation {
    pub(in crate::hierarchy) fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Static, value) | (value, Self::Static) => value,
            (left, right) if left == right => left,
            (Self::Clock(left), Self::Clock(right)) => Self::Clocks(BTreeSet::from([left, right])),
            (Self::Clocks(mut clocks), Self::Clock(clock))
            | (Self::Clock(clock), Self::Clocks(mut clocks)) => {
                clocks.insert(clock);
                Self::Clocks(clocks)
            }
            (Self::Clocks(mut left), Self::Clocks(right)) => {
                left.extend(right);
                Self::Clocks(left)
            }
            _ => Self::Mixed,
        }
    }

    fn symbol(symbol: &SymbolContract) -> Self {
        match symbol {
            SymbolContract::Field(_, _, ActivationSyntax::Periodic(clock)) => {
                Self::Clock(clock.clone())
            }
            SymbolContract::Port(PortContract::Signal {
                activation: ActivationSyntax::Periodic(clock),
                ..
            }) => Self::Clock(clock.clone()),
            SymbolContract::Field(..) | SymbolContract::Port(_) => Self::Continuous,
            SymbolContract::Alias(alias) => alias.activation.clone(),
            _ => Self::Static,
        }
    }

    pub(super) fn infer(scope: &DefinitionScope<'_, '_>, expression: &Expr) -> Self {
        Self::infer_with(
            expression,
            |expression| match expression.kind() {
                ExprKind::Reduction { binder, value, .. } => {
                    let extent = scope
                        .index_sets
                        .get(binder.set().as_str())
                        .copied()
                        .flatten()?;
                    let mut profile = Self::Static;
                    for ordinal in 0..extent {
                        let term = crate::hierarchy::reductions::instantiate(
                            scope.file, value, binder, ordinal,
                        )
                        .ok()?;
                        profile = profile.join(Self::infer(scope, &term));
                    }
                    Some(profile)
                }
                ExprKind::Name(name) => scope.symbols.get(name).map(Self::symbol),
                ExprKind::Path(path) => scope.resolve_symbol(path).ok().as_ref().map(Self::symbol),
                ExprKind::Member { .. } => scope
                    .indexed_member(expression)
                    .ok()
                    .and_then(|(path, _)| scope.resolve_symbol(&path).ok())
                    .as_ref()
                    .map(Self::symbol),
                _ => None,
            },
            |clock| clock.to_owned(),
        )
    }

    pub(in crate::hierarchy) fn infer_with(
        expression: &Expr,
        mut symbol: impl FnMut(&Expr) -> Option<Self>,
        mut clock: impl FnMut(&str) -> String,
    ) -> Self {
        let mut profile = Self::Static;
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            let contribution = match expression.kind() {
                ExprKind::Name(name) if name == "time" => Self::Continuous,
                ExprKind::Name(_) | ExprKind::Path(_) | ExprKind::Member { .. } => {
                    symbol(expression).unwrap_or(Self::Static)
                }
                ExprKind::BoundaryPortSelection { .. } => Self::Continuous,
                ExprKind::Reduction { .. } => symbol(expression).unwrap_or(Self::Mixed),
                ExprKind::Array(elements) => {
                    pending.extend(elements);
                    Self::Static
                }
                ExprKind::Index { value, .. } => {
                    pending.push(value);
                    Self::Static
                }
                ExprKind::Unary { value, .. } => {
                    pending.push(value);
                    Self::Static
                }
                ExprKind::Binary { left, right, .. } => {
                    pending.extend([left.as_ref(), right.as_ref()]);
                    Self::Static
                }
                ExprKind::Call { callee, .. } if callee.as_str() == "hold" => Self::Continuous,
                ExprKind::Call { callee, arguments } if callee.as_str() == "sample" => {
                    match arguments
                        .positional()
                        .and_then(|arguments| arguments.get(1))
                        .map(Expr::kind)
                    {
                        Some(ExprKind::Name(name)) => Self::Clock(clock(name)),
                        _ => Self::Mixed,
                    }
                }
                ExprKind::Call { arguments, .. } => {
                    // Evolution operators retain their target's declared activation too.
                    // The intrinsic checker has already validated identity-only targets.
                    pending.extend(arguments.expressions());
                    Self::Static
                }
                _ => Self::Static,
            };
            profile = profile.join(contribution);
        }
        profile
    }

    pub(super) fn validate(
        &self,
        scope: &DefinitionScope<'_, '_>,
        declaration: &eqiora_lang::NamedDefinitionDecl,
    ) -> Result<(), Diagnostic> {
        let Some(clock) = declaration.activation() else {
            return Ok(());
        };
        if !matches!(scope.symbols.get(clock), Some(SymbolContract::Clock)) {
            return Err(scope.wrong_local_kind(
                declaration.range(),
                clock,
                "let alias clock activation",
            ));
        }
        let deferred = match self {
            Self::Clock(dependency) => {
                scope.borrowed_clocks.contains(dependency) || scope.borrowed_clocks.contains(clock)
            }
            Self::Clocks(dependencies) => {
                dependencies
                    .iter()
                    .all(|name| scope.borrowed_clocks.contains(name))
                    && scope.borrowed_clocks.contains(clock)
            }
            _ => false,
        };
        if self != &Self::Clock(clock.to_owned()) && !deferred {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                declaration.range(),
                "let alias activation assertion does not match its exact declared dependency clock",
            ));
        }
        Ok(())
    }
}
