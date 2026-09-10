//! Shared source-level SI-dimension checking.
//!
//! Flat and hierarchical source paths consume these exact operations before
//! canonical lowering. Keeping them here prevents either path from becoming
//! the accidental owner of physical-dimension semantics.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents};
use eqiora_lang::{BinaryOp, Document, Expr, ExprKind, SourceAstFactory, TextRange, UnaryOp};

use crate::diagnostics::source_error;
use crate::units::coherent_dimension;

mod resolved;
pub(crate) use resolved::bind_resolved;

pub(crate) fn lower_dimension(file: &str, expression: &Expr) -> Result<DimExponents, Diagnostic> {
    lower_dimension_with_aliases(file, expression, &BTreeMap::new(), None)
}

fn lower_dimension_with_aliases(
    file: &str,
    expression: &Expr,
    aliases: &BTreeMap<String, DimExponents>,
    declared_names: Option<&BTreeSet<String>>,
) -> Result<DimExponents, Diagnostic> {
    match expression.kind() {
        ExprKind::Number(value) if value.to_i64().ok() == Some(1) => {
            Ok(DimExponents::DIMENSIONLESS)
        }
        ExprKind::Name(name) => coherent_dimension(name)
            .or_else(|| aliases.get(name).copied())
            .ok_or_else(|| {
                let message = if declared_names.is_some_and(|names| names.contains(name)) {
                    format!("dimension alias `{name}` is a forward or self reference")
                } else if aliases.is_empty() && declared_names.is_none() {
                    format!("unknown SI base-dimension symbol `{name}`")
                } else {
                    format!("unknown coherent-SI dimension symbol or alias `{name}`")
                };
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    message,
                )
            }),
        ExprKind::Path(path) => aliases.get(&path.to_string()).copied().ok_or_else(|| {
            source_error(codes::LANGUAGE_TYPE_ERROR, file, expression.range(),
                format!("unknown or private dimension alias `{path}`; imports require one direct alias-qualified name"))
        }),
        ExprKind::Binary { op, left, right } if matches!(op, BinaryOp::Mul | BinaryOp::Div) => {
            let left = lower_dimension_with_aliases(file, left, aliases, declared_names)?;
            let right = lower_dimension_with_aliases(file, right, aliases, declared_names)?;
            let operation = if *op == BinaryOp::Mul {
                DimExponents::mul
            } else {
                DimExponents::div
            };
            operation(left, right).ok_or_else(|| dimension_overflow(file, expression.range()))
        }
        ExprKind::Binary {
            op: BinaryOp::Pow,
            left,
            right,
        } => {
            let dimension = lower_dimension_with_aliases(file, left, aliases, declared_names)?;
            let (numerator, denominator) = rational_literal(right).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    right.range(),
                    "dimension power must be a bounded integer or a ratio of integers with positive denominator",
                )
            })?;
            dimension
                .pow(numerator, denominator)
                .ok_or_else(|| dimension_overflow(file, expression.range()))
        }
        _ => Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            expression.range(),
            "dimension must use `1`, SI base symbols, `*`, `/`, and exact rational powers",
        )),
    }
}

pub(crate) fn elaborate_dimension_aliases<'a>(
    file: &str,
    document: &'a Document,
) -> Result<Cow<'a, Document>, Vec<Diagnostic>> {
    if document.dimensions().is_empty() {
        return Ok(Cow::Borrowed(document));
    }
    let aliases = resolve_aliases(file, document, BTreeMap::new())?;
    let mut elaborated = document.clone();
    rewrite_document(&mut elaborated, &aliases);
    Ok(Cow::Owned(elaborated))
}

