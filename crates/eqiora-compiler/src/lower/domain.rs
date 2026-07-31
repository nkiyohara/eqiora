use eqiora_lang::CartesianCoordinateSyntax;
use eqiora_schema::kernel::{CartesianAxisDefinition, CartesianCoordinateSource};

use super::*;

pub(super) fn lower_domain(
    file: &str,
    range: TextRange,
    id: Id<kinds::Domain>,
    contract: DomainContract,
    lowering_contract: &LoweringDomainContract,
    bindings: &BTreeMap<String, Binding>,
) -> Result<(DomainDef, Option<RawId>, Vec<RawId>), Diagnostic> {
    match (lowering_contract, contract) {
        (
            LoweringDomainContract::Source(DomainSyntax::CartesianBox(bounds)),
            DomainContract::Spatial { .. },
        ) => {
            let mut lowered = Vec::with_capacity(bounds.len());
            let mut dependencies = BTreeSet::new();
            for (lower, upper) in bounds {
                lowered.push(CartesianAxisDefinition::new(
                    lower_coordinate_source(file, lower, bindings, &mut dependencies)?,
                    lower_coordinate_source(file, upper, bindings, &mut dependencies)?,
                ));
            }
            DomainDef::cartesian_box_from_sources(id, lowered)
                .map(|definition| (definition, None, dependencies.into_iter().collect()))
                .map_err(|diagnostic| {
                    source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        file,
                        range,
                        diagnostic.message(),
                    )
                })
        }
        (
            LoweringDomainContract::Source(DomainSyntax::Boundary { parent, axis, side }),
            DomainContract::Spatial { .. },
        ) => {
            let Some(parent_binding) = bindings.get(parent) else {
                return Err(unresolved(file, range, parent, "boundary parent Domain"));
            };
            let Binding::Domain(parent_id, DomainContract::Spatial { .. }) = parent_binding else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    format!("boundary parent `{parent}` is not a spatial Domain"),
                ));
            };
            let side = match side {
                BoundarySideSyntax::Lower => BoundarySide::Lower,
                BoundarySideSyntax::Upper => BoundarySide::Upper,
            };
            Ok((
                DomainDef::cartesian_boundary(id, *axis, side),
                Some(parent_id.erase()),
                Vec::new(),
            ))
        }
        (
            LoweringDomainContract::Source(DomainSyntax::ScalarPhysical { .. }),
            DomainContract::ScalarPhysical {
                across_dimension,
                through_dimension,
            },
        ) => Ok((
            DomainDef::scalar_physical(id, across_dimension, through_dimension),
            None,
            Vec::new(),
        )),
        (
            LoweringDomainContract::BoundaryPhysical(_),
            DomainContract::BoundaryPhysical(connector),
        ) => Ok((
            DomainDef::boundary_physical(id, connector),
            None,
            Vec::new(),
        )),
        _ => Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "Domain syntax is newer than this compiler",
        )),
    }
}

fn lower_coordinate_source(
    file: &str,
    coordinate: &CartesianCoordinateSyntax,
    bindings: &BTreeMap<String, Binding>,
    dependencies: &mut BTreeSet<RawId>,
) -> Result<CartesianCoordinateSource, Diagnostic> {
    match coordinate {
        CartesianCoordinateSyntax::Fixed { value, range } => CartesianCoordinateSource::fixed(
            DynQuantity::new(normalize_zero(*value), length_dimension()),
        )
        .map_err(|diagnostic| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                *range,
                diagnostic.message(),
            )
        }),
        CartesianCoordinateSyntax::Parameter { name, range } => {
            let Some(binding) = bindings.get(name) else {
                return Err(unresolved(
                    file,
                    *range,
                    name,
                    "Cartesian coordinate Parameter",
                ));
            };
            let Binding::Parameter(parameter, dimension) = binding else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    *range,
                    format!("Cartesian coordinate `{name}` is not a root Model Parameter"),
                ));
            };
            if *dimension != length_dimension() {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    *range,
                    format!("Cartesian coordinate Parameter `{name}` is not a length"),
                ));
            }
            dependencies.insert(parameter.erase());
            Ok(CartesianCoordinateSource::parameter(*parameter))
        }
    }
}
