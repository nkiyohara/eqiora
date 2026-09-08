//! Occurrence expansion with one exact nominal index in each parent scope.
use super::*;

impl<'d> RootExpansion<'_, 'd> {
    pub(super) fn expand_model_children(
        &mut self,
        model: &ModelDefinition<'d>,
        scope: &mut Scope,
    ) -> Result<(), Vec<Diagnostic>> {
        self.expand_children(
            &model.namespace,
            model.owned_items().filter_map(|item| match item {
                Item::Instance(instance) => Some(instance),
                _ => None,
            }),
            model.file,
            &self.root_path.clone(),
            "",
            scope,
        )?;
        Ok(())
    }
    pub(super) fn expand_children<'i>(
        &mut self,
        namespace: &DefinitionNamespace,
        instances: impl IntoIterator<Item = &'i InstanceDecl>,
        file: &str,
        parent_path: &InstancePath,
        parent_display: &str,
        scope: &mut Scope,
    ) -> Result<(), Vec<Diagnostic>> {
        for instance in instances {
            let component = self
                .elaborator
                .resolve_component(namespace, instance.definition(), file, instance.range())
                .map_err(one_diagnostic)?;
            self.expand_child_occurrences(
                component,
                instance,
                file,
                parent_path,
                parent_display,
                scope,
            )?;
        }
        Ok(())
    }
    fn expand_child_occurrences(
        &mut self,
        component: ComponentDefinition<'d>,
        instance: &InstanceDecl,
        file: &str,
        parent_path: &InstancePath,
        parent_display: &str,
        scope: &mut Scope,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(family) = instance.family() else {
            let path = child_instance_path(
                parent_path,
                instance.name(),
                self.elaborator.limits.identity,
            )
            .map_err(one_diagnostic)?;
            let interface = self.expand_component(
                component,
                instance,
                file,
                path,
                display_child(parent_display, instance.name()),
                scope,
            )?;
            scope.insert_child(instance.name().to_owned(), interface);
            return Ok(());
        };
        let set = scope
            .index_set(family.set().as_str())
            .cloned()
            .ok_or_else(|| {
                vec![source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    family.range(),
                    format!(
                        "indexed instance requires resolved IndexSet `{}`",
                        family.set()
                    ),
                )]
            })?;
        // The definition graph preflights aggregate nested multiplicities before
        // RootExpansion allocation. Retain a local bound before starting this loop.
        if usize::try_from(set.extent())
            .map_or(true, |extent| extent > self.elaborator.limits.max_instances)
        {
            return Err(vec![source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                family.range(),
                "indexed instance extent exceeds the instance expansion limit",
            )]);
        }
        for ordinal in 0..set.extent() {
            let member_scope = scope
                .with_index_member(family.member(), &set, ordinal)
                .map_err(one_diagnostic)?;
            let name = format!("{}[{ordinal}]", instance.name());
            let path = child_instance_path(parent_path, &name, self.elaborator.limits.identity)
                .map_err(one_diagnostic)?;
            let interface = self.expand_component(
                component.clone(),
                instance,
                file,
                path,
                display_child(parent_display, &name),
                &member_scope,
            )?;
            scope.insert_child(name, interface);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_instances_keep_distinct_occurrences_and_exact_ordinals() {
        let compiled = crate::compile("indexed.eqi", "component Cell(parameter value:1) {} model M() { indexset Stages=range(3); instance cell[i in Stages]:Cell(value=to_real(ordinal(i))); }").unwrap_or_else(|errors|panic!("{errors:?}"));
        let model = &compiled[0];
        let mut ids = BTreeSet::new();
        for ordinal in 0..3 {
            let id = model
                .symbols()
                .get(&format!("cell[{ordinal}].value"))
                .expect("indexed display identity");
            assert!(ids.insert(id));
            let value = model
                .transaction()
                .ops()
                .iter()
                .find_map(|op| match op {
                    eqiora_graph::Op::DefineKernelNode {
                        node: eqiora_schema::kernel::KernelNode::Parameter(parameter),
                    } if parameter.id().erase() == id => parameter.value().real_scalar_value(),
                    _ => None,
                })
                .unwrap();
            assert_eq!(value.value(), f64::from(ordinal));
        }
        assert!(model.symbols().get("cell.value").is_none());
    }
    #[test]
    fn nested_families_keep_parent_and_local_binder_identity() {
        let compiled=crate::compile("nested-indexed.eqi", "component Cell(parameter value:integer) {} component Row(parameter offset:integer) { indexset Columns=range(2); instance cell[j in Columns]:Cell(value=offset+ordinal(j)); } model M() { indexset Rows=range(2); instance row[i in Rows]:Row(offset=ordinal(i)*10); }").unwrap_or_else(|errors|panic!("{errors:?}"));
        for (name, expected) in [
            ("row[0].cell[0].value", 0),
            ("row[0].cell[1].value", 1),
            ("row[1].cell[0].value", 10),
            ("row[1].cell[1].value", 11),
        ] {
            let id = compiled[0].symbols().get(name).expect("nested occurrence");
            let actual = compiled[0]
                .transaction()
                .ops()
                .iter()
                .find_map(|op| match op {
                    eqiora_graph::Op::DefineKernelNode {
                        node: eqiora_schema::kernel::KernelNode::Parameter(parameter),
                    } if parameter.id().erase() == id => parameter.value().integer_scalar_value(),
                    _ => None,
                });
            assert_eq!(actual, Some(expected));
        }
    }
}
