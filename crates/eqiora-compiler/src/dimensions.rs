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

#[derive(Default)]
struct DimensionEnvironment {
    aliases: BTreeMap<String, DimExponents>,
}

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
        ExprKind::Number(value) if *value == 1.0 => Ok(DimExponents::DIMENSIONLESS),
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
        ExprKind::Binary { op, left, right } if matches!(op, BinaryOp::Mul | BinaryOp::Div) => {
            let left = lower_dimension_with_aliases(file, left, aliases, declared_names)?;
            let right = lower_dimension_with_aliases(file, right, aliases, declared_names)?;
            let operation = if *op == BinaryOp::Mul {
                i8::checked_add
            } else {
                i8::checked_sub
            };
            checked_dimensions(left, right, operation)
                .ok_or_else(|| dimension_overflow(file, expression.range()))
        }
        ExprKind::Binary {
            op: BinaryOp::Pow,
            left,
            right,
        } => {
            let dimension = lower_dimension_with_aliases(file, left, aliases, declared_names)?;
            let exponent = integer_literal(right).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    right.range(),
                    "dimension power must be an i32 integer literal",
                )
            })?;
            checked_scale_dimension(dimension, exponent)
                .ok_or_else(|| dimension_overflow(file, expression.range()))
        }
        _ => Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            expression.range(),
            "dimension must use `1`, SI base symbols, `*`, `/`, and integer powers",
        )),
    }
}

pub(crate) fn elaborate_dimension_aliases<'a>(
    file: &str,
    document: &'a Document,
) -> Result<Cow<'a, Document>, Vec<Diagnostic>> {
    if document.dimension_syntax().len() == 0 {
        return Ok(Cow::Borrowed(document));
    }
    let declared_names = document
        .dimension_syntax()
        .map(|(name, _, _)| name.to_owned())
        .collect::<BTreeSet<_>>();
    let mut environment = DimensionEnvironment::default();
    let mut seen = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for (name, expression, range) in document.dimension_syntax() {
        if name == crate::math::ROOT {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                "identifier `math` is reserved for compiler-owned scalar mathematics",
            ));
            continue;
        }
        if coherent_dimension(name).is_some() {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                format!("dimension alias `{name}` cannot shadow a coherent-SI symbol"),
            ));
            continue;
        }
        if !seen.insert(name.to_owned()) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                format!("duplicate dimension alias `{name}`"),
            ));
            continue;
        }
        match lower_dimension_with_aliases(
            file,
            expression,
            &environment.aliases,
            Some(&declared_names),
        ) {
            Ok(dimension) => {
                environment.aliases.insert(name.to_owned(), dimension);
            }
            Err(error) => diagnostics.push(error),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut elaborated = document.clone();
    SourceAstFactory::rewrite_dimension_expressions(&mut elaborated, |expression| {
        rewrite_alias_uses(expression, &environment.aliases)
    });
    Ok(Cow::Owned(elaborated))
}

