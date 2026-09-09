//! Nominal connector domain specialization and allocation.

use super::*;

impl<'a, 'd> RootExpansion<'a, 'd> {
    pub(super) fn port_quantities(
        &self,
        syntax: &PortSyntax,
        namespace: &DefinitionNamespace,
        file: &str,
        range: eqiora_lang::TextRange,
    ) -> Result<Option<PhysicalMemberNames>, Diagnostic> {
        match syntax {
            PortSyntax::Signal { .. } => Ok(None),
            PortSyntax::ScalarPhysicalConnector { connector }
            | PortSyntax::FieldPhysical { connector, .. } => {
                let definition = self
                    .elaborator
                    .resolve_connector(namespace, connector, file, range)?;
                PhysicalMemberNames::from_connector(definition.syntax())
                    .map(Some)
                    .ok_or_else(|| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            range,
                            "Connector has no physical quantities",
                        )
                    })
            }
            PortSyntax::ScalarPhysical { domain } => self
                .model
                .owned_items()
                .find_map(|item| {
                    let Item::Domain(declaration) = item else {
                        return None;
                    };
                    if declaration.name() != domain {
                        return None;
                    }
                    let DomainSyntax::ScalarPhysical {
                        across_name,
                        through_name,
                        ..
                    } = declaration.syntax()
                    else {
                        return None;
                    };
                    Some(PhysicalMemberNames::Scalar {
                        across: across_name.clone(),
                        through: through_name.clone(),
                    })
                })
                .map(Some)
                .ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        range,
                        format!("`{domain}` is not an exact scalar physical Domain"),
                    )
                }),
            _ => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                "unknown Port contract",
            )),
        }
    }

    pub(super) fn connector_domain(
        &mut self,
        connector: ConnectorDefinition<'d>,
        ambient_dimension: Option<usize>,
    ) -> Result<FlatSymbol, Diagnostic> {
        let definition = DefinitionKey {
            namespace: connector.namespace.clone(),
            name: connector.name().to_owned(),
        };
        let (shape, contract) = match connector.syntax() {
            ConnectorSyntax::ScalarPhysical {
                across_name,
                across_type,
                through_name,
                through_type,
            } => {
                crate::value_types::lower_scalar_type(connector.file, across_type)?;
                crate::value_types::lower_scalar_type(connector.file, through_type)?;
                (
                    ValueShape::scalar(),
                    LoweringDomainContract::Source(DomainSyntax::ScalarPhysical {
                        across_name: across_name.clone(),
                        across_type: across_type.clone(),
                        through_type: through_type.clone(),
                        through_name: through_name.clone(),
                    }),
                )
            }
            ConnectorSyntax::FieldPhysical {
                trace,
                flux,
                shape,
                frame,
                pairing,
            } => {
                let ambient_dimension = ambient_dimension.ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        connector.file,
                        connector.range(),
                        "field-physical Connector specialization requires an exact boundary support",
                    )
                })?;
                let shape = super::super::body_check::resolve_value_shape(
                    connector.file,
                    connector.range(),
                    shape,
                    ambient_dimension,
                )
                .map_err(|mut errors| {
                    errors.pop().unwrap_or_else(|| {
                        hierarchy_error("value-shape specialization failed without a diagnostic")
                    })
                })?;
                let frame = match frame {
                    FrameSyntax::Invariant => ValueFrame::Invariant,
                    FrameSyntax::Spatial => ValueFrame::SpatialCartesian,
                    _ => {
                        return Err(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            connector.file,
                            connector.range(),
                            "Connector frame is newer than this compiler",
                        ));
                    }
                };
                if frame == ValueFrame::SpatialCartesian
                    && (shape.is_scalar()
                        || shape.extents().iter().any(|extent| {
                            usize::try_from(extent.get()).ok() != Some(ambient_dimension)
                        }))
                {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        connector.file,
                        connector.range(),
                        "spatial Connector extents must all equal the exact support ambient dimension",
                    ));
                }
                let pairing = match pairing {
                    BoundaryPairingSyntax::EuclideanBoundaryDuality => {
                        BoundaryPairing::EuclideanBoundaryDuality
                    }
                    _ => {
                        return Err(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            connector.file,
                            connector.range(),
                            "Connector pairing is newer than this compiler",
                        ));
                    }
                };
                let quantity_type = |dimension: &eqiora_lang::Expr| {
                    eqiora_core::ValueType::shaped(
                        eqiora_core::ScalarDomain::Real,
                        lower_dimension(connector.file, dimension)?,
                        shape.clone(),
                        frame,
                    )
                    .map_err(|error| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            connector.file,
                            dimension.range(),
                            error.to_string(),
                        )
                    })
                };
                let contract = BoundaryPhysicalConnector::new(
                    quantity_type(trace.dimension())?,
                    quantity_type(flux.dimension())?,
                    pairing,
                )
                .map_err(|violation| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        connector.file,
                        connector.range(),
                        format!("invalid field-physical Connector: {violation:?}"),
                    )
                })?;
                (shape, LoweringDomainContract::BoundaryPhysical(contract))
            }
            _ => {
                return Err(source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    connector.file,
                    connector.range(),
                    "Connector syntax is newer than this compiler",
                ));
            }
        };
        let key = ConnectorSpecializationKey {
            definition,
            shape: shape.clone(),
        };
        if let Some(domain) = self.connector_domains.get(&key) {
            return Ok(domain.clone());
        }
        let mut declaration_path = connector.namespace.declaration_prefix();
        declaration_path.extend(["connector".to_owned(), connector.name().to_owned()]);
        if matches!(&contract, LoweringDomainContract::BoundaryPhysical(_)) {
            declaration_path.push("shape".to_owned());
            let specialization = if shape.is_scalar() {
                "scalar".to_owned()
            } else {
                shape
                    .extents()
                    .iter()
                    .map(|extent| extent.get().to_string())
                    .collect::<Vec<_>>()
                    .join("x")
            };
            declaration_path.push(specialization);
        }
        let identity = self.entity_identity(
            &self.root_path,
            declaration_path,
            EntityKind::Domain,
            SourceLocation::new(connector.file, connector.range()),
            SourceLocation::new(self.model.file, self.model.range()),
            Vec::new(),
        )?;
        let internal_name = internal_name(identity.full);
        self.items.push(FlatItemBlueprint::Domain {
            name: internal_name.clone(),
            contract,
            range: connector.range(),
            identity: identity.clone(),
        });
        let mut display_name = match &connector.namespace {
            DefinitionNamespace::Local => format!("connector::{}", connector.name()),
            DefinitionNamespace::Resolved(owner) => {
                format!("connector::{owner}::{}", connector.name())
            }
        };
        if !shape.is_scalar() {
            display_name.push_str(&format!("::{shape:?}"));
        }
        let symbol = FlatSymbol {
            internal_name,
            display_name: display_name.clone(),
            full_identity: identity.full,
            kind: SymbolKind::Domain,
        };
        self.display_symbols.insert(
            display_name,
            DisplayIdentity {
                full: identity.full,
                kind: EntityKind::Domain,
            },
        );
        self.connector_domains.insert(key, symbol.clone());
        Ok(symbol)
    }
}

