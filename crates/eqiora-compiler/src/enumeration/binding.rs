//! Bind authored enum names to declaration identities before value evaluation.
use std::collections::BTreeMap;

use eqiora_core::{Diagnostic, EntityKind, RawId, diagnostic::codes, entity::kinds};
use eqiora_lang::{
    Document, ExprKind, NamePath, SourceAstFactory, ValueTypeSyntaxKind, VisibilitySyntax,
};
use eqiora_schema::kernel::EnumDef;

use crate::diagnostics::source_error;
use crate::identity::{
    DeclarationPath, ElaborationKey, IdentityNamespace, InstancePath, StagingIdAllocator,
};

#[derive(Clone, Debug)]
pub(crate) struct BoundEnum {
    pub(crate) definition: EnumDef,
    pub(crate) key: ElaborationKey,
    pub(crate) file: String,
    pub(crate) range: eqiora_lang::TextRange,
    pub(crate) visibility: VisibilitySyntax,
}

pub(crate) fn declarations(
    file: &str,
    document: &Document,
    namespace: &IdentityNamespace,
    mut native_identity: impl FnMut(&str) -> Option<RawId>,
) -> Result<BTreeMap<String, BoundEnum>, Vec<Diagnostic>> {
    let mut result = BTreeMap::new();
    for declaration in document.enumerations() {
        let invalid = |message: String| {
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
        {
            return Err(vec![invalid(
                "enum name duplicates a type or dimension declaration".into(),
            )]);
        }
        let key = ElaborationKey::entity(
            namespace.clone(),
            InstancePath::new(["$definitions"]).map_err(|e| vec![e])?,
            DeclarationPath::new(["enum", declaration.name()]).map_err(|e| vec![e])?,
            EntityKind::Enum,
        )
        .map_err(|e| vec![e])?;
        let id = if let Some(raw) = native_identity(declaration.name()) {
            raw.downcast::<kinds::Enum>()
                .ok_or_else(|| vec![invalid("native enum has a foreign entity kind".into())])?
        } else {
            let mut staging = StagingIdAllocator::new();
            let full = staging.stage(&key).map_err(|e| vec![e])?;
            staging
                .finish()
                .resolve::<kinds::Enum>(full)
                .map_err(|e| vec![e])?
                .id()
        };
        let definition = EnumDef::new(
            id,
            declaration.tags().iter().map(|tag| tag.as_str().to_owned()),
        )
        .map_err(|e| vec![invalid(e.message().to_owned())])?;
        result.insert(
            declaration.name().to_owned(),
            BoundEnum {
                definition,
                key,
                file: file.to_owned(),
                range: declaration.range(),
                visibility: declaration.visibility(),
            },
        );
    }
    Ok(result)
}

pub(crate) fn bind_document(
    file: &str,
    document: &mut Document,
    enums: &BTreeMap<String, BoundEnum>,
) -> Result<(), Vec<Diagnostic>> {
    let mut errors = Vec::new();
    SourceAstFactory::visit_value_types(document, |_, syntax| {
        let ValueTypeSyntaxKind::Named(name) = syntax.kind() else {
            return;
        };
        if let Some(declaration) = enums.get(name.as_str())
            && let Err(error) = SourceAstFactory::bind_nominal_value_type(
                syntax,
                declaration.definition.value_type(),
            )
        {
            errors.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                syntax.range(),
                error.message(),
            ));
        }
    });
    SourceAstFactory::visit_expressions(document, |_, expression| {
        let result = match expression.kind() {
            ExprKind::Path(path) => split_member(path, enums).map(|(name, declaration)| {
                SourceAstFactory::bind_enum_member(expression, &name, &declaration.definition)
            }),
            ExprKind::Case { value, arms } => {
                let value = value.clone();
                let mut arms = arms.clone();
                for arm in &mut arms {
                    let Some((name, declaration)) = split_member(arm.pattern(), enums) else {
                        errors.push(source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            arm.range(),
                            "case pattern requires a visible exact enum member",
                        ));
                        return;
                    };
                    if let Err(error) =
                        SourceAstFactory::bind_case_pattern(arm, &name, &declaration.definition)
                    {
                        errors.push(source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            arm.range(),
                            error.message(),
                        ));
                        return;
                    }
                }
                let range = expression.range();
                Some(
                    SourceAstFactory::expression(ExprKind::Case { value, arms }, range)
                        .map(|value| *expression = value),
                )
            }
            _ => None,
        };
        if let Some(Err(error)) = result {
            errors.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                expression.range(),
                error.message(),
            ));
        }
    });
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn split_member<'a>(
    path: &NamePath,
    enums: &'a BTreeMap<String, BoundEnum>,
) -> Option<(NamePath, &'a BoundEnum)> {
    let (prefix, _) = path.as_str().rsplit_once('.')?;
    let declaration = enums.get(prefix)?;
    Some((
        NamePath::from_segments(prefix.split('.'), path.range()).ok()?,
        declaration,
    ))
}

pub(crate) fn resolved_namespace(
    module: &crate::resolved::CompilationModuleId,
) -> Result<IdentityNamespace, Diagnostic> {
    IdentityNamespace::new(
        std::iter::once("resolved-enum-v1".to_owned())
            .chain(module.owner().segments().iter().cloned())
            .chain(std::iter::once("module".to_owned()))
            .chain(module.name().segments().iter().cloned()),
    )
}

pub(crate) fn bind_resolved(
    units: &mut [crate::resolved::AnalyzedSourceUnit],
    aliases: &[crate::resolved::ResolvedAlias],
) -> Result<(), Vec<Diagnostic>> {
    let definitions = units
        .iter()
        .map(|unit| {
            let namespace = resolved_namespace(&unit.module).map_err(|e| vec![e])?;
            declarations(&unit.file, &unit.document, &namespace, |name| {
                unit.native
                    .as_ref()
                    .and_then(|module| module.nominal_identity(name))
            })
            .map(|values| (unit.module.clone(), values))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    for unit in units {
        let mut visible = definitions[&unit.module].clone();
        for alias in aliases
            .iter()
            .filter(|alias| alias.declaring_module() == &unit.module)
        {
            if let Some(target) = definitions.get(alias.target_module()) {
                for (name, value) in target
                    .iter()
                    .filter(|(_, value)| value.visibility == VisibilitySyntax::Public)
                {
                    visible.insert(format!("{}.{}", alias.alias(), name), value.clone());
                }
            }
        }
        bind_document(&unit.file, &mut unit.document, &visible)?;
    }
    Ok(())
}
