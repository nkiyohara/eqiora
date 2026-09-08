//! Materialize owned Model declarations after all occurrence symbols are bound.
use super::*;

impl RootExpansion<'_, '_> {
    pub(super) fn materialize_model_items(
        &mut self,
        scope: &Scope,
        identities: &ScopeIdentities,
    ) -> Result<(), Diagnostic> {
        let model = self.model.clone();
        let mut initial_duplicates = BTreeMap::<String, usize>::new();
        for item in model.owned_items() {
            match item {
                Item::Initial(declaration) => {
                    let name = crate::source_identity::initial_declaration_name(declaration)?;
                    let duplicate = initial_duplicates.entry(name.clone()).or_default();
                    let name = format!("{name}-{duplicate}");
                    *duplicate += 1;
                    let identity = self.relation_identity(
                        &self.root_path,
                        definition_path(&self.model.namespace, "model", self.model.name(), &name),
                        SourceLocation::new(self.model.file, declaration.range()),
                        SourceLocation::new(self.model.file, self.model.range()),
                        Vec::new(),
                    )?;
                    let equations =
                        rewrite_equations(self.model.file, declaration.equations(), scope, None)?;
                    self.items.push(FlatItemBlueprint::Relation {
                        name: internal_name(identity.entity.full),
                        activation: eqiora_lang::ActivationSyntax::Continuous,
                        domain: None,
                        equations,
                        initial: true,
                        range: declaration.range(),
                        identity,
                    });
                }

                Item::Domain(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    let syntax = match declaration.syntax() {
                        DomainSyntax::CartesianBox(bounds) => DomainSyntax::CartesianBox(
                            super::cartesian::rewrite_coordinates(self.model.file, bounds, scope)?,
                        ),
                        DomainSyntax::Boundary { parent, axis, side } => {
                            let parent = resolve_local_kind(
                                self.model.file,
                                declaration.range(),
                                scope,
                                parent,
                                |kind| matches!(kind, SymbolKind::Domain),
                                "boundary parent Domain",
                            )?;
                            DomainSyntax::Boundary {
                                parent: parent.internal_name.clone(),
                                axis: *axis,
                                side: *side,
                            }
                        }
                        DomainSyntax::ScalarPhysical {
                            across_type,
                            through_type,
                        } => DomainSyntax::ScalarPhysical {
                            across_type: across_type.clone(),
                            through_type: through_type.clone(),
                        },
                        _ => {
                            return Err(source_error(
                                codes::LANGUAGE_LOWERING_ERROR,
                                self.model.file,
                                declaration.range(),
                                "Domain syntax is newer than hierarchy elaboration",
                            ));
                        }
                    };
                    self.items.push(FlatItemBlueprint::Domain {
                        name: internal_name(identity.full),
                        contract: LoweringDomainContract::Source(syntax),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Field(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    let (domain, activation) =
                        rewrite_field_scope(self.model.file, declaration, scope)?;
                    let representation = self.add_support_representation(domain.as_deref())?;
                    self.items.push(FlatItemBlueprint::Field {
                        name: internal_name(identity.full),
                        domain,
                        representation,
                        value_type: declaration.value_type().clone(),
                        role: declaration.role(),
                        activation,
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Parameter(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    self.items.push(FlatItemBlueprint::Parameter {
                        name: internal_name(identity.full),
                        value: scope
                            .parameter(declaration.name())
                            .expect("allocated model parameter")
                            .value
                            .clone(),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Port(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    let syntax = rewrite_model_port(
                        self.model.file,
                        declaration.syntax(),
                        declaration.range(),
                        scope,
                    )?;
                    self.items.push(FlatItemBlueprint::Port {
                        name: internal_name(identity.full),
                        contract: LoweringPortContract::Source(syntax),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Event(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    self.items.push(FlatItemBlueprint::Event {
                        name: internal_name(identity.full),
                        guard: crate::hierarchy::scope::rewrite_expression_with_boundary_member(
                            self.model.file,
                            declaration.guard(),
                            scope,
                            None,
                        )?,
                        direction: declaration.direction(),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Clock(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    self.items.push(FlatItemBlueprint::Clock {
                        supplied_id: None,
                        name: internal_name(identity.full),
                        period: declaration.period().clone(),
                        phase: declaration.phase().clone(),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Relation(declaration) => {
                    let identity = identities.relations[declaration.name()].clone();
                    let (activation, domain, equations) =
                        rewrite_relation(self.model.file, declaration, scope)?;
                    self.record_physical_relation_owners(
                        self.model.file,
                        declaration.range(),
                        identity.entity.full,
                        &equations,
                    )?;
                    self.items.push(FlatItemBlueprint::Relation {
                        initial: false,
                        name: internal_name(identity.entity.full),
                        activation,
                        domain,
                        equations,
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::RelationFamily(family) => {
                    self.add_indexed_relations(
                        family,
                        scope,
                        (&self.root_path.clone(), ""),
                        definition_path(
                            &model.namespace,
                            "model",
                            model.name(),
                            family.relation().name(),
                        ),
                        EntitySourceOrigin {
                            definition: SourceLocation::new(model.file, family.range()),
                            instance: SourceLocation::new(model.file, model.range()),
                            bindings: Vec::new(),
                        },
                    )?;
                }
                Item::Connection(declaration) => {
                    self.add_connection(
                        declaration,
                        scope,
                        &self.root_path.clone(),
                        definition_path(&self.model.namespace, "model", self.model.name(), "net"),
                        ConnectionOrigin {
                            instance: SourceLocation::new(self.model.file, self.model.range()),
                            bindings: Vec::new(),
                            definition_file: self.model.file.to_owned(),
                        },
                    )?;
                }
                Item::BoundaryConnection(declaration) => {
                    self.add_boundary_connection(
                        declaration,
                        scope,
                        None,
                        &self.root_path.clone(),
                        definition_path(&self.model.namespace, "model", self.model.name(), "net"),
                        ConnectionOrigin {
                            instance: SourceLocation::new(self.model.file, self.model.range()),
                            bindings: Vec::new(),
                            definition_file: self.model.file.to_owned(),
                        },
                    )?;
                }
                Item::Let(_) | Item::IndexSet(_) => {}
                Item::Instance(instance) => {
                    self.add_input_bindings(
                        instance,
                        scope,
                        &self.root_path.clone(),
                        definition_path(
                            &self.model.namespace,
                            "model",
                            self.model.name(),
                            "input_binding",
                        ),
                        &self.model.namespace.clone(),
                        ConnectionOrigin {
                            instance: SourceLocation::new(self.model.file, self.model.range()),
                            bindings: Vec::new(),
                            definition_file: self.model.file.to_owned(),
                        },
                    )?;
                }
                _ => {
                    return Err(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        self.model.file,
                        self.model.range(),
                        "model item is newer than hierarchy elaboration",
                    ));
                }
            }
        }
        let ports = self
            .model
            .signature()
            .iter()
            .filter_map(|item| match item {
                eqiora_lang::SignatureItem::Input(value)
                | eqiora_lang::SignatureItem::Output(value) => Some(value.name()),
                eqiora_lang::SignatureItem::Port(value) => Some(value.name()),
                _ => None,
            })
            .map(|name| {
                scope
                    .symbol(name)
                    .expect("allocated signature endpoint")
                    .internal_name
                    .clone()
            })
            .collect();
        self.items.push(FlatItemBlueprint::Boundary {
            ports,
            range: self.model.range(),
        });
        Ok(())
    }
}
