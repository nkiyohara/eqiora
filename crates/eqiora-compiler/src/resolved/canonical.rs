use super::*;

pub(super) fn collect_canonical_declarations(
    units: &[AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<CanonicalDeclarationIdentity> {
    let mut result = Vec::new();
    let mut paths = BTreeSet::new();
    for unit in units {
        let operator_formals = visible_operator_formals(unit, units, aliases);
        let resolved_aliases = aliases
            .iter()
            .filter(|alias| alias.declaring_module() == &unit.module)
            .map(|alias| (alias.alias().to_owned(), canonical_alias_target(alias)))
            .collect::<BTreeMap<_, _>>();
        for declaration in unit.document.dimensions() {
            let document = SourceAstFactory::document_with_dimensions(
                Vec::new(),
                vec![declaration.clone()],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .expect("parsed dimension document");
            push_canonical(
                &mut result,
                &mut paths,
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, declaration.name()),
                CanonicalDeclarationKind::Dimension,
                declaration.visibility(),
                &document,
                &resolved_aliases,
                &operator_formals,
                diagnostics,
            );
        }
        for (name, visibility, document) in unit.document.isolated_property_declarations() {
            let kind = if document.property_contract_syntax().next().is_some() {
                CanonicalDeclarationKind::PropertyContract
            } else if document.property_release_syntax().next().is_some() {
                CanonicalDeclarationKind::PropertyRelease
            } else {
                CanonicalDeclarationKind::MaterialComposition
            };
            push_canonical(
                &mut result,
                &mut paths,
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, &name),
                kind,
                visibility,
                &document,
                &resolved_aliases,
                &operator_formals,
                diagnostics,
            );
        }
        for enumeration in unit.document.enumerations() {
            let document = SourceAstFactory::document(
                vec![enumeration.clone()],
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .expect("parsed enum document");
            push_canonical(
                &mut result,
                &mut paths,
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, enumeration.name()),
                CanonicalDeclarationKind::Enum,
                enumeration.visibility(),
                &document,
                &resolved_aliases,
                &operator_formals,
                diagnostics,
            );
        }
        for connector in unit.document.connectors() {
            let document = SourceAstFactory::document(
                Vec::new(),
                vec![connector.clone()],
                Vec::new(),
                Vec::new(),
            )
            .expect("one parsed Connector is a valid document");
            push_canonical(
                &mut result,
                &mut paths,
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, connector.name()),
                CanonicalDeclarationKind::Connector,
                connector.visibility(),
                &document,
                &resolved_aliases,
                &operator_formals,
                diagnostics,
            );
        }
        let operators = match crate::pure_operator::compile_definitions(&unit.file, &unit.document)
        {
            Ok(definitions) => definitions,
            Err(error) => {
                diagnostics.push(error);
                BTreeMap::new()
            }
        };
        for operator in unit.document.pure_operators() {
            let path = canonical_declaration_path(&unit.module, operator.name());
            if !paths.insert((unit.module.owner().clone(), path.clone())) {
                diagnostics.push(resolved_error(format!(
                    "duplicate top-level declaration `{}` in module `{}`",
                    operator.name(),
                    unit.module
                )));
                continue;
            }
            if let Some(definition) = operators.get(operator.name()) {
                result.push(CanonicalDeclarationIdentity {
                    namespace: unit.module.owner().clone(),
                    path,
                    kind: CanonicalDeclarationKind::PureOperator,
                    visibility: operator.visibility(),
                    canonical_form: pure_operator_identity_form(definition.digest().bytes()),
                });
            }
        }
        for component in unit.document.components() {
            let document = SourceAstFactory::document(
                Vec::new(),
                Vec::new(),
                vec![component.clone()],
                Vec::new(),
            )
            .expect("one parsed component is a valid document");
            push_canonical(
                &mut result,
                &mut paths,
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, component.name()),
                CanonicalDeclarationKind::Component,
                component.visibility(),
                &document,
                &resolved_aliases,
                &operator_formals,
                diagnostics,
            );
        }
        for model in unit.document.models() {
            let document =
                SourceAstFactory::document(Vec::new(), Vec::new(), Vec::new(), vec![model.clone()])
                    .expect("one parsed Model is a valid document");
            push_canonical(
                &mut result,
                &mut paths,
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, model.name()),
                CanonicalDeclarationKind::Model,
                model.visibility(),
                &document,
                &resolved_aliases,
                &operator_formals,
                diagnostics,
            );
        }
    }
    result.sort_by(|left, right| {
        (
            &left.namespace,
            &left.path,
            left.kind,
            visibility_rank(left.visibility),
            &left.canonical_form,
        )
            .cmp(&(
                &right.namespace,
                &right.path,
                right.kind,
                visibility_rank(right.visibility),
                &right.canonical_form,
            ))
    });
    result
}

fn visible_operator_formals(
    unit: &AnalyzedSourceUnit,
    units: &[AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
) -> BTreeMap<String, Vec<String>> {
    let names = |operator: &eqiora_lang::PureOperatorDecl| {
        operator
            .formals()
            .iter()
            .map(|formal| formal.name().to_owned())
            .collect()
    };
    let mut visible = unit
        .document
        .pure_operators()
        .iter()
        .map(|operator| (operator.name().to_owned(), names(operator)))
        .collect::<BTreeMap<_, _>>();
    for alias in aliases
        .iter()
        .filter(|alias| alias.declaring_module() == &unit.module)
    {
        for target in units
            .iter()
            .filter(|target| &target.module == alias.target_module())
        {
            for operator in target
                .document
                .pure_operators()
                .iter()
                .filter(|operator| operator.visibility() == VisibilitySyntax::Public)
            {
                visible.insert(
                    format!("{}.{}", alias.alias(), operator.name()),
                    names(operator),
                );
            }
        }
    }
    visible
}

fn canonical_alias_target(alias: &ResolvedAlias) -> ResolvedAliasTarget {
    let target = alias.target_module();
    if alias.declaring_module().owner() == target.owner() {
        ResolvedAliasTarget::local_module(target.name().segments())
    } else {
        ResolvedAliasTarget::external_module(target.owner().segments(), target.name().segments())
    }
}

pub(super) fn canonical_declaration_path(
    module: &CompilationModuleId,
    declaration: &str,
) -> String {
    format!("{}.{}", module.name(), declaration)
}

#[allow(clippy::too_many_arguments)]
fn push_canonical(
    result: &mut Vec<CanonicalDeclarationIdentity>,
    paths: &mut BTreeSet<(CompilationNamespaceId, String)>,
    namespace: &CompilationNamespaceId,
    path: &str,
    kind: CanonicalDeclarationKind,
    visibility: VisibilitySyntax,
    document: &Document,
    resolved_aliases: &BTreeMap<String, ResolvedAliasTarget>,
    operator_formals: &BTreeMap<String, Vec<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !paths.insert((namespace.clone(), path.to_owned())) {
        diagnostics.push(resolved_error(format!(
            "duplicate top-level declaration `{path}` in namespace `{namespace}`"
        )));
        return;
    }
    let identity = match LocalSourceIdentity::from_document_with_resolved_aliases(
        document,
        resolved_aliases,
        operator_formals,
    ) {
        Ok(identity) => identity,
        Err(error) => {
            diagnostics.push(error);
            return;
        }
    };
    result.push(CanonicalDeclarationIdentity {
        namespace: namespace.clone(),
        path: path.to_owned(),
        kind,
        visibility,
        canonical_form: declaration_identity_form(identity.digest()),
    });
}
