use eqiora_core::Diagnostic;
use eqiora_lang::{Expr, TextRange};

use super::{Budget, Encoder, encode_expression, encode_name, source_identity_error};

pub(super) fn encode_dimensions<'a>(
    declarations: impl ExactSizeIterator<Item = (&'a str, &'a Expr, TextRange)>,
    budget: &mut Budget,
) -> Result<Vec<Vec<u8>>, Diagnostic> {
    let mut records = Vec::new();
    records
        .try_reserve_exact(declarations.len())
        .map_err(|_| source_identity_error("cannot reserve canonical dimension records"))?;
    for (name, expression, _) in declarations {
        let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
        encoder.field(1, |encoder| encode_name(encoder, name, budget))?;
        encoder.field(2, |encoder| {
            encode_expression(encoder, expression, budget, 1)
        })?;
        let record = encoder.finish()?;
        budget.account_materialized_bytes(record.len())?;
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use eqiora_lang::{format, parse};

    use eqiora_graph::Op;
    use eqiora_schema::kernel::KernelNode;

    use super::super::{LocalSourceIdentity, LocalSourceIdentityLimits};

    fn identity(source: &str) -> LocalSourceIdentity {
        let document = parse("dimension-identity.eqi", source)
            .into_document()
            .expect("source parses");
        LocalSourceIdentity::from_document(&document).expect("source identity")
    }

    #[test]
    fn aliases_are_exact_ordered_source_provenance() {
        let first = "dimension Speed = m / s; dimension Acceleration = Speed / s; model M { field x: Acceleration = 0; }";
        let renamed = "dimension Velocity = m / s; dimension Acceleration = Velocity / s; model M { field x: Acceleration = 0; }";
        let changed = "dimension Speed = m / s; dimension Acceleration = m / s ^ 2; model M { field x: Acceleration = 0; }";
        let reordered = "dimension Acceleration = m / s ^ 2; dimension Speed = m / s; model M { field x: Acceleration = 0; }";
        let expanded = "model M { field x: m / s ^ 2 = 0; }";

        assert_ne!(identity(first), identity(renamed));
        assert_ne!(identity(first), identity(changed));
        assert_ne!(identity(first), identity(reordered));
        assert_ne!(identity(first), identity(expanded));
        let document = parse("dimension-identity.eqi", first)
            .into_document()
            .expect("source parses");
        assert_eq!(identity(first), identity(&format(&document)));

        let limits = LocalSourceIdentityLimits {
            max_top_level_declarations: 2,
            ..LocalSourceIdentityLimits::default()
        };
        assert!(LocalSourceIdentity::from_document_with_limits(&document, limits).is_err());
    }

    #[test]
    fn aliases_elaborate_inside_an_exact_resolved_source_unit() {
        let namespace = crate::CompilationNamespaceId::new(["org", "example", "dimensions"])
            .expect("namespace");
        let input = crate::ResolvedHierarchyInput::new(
            namespace.clone(),
            vec![crate::ResolvedSourceUnit::new(
                namespace,
                "model.eqi",
                "dimension Speed = m / s; model Main { field velocity: Speed = 0; relation balance continuous { velocity = 0; } }",
            )],
            Vec::new(),
        );
        let compiled = crate::analyze_resolved_hierarchy(input)
            .expect("resolved unit analyzes")
            .validate_definitions()
            .expect("definitions validate")
            .compile_root("Main")
            .expect("resolved alias lowers");
        assert!(
            compiled
                .transaction()
                .ops()
                .iter()
                .any(|operation| matches!(
                    operation,
                    Op::DefineKernelNode { node: KernelNode::Field(field) }
                        if field.dimension().length == 1 && field.dimension().time == -1
                ))
        );
    }
}
