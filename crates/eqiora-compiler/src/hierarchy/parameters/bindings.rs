//! Call-site inputs stay in their frozen parent scope while child types specialize.
use super::*;

pub(super) struct Bindings {
    file: String,
    expressions: BTreeMap<String, Expr>,
    parent: SymbolicParameterMap,
    clocks: BTreeMap<String, Option<eqiora_schema::kernel::RationalTime>>,
    frames: BTreeMap<String, SpatialSupport<String>>,
    member: Option<String>,
    closed: bool,
}

impl Bindings {
    pub(super) fn values(&self) -> &SymbolicParameterMap {
        &self.parent
    }
    pub(super) fn closed(
        file: &str,
        expressions: BTreeMap<String, Expr>,
        clocks: BTreeMap<String, Option<eqiora_schema::kernel::RationalTime>>,
        frames: BTreeMap<String, SpatialSupport<String>>,
    ) -> Self {
        Self {
            file: file.to_owned(),
            expressions,
            parent: BTreeMap::new(),
            clocks,
            frames,
            member: None,
            closed: true,
        }
    }

    pub(super) fn freeze(
        file: &str,
        component: &ComponentDecl,
        instance: &InstanceDecl,
        mut parent: impl FnMut(&str) -> Option<SymbolicParameterValue>,
        (clock, frame): StaticContexts<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let declarations = parameter_declarations(component);
        let mut diagnostics =
            super::super::named_bindings::validate_names(file, component, instance);
        let mut result = Self {
            file: file.to_owned(),
            expressions: BTreeMap::new(),
            parent: BTreeMap::new(),
            clocks: BTreeMap::new(),
            frames: BTreeMap::new(),
            member: instance.family().map(|family| family.member().to_owned()),
            closed: false,
        };
        for binding in instance.bindings() {
            let Some(declaration) = declarations.get(binding.name()) else {
                continue;
            };
            if declaration.visibility() != VisibilitySyntax::Public {
                diagnostics.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    binding.range(),
                    format!(
                        "private Parameter `{}` cannot be bound on instance `{}`",
                        binding.name(),
                        instance.name()
                    ),
                ));
                continue;
            }
            let expression = binding.value().rewrite_name_paths(|name| {
                if let Some(value) = parent(name.as_str()) {
                    result.parent.insert(name.as_str().to_owned(), value);
                }
                if let Some(value) = clock(name.as_str()) {
                    result.clocks.insert(name.as_str().to_owned(), value);
                }
                if let Some(value) = frame(name.as_str()) {
                    result.frames.insert(name.as_str().to_owned(), value);
                }
                None
            });
            if result
                .expressions
                .insert(binding.name().to_owned(), expression)
                .is_some()
            {
                diagnostics.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    binding.range(),
                    format!(
                        "duplicate binding for Parameter `{}` in instance `{}`",
                        binding.name(),
                        instance.name()
                    ),
                ));
            }
        }
        if result
            .member
            .as_ref()
            .is_some_and(|member| result.parent.contains_key(member))
        {
            // An expanded occurrence carries the exact nominal member value.
            // Only a definition-time family binder remains symbolic.
            result.member = None;
        }
        if diagnostics.is_empty() {
            Ok(result)
        } else {
            Err(diagnostics)
        }
    }

    pub(super) fn expression(&self, name: &str) -> Option<&Expr> {
        self.expressions.get(name)
    }

    pub(super) fn evaluate(
        &self,
        expression: &Expr,
        target: ValueType,
    ) -> Result<SymbolicParameterValue, Diagnostic> {
        if self.closed {
            // Native selected-entry inputs are closed declared values, not source
            // call-site expressions. Preserve their existing target-unit context.
            let value = super::static_values::closed_value_with_frames(
                &self.file,
                expression,
                target,
                &mut |name| self.frames.get(name).cloned(),
            )?;
            return Ok(SymbolicParameterValue {
                value_type: value.value_type().clone(),
                expression: Some(LoweringExpression::literal(
                    value.clone(),
                    expression.range(),
                )),
                value: Some(value),
                lineage: Some(ParameterLineage::Constant),
            });
        }
        let context = self.member.as_deref().map_or(
            ExpressionContext::Binding,
            ExpressionContext::IndexedBinding,
        );
        evaluate_initializer(
            &self.file,
            expression,
            context,
            &mut |name, range| {
                self.parent.get(name).cloned().ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        &self.file,
                        range,
                        context.unknown_name_message(name),
                    )
                })
            },
            target.clone(),
            "Parameter binding",
            (&mut |name| self.clocks.get(name).copied(), &mut |name| {
                self.frames.get(name).cloned()
            }),
        )
        .and_then(|value| coerce_parameter(&self.file, expression.range(), value, target))
    }
}
