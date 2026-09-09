//! Target-directed classification of the single source named-binding family.
use crate::diagnostics::source_error;
use eqiora_core::{Diagnostic, diagnostic::codes};
use eqiora_lang::{
    ComponentDecl, ExprKind, InstanceDecl, NamedBindingDecl, SignatureItem, TextRange,
};

pub(super) struct ReferenceBinding<'a> {
    binding: &'a NamedBindingDecl,
    target: &'a str,
}
impl<'a> ReferenceBinding<'a> {
    pub(super) fn slot(&self) -> &'a str {
        self.binding.name()
    }
    pub(super) fn target(&self) -> &'a str {
        self.target
    }
    pub(super) fn range(&self) -> TextRange {
        self.binding.range()
    }
}

pub(super) fn references<'a>(
    file: &str,
    instance: &'a InstanceDecl,
    accepts: impl Fn(&NamedBindingDecl) -> bool,
    errors: &mut Vec<Diagnostic>,
) -> Vec<ReferenceBinding<'a>> {
    exact_references(file, instance, accepts, errors, false)
}

pub(super) fn field_references<'a>(
    file: &str,
    instance: &'a InstanceDecl,
    accepts: impl Fn(&NamedBindingDecl) -> bool,
    errors: &mut Vec<Diagnostic>,
) -> Vec<ReferenceBinding<'a>> {
    exact_references(file, instance, accepts, errors, true)
}

fn exact_references<'a>(
    file: &str,
    instance: &'a InstanceDecl,
    accepts: impl Fn(&NamedBindingDecl) -> bool,
    errors: &mut Vec<Diagnostic>,
    allow_member: bool,
) -> Vec<ReferenceBinding<'a>> {
    instance
        .bindings()
        .iter()
        .filter(|binding| accepts(binding))
        .filter_map(|binding| {
            let target = match binding.value().kind() {
                ExprKind::Name(target) => Some(target.as_str()),
                ExprKind::Path(target) if allow_member => Some(target.as_str()),
                _ => None,
            };
            if let Some(target) = target {
                Some(ReferenceBinding { binding, target })
            } else {
                errors.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    binding.value().range(),
                    format!(
                        "binding `{}` requires one exact enclosing named reference",
                        binding.name()
                    ),
                ));
                None
            }
        })
        .collect()
}

pub(super) fn validate_names(
    file: &str,
    component: &ComponentDecl,
    instance: &InstanceDecl,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for binding in instance.bindings() {
        if !seen.insert(binding.name()) {
            errors.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                binding.range(),
                format!("duplicate named binding `{}`", binding.name()),
            ));
        }
        match component
            .signature()
            .iter()
            .find(|item| item.name() == binding.name())
        {
            None => errors.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                binding.range(),
                format!(
                    "`{}` is not a public requirement of `{}`",
                    binding.name(),
                    component.name()
                ),
            )),
            Some(
                SignatureItem::Output(_) | SignatureItem::Port(_) | SignatureItem::PortFamily(_),
            ) => errors.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                binding.range(),
                format!(
                    "`{}` is an owned exposed endpoint, not a borrowed requirement",
                    binding.name()
                ),
            )),
            _ => {}
        }
    }
    errors
}

pub(super) struct BoundarySetBinding<'a> {
    binding: &'a NamedBindingDecl,
    members: Vec<BoundaryMember<'a>>,
}
impl<'a> BoundarySetBinding<'a> {
    pub(super) fn slot(&self) -> &'a str {
        self.binding.name()
    }
    pub(super) fn range(&self) -> TextRange {
        self.binding.range()
    }
    pub(super) fn members(&self) -> &[BoundaryMember<'a>] {
        &self.members
    }
}
pub(super) struct BoundaryMember<'a> {
    target: &'a str,
    range: TextRange,
}
impl<'a> BoundaryMember<'a> {
    pub(super) fn target(&self) -> &'a str {
        self.target
    }
    pub(super) fn range(&self) -> TextRange {
        self.range
    }
}
pub(super) fn is_boundary_set(binding: &NamedBindingDecl) -> bool {
    matches!(binding.value().kind(), ExprKind::Call { callee,.. } if callee.as_str()=="boundaries")
}
pub(super) fn boundary_sets<'a>(
    file: &str,
    instance: &'a InstanceDecl,
    accepts: impl Fn(&str) -> bool,
    errors: &mut Vec<Diagnostic>,
) -> Vec<BoundarySetBinding<'a>> {
    instance
        .bindings()
        .iter()
        .filter(|binding| accepts(binding.name()) && is_boundary_set(binding))
        .filter_map(|binding| {
            let ExprKind::Call {
                arguments: eqiora_lang::CallArguments::Positional(arguments),
                ..
            } = binding.value().kind()
            else {
                unreachable!()
            };
            let mut members = Vec::with_capacity(arguments.len());
            for argument in arguments {
                if let ExprKind::Name(target) = argument.kind() {
                    members.push(BoundaryMember {
                        target,
                        range: argument.range(),
                    });
                } else {
                    errors.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        argument.range(),
                        "boundary member must name one enclosing boundary",
                    ));
                    return None;
                }
            }
            Some(BoundarySetBinding { binding, members })
        })
        .collect()
}

/// Input arguments are explicit directed endpoint connections, not value synthesis.
pub(super) fn input_connections(
    file: &str,
    component: &ComponentDecl,
    instance: &InstanceDecl,
) -> Result<Vec<eqiora_lang::ConnectionDecl>, Diagnostic> {
    use eqiora_lang::{NamePath, SourceAstFactory};
    instance.bindings().iter().filter(|binding| component.signature().iter().any(|item|
        matches!(item, SignatureItem::Input(input) if input.name() == binding.name())))
        .map(|binding| {
            let fail = |message: &str| source_error(codes::LANGUAGE_TYPE_ERROR, file, binding.range(), message);
            let source = match binding.value().kind() {
                ExprKind::Name(_) | ExprKind::Path(_) | ExprKind::Member { .. } => binding.value().clone(),
                _ => return Err(fail("Input binding requires an exact causal endpoint; arbitrary value expressions do not create a driver")),
            };
            let target = NamePath::from_segments([instance.name(), binding.name()], binding.range()).map_err(|error| fail(error.message()))?;
            SourceAstFactory::connection(eqiora_lang::ConnectionSyntax::Signal, None, vec![source, SourceAstFactory::expression(ExprKind::Path(target), binding.range()).map_err(|error| fail(error.message()))?], binding.range()).map_err(|error| fail(error.message()))
        }).collect()
}
