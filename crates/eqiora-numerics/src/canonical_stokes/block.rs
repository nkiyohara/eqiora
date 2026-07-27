//! Shared discrete block projection for the accepted steady MINI path.

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id, RawId, ValueShape};
use eqiora_meshing::{MeshTopology, SimplicialMesh};
use eqiora_realization::{
    AlgebraicBlock, MeshArtifactReference, ResolvedFieldwiseRealization,
    ResolvedTransientFieldwiseRealization, Space,
};
use eqiora_schema::kernel::ValueFrame;
use eqiora_sem::KernelProgram;
use eqiora_solver::LinearOperatorProperties;

use super::{
    IncompressibleFlowScaleProfile2d, SteadyIncompressibleStokesCartesianModel2d,
    SteadyStokesScaleProfile2d, TransientIncompressibleNavierStokesCartesianModel2d,
};
use crate::canonical_boundary::BoundaryRelationBinding2d;
use crate::canonical_boundary::{
    CartesianBoundaryInventory2d, PhysicalBoundaryDisposition, PhysicalBoundaryQuantity,
};
use crate::discrete_block::{
    AlgebraicClosure, AuxiliaryBlock, BlockRealizationIdentity, BlockSupport, BlockTransformation,
    ContributionBatch, ContributionTerm, DiscreteBlockContext, DiscreteBlockSystem, FieldBlock,
    FieldBlockRole, RelationBlock, RelationDisposition, ResidualBlock, ResidualOrigin,
    boundary_treatment,
};
use crate::simplicial_stokes::{
    SimplicialMiniStokesBoundary2d, SimplicialMiniStokesBoundaryCondition2d,
};

const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const PRESSURE: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -2,
    ..DimExponents::DIMENSIONLESS
};

