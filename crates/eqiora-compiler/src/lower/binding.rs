use super::*;

#[derive(Debug, Clone)]
pub(super) enum Binding {
    Domain(Id<kinds::Domain>, DomainContract),
    Representation(Id<kinds::Representation>),
    Field(Id<kinds::Field>, FieldContract),
    Parameter(Id<kinds::Parameter>, DimExponents),
    Port(Id<kinds::Port>, PortContract),
    Clock(Id<kinds::ClockDomain>),
    Relation {
        relation: Id<kinds::Relation>,
        activation: Id<kinds::Activation>,
    },
}

impl Binding {
    pub(super) fn primary_id(&self) -> RawId {
        match self {
            Self::Domain(id, _) => id.erase(),
            Self::Representation(id) => id.erase(),
            Self::Field(id, _) => id.erase(),
            Self::Parameter(id, _) => id.erase(),
            Self::Port(id, _) => id.erase(),
            Self::Clock(id) => id.erase(),
            Self::Relation { relation, .. } => relation.erase(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum DomainContract {
    Spatial {
        dimensions: Option<usize>,
    },
    ScalarPhysical {
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    },
    BoundaryPhysical(BoundaryPhysicalConnector),
}

#[derive(Debug, Clone)]
pub(super) struct FieldContract {
    pub(super) dimension: DimExponents,
    pub(super) shape: Option<ValueShapeSyntax>,
    pub(super) domain: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedFieldContract {
    pub(super) dimension: DimExponents,
    pub(super) shape: ValueShape,
    pub(super) frame: ValueFrame,
}

#[derive(Debug, Clone)]
pub(super) enum PortContract {
    Signal {
        direction: SignalDirectionSyntax,
        dimension: DimExponents,
    },
    ConservingMarker {
        dimension: DimExponents,
    },
    ScalarPhysical {
        domain: String,
    },
    BoundaryPhysical {
        connector: String,
        boundary: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResolvedPortContract {
    Signal {
        direction: SignalDirectionSyntax,
        dimension: DimExponents,
    },
    ConservingMarker {
        dimension: DimExponents,
    },
    ScalarPhysical {
        domain: Id<kinds::Domain>,
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    },
    BoundaryPhysical {
        connector: Id<kinds::Domain>,
        boundary: Id<kinds::Domain>,
        trace_dimension: DimExponents,
        flux_dimension: DimExponents,
    },
}

pub(super) fn bind_domain(
    file: &str,
    range: TextRange,
    syntax: &DomainSyntax,
) -> Result<DomainContract, Diagnostic> {
    match syntax {
        DomainSyntax::ScalarPhysical {
            across_dimension,
            through_dimension,
        } => Ok(DomainContract::ScalarPhysical {
            across_dimension: lower_dimension(file, across_dimension)?,
            through_dimension: lower_dimension(file, through_dimension)?,
        }),
        DomainSyntax::CartesianBox(bounds) => Ok(DomainContract::Spatial {
            dimensions: Some(bounds.len()),
        }),
        DomainSyntax::Boundary { .. } => Ok(DomainContract::Spatial { dimensions: None }),
        _ => Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "Domain syntax is newer than this compiler",
        )),
    }
}

pub(super) fn resolve_field_contract(
    file: &str,
    range: TextRange,
    contract: &FieldContract,
    bindings: &BTreeMap<String, Binding>,
) -> Result<ResolvedFieldContract, Diagnostic> {
    let (shape, frame) = match contract.shape.as_ref() {
        None | Some(ValueShapeSyntax::Scalar) => (ValueShape::scalar(), ValueFrame::Invariant),
        Some(ValueShapeSyntax::Exact(extents)) => (
            ValueShape::new(extents.iter().copied()).map_err(|error| {
                source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string())
            })?,
            ValueFrame::Invariant,
        ),
        Some(ValueShapeSyntax::SpatialVector) => {
            let Some(domain) = contract.domain.as_deref() else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "`spatial_vector` Field shape requires an exact spatial Domain",
                ));
            };
            let Some(Binding::Domain(
                _,
                DomainContract::Spatial {
                    dimensions: Some(dimensions),
                },
            )) = bindings.get(domain)
            else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    format!(
                        "`spatial_vector` Field support `{domain}` has no exact ambient dimension"
                    ),
                ));
            };
            let extent = u32::try_from(*dimensions).map_err(|_| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "spatial ambient dimension exceeds the portable u32 shape range",
                )
            })?;
            (
                ValueShape::new([extent]).map_err(|error| {
                    source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string())
                })?,
                ValueFrame::SpatialCartesian,
            )
        }
        Some(_) => {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                "Field value shape is newer than this compiler",
            ));
        }
    };
    Ok(ResolvedFieldContract {
        dimension: contract.dimension,
        shape,
        frame,
    })
}

