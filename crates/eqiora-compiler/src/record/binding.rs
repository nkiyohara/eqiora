//! Bind closed record members after lexical enum, dimension and finite-space binding.
use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::{Diagnostic, EntityKind, RawId, diagnostic::codes, entity::kinds};
use eqiora_lang::{Document, ValueTypeSyntax, ValueTypeSyntaxKind, VisibilitySyntax};
use eqiora_schema::kernel::RecordDef;

use crate::diagnostics::source_error;
use crate::identity::{
    DeclarationPath, ElaborationKey, IdentityNamespace, InstancePath, StagingIdAllocator,
};
use crate::resolved::{AnalyzedSourceUnit, CompilationModuleId, ResolvedAlias};

#[derive(Clone, Debug)]
pub(crate) struct BoundRecord {
    pub(crate) definition: RecordDef,
    pub(crate) key: ElaborationKey,
    pub(crate) file: String,
    pub(crate) range: eqiora_lang::TextRange,
    pub(crate) visibility: VisibilitySyntax,
    // Retain resolved source types for ordinary leaf expansion, including their ranges.
    pub(crate) member_syntax: Vec<ValueTypeSyntax>,
}

/// Bind local declarations from an already lexically elaborated source document.
pub(crate) fn declarations(
    file: &str,
    document: &Document,
    namespace: &IdentityNamespace,
    native_identity: impl FnMut(&str) -> Option<RawId>,
) -> Result<BTreeMap<String, BoundRecord>, Vec<Diagnostic>> {
    let names = document
        .records()
        .iter()
        .map(|value| value.name().to_owned())
        .collect();
    declarations_with_names(file, document, namespace, native_identity, &names)
}

fn declarations_with_names(
    file: &str,
    document: &Document,
    namespace: &IdentityNamespace,
    mut native_identity: impl FnMut(&str) -> Option<RawId>,
    record_names: &BTreeSet<String>,
) -> Result<BTreeMap<String, BoundRecord>, Vec<Diagnostic>> {
    let mut result = BTreeMap::new();
    for declaration in document.records() {
        let invalid = |message: &str| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                message,
            )
        };
        if result.contains_key(declaration.name())
            || crate::units::coherent_dimension(declaration.name()).is_some()
            || document
                .dimension_syntax()
                .any(|(name, _, _)| name == declaration.name())
            || document
                .enumerations()
                .iter()
                .any(|value| value.name() == declaration.name())
            || document
                .finite_spaces()
                .iter()
                .any(|value| value.name() == declaration.name())
        {
            return Err(vec![invalid(
                "record name duplicates a type or dimension declaration",
            )]);
        }
        let mut members = Vec::with_capacity(declaration.members().len());
        for member in declaration.members() {
            if contains_record(member.value_type(), record_names) {
                return Err(vec![source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    member.range(),
                    "closed record members require ordinary mathematical leaf types; nested records are not supported",
                )]);
            }
            let value_type =
                crate::value_types::lower_value_type::<()>(file, member.value_type(), None)
                    .map_err(|error| vec![error])?;
            members.push((member.name().to_owned(), value_type));
        }
        let key = ElaborationKey::entity(
            namespace.clone(),
            InstancePath::new(["$definitions"]).map_err(|error| vec![error])?,
            DeclarationPath::new(["record", declaration.name()]).map_err(|error| vec![error])?,
            EntityKind::Record,
        )
        .map_err(|error| vec![error])?;
        let id = if let Some(raw) = native_identity(declaration.name()) {
            raw.downcast::<kinds::Record>()
                .ok_or_else(|| vec![invalid("native record has a foreign entity kind")])?
        } else {
            let mut staging = StagingIdAllocator::new();
            let full = staging.stage(&key).map_err(|error| vec![error])?;
            staging
                .finish()
                .resolve::<kinds::Record>(full)
                .map_err(|error| vec![error])?
                .id()
        };
        let definition =
            RecordDef::new(id, members).map_err(|error| vec![invalid(error.message())])?;
        result.insert(
            declaration.name().to_owned(),
            BoundRecord {
                definition,
                key,
                file: file.to_owned(),
                range: declaration.range(),
                visibility: declaration.visibility(),
                member_syntax: declaration
                    .members()
                    .iter()
                    .map(|member| member.value_type().clone())
                    .collect(),
            },
        );
    }
    Ok(result)
}

fn contains_record(syntax: &ValueTypeSyntax, names: &BTreeSet<String>) -> bool {
    match syntax.kind() {
        ValueTypeSyntaxKind::Named(name) => names.contains(name.as_str()),
        ValueTypeSyntaxKind::Array { element, .. } => contains_record(element, names),
        ValueTypeSyntaxKind::Vector { scalar, .. } | ValueTypeSyntaxKind::Tensor { scalar, .. } => {
            contains_record(scalar, names)
        }
        _ => false,
    }
}

pub(crate) fn resolved_namespace(
    module: &CompilationModuleId,
) -> Result<IdentityNamespace, Diagnostic> {
    IdentityNamespace::new(
        std::iter::once("resolved-record-v1".to_owned())
            .chain(module.owner().segments().iter().cloned())
            .chain(std::iter::once("module".to_owned()))
            .chain(module.name().segments().iter().cloned()),
    )
}

