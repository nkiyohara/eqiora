//! Destructure nominal static products before the ordinary Parameter resolver.
use std::collections::BTreeMap;

use eqiora_core::{Diagnostic, Id, diagnostic::codes, entity::kinds};
use eqiora_lang::{
    ComponentParameterDecl, Expr, ExprKind, NamePath, SourceAstFactory, TextRange, VisibilitySyntax,
};

use super::BoundRecord;
use crate::diagnostics::source_error;

/// Select a constructor or exact typed reference, retaining ordinary leaf expressions.
/// Both callbacks resolve in the expression's lexical scope, not the target's scope.
/// The reference callback admits static record Parameters, never record Fields.
pub(crate) fn member_initializers(
    file: &str,
    record: &BoundRecord,
    expression: &Expr,
    mut visible_record: impl FnMut(&NamePath) -> Option<Id<kinds::Record>>,
    mut reference_record: impl FnMut(&NamePath) -> Option<Id<kinds::Record>>,
) -> Result<Vec<Expr>, Diagnostic> {
    let invalid =
        |range, message: &str| source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message);
    match expression.kind() {
        ExprKind::Call { callee, arguments } => {
            if visible_record(callee) != Some(record.definition.id()) {
                return Err(invalid(
                    callee.range(),
                    "record constructor requires the exact declared record identity",
                ));
            }
            let Some(arguments) = arguments.named() else {
                return Err(invalid(
                    expression.range(),
                    "record constructor requires every member exactly once as a named argument",
                ));
            };
            let mut named = BTreeMap::new();
            for argument in arguments {
                if named.insert(argument.name(), argument.value()).is_some() {
                    return Err(invalid(
                        argument.range(),
                        "record constructor contains a duplicate member",
                    ));
                }
            }
            let mut ordered = Vec::with_capacity(record.definition.members().len());
            for (name, _) in record.definition.members() {
                let Some(value) = named.remove(name.as_str()) else {
                    return Err(invalid(
                        expression.range(),
                        "record constructor is missing a declared member",
                    ));
                };
                ordered.push(value.clone());
            }
            if !named.is_empty() {
                return Err(invalid(
                    expression.range(),
                    "record constructor contains an undeclared member",
                ));
            }
            Ok(ordered)
        }
        ExprKind::Name(_) | ExprKind::Path(_) => {
            let path = match expression.kind() {
                ExprKind::Name(name) => {
                    NamePath::from_segments([name.as_str()], expression.range())
                        .map_err(|error| invalid(expression.range(), error.message()))?
                }
                ExprKind::Path(path) => path.clone(),
                _ => unreachable!("matched named reference"),
            };
            if reference_record(&path) != Some(record.definition.id()) {
                return Err(invalid(
                    expression.range(),
                    "record Parameter forwarding requires the exact declared record identity",
                ));
            }
            record
                .definition
                .members()
                .iter()
                .map(|(name, _)| {
                    let member = NamePath::from_segments(
                        path.segments().chain(std::iter::once(name.as_str())),
                        expression.range(),
                    )
                    .map_err(|error| invalid(expression.range(), error.message()))?;
                    SourceAstFactory::expression(ExprKind::Path(member), expression.range())
                        .map_err(|error| invalid(expression.range(), error.message()))
                })
                .collect()
        }
        _ => Err(invalid(
            expression.range(),
            "record Parameter requires a named constructor or an exact typed record reference",
        )),
    }
}