pub(super) fn bind_port(
    file: &str,
    range: TextRange,
    syntax: &PortSyntax,
) -> Result<PortContract, Diagnostic> {
    match syntax {
        PortSyntax::Signal {
            direction,
            dimension,
        } => Ok(PortContract::Signal {
            direction: *direction,
            dimension: lower_dimension(file, dimension)?,
        }),
        PortSyntax::ConservingMarker { dimension } => Ok(PortContract::ConservingMarker {
            dimension: lower_dimension(file, dimension)?,
        }),
        PortSyntax::ScalarPhysical { domain } => Ok(PortContract::ScalarPhysical {
            domain: domain.clone(),
        }),
        _ => Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "Port syntax is newer than this compiler",
        )),
    }
}

pub(super) fn resolve_port_contract(
    file: &str,
    range: TextRange,
    contract: &PortContract,
    bindings: &BTreeMap<String, Binding>,
) -> Result<ResolvedPortContract, Diagnostic> {
    match contract {
        PortContract::Signal {
            direction,
            dimension,
        } => Ok(ResolvedPortContract::Signal {
            direction: *direction,
            dimension: *dimension,
        }),
        PortContract::ConservingMarker { dimension } => {
            Ok(ResolvedPortContract::ConservingMarker {
                dimension: *dimension,
            })
        }
        PortContract::ScalarPhysical { domain } => {
            let Some(Binding::Domain(
                domain_id,
                DomainContract::ScalarPhysical {
                    across_dimension,
                    through_dimension,
                },
            )) = bindings.get(domain)
            else {
                return match bindings.get(domain) {
                    Some(_) => Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        range,
                        format!("physical Port Domain `{domain}` is not scalar physical"),
                    )),
                    None => Err(unresolved(file, range, domain, "scalar physical Domain")),
                };
            };
            Ok(ResolvedPortContract::ScalarPhysical {
                domain: *domain_id,
                across_dimension: *across_dimension,
                through_dimension: *through_dimension,
            })
        }
        PortContract::BoundaryPhysical {
            connector,
            boundary,
        } => {
            let Some(Binding::Domain(connector_id, DomainContract::BoundaryPhysical(contract))) =
                bindings.get(connector)
            else {
                return Err(unresolved(
                    file,
                    range,
                    connector,
                    "field-physical Connector Domain",
                ));
            };
            let Some(Binding::Domain(boundary_id, DomainContract::Spatial { .. })) =
                bindings.get(boundary)
            else {
                return Err(unresolved(
                    file,
                    range,
                    boundary,
                    "field-physical boundary Domain",
                ));
            };
            Ok(ResolvedPortContract::BoundaryPhysical {
                connector: *connector_id,
                boundary: *boundary_id,
                trace_dimension: contract.trace_dimension(),
                flux_dimension: contract.flux_dimension(),
            })
        }
    }
}

pub(super) fn insert_binding(
    file: &str,
    bindings: &mut BTreeMap<String, Binding>,
    name: &str,
    binding: Binding,
    range: TextRange,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if is_reserved(name) {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("`{name}` is reserved by Eqiora Language v0"),
        ));
    } else if bindings.insert(name.to_owned(), binding).is_some() {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("duplicate declaration name `{name}`"),
        ));
    }
}

fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "model"
            | "domain"
            | "representation"
            | "field"
            | "parameter"
            | "port"
            | "clock"
            | "relation"
            | "connect"
            | "boundary"
            | "box"
            | "axis"
            | "side"
            | "lower"
            | "upper"
            | "continuum"
            | "on"
            | "as"
            | "continuous"
            | "periodic"
            | "signal"
            | "conserving"
            | "scalar_physical"
            | "input"
            | "output"
            | "period"
            | "phase"
            | "time"
            | "derivative"
            | "pre"
            | "next"
            | "grad"
            | "div"
            | "symmetric_part"
            | "isotropic_lift"
            | "trace"
            | "normal"
            | "across"
            | "through"
            | "kg"
            | "m"
            | "s"
            | "A"
            | "K"
            | "mol"
            | "cd"
    )
}