fn resolve_aliases(
    file: &str,
    document: &Document,
    mut aliases: BTreeMap<String, DimExponents>,
) -> Result<BTreeMap<String, DimExponents>, Vec<Diagnostic>> {
    let mut pending = BTreeMap::new();
    for declaration in document.dimensions() {
        let (name, expression, range) =
            (declaration.name(), declaration.value(), declaration.range());
        let error = if name == crate::math::ROOT {
            Some("identifier `math` is reserved for compiler-owned scalar mathematics".to_owned())
        } else if coherent_dimension(name).is_some() {
            Some(format!(
                "dimension alias `{name}` cannot shadow a coherent-SI symbol"
            ))
        } else if pending.insert(name, (expression, range)).is_some() {
            Some(format!("duplicate dimension alias `{name}`"))
        } else {
            None
        };
        if let Some(message) = error {
            return Err(vec![source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                message,
            )]);
        }
    }
    let declared = pending
        .keys()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    let mut depth = BTreeMap::<String, usize>::new();
    while !pending.is_empty() {
        let mut ready = Vec::new();
        for (&name, &(expression, range)) in &pending {
            let mut references = BTreeSet::new();
            dimension_references(expression, &mut references);
            for reference in &references {
                if coherent_dimension(reference).is_none()
                    && !aliases.contains_key(reference)
                    && !declared.contains(reference)
                {
                    return Err(vec![source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        range,
                        format!(
                            "unknown coherent-SI dimension symbol or alias `{reference}` (unknown or private direct import)"
                        ),
                    )]);
                }
            }
            if references
                .iter()
                .any(|reference| pending.contains_key(reference.as_str()))
            {
                continue;
            }
            let level = references
                .iter()
                .filter_map(|reference| depth.get(reference))
                .max()
                .copied()
                .unwrap_or(0)
                + 1;
            if level > 256 {
                return Err(vec![source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "dimension alias dependency depth exceeds 256",
                )]);
            }
            let value = lower_dimension_with_aliases(file, expression, &aliases, Some(&declared))
                .map_err(|error| vec![error])?;
            ready.push((name, value, level));
        }
        if ready.is_empty() {
            let (name, (_, range)) = pending.first_key_value().expect("nonempty aliases");
            return Err(vec![source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                *range,
                format!("dimension alias dependency cycle includes `{name}`"),
            )]);
        }
        for (name, value, level) in ready {
            pending.remove(name);
            aliases.insert(name.to_owned(), value);
            depth.insert(name.to_owned(), level);
        }
    }
    Ok(aliases)
}

fn dimension_references(expression: &Expr, references: &mut BTreeSet<String>) {
    match expression.kind() {
        ExprKind::Name(name) => {
            references.insert(name.clone());
        }
        ExprKind::Path(path) => {
            references.insert(path.to_string());
        }
        ExprKind::Binary { left, right, .. } => {
            dimension_references(left, references);
            dimension_references(right, references);
        }
        ExprKind::Unary { value, .. } => dimension_references(value, references),
        _ => {}
    }
}

fn rewrite_document(document: &mut Document, aliases: &BTreeMap<String, DimExponents>) {
    SourceAstFactory::rewrite_dimension_expressions(document, |expression| {
        rewrite_alias_uses(expression, aliases)
    });
}

fn rewrite_alias_uses(expression: &Expr, aliases: &BTreeMap<String, DimExponents>) -> Expr {
    let range = expression.range();
    let kind = match expression.kind() {
        ExprKind::Name(name) => {
            if let Some(dimension) = aliases.get(name) {
                return dimension_expression(*dimension, range);
            }
            ExprKind::Name(name.clone())
        }
        ExprKind::Path(path) => {
            if let Some(dimension) = aliases.get(&path.to_string()) {
                return dimension_expression(*dimension, range);
            }
            ExprKind::Path(path.clone())
        }
        ExprKind::Number(value) => ExprKind::Number(value.clone()),
        ExprKind::Unary { op, value } => ExprKind::Unary {
            op: *op,
            value: Box::new(rewrite_alias_uses(value, aliases)),
        },
        ExprKind::Binary { op, left, right } => ExprKind::Binary {
            op: *op,
            left: Box::new(rewrite_alias_uses(left, aliases)),
            right: Box::new(rewrite_alias_uses(right, aliases)),
        },
        other => other.clone(),
    };
    SourceAstFactory::expression(kind, range).expect("parsed dimension expression remains valid")
}

