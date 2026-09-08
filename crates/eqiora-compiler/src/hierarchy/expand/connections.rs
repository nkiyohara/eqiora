//! Resolve ordinary and indexed connection fragments through the shared owner.
use super::*;

impl RootExpansion<'_, '_> {
    pub(super) fn add_connection(
        &mut self,
        declaration: &ConnectionDecl,
        scope: &Scope,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        origin: ConnectionOrigin,
    ) -> Result<(), Diagnostic> {
        if let Some(binder) = declaration.binder() {
            let set = scope.index_set(binder.set().as_str()).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    &origin.definition_file,
                    binder.range(),
                    "Connection family requires a resolved exact IndexSet",
                )
            })?;
            for ordinal in 0..set.extent() {
                let member = scope.with_index_member(binder.member(), set, ordinal)?;
                let ports = declaration
                    .port_expressions()
                    .iter()
                    .map(|expression| member.endpoint(&origin.definition_file, expression))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut path = declaration_path.clone();
                path.extend([
                    "index_member".to_owned(),
                    set.id().to_string(),
                    ordinal.to_string(),
                ]);
                self.add_resolved_connection(
                    declaration.syntax(),
                    ports,
                    declaration.range(),
                    instance_path,
                    path,
                    ConnectionOrigin {
                        definition_file: origin.definition_file.clone(),
                        instance: origin.instance.clone(),
                        bindings: origin.bindings.clone(),
                    },
                )?;
            }
            return Ok(());
        }
        let ports = declaration
            .port_expressions()
            .iter()
            .map(|expression| scope.endpoint(&origin.definition_file, expression))
            .collect::<Result<Vec<_>, _>>()?;
        self.add_resolved_connection(
            declaration.syntax(),
            ports,
            declaration.range(),
            instance_path,
            declaration_path,
            origin,
        )
    }

    pub(super) fn add_boundary_connection(
        &mut self,
        declaration: &BoundaryConnectionDecl,
        scope: &Scope,
        active: Option<ActiveBoundaryMember<'_>>,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        origin: ConnectionOrigin,
    ) -> Result<(), Diagnostic> {
        let ports = declaration
            .ports()
            .iter()
            .map(|reference| {
                resolve_boundary_port_reference(&origin.definition_file, reference, scope, active)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if ports.len() < 2 {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                &origin.definition_file,
                declaration.range(),
                "Connection requires at least two visible Ports",
            ));
        }
        self.add_resolved_connection(
            declaration.syntax(),
            ports,
            declaration.range(),
            instance_path,
            declaration_path,
            origin,
        )
    }

    pub(super) fn add_resolved_connection(
        &mut self,
        syntax: ConnectionSyntax,
        mut ports: Vec<&FlatSymbol>,
        range: eqiora_lang::TextRange,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        origin: ConnectionOrigin,
    ) -> Result<(), Diagnostic> {
        if ports.len() < 2 {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                &origin.definition_file,
                range,
                "Connection requires at least two visible Ports",
            ));
        }
        match syntax {
            ConnectionSyntax::Conserving | ConnectionSyntax::SpatialPeriodic => {
                ports.sort_unstable_by_key(|port| port.full_identity);
            }
            ConnectionSyntax::Signal => {
                if let Some((_, inputs)) = ports.split_first_mut() {
                    inputs.sort_unstable_by_key(|port| port.full_identity);
                }
            }
        }
        let path_display = declaration_path.join("/");
        let path = DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)
            .map_err(|diagnostic| {
                source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    &origin.definition_file,
                    range,
                    format!(
                        "cannot identify Connection at declaration path `{path_display}`: {}",
                        diagnostic.message()
                    ),
                )
            })?;
        let source = EntitySourceOrigin {
            definition: SourceLocation::new(&origin.definition_file, range),
            instance: origin.instance,
            bindings: origin.bindings,
        };
        if matches!(
            syntax,
            ConnectionSyntax::Conserving | ConnectionSyntax::SpatialPeriodic
        ) && ports
            .first()
            .is_some_and(|port| self.physical_ports.contains_key(&port.full_identity))
        {
            if let Some(non_physical) = ports
                .iter()
                .find(|port| !self.physical_ports.contains_key(&port.full_identity))
            {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    &origin.definition_file,
                    range,
                    format!(
                        "physical Connection cannot include non-physical Port `{}`",
                        non_physical.display_name
                    ),
                ));
            }
            if syntax == ConnectionSyntax::SpatialPeriodic {
                if ports.len() != 2 {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        &origin.definition_file,
                        range,
                        "spatial-periodic Connection requires exactly two field-physical Ports",
                    ));
                }
                if ports.iter().any(|port| {
                    self.spatial_periodic_ports.contains(&port.full_identity)
                        || self.physical_connections.iter().any(|fragment| {
                            fragment.topology.members().contains(&port.full_identity)
                        })
                }) {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        &origin.definition_file,
                        range,
                        "spatial-periodic Port already belongs to another physical Connection",
                    ));
                }
                self.spatial_periodic_ports
                    .extend(ports.iter().map(|port| port.full_identity));
                let key = ElaborationKey::anonymous_connection_with_limits(
                    self.namespace.clone(),
                    instance_path.clone(),
                    path,
                    ports.iter().map(|port| port.full_identity),
                    self.elaborator.limits.identity,
                )?;
                let full = key.full_identity()?;
                self.items.push(FlatItemBlueprint::Connection {
                    syntax,
                    ports: ports
                        .into_iter()
                        .map(|port| port.internal_name.clone())
                        .collect(),
                    range,
                    identity: ConnectionIdentity {
                        key,
                        full,
                        origins: vec![source],
                    },
                });
                return Ok(());
            }
            let topology = ConnectionFragment::try_new(
                ports.iter().map(|port| port.full_identity),
                self.elaborator.limits.connection_sets,
            )
            .map_err(|error| {
                source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    &origin.definition_file,
                    range,
                    format!("cannot stage physical Connection fragment: {error}"),
                )
            })?;
            self.physical_connections.push(StagedPhysicalConnection {
                topology,
                origin: PhysicalConnectionOrigin {
                    declaration_path: path,
                    instance_path: instance_path.clone(),
                    source,
                },
            });
            return Ok(());
        }
        let key = ElaborationKey::anonymous_connection_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            path,
            ports.iter().map(|port| port.full_identity),
            self.elaborator.limits.identity,
        )?;
        let full = key.full_identity()?;
        self.items.push(FlatItemBlueprint::Connection {
            syntax,
            ports: ports
                .into_iter()
                .map(|port| port.internal_name.clone())
                .collect(),
            range,
            identity: ConnectionIdentity {
                key,
                full,
                origins: vec![source],
            },
        });
        Ok(())
    }
}