/// Derive definitions only after each unit's member types have lexical bindings.
pub(crate) fn resolved_declarations(
    units: &[AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
) -> Result<BTreeMap<CompilationModuleId, BTreeMap<String, BoundRecord>>, Vec<Diagnostic>> {
    let modules = units
        .iter()
        .map(|unit| (&unit.module, unit))
        .collect::<BTreeMap<_, _>>();
    units
        .iter()
        .map(|unit| {
            let namespace = resolved_namespace(&unit.module).map_err(|error| vec![error])?;
            let mut names = unit
                .document
                .records()
                .iter()
                .map(|record| record.name().to_owned())
                .collect::<BTreeSet<_>>();
            for alias in aliases
                .iter()
                .filter(|alias| alias.declaring_module() == &unit.module)
            {
                if let Some(target) = modules.get(alias.target_module()) {
                    names.extend(
                        target
                            .document
                            .records()
                            .iter()
                            .filter(|record| record.visibility() == VisibilitySyntax::Public)
                            .map(|record| format!("{}.{}", alias.alias(), record.name())),
                    );
                }
            }
            declarations_with_names(
                &unit.file,
                &unit.document,
                &namespace,
                |name| {
                    unit.native
                        .as_ref()
                        .and_then(|native| native.nominal_identity(name))
                },
                &names,
            )
            .map(|records| (unit.module.clone(), records))
        })
        .collect()
}

/// Return precisely the local and explicitly imported public declarations.
pub(crate) fn visible(
    definitions: &BTreeMap<CompilationModuleId, BTreeMap<String, BoundRecord>>,
    module: &CompilationModuleId,
    aliases: &[ResolvedAlias],
) -> BTreeMap<String, BoundRecord> {
    let mut result = definitions.get(module).cloned().unwrap_or_default();
    for alias in aliases
        .iter()
        .filter(|alias| alias.declaring_module() == module)
    {
        if let Some(target) = definitions.get(alias.target_module()) {
            for (name, record) in target
                .iter()
                .filter(|(_, record)| record.visibility == VisibilitySyntax::Public)
            {
                result.insert(format!("{}.{}", alias.alias(), name), record.clone());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(source: &str) -> Document {
        eqiora_lang::parse("record.eqi", source)
            .into_document()
            .unwrap()
    }

    #[test]
    fn closed_members_keep_order_complete_types_and_exact_declaration_owner() {
        let source = document("record Bus { voltage: V, valid: bool, samples: array<integer,3> }");
        let first_namespace = IdentityNamespace::new(["first"]).unwrap();
        let second_namespace = IdentityNamespace::new(["second"]).unwrap();
        let first = declarations("record.eqi", &source, &first_namespace, |_| None).unwrap();
        let second = declarations("record.eqi", &source, &second_namespace, |_| None).unwrap();
        let members = first["Bus"].definition.members();
        assert_eq!(
            members
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["voltage", "valid", "samples"]
        );
        assert_eq!(
            members[0].1.dimension(),
            eqiora_core::DimExponents::from_integers([1, 2, -3, -1, 0, 0, 0]).unwrap()
        );
        assert_eq!(members[1].1, eqiora_core::ValueType::boolean());
        assert_eq!(
            members[2].1,
            eqiora_core::ValueType::scalar(
                eqiora_core::ScalarDomain::Integer,
                eqiora_core::DimExponents::DIMENSIONLESS
            )
            .unwrap()
            .array(3)
            .unwrap()
        );
        assert_ne!(first["Bus"].definition.id(), second["Bus"].definition.id());
        assert_eq!(
            first["Bus"].member_syntax[2],
            source.records()[0].members()[2].value_type().clone()
        );
    }

    #[test]
    fn native_identity_is_preserved_and_foreign_kind_is_rejected() {
        let source = document("record Bus { valid: bool }");
        let namespace = IdentityNamespace::new(["native"]).unwrap();
        let id = eqiora_core::Id::<kinds::Record>::new();
        let records =
            declarations("record.eqi", &source, &namespace, |_| Some(id.erase())).unwrap();
        assert_eq!(records["Bus"].definition.id(), id);
        let foreign = eqiora_core::Id::<kinds::Enum>::new();
        assert!(
            declarations("record.eqi", &source, &namespace, |_| Some(foreign.erase())).is_err()
        );
    }

    #[test]
    fn enum_members_reuse_the_exact_prior_lexical_binding() {
        let mut source = document("enum Mode { Off, On } record Bus { mode: Mode }");
        let namespace = IdentityNamespace::new(["lexical"]).unwrap();
        let enums =
            crate::enumeration::declarations("record.eqi", &source, &namespace, |_| None).unwrap();
        crate::enumeration::bind_document("record.eqi", &mut source, &enums).unwrap();
        let records = declarations("record.eqi", &source, &namespace, |_| None).unwrap();
        assert_eq!(
            records["Bus"].definition.members()[0].1,
            enums["Mode"].definition.value_type()
        );
    }

    #[test]
    fn nested_records_and_type_collisions_fail_closed() {
        let namespace = IdentityNamespace::new(["test"]).unwrap();
        for source in [
            "record Inner { x: integer } record Outer { child: Inner }",
            "record Inner { x: integer } record Outer { child: array<Inner,2> }",
            "record V { x: integer }",
            "record Bus { x: integer } record Bus { y: integer }",
        ] {
            assert!(
                declarations("record.eqi", &document(source), &namespace, |_| None).is_err(),
                "{source}"
            );
        }
    }
}