pub(crate) fn dimension_expression(dimension: DimExponents, range: TextRange) -> Expr {
    let factors = ["kg", "m", "s", "A", "K", "mol", "cd"]
        .into_iter()
        .zip(dimension.exponents());
    let mut expression = None;
    for (name, (numerator, denominator)) in factors {
        if numerator == 0 {
            continue;
        }
        let name = SourceAstFactory::expression(ExprKind::Name(name.to_owned()), range)
            .expect("coherent-SI name expression");
        let factor = if (numerator, denominator) == (1, 1) {
            name
        } else {
            let magnitude = SourceAstFactory::expression(
                ExprKind::Number(
                    eqiora_lang::DecimalLiteral::parse(&numerator.unsigned_abs().to_string())
                        .expect("bounded dimension"),
                ),
                range,
            )
            .expect("bounded exponent");
            let exponent = if numerator < 0 {
                SourceAstFactory::expression(
                    ExprKind::Unary {
                        op: UnaryOp::Neg,
                        value: Box::new(magnitude),
                    },
                    range,
                )
                .expect("negative exponent")
            } else {
                magnitude
            };
            let exponent = if denominator == 1 {
                exponent
            } else {
                SourceAstFactory::expression(
                    ExprKind::Binary {
                        op: BinaryOp::Div,
                        left: Box::new(exponent),
                        right: Box::new(
                            SourceAstFactory::expression(
                                ExprKind::Number(
                                    eqiora_lang::DecimalLiteral::parse(&denominator.to_string())
                                        .expect("bounded dimension"),
                                ),
                                range,
                            )
                            .expect("positive dimension denominator"),
                        ),
                    },
                    range,
                )
                .expect("rational dimension exponent")
            };
            SourceAstFactory::expression(
                ExprKind::Binary {
                    op: BinaryOp::Pow,
                    left: Box::new(name),
                    right: Box::new(exponent),
                },
                range,
            )
            .expect("dimension power")
        };
        expression = Some(match expression {
            None => factor,
            Some(left) => SourceAstFactory::expression(
                ExprKind::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(left),
                    right: Box::new(factor),
                },
                range,
            )
            .expect("dimension product"),
        });
    }
    expression.unwrap_or_else(|| {
        SourceAstFactory::expression(
            ExprKind::Number(eqiora_lang::DecimalLiteral::parse("1.0").expect("exact literal")),
            range,
        )
        .expect("dimensionless expression")
    })
}

pub(crate) fn rational_literal(expression: &Expr) -> Option<(i32, i32)> {
    let (numerator, denominator) = match expression.kind() {
        ExprKind::Binary {
            op: BinaryOp::Div,
            left,
            right,
        } => (integer_literal(left)?, integer_literal(right)?),
        _ => (integer_literal(expression)?, 1),
    };
    (numerator != i32::MIN && denominator > 0).then_some((numerator, denominator))
}

pub(crate) fn integer_literal(expression: &Expr) -> Option<i32> {
    let value = match expression.kind() {
        ExprKind::Number(value) => value.to_i64().ok()?,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => match value.kind() {
            ExprKind::Number(value) => value.to_i64().ok()?.checked_neg()?,
            _ => return None,
        },
        _ => return None,
    };
    i32::try_from(value).ok()
}

pub(crate) const fn time_dimension() -> DimExponents {
    DimExponents::from_integers([0, 0, 1, 0, 0, 0, 0]).expect("bounded dimension")
}

pub(crate) const fn length_dimension() -> DimExponents {
    DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension")
}

pub(crate) fn dimension_overflow(file: &str, range: TextRange) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        range,
        "physical-dimension arithmetic exceeds rational exponent bounds",
    )
}

#[cfg(test)]
mod tests {
    use eqiora_graph::Op;
    use eqiora_lang::parse;
    use eqiora_schema::kernel::KernelNode;

    use super::lower_dimension;

    fn parameter_dimension(source: &str) -> eqiora_core::DimExponents {
        let source = format!("model M() {{ parameter value: {source} = 1; }}");
        let document = parse("dimension.eqi", &source)
            .into_document()
            .expect("dimension source parses");
        let eqiora_lang::Item::Parameter(parameter) = &document.models()[0].items()[0] else {
            panic!("one Parameter declaration");
        };
        crate::value_types::lower_value_type::<()>("dimension.eqi", parameter.value_type(), None)
            .expect("dimension lowers")
            .dimension()
    }

    #[test]
    fn coherent_aliases_equal_their_base_si_expansions() {
        for (alias, expanded) in [
            ("Hz", "1 / s"),
            ("N", "kg * m / s ^ 2"),
            ("Pa", "kg / (m * s ^ 2)"),
            ("J", "kg * m ^ 2 / s ^ 2"),
            ("W", "kg * m ^ 2 / s ^ 3"),
        ] {
            assert_eq!(parameter_dimension(alias), parameter_dimension(expanded));
        }
    }

    #[test]
    fn rational_dimensions_normalize_and_round_trip_alias_expressions() {
        for (source, equivalent) in [
            ("m ^ (2 / 4)", "m ^ (1 / 2)"),
            ("(m ^ 2) ^ (1 / 2)", "m"),
            ("Hz ^ (-1 / 2)", "s ^ (1 / 2)"),
            ("(m ^ (-1 / 2)) ^ 2 * m", "1"),
            ("m ^ (0 / 7)", "1"),
            ("m ^ 2147483647 / m ^ 2147483646", "m"),
        ] {
            let dimension = parameter_dimension(source);
            assert_eq!(dimension, parameter_dimension(equivalent), "{source}");
            let expression =
                super::dimension_expression(dimension, eqiora_lang::TextRange::new(0, 1));
            assert_eq!(
                lower_dimension("roundtrip.eqi", &expression).unwrap(),
                dimension
            );
        }
        assert_ne!(
            parameter_dimension("(m ^ -1) ^ 2 * m"),
            eqiora_core::DimExponents::DIMENSIONLESS,
        );
    }