/// Build ordinary scalar/shaped declarations under lexical `record.member` map keys.
/// The scalar resolver owns evaluation, dependency order, coercion and derivative lineage.
pub(crate) fn parameter_leaves(
    file: &str,
    name: &str,
    record: &BoundRecord,
    visibility: VisibilitySyntax,
    initializers: Option<&[Expr]>,
    range: TextRange,
) -> Result<Vec<(String, ComponentParameterDecl)>, Diagnostic> {
    let invalid = |message: &str| source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message);
    let members = record.definition.members();
    if record.member_syntax.len() != members.len()
        || initializers.is_some_and(|values| values.len() != members.len())
    {
        return Err(invalid(
            "record Parameter expansion requires exactly one type and initializer per declared member",
        ));
    }
    members
        .iter()
        .zip(&record.member_syntax)
        .enumerate()
        .map(|(index, ((member, _), syntax))| {
            let declaration = SourceAstFactory::component_parameter(
                visibility,
                member,
                syntax.clone(),
                initializers.map(|values| values[index].clone()),
                range,
            )
            .map_err(|error| invalid(error.message()))?;
            Ok((format!("{name}.{member}"), declaration))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> BoundRecord {
        let document = eqiora_lang::parse("record.eqi", "record Config { gain: 1, valid: bool }")
            .into_document()
            .unwrap();
        let namespace = crate::identity::IdentityNamespace::new(["test"]).unwrap();
        super::super::declarations("record.eqi", &document, &namespace, |_| None)
            .unwrap()
            .remove("Config")
            .unwrap()
    }

    fn expression(source: &str) -> Expr {
        let document = eqiora_lang::parse(
            "record.eqi",
            &format!("model M() {{ parameter c: Config = {source}; }}"),
        )
        .into_document()
        .unwrap();
        let eqiora_lang::Item::Parameter(parameter) = &document.models()[0].items()[0] else {
            panic!("Parameter")
        };
        parameter.value().clone()
    }

    #[test]
    fn constructor_alias_orders_members_and_preserves_expressions_and_ranges() {
        let record = record();
        let value = expression("alias.Config(valid=true, gain=2*p)");
        let leaves = member_initializers(
            "record.eqi",
            &record,
            &value,
            |path| (path.as_str() == "alias.Config").then_some(record.definition.id()),
            |_| None,
        )
        .unwrap();
        let ExprKind::Call { arguments, .. } = value.kind() else {
            panic!("constructor")
        };
        let arguments = arguments.named().unwrap();
        assert_eq!(
            leaves,
            [arguments[1].value().clone(), arguments[0].value().clone()]
        );
        let declarations = parameter_leaves(
            "record.eqi",
            "c",
            &record,
            VisibilitySyntax::Public,
            Some(&leaves),
            value.range(),
        )
        .unwrap();
        assert_eq!(declarations[0].0, "c.gain");
        assert_eq!(declarations[0].1.name(), "gain");
        assert_eq!(declarations[0].1.default(), Some(&leaves[0]));
        assert_eq!(declarations[0].1.range(), value.range());
        assert_eq!(declarations[1].1.value_type(), &record.member_syntax[1]);
    }

    #[test]
    fn forwarding_keeps_lexical_member_paths_for_parameter_lineage() {
        let record = record();
        let value = expression("source");
        let leaves = member_initializers(
            "record.eqi",
            &record,
            &value,
            |_| None,
            |path| (path.as_str() == "source").then_some(record.definition.id()),
        )
        .unwrap();
        for (leaf, expected) in leaves.iter().zip(["source.gain", "source.valid"]) {
            let ExprKind::Path(path) = leaf.kind() else {
                panic!("member reference")
            };
            assert_eq!(path.as_str(), expected);
            assert_eq!(leaf.range(), value.range());
        }
        let declarations = parameter_leaves(
            "record.eqi",
            "c",
            &record,
            VisibilitySyntax::Public,
            None,
            value.range(),
        )
        .unwrap();
        assert!(
            declarations
                .iter()
                .all(|(_, declaration)| declaration.default().is_none())
        );
        assert!(
            member_initializers("record.eqi", &record, &value, |_| None, |_| Some(Id::new()))
                .is_err()
        );
    }

    #[test]
    fn constructors_reject_foreign_identity_positional_missing_and_extra_members() {
        let record = record();
        for source in [
            "Config(2,true)",
            "Config(gain=2)",
            "Config(gain=2,valid=true,extra=3)",
        ] {
            assert!(
                member_initializers(
                    "record.eqi",
                    &record,
                    &expression(source),
                    |_| Some(record.definition.id()),
                    |_| None
                )
                .is_err(),
                "{source}"
            );
        }
        assert!(
            member_initializers(
                "record.eqi",
                &record,
                &expression("Config(gain=2,valid=true)"),
                |_| Some(Id::new()),
                |_| None
            )
            .is_err()
        );
    }
}