pub(crate) fn elaborate_dimension_aliases_in_place(
    file: &str,
    document: &mut Document,
) -> Result<(), Vec<Diagnostic>> {
    if document.dimension_syntax().len() != 0 {
        *document = elaborate_dimension_aliases(file, document)?.into_owned();
    }
    Ok(())
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
        ExprKind::Number(value) => ExprKind::Number(*value),
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

fn dimension_expression(dimension: DimExponents, range: TextRange) -> Expr {
    let factors = [
        ("kg", dimension.mass),
        ("m", dimension.length),
        ("s", dimension.time),
        ("A", dimension.current),
        ("K", dimension.temperature),
        ("mol", dimension.amount),
        ("cd", dimension.luminous_intensity),
    ];
    let mut expression = None;
    for (name, exponent) in factors {
        if exponent == 0 {
            continue;
        }
        let name = SourceAstFactory::expression(ExprKind::Name(name.to_owned()), range)
            .expect("coherent-SI name expression");
        let factor = if exponent == 1 {
            name
        } else {
            let magnitude = SourceAstFactory::expression(
                ExprKind::Number(f64::from(exponent.unsigned_abs())),
                range,
            )
            .expect("bounded exponent");
            let exponent = if exponent < 0 {
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
        SourceAstFactory::expression(ExprKind::Number(1.0), range)
            .expect("dimensionless expression")
    })
}

pub(crate) fn checked_dimensions(
    left: DimExponents,
    right: DimExponents,
    operation: fn(i8, i8) -> Option<i8>,
) -> Option<DimExponents> {
    Some(DimExponents {
        mass: operation(left.mass, right.mass)?,
        length: operation(left.length, right.length)?,
        time: operation(left.time, right.time)?,
        current: operation(left.current, right.current)?,
        temperature: operation(left.temperature, right.temperature)?,
        amount: operation(left.amount, right.amount)?,
        luminous_intensity: operation(left.luminous_intensity, right.luminous_intensity)?,
    })
}

pub(crate) fn checked_scale_dimension(
    dimension: DimExponents,
    exponent: i32,
) -> Option<DimExponents> {
    fn scale(value: i8, exponent: i32) -> Option<i8> {
        i32::from(value)
            .checked_mul(exponent)
            .and_then(|value| i8::try_from(value).ok())
    }
    Some(DimExponents {
        mass: scale(dimension.mass, exponent)?,
        length: scale(dimension.length, exponent)?,
        time: scale(dimension.time, exponent)?,
        current: scale(dimension.current, exponent)?,
        temperature: scale(dimension.temperature, exponent)?,
        amount: scale(dimension.amount, exponent)?,
        luminous_intensity: scale(dimension.luminous_intensity, exponent)?,
    })
}

pub(crate) fn integer_literal(expression: &Expr) -> Option<i32> {
    let value = match expression.kind() {
        ExprKind::Number(value) => *value,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => match value.kind() {
            ExprKind::Number(value) => -*value,
            _ => return None,
        },
        _ => return None,
    };
    (value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

pub(crate) const fn time_dimension() -> DimExponents {
    DimExponents {
        time: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

pub(crate) const fn length_dimension() -> DimExponents {
    DimExponents {
        length: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

fn coherent_dimension(name: &str) -> Option<DimExponents> {
    let mut dimension = DimExponents::DIMENSIONLESS;
    match name {
        "kg" => dimension.mass = 1,
        "m" => dimension.length = 1,
        "s" => dimension.time = 1,
        "A" => dimension.current = 1,
        "K" => dimension.temperature = 1,
        "mol" => dimension.amount = 1,
        "cd" => dimension.luminous_intensity = 1,
        "Hz" => dimension.time = -1,
        "N" => {
            dimension.mass = 1;
            dimension.length = 1;
            dimension.time = -2;
        }
        "Pa" => {
            dimension.mass = 1;
            dimension.length = -1;
            dimension.time = -2;
        }
        "J" => {
            dimension.mass = 1;
            dimension.length = 2;
            dimension.time = -2;
        }
        "W" => {
            dimension.mass = 1;
            dimension.length = 2;
            dimension.time = -3;
        }
        _ => return None,
    }
    Some(dimension)
}

pub(crate) fn dimension_overflow(file: &str, range: TextRange) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        range,
        "physical-dimension exponent arithmetic overflows i8",
    )
}

#[cfg(test)]
mod tests {
    use eqiora_graph::Op;
    use eqiora_lang::parse;
    use eqiora_schema::kernel::KernelNode;

    use super::lower_dimension;

    fn parameter_dimension(source: &str) -> eqiora_core::DimExponents {
        let source = format!("model M {{ parameter value: {source} = 1; }}");
        let document = parse("dimension.eqi", &source)
            .into_document()
            .expect("dimension source parses");
        let eqiora_lang::Item::Parameter(parameter) = &document.models()[0].items()[0] else {
            panic!("one Parameter declaration");
        };
        lower_dimension("dimension.eqi", parameter.dimension()).expect("dimension lowers")
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
    fn coherent_aliases_compile_across_model_declarations() {
        let source = r#"
model Catalog {
  parameter length: m = 2;
  parameter force: N = 3;
  parameter duration: s = 1;
  let energy: J = force * length;
  field power: W = 0;
  field pressure: Pa = 0;
  port frequency: signal input Hz;
  relation balance continuous {
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

connector Motion = scalar_physical(across = Speed, through = Momentum);
component Law {
  public parameter target: Speed;
  public port input: signal input Speed;
  relation balance continuous { input - target = 0; }
}
model Example {
  parameter target: Speed = 2;
  let doubled: Speed = target * 2;
  field velocity: Speed = 0;
  port input: signal input Speed;
  relation balance continuous { velocity + input - doubled = 0; }
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
                    } => Some(parameter.value().dim()),
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
    fn structural_aliases_reject_ambiguous_or_invalid_names_and_overflow() {
        for (case, source, message) in [
            (
                "forward",
                "dimension A1 = B1; dimension B1 = m; model M { field x: A1 = 0; }",
                "forward or self reference",
            ),
            (
                "self",
                "dimension A1 = A1; model M { field x: A1 = 0; }",
                "forward or self reference",
            ),
            (
                "duplicate",
                "dimension D = m; dimension D = s; model M { field x: D = 0; }",
                "duplicate dimension alias",
            ),
            (
                "builtin",
                "dimension Pa = m; model M { field x: Pa = 0; }",
                "cannot shadow",
            ),
            (
                "unknown",
                "dimension D = Missing; model M { field x: D = 0; }",
                "unknown coherent-SI dimension symbol or alias",
            ),
            (
                "overflow",
                "dimension D = m ^ 128; model M { field x: D = 0; }",
                "overflows i8",
            ),
            (
                "malformed",
                "dimension D = m + s; model M { field x: D = 0; }",
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
public property contract Diffusivity { scalar value: DiffusionDimension; }
property release Reference implements Diffusivity {
  value = 25;
  source_unit: DiffusionDimension = 1 / 1000;
  validity = unconditional;
  citation = org.example.measurement;
  license = spdx.CC0_1_0;
}
public component Diffusion {
  public property diffusivity: Diffusivity;
  relation law continuous { diffusivity = 0; }
}
model Main { instance domain: Diffusion(property diffusivity = Reference); }
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
            analyzed.property_bindings().next().expect("binding").5,
            0.025
        );
        analyzed
            .validate_definitions()
            .expect("definitions validate")
            .compile_root("Main")
            .expect("property alias compiles");
    }
}