pub(super) fn steady_stokes_block_system(
    model: &SteadyIncompressibleStokesCartesianModel2d,
    resolved: &ResolvedFieldwiseRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    boundary: &SimplicialMiniStokesBoundary2d,
    scales: SteadyStokesScaleProfile2d,
) -> Result<DiscreteBlockSystem, Diagnostic> {
    let domain = domain_id(model.domain())?;
    let velocity = field(model.velocity())?;
    let pressure = field(model.pressure())?;
    let force = field(model.force_potential())?;
    let plan = resolved.plan();
    let space_for = |field| {
        plan.spatial()
            .field_spaces()
            .iter()
            .find(|binding| binding.field() == field)
            .map(|binding| binding.space())
            .expect("accepted Stokes plan contains both exact Field spaces")
    };
    let mut fields = vec![
        FieldBlock::discrete(
            domain,
            velocity,
            space_for(velocity),
            ValueShape::new([2]).expect("two-component vector is representable"),
            VELOCITY,
            ValueFrame::SpatialCartesian,
            scales.velocity(),
            FieldBlockRole::Algebraic,
        )?,
        FieldBlock::discrete(
            domain,
            pressure,
            space_for(pressure),
            ValueShape::scalar(),
            PRESSURE,
            ValueFrame::Invariant,
            scales.pressure(),
            FieldBlockRole::Algebraic,
        )?,
        FieldBlock::coefficient(
            domain,
            force,
            ValueShape::scalar(),
            PRESSURE,
            ValueFrame::Invariant,
        ),
    ];

    let mut boundary_coefficients = Vec::new();
    for axis in 0..2 {
        for side in [
            eqiora_schema::kernel::BoundarySide::Lower,
            eqiora_schema::kernel::BoundarySide::Upper,
        ] {
            let Some(pressure_law) = model.normal_pressure(axis, side) else {
                continue;
            };
            let (Some(raw_field), Some(raw_definition)) = (
                pressure_law.coefficient_field(),
                pressure_law.definition_relation(),
            ) else {
                continue;
            };
            let coefficient = field(raw_field)?;
            let definition = relation(raw_definition)?;
            if let Some((_, existing)) = boundary_coefficients
                .iter()
                .find(|(field, _)| *field == coefficient)
            {
                if *existing != definition {
                    return Err(invalid_identity(
                        "one definition Relation for a normal-pressure Field",
                        raw_definition,
                    ));
                }
            } else {
                boundary_coefficients.push((coefficient, definition));
                fields.push(FieldBlock::coefficient(
                    domain,
                    coefficient,
                    ValueShape::scalar(),
                    PRESSURE,
                    ValueFrame::Invariant,
                ));
            }
        }
    }

    let force_definition = relation(model.force_potential_definition())?;
    let momentum = relation(model.momentum_relation())?;
    let incompressibility = relation(model.incompressibility_relation())?;
    let mut relations = vec![
        RelationBlock::new(
            force_definition,
            BlockSupport::Volume(domain),
            RelationDisposition::CoefficientDefinition { field: force },
        ),
        RelationBlock::new(
            momentum,
            BlockSupport::Volume(domain),
            RelationDisposition::Residual {
                tested: AlgebraicBlock::Field(velocity),
            },
        ),
        RelationBlock::new(
            incompressibility,
            BlockSupport::Volume(domain),
            RelationDisposition::Residual {
                tested: AlgebraicBlock::Field(pressure),
            },
        ),
    ];
    relations.extend(boundary_coefficients.iter().map(|(field, definition)| {
        RelationBlock::new(
            *definition,
            BlockSupport::Volume(domain),
            RelationDisposition::CoefficientDefinition { field: *field },
        )
    }));
    relations.extend(boundary_relation_blocks(
        model.boundary_inventory(),
        model.boundary_relations(),
        velocity,
    )?);

    let constraint = plan.spatial().constraints().first().copied();
    let auxiliaries = constraint
        .map(|constraint| AuxiliaryBlock::new(constraint, scales.gauge()))
        .transpose()?
        .into_iter()
        .collect::<Vec<_>>();
    let mut residuals = vec![
        ResidualBlock::new(
            AlgebraicBlock::Field(velocity),
            BlockSupport::Volume(domain),
            [ResidualOrigin::Relation(momentum)],
        )?,
        ResidualBlock::new(
            AlgebraicBlock::Field(pressure),
            BlockSupport::Volume(domain),
            [ResidualOrigin::Relation(incompressibility)],
        )?,
    ];
    if let Some(constraint) = constraint {
        residuals.push(ResidualBlock::new(
            AlgebraicBlock::ConstraintMultiplier { field: pressure },
            BlockSupport::Volume(domain),
            [ResidualOrigin::AlgebraicConstraint(constraint)],
        )?);
    }

    let essential = essential_relations(model)?;
    let transformations = (!essential.is_empty())
        .then_some(BlockTransformation::EssentialElimination {
            field: velocity,
            boundary_relations: essential.clone(),
        })
        .into_iter()
        .collect::<Vec<_>>();

    let cell_count = mesh
        .entity_count(2)
        .expect("accepted 2D Stokes mesh owns cells");
    let mut packet_cursor = cell_count;
    let mut contributions = vec![ContributionBatch::new(
        [BlockSupport::Volume(domain)],
        0..cell_count,
        [0, 1, 2],
        [
            ResidualOrigin::Relation(force_definition),
            ResidualOrigin::Relation(momentum),
            ResidualOrigin::Relation(incompressibility),
        ],
        parameter_inventory([
            model.dynamic_viscosity_expression().parameter_fields(),
            model.force_potential_expression().parameter_fields(),
        ]),
        [
            AlgebraicBlock::Field(velocity),
            AlgebraicBlock::Field(pressure),
        ],
        [
            AlgebraicBlock::Field(velocity),
            AlgebraicBlock::Field(pressure),
        ],
        [
            ContributionTerm::Stiffness,
            ContributionTerm::MixedConstraint,
            ContributionTerm::Load,
        ],
    )?];
    if let Some(constraint) = constraint {
        let end = packet_cursor + cell_count;
        contributions.push(ContributionBatch::new(
            [BlockSupport::Volume(domain)],
            packet_cursor..end,
            [0, 1, 2],
            [ResidualOrigin::AlgebraicConstraint(constraint)],
            [],
            [
                AlgebraicBlock::Field(pressure),
                AlgebraicBlock::ConstraintMultiplier { field: pressure },
            ],
            [
                AlgebraicBlock::Field(pressure),
                AlgebraicBlock::ConstraintMultiplier { field: pressure },
            ],
            [ContributionTerm::AlgebraicConstraint],
        )?);
        packet_cursor = end;
    }
    let traction_count = boundary
        .facets()
        .iter()
        .filter(|entry| {
            matches!(
                entry.condition(),
                SimplicialMiniStokesBoundaryCondition2d::ConstantTraction { .. }
            )
        })
        .count();
    if traction_count > 0 {
        let traction_relations = traction_relations(model)?;
        let boundary_origins = traction_relations
            .iter()
            .copied()
            .map(ResidualOrigin::Relation)
            .chain(
                boundary_coefficients
                    .iter()
                    .map(|(_, definition)| ResidualOrigin::Relation(*definition)),
            )
            .collect::<Vec<_>>();
        let mut supports = vec![BlockSupport::Volume(domain)];
        for binding in model.boundary_relations() {
            if traction_relations.contains(&relation(binding.relation())?) {
                let boundary = binding
                    .boundary()
                    .downcast::<kinds::Domain>()
                    .ok_or_else(|| invalid_identity("Boundary Domain", binding.boundary()))?;
                let support = BlockSupport::Boundary(boundary);
                if !supports.contains(&support) {
                    supports.push(support);
                }
            }
        }
        contributions.push(ContributionBatch::new(
            supports,
            packet_cursor..packet_cursor + traction_count,
            [0, 1],
            boundary_origins,
            boundary_parameter_inventory(model),
            [AlgebraicBlock::Field(velocity)],
            [AlgebraicBlock::Field(velocity)],
            [ContributionTerm::Boundary],
        )?);
        packet_cursor += traction_count;
    }

    let mut closures = vec![AlgebraicClosure::EssentialBoundary {
        field: velocity,
        relations: essential,
    }];
    if constraint.is_some() {
        closures.push(AlgebraicClosure::ZeroIntegral { field: pressure });
    } else {
        closures.push(AlgebraicClosure::BoundaryTraction {
            field: pressure,
            relations: traction_relations(model)?,
        });
    }

    DiscreteBlockSystem::new(
        DiscreteBlockContext::new(
            resolved.model(),
            resolved.semantic_revision(),
            BlockRealizationIdentity::Explicit(resolved.realization_revision()),
            Some(mesh_artifact),
        ),
        fields,
        auxiliaries,
        relations,
        residuals,
        transformations,
        closures,
        contributions,
        packet_cursor,
        3,
        0,
        LinearOperatorProperties::SymmetricIndefinite,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn transient_navier_stokes_block_system(
    program: &KernelProgram,
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    boundary: &SimplicialMiniStokesBoundary2d,
    resolved: &ResolvedTransientFieldwiseRealization,
    scales: IncompressibleFlowScaleProfile2d,
) -> Result<DiscreteBlockSystem, Diagnostic> {
    let domain = domain_id(model.domain())?;
    let velocity = field(model.velocity())?;
    let pressure = field(model.pressure())?;
    let force = field(model.force_potential())?;
    let momentum = relation(model.momentum_relation())?;
    let incompressibility = relation(model.incompressibility_relation())?;
    let force_definition = relation(model.force_potential_definition())?;
    let fields = vec![
        FieldBlock::discrete(
            domain,
            velocity,
            Space::simplex_p1_bubble(),
            ValueShape::new([2]).expect("two-component vector is representable"),
            VELOCITY,
            ValueFrame::SpatialCartesian,
            scales.velocity(),
            FieldBlockRole::Algebraic,
        )?,
        FieldBlock::discrete(
            domain,
            pressure,
            Space::continuous_lagrange(std::num::NonZeroU16::MIN),
            ValueShape::scalar(),
            PRESSURE,
            ValueFrame::Invariant,
            scales.pressure(),
            FieldBlockRole::Algebraic,
        )?,
        FieldBlock::coefficient(
            domain,
            force,
            ValueShape::scalar(),
            PRESSURE,
            ValueFrame::Invariant,
        ),
    ];
    let mut relations = vec![
        RelationBlock::new(
            force_definition,
            BlockSupport::Volume(domain),
            RelationDisposition::CoefficientDefinition { field: force },
        ),
        RelationBlock::new(
            momentum,
            BlockSupport::Volume(domain),
            RelationDisposition::Residual {
                tested: AlgebraicBlock::Field(velocity),
            },
        ),
        RelationBlock::new(
            incompressibility,
            BlockSupport::Volume(domain),
            RelationDisposition::Residual {
                tested: AlgebraicBlock::Field(pressure),
            },
        ),
    ];
    relations.extend(boundary_relation_blocks(
        model.boundary_inventory(),
        model.boundary_relations(),
        velocity,
    )?);

    let constraint = resolved
        .plan()
        .fieldwise()
        .spatial()
        .constraints()
        .first()
        .copied();
    let auxiliaries = constraint
        .map(|constraint| AuxiliaryBlock::new(constraint, scales.gauge()))
        .transpose()?
        .into_iter()
        .collect::<Vec<_>>();
    let mut residuals = vec![
        ResidualBlock::new(
            AlgebraicBlock::Field(velocity),
            BlockSupport::Volume(domain),
            [ResidualOrigin::Relation(momentum)],
        )?,
        ResidualBlock::new(
            AlgebraicBlock::Field(pressure),
            BlockSupport::Volume(domain),
            [ResidualOrigin::Relation(incompressibility)],
        )?,
    ];
    if let Some(constraint) = constraint {
        residuals.push(ResidualBlock::new(
            AlgebraicBlock::ConstraintMultiplier { field: pressure },
            BlockSupport::Volume(domain),
            [ResidualOrigin::AlgebraicConstraint(constraint)],
        )?);
    }

    let essential = transient_boundary_relations(model, |disposition| {
        matches!(disposition, PhysicalBoundaryDisposition::TraceZero)
            || matches!(
                disposition,
                PhysicalBoundaryDisposition::Prescribed(law)
                    if law.quantity() == PhysicalBoundaryQuantity::Trace
            )
    })?;
    let traction = transient_boundary_relations(model, |disposition| {
        matches!(disposition, PhysicalBoundaryDisposition::FluxZero)
            || matches!(
                disposition,
                PhysicalBoundaryDisposition::Prescribed(law)
                    if law.quantity() == PhysicalBoundaryQuantity::Flux
            )
    })?;
    let mut transformations = vec![BlockTransformation::EssentialElimination {
        field: velocity,
        boundary_relations: essential.clone(),
    }];
    transformations.extend([
        BlockTransformation::BackwardEulerDerivative {
            relation: momentum,
            state: velocity,
            duration: resolved.plan().time_step().duration(),
        },
        BlockTransformation::EnergySkewConvection {
            relation: momentum,
            velocity,
        },
    ]);

    let cell_count = mesh
        .entity_count(2)
        .expect("accepted 2D transient mesh owns cells");
    let mut packet_cursor = cell_count;
    let mut contributions = vec![ContributionBatch::new(
        [BlockSupport::Volume(domain)],
        0..cell_count,
        [0, 1],
        [
            ResidualOrigin::Relation(force_definition),
            ResidualOrigin::Relation(momentum),
            ResidualOrigin::Relation(incompressibility),
        ],
        parameter_inventory([
            model.mass_density_expression().parameter_fields(),
            model.dynamic_viscosity_expression().parameter_fields(),
            model.force_potential_expression().parameter_fields(),
        ]),
        [
            AlgebraicBlock::Field(velocity),
            AlgebraicBlock::Field(pressure),
        ],
        [
            AlgebraicBlock::Field(velocity),
            AlgebraicBlock::Field(pressure),
        ],
        [
            ContributionTerm::Mass,
            ContributionTerm::Advection,
            ContributionTerm::Stiffness,
            ContributionTerm::MixedConstraint,
            ContributionTerm::Load,
        ],
    )?];
    if let Some(constraint) = constraint {
        let end = packet_cursor + cell_count;
        contributions.push(ContributionBatch::new(
            [BlockSupport::Volume(domain)],
            packet_cursor..end,
            [0, 1],
            [ResidualOrigin::AlgebraicConstraint(constraint)],
            [],
            [
                AlgebraicBlock::Field(pressure),
                AlgebraicBlock::ConstraintMultiplier { field: pressure },
            ],
            [
                AlgebraicBlock::Field(pressure),
                AlgebraicBlock::ConstraintMultiplier { field: pressure },
            ],
            [ContributionTerm::AlgebraicConstraint],
        )?);
        packet_cursor = end;
    }
    let traction_count = boundary
        .facets()
        .iter()
        .filter(|facet| {
            matches!(
                facet.condition(),
                SimplicialMiniStokesBoundaryCondition2d::ConstantTraction { .. }
            )
        })
        .count();
    if traction_count > 0 {
        let mut supports = vec![BlockSupport::Volume(domain)];
        for binding in model.boundary_relations() {
            if traction.contains(&relation(binding.relation())?) {
                let support = BlockSupport::Boundary(domain_id(binding.boundary())?);
                if !supports.contains(&support) {
                    supports.push(support);
                }
            }
        }
        contributions.push(ContributionBatch::new(
            supports,
            packet_cursor..packet_cursor + traction_count,
            [0, 1],
            traction.iter().copied().map(ResidualOrigin::Relation),
            [],
            [AlgebraicBlock::Field(velocity)],
            [AlgebraicBlock::Field(velocity)],
            [ContributionTerm::Boundary],
        )?);
        packet_cursor += traction_count;
    }

    let mut closures = vec![AlgebraicClosure::EssentialBoundary {
        field: velocity,
        relations: essential,
    }];
    if constraint.is_some() {
        closures.push(AlgebraicClosure::ZeroIntegral { field: pressure });
    } else {
        closures.push(AlgebraicClosure::BoundaryTraction {
            field: pressure,
            relations: traction,
        });
    }
    DiscreteBlockSystem::new(
        DiscreteBlockContext::new(
            program.model(),
            eqiora_realization::SemanticRevision::new(program.revision().0),
            BlockRealizationIdentity::Explicit(resolved.realization_revision()),
            Some(mesh_artifact),
        ),
        fields,
        auxiliaries,
        relations,
        residuals,
        transformations,
        closures,
        contributions,
        packet_cursor,
        2,
        0,
        LinearOperatorProperties::General,
    )
}

fn parameter_inventory<'a>(
    fields: impl IntoIterator<Item = &'a [Id<kinds::Parameter>]>,
) -> Vec<Id<kinds::Parameter>> {
    let mut result = fields.into_iter().flatten().copied().collect::<Vec<_>>();
    result.sort_by_key(Id::ulid);
    result.dedup();
    result
}

fn boundary_parameter_inventory(
    model: &SteadyIncompressibleStokesCartesianModel2d,
) -> Vec<Id<kinds::Parameter>> {
    use eqiora_schema::kernel::BoundarySide;

    parameter_inventory((0..2).flat_map(|axis| {
        [BoundarySide::Lower, BoundarySide::Upper]
            .into_iter()
            .filter_map(move |side| model.normal_pressure(axis, side))
            .map(|pressure| pressure.expression().parameter_fields())
    }))
}

fn boundary_relation_blocks(
    inventory: &CartesianBoundaryInventory2d,
    bindings: &[BoundaryRelationBinding2d],
    field: Id<kinds::Field>,
) -> Result<Vec<RelationBlock>, Diagnostic> {
    bindings
        .iter()
        .map(|binding| {
            Ok(RelationBlock::new(
                relation(binding.relation())?,
                BlockSupport::Boundary(domain_id(binding.boundary())?),
                RelationDisposition::BoundaryCondition {
                    field,
                    treatment: boundary_treatment(inventory, *binding)?,
                },
            ))
        })
        .collect()
}

fn essential_relations(
    model: &SteadyIncompressibleStokesCartesianModel2d,
) -> Result<Vec<Id<kinds::Relation>>, Diagnostic> {
    relations_with_disposition(model, |disposition| {
        disposition == PhysicalBoundaryDisposition::TraceZero
    })
}

fn traction_relations(
    model: &SteadyIncompressibleStokesCartesianModel2d,
) -> Result<Vec<Id<kinds::Relation>>, Diagnostic> {
    relations_with_disposition(model, |disposition| {
        matches!(
            disposition,
            PhysicalBoundaryDisposition::FluxZero | PhysicalBoundaryDisposition::Prescribed(_)
        )
    })
}

fn relations_with_disposition(
    model: &SteadyIncompressibleStokesCartesianModel2d,
    predicate: impl Fn(PhysicalBoundaryDisposition) -> bool,
) -> Result<Vec<Id<kinds::Relation>>, Diagnostic> {
    model
        .boundary_relations()
        .iter()
        .filter(|binding| {
            model.boundary_inventory().entries().any(|(_, entry)| {
                entry.boundary() == binding.boundary() && predicate(entry.disposition())
            })
        })
        .map(|binding| relation(binding.relation()))
        .collect()
}

fn transient_boundary_relations(
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
    predicate: impl Fn(PhysicalBoundaryDisposition) -> bool,
) -> Result<Vec<Id<kinds::Relation>>, Diagnostic> {
    model
        .boundary_relations()
        .iter()
        .filter(|binding| {
            model.boundary_inventory().entries().any(|(_, entry)| {
                entry.boundary() == binding.boundary() && predicate(entry.disposition())
            })
        })
        .map(|binding| relation(binding.relation()))
        .collect()
}

fn domain_id(id: RawId) -> Result<Id<kinds::Domain>, Diagnostic> {
    id.downcast::<kinds::Domain>()
        .ok_or_else(|| invalid_identity("Domain", id))
}

fn field(id: RawId) -> Result<Id<kinds::Field>, Diagnostic> {
    id.downcast::<kinds::Field>()
        .ok_or_else(|| invalid_identity("Field", id))
}

fn relation(id: RawId) -> Result<Id<kinds::Relation>, Diagnostic> {
    id.downcast::<kinds::Relation>()
        .ok_or_else(|| invalid_identity("Relation", id))
}

fn invalid_identity(expected: &str, id: RawId) -> Diagnostic {
    Diagnostic::error(
        eqiora_core::diagnostic::codes::INVALID_REALIZATION,
        format!("steady Stokes block inventory expected {expected} identity, received {id}"),
    )
}