impl<'a, 'd> RootExpansion<'a, 'd> {
    pub(super) fn component_port_syntax(
        &mut self,
        component: &ComponentDefinition<'d>,
        declaration: &ComponentPortDecl,
        scope: &Scope,
    ) -> Result<(LoweringPortContract, Option<PhysicalPortMaterialization>), Diagnostic> {
        match declaration.syntax() {
            PortSyntax::Signal { .. } => Ok((
                LoweringPortContract::Source(rewrite_model_port(
                    component.file,
                    declaration.syntax(),
                    declaration.range(),
                    scope,
                )?),
                None,
            )),
            PortSyntax::ScalarPhysicalConnector { connector } => {
                let connector = self.elaborator.resolve_connector(
                    &component.namespace,
                    connector,
                    component.file,
                    declaration.range(),
                )?;
                let domain = self.connector_domain(connector, None)?;
                Ok((
                    LoweringPortContract::Source(PortSyntax::ScalarPhysical {
                        domain: domain.internal_name,
                    }),
                    Some(PhysicalPortMaterialization {
                        contract: PhysicalExposureContractIdentity::ScalarPhysical {
                            connector: domain.full_identity,
                        },
                    }),
                ))
            }
            PortSyntax::FieldPhysical { connector, support } => {
                let exact_support = scope.spatial_support(support).ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        component.file,
                        declaration.range(),
                        format!("unresolved field-physical boundary support `{support}`"),
                    )
                })?;
                let SpatialSupport::Boundary { dimensions, .. } = exact_support else {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        component.file,
                        declaration.range(),
                        "field-physical Port `over` support must resolve to an exact boundary",
                    ));
                };
                let boundary = scope.symbol(support).ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        component.file,
                        declaration.range(),
                        "resolved boundary support has no flattened Domain symbol",
                    )
                })?;
                let connector = self.elaborator.resolve_connector(
                    &component.namespace,
                    connector,
                    component.file,
                    declaration.range(),
                )?;
                let connector = self.connector_domain(connector, Some(*dimensions))?;
                Ok((
                    LoweringPortContract::BoundaryPhysical {
                        connector: connector.internal_name.clone(),
                        boundary: boundary.internal_name.clone(),
                    },
                    Some(PhysicalPortMaterialization {
                        contract: PhysicalExposureContractIdentity::FieldBoundary {
                            connector: connector.full_identity,
                            boundary: boundary.full_identity,
                        },
                    }),
                ))
            }
            _ => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                component.file,
                declaration.range(),
                "component Port must be an explicit signal or nominal Connector interface",
            )),
        }
    }

    pub(super) fn component_port_family_syntax(
        &mut self,
        component: &ComponentDefinition<'d>,
        family: &ComponentPortFamilyDecl,
        boundary: FullElaborationIdentity,
        boundary_internal_name: &str,
        dimensions: usize,
    ) -> Result<(LoweringPortContract, PhysicalPortMaterialization), Diagnostic> {
        let PortSyntax::FieldPhysical { connector, .. } = family.port().syntax() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                component.file,
                family.range(),
                "boundary Port family requires a field-physical Connector",
            ));
        };
        let connector = self.elaborator.resolve_connector(
            &component.namespace,
            connector,
            component.file,
            family.range(),
        )?;
        let connector = self.connector_domain(connector, Some(dimensions))?;
        Ok((
            LoweringPortContract::BoundaryPhysical {
                connector: connector.internal_name.clone(),
                boundary: boundary_internal_name.to_owned(),
            },
            PhysicalPortMaterialization {
                contract: PhysicalExposureContractIdentity::FieldBoundary {
                    connector: connector.full_identity,
                    boundary,
                },
            },
        ))
    }
}