    #[test]
    fn coherent_aliases_compile_across_model_declarations() {
        let source = r#"
model Catalog() {
  parameter length: m = 2;
  parameter force: N = 3;
  parameter duration: s = 1;
  let energy: J = force * length;
  variable power: W; initial { power = 0; }
  variable pressure: Pa; initial { pressure = 0; }
  port frequency: signal input Hz;
  relation balance {
    power = energy / duration;
    pressure = 0;
  }
}
"#;
        let mut compiled = crate::compile("catalog.eqi", source)
            .expect("coherent aliases compile through the shared dimension checker");
        let compiled = compiled.pop().expect("one Model");
        assert!(compiled.symbols().get("energy").is_none());
        assert!(compiled.symbols().get("power").is_some());
        assert!(compiled.symbols().get("pressure").is_some());
        assert!(compiled.symbols().get("frequency").is_some());
    }

    #[test]
    fn structural_aliases_resolve_once_across_hierarchy_and_kernel_dimensions() {
        let aliased = r#"
dimension Speed = m / s;
dimension Momentum = N * s;

connector Motion {
  across potential: Speed;
  through flow: Momentum;
}
component Law(parameter target: Speed, input input: Speed) {
  
  
  relation balance { input - target = 0; }
}
model Example() {
  parameter target: Speed = 2[m / s];
  let doubled: Speed = target * 2;
  variable velocity: Speed; initial { velocity = 0; }
  port input: signal input Speed;
  relation balance { velocity + input - doubled = 0; }
  instance law: Law(target = target);
}
"#;
        let expanded = aliased
            .replace(
                "dimension Speed = m / s;\ndimension Momentum = N * s;\n",
                "",
            )
            .replace("Speed", "m / s")
            .replace("Momentum", "N * s");

        let compiled = crate::compile("aliases.eqi", aliased).expect("aliases compile");
        let expanded = crate::compile("expanded.eqi", &expanded).expect("expansion compiles");
        let dimensions = |compiled: &crate::CompiledModel| {
            compiled
                .transaction()
                .ops()
                .iter()
                .filter_map(|operation| match operation {
                    Op::DefineKernelNode {
                        node: KernelNode::Field(field),
                    } => Some(field.dimension()),
                    Op::DefineKernelNode {
                        node: KernelNode::Parameter(parameter),
                    } => Some(parameter.value_type().dimension()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(dimensions(&compiled[0]), dimensions(&expanded[0]));
        assert_eq!(
            compiled[0].symbols().iter().count(),
            expanded[0].symbols().iter().count()
        );
    }

    #[test]
    fn nested_aliases_preserve_complete_value_types_and_source_ranges() {
        use eqiora_schema::kernel::typing::SpatialSupport;

        let support = SpatialSupport::Volume {
            domain: "body",
            dimensions: 3,
        };
        for (aliased, expanded) in [
            ("array<Length, 2>", "array<m, 2>"),
            ("array<array<Length, 3>, 2>", "array<array<m, 3>, 2>"),
            ("array<complex<Length>, 2>", "array<complex<m>, 2>"),
            ("vector<Length, 3>", "vector<m, 3>"),
            (
                "array<tensor<Length, 3, 3>, 2>",
                "array<tensor<m, 3, 3>, 2>",
            ),
        ] {
            let source = format!("dimension Length = m; model M() {{ variable x: {aliased}; }}");
            let document = parse("nested-alias.eqi", &source).into_document().unwrap();
            let eqiora_lang::Item::Field(original) = &document.models()[0].items()[0] else {
                panic!("field declaration");
            };
            let elaborated = super::elaborate_dimension_aliases("nested-alias.eqi", &document)
                .expect("nested dimension aliases elaborate");
            let eqiora_lang::Item::Field(field) = &elaborated.models()[0].items()[0] else {
                panic!("field declaration");
            };
            assert_eq!(field.value_type().range(), original.value_type().range());
            let actual = crate::value_types::lower_value_type(
                "nested-alias.eqi",
                field.value_type(),
                Some(&support),
            )
            .unwrap();
            let expanded_source = format!("model M() {{ variable x: {expanded}; }}");
            let expanded_document = parse("expanded.eqi", &expanded_source)
                .into_document()
                .unwrap();
            let eqiora_lang::Item::Field(expected) = &expanded_document.models()[0].items()[0]
            else {
                panic!("field declaration");
            };
            let expected = crate::value_types::lower_value_type(
                "expanded.eqi",
                expected.value_type(),
                Some(&support),
            )
            .unwrap();
            assert_eq!(actual, expected, "{aliased}");
            assert_eq!(actual.dimension(), super::length_dimension());
        }

        let source = "dimension Length = m; model M() { variable x: Length; }";
        let mut document = parse("bound.eqi", source).into_document().unwrap();
        let bound = eqiora_schema::kernel::EnumDef::new(eqiora_core::Id::new(), ["Member".into()])
            .unwrap()
            .value_type();
        eqiora_lang::SourceAstFactory::visit_value_types(&mut document, |_, syntax| {
            eqiora_lang::SourceAstFactory::bind_nominal_value_type(syntax, bound.clone()).unwrap();
        });
        let elaborated = super::elaborate_dimension_aliases("bound.eqi", &document).unwrap();
        let eqiora_lang::Item::Field(field) = &elaborated.models()[0].items()[0] else {
            panic!("field declaration");
        };
        assert_eq!(field.value_type().resolved_nominal(), Some(&bound));
    }

    #[test]
    fn structural_aliases_reject_ambiguous_or_invalid_names_and_overflow() {
        for (case, source, message) in [
            (
                "self",
                "dimension A1 = A1; model M() { variable x: A1; initial { x = 0; } }",
                "dependency cycle",
            ),
            (
                "duplicate",
                "dimension D = m; dimension D = s; model M() { variable x: D; initial { x = 0; } }",
                "duplicate dimension alias",
            ),
            (
                "builtin",
                "dimension Pa = m; model M() { variable x: Pa; initial { x = 0; } }",
                "cannot shadow",
            ),
            (
                "unknown",
                "dimension D = Missing; model M() { variable x: D; initial { x = 0; } }",
                "unknown coherent-SI dimension symbol or alias",
            ),
            (
                "overflow",
                "dimension D = m ^ 2147483647 * m; model M() { variable x: D; initial { x = 0; } }",
                "exceeds rational exponent bounds",
            ),
            (
                "denominator-overflow",
                "dimension D = (m ^ (1 / 2147483647)) ^ (1 / 2); model M() { variable x: D; initial { x = 0; } }",
                "exceeds rational exponent bounds",
            ),
            (
                "zero-denominator",
                "dimension D = m ^ (1 / 0); model M() { variable x: D; initial { x = 0; } }",
                "positive denominator",
            ),
            (
                "negative-denominator",
                "dimension D = m ^ (1 / -2); model M() { variable x: D; initial { x = 0; } }",
                "positive denominator",
            ),
            (
                "malformed",
                "dimension D = 2; model M() { variable x: D; initial { x = 0; } }",
                "dimension must use",
            ),
        ] {
            let diagnostics = crate::compile(&format!("{case}.eqi"), source)
                .expect_err("invalid alias must fail closed");
            assert!(
                diagnostics
                    .iter()
                    .any(|error| error.message().contains(message)),
                "{case}: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn structural_aliases_feed_property_dimension_validation() {
        let namespace =
            crate::CompilationNamespaceId::new(["org", "example", "property"]).expect("namespace");
        let source = r#"
dimension DiffusionDimension = m ^ 2 / s;
public property contract Diffusivity(): DiffusionDimension { derivatives value_only; }
property release Reference: Diffusivity {
  analytic { value = 25;
  source_unit: DiffusionDimension = 1 / 1000; }
  validity unconditional; outside reject; branch single;
  citation org.example.measurement;
  license spdx.CC0_1_0;
}
public component Diffusion(property diffusivity: Diffusivity) {
  
  relation law { diffusivity = 0; }
}
model Main() { instance domain: Diffusion(diffusivity = Reference); }
"#;
        let input = crate::ResolvedHierarchyInput::new(
            namespace.clone(),
            vec![
                crate::ResolvedSourceUnit::new(namespace, "src/main.eqi", source)
                    .expect("source path"),
            ],
            Vec::new(),
        );
        let analyzed = crate::analyze_resolved_hierarchy(input).expect("property alias analyzes");
        assert_eq!(
            analyzed
                .property_bindings()
                .next()
                .expect("binding")
                .5
                .constant_value()
                .expect("constant property")
                .component(0)
                .unwrap()
                .0,
            0.025
        );
        analyzed
            .validate_definitions()
            .expect("definitions validate")
            .compile_root("Main")
            .expect("property alias compiles");
    }
}
