//! Nominal index-set lookup and immutable per-occurrence binder values.
use super::*;
use crate::hierarchy::hierarchy_error;
use crate::hierarchy::parameters::{ParameterLineage, SymbolicParameterValue};
use eqiora_schema::kernel::IndexSetDef;

#[derive(Debug, Clone)]
pub(super) struct ScopedIndexSet {
    definition: IndexSetDef,
    dependencies: std::rc::Rc<std::cell::RefCell<std::collections::BTreeSet<String>>>,
}

impl Scope {
    pub(in crate::hierarchy) fn index_set(&self, name: &str) -> Option<&IndexSetDef> {
        self.index_sets.get(name).map(|value| &value.definition)
    }

    pub(in crate::hierarchy) fn insert_index_set(
        &mut self,
        name: String,
        definition: IndexSetDef,
    ) -> Result<(), Diagnostic> {
        if self
            .index_sets
            .insert(
                name,
                ScopedIndexSet {
                    definition,
                    dependencies: Default::default(),
                },
            )
            .is_some()
        {
            return Err(hierarchy_error(
                "index set name is declared more than once in this occurrence",
            ));
        }
        Ok(())
    }

    pub(in crate::hierarchy) fn with_index_member(
        &self,
        name: &str,
        definition: &IndexSetDef,
        ordinal: u32,
    ) -> Result<Self, Diagnostic> {
        let value =
            eqiora_core::ValueLiteral::from_integer(definition.value_type(), i64::from(ordinal))
                .map_err(|error| hierarchy_error(error.to_string()))?;
        let mut scope = self.clone();
        scope
            .insert_let(
                name.to_owned(),
                SymbolicParameterValue {
                    value_type: value.value_type().clone(),
                    expression: Some(LoweringExpression::literal(
                        value.clone(),
                        TextRange::new(0, 0),
                    )),
                    value: Some(value),
                    lineage: Some(ParameterLineage::Constant),
                },
            )
            .map_err(hierarchy_error)?;
        Ok(scope)
    }
}

impl Scope {
    pub(in crate::hierarchy) fn indexed_input(
        &self,
        family: &str,
        ordinal: u32,
        member: &str,
    ) -> Option<&FlatSymbol> {
        self.children
            .get(&format!("{family}[{ordinal}]"))?
            .public_ports
            .get(member)
    }

    pub(in crate::hierarchy) fn indexed_port(
        &self,
        file: &str,
        expression: &eqiora_lang::Expr,
    ) -> Result<&FlatSymbol, Diagnostic> {
        let invalid = |message: &str| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                expression.range(),
                message,
            )
        };
        let ExprKind::Member { value, member } = expression.kind() else {
            return Err(invalid("expected indexed member"));
        };
        let ExprKind::Index { value, index } = value.kind() else {
            return Err(invalid(
                "member access requires one indexed family occurrence",
            ));
        };
        let ExprKind::Name(family) = value.kind() else {
            return Err(invalid("indexed family requires a local instance name"));
        };
        let ExprKind::Call { callee, arguments } = index.kind() else {
            return Err(invalid("family selection requires index(Set, ordinal)"));
        };
        let [set, ordinal] = arguments.as_slice() else {
            return Err(invalid("index requires a named set and ordinal"));
        };
        let set_name = match set.kind() {
            ExprKind::Name(name) => name.as_str(),
            ExprKind::Path(path) => path.as_str(),
            _ => return Err(invalid("index requires an exact named set")),
        };
        if callee.as_str() != "index" {
            return Err(invalid(
                "family selection requires a nominal index constructor",
            ));
        }
        let set = self
            .index_sets
            .get(set_name)
            .ok_or_else(|| invalid("unresolved index set"))?;
        let (ordinal, dependencies) = crate::hierarchy::parameters::structural_index(
            file,
            ordinal,
            &self.symbolic_parameters(),
        )?
        .ok_or_else(|| invalid("structural selector remains unresolved"))?;
        if ordinal >= set.definition.extent() {
            return Err(invalid(
                "index ordinal is outside the exact IndexSet bounds",
            ));
        }
        let child = self
            .children
            .get(&format!("{family}[{ordinal}]"))
            .ok_or_else(|| invalid("unresolved indexed child occurrence"))?;
        if child.index_set != Some(set.definition.id()) {
            return Err(invalid(
                "family selection requires the exact same nominal IndexSet",
            ));
        }
        set.dependencies.borrow_mut().extend(dependencies);
        child
            .public_ports
            .get(member)
            .ok_or_else(|| invalid("indexed child has no exposed Port with this name"))
    }

    pub(in crate::hierarchy) fn index_dependencies(
        &self,
    ) -> Vec<(eqiora_core::RawId, Vec<String>)> {
        self.index_sets
            .values()
            .map(|set| {
                (
                    set.definition.id().erase(),
                    set.dependencies.borrow().iter().cloned().collect(),
                )
            })
            .collect()
    }

    pub(in crate::hierarchy) fn endpoint(
        &self,
        file: &str,
        expression: &eqiora_lang::Expr,
    ) -> Result<&FlatSymbol, Diagnostic> {
        if matches!(expression.kind(), ExprKind::Member { .. }) {
            return self.indexed_port(file, expression);
        }
        let path = crate::source_endpoints::path(file, expression)?;
        self.resolve_port(&path).ok_or_else(|| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                expression.range(),
                "unresolved exact Port endpoint",
            )
        })
    }
}
