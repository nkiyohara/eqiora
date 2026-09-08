//! Definition-time checking of exact nominal family selections without allocating occurrences.
use super::*;
use eqiora_lang::{Expr, ExprKind, NamedDefinitionDecl};

impl DefinitionScope<'_, '_> {
    pub(in crate::hierarchy::body_check) fn bind_index_set(
        &mut self,
        declaration: &NamedDefinitionDecl,
    ) -> Result<(), Diagnostic> {
        let invalid = |message: &str| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                declaration.range(),
                message,
            )
        };
        let ExprKind::Call { callee, arguments } = declaration.value().kind() else {
            return Err(invalid("index set requires range(extent)"));
        };
        let [extent] = arguments.as_slice() else {
            return Err(invalid(
                "range requires exactly one positive integer extent",
            ));
        };
        if callee.as_str() != "range"
            || declaration.value_type().is_some()
            || declaration.domain().is_some()
            || declaration.activation().is_some()
        {
            return Err(invalid("index set requires only its range definition"));
        }
        let extent = crate::hierarchy::parameters::structural_extent(
            self.file,
            extent,
            &self.static_values,
        )?
        .map(|(value, _)| value);
        if extent.is_none()
            && !matches!(
                self.namespace,
                crate::hierarchy::preflight::DefinitionNamespace::Local
            )
        {
            return Err(invalid(
                "package definition with an unresolved IndexSet extent is outside the admitted indexed profile; supply a closed static extent",
            ));
        }
        if self
            .index_sets
            .insert(declaration.name().to_owned(), extent)
            .is_some()
        {
            return Err(invalid("index set name is declared more than once"));
        }
        Ok(())
    }

    pub(in crate::hierarchy::body_check) fn indexed_member(
        &self,
        expression: &Expr,
    ) -> Result<(NamePath, Vec<String>), Diagnostic> {
        let invalid = |message: &str| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                message,
            )
        };
        let ExprKind::Member { value, member } = expression.kind() else {
            return Err(invalid("indexed member requires a family selection"));
        };
        let ExprKind::Index { value, index } = value.kind() else {
            return Err(invalid(
                "member access requires one indexed family occurrence",
            ));
        };
        let ExprKind::Name(family_name) = value.kind() else {
            return Err(invalid(
                "indexed family must be a local instance declaration",
            ));
        };
        let family = self
            .child_instances
            .get(family_name)
            .and_then(|instance| instance.family())
            .ok_or_else(|| invalid("indexed member target is not an instance family"))?;
        let ExprKind::Call { callee, arguments } = index.kind() else {
            return Err(invalid("family selection requires index(Set, ordinal)"));
        };
        let [set, ordinal] = arguments.as_slice() else {
            return Err(invalid("index requires a named set and an ordinal"));
        };
        let set = match set.kind() {
            ExprKind::Name(name) => name.as_str(),
            ExprKind::Path(name) => name.as_str(),
            _ => return Err(invalid("index requires an exact named index set")),
        };
        if callee.as_str() != "index" || set != family.set().as_str() {
            return Err(invalid(
                "family selection requires the exact same nominal IndexSet",
            ));
        }
        let extent = self
            .index_sets
            .get(set)
            .ok_or_else(|| invalid("unresolved index set in family selection"))?;
        let ordinal = crate::hierarchy::parameters::structural_index(
            self.file,
            ordinal,
            &self.static_values,
        )?
        .map(|(value, _)| value);
        if ordinal.is_none()
            && !matches!(
                self.namespace,
                crate::hierarchy::preflight::DefinitionNamespace::Local
            )
        {
            return Err(invalid(
                "package definition with an unresolved structural selector is outside the admitted indexed profile",
            ));
        }
        if let (Some(extent), Some(ordinal)) = (extent, ordinal)
            && ordinal >= *extent
        {
            return Err(invalid(
                "index ordinal is outside the exact IndexSet bounds",
            ));
        }
        let path =
            NamePath::from_segments([family_name.as_str(), member.as_str()], expression.range())
                .map_err(|error| invalid(error.message()))?;
        let key = vec![
            family_name.clone(),
            ordinal.map_or_else(
                || format!("<deferred-at-{}>", expression.range().start()),
                |value| value.to_string(),
            ),
            member.clone(),
        ];
        Ok((path, key))
    }
}
