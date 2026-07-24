//! L4 composition from accepted numerical Fields to durable spatial artifacts.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use eqiora_artifact::{
    DiscreteFieldEnvelopeV1, FieldSnapshotEnvelopeV1, SimplicialMeshEnvelopeV1,
    ValidatedFixedSpatialContextV1,
};
use eqiora_core::Diagnostic;
use eqiora_meshing::{
    DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape, MeshEntity, MeshTopology,
};
use eqiora_numerics::fsi::ResolvedFixedReferenceFsiSolution2d;

/// Durable logical snapshots and their exact normalized numeric leaves.
///
/// The bundle is an in-memory L4 composition, not another wire artifact. It
/// keeps every `DiscreteFieldEnvelopeV1` available for independent replay of
/// the corresponding logical snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct FixedReferenceFsiSnapshotSetV1 {
    snapshots: Vec<FieldSnapshotEnvelopeV1>,
    blocks: BTreeMap<eqiora_core::RawId, Vec<DiscreteFieldEnvelopeV1>>,
}

impl FixedReferenceFsiSnapshotSetV1 {
    /// Four exact FSI Field snapshots in canonical Field identity order.
    #[must_use]
    pub fn snapshots(&self) -> &[FieldSnapshotEnvelopeV1] {
        &self.snapshots
    }

    /// Normalized coefficient blocks for one exact Field.
    #[must_use]
    pub fn blocks(
        &self,
        field: eqiora_core::Id<eqiora_core::entity::kinds::Field>,
    ) -> Option<&[DiscreteFieldEnvelopeV1]> {
        self.blocks.get(&field.erase()).map(Vec::as_slice)
    }
}

/// Project one accepted fixed-reference FSI solution into four complete logical snapshots.
///
/// The function adds no alternate FSI semantics. It expands the accepted
/// coefficients into the existing mesh-wide discrete-field contract, with
/// canonical positive zero outside each exact Domain support. Fluid MINI
/// velocity retains both its vertex and cell-bubble blocks.
///
/// # Errors
/// Returns a structured artifact diagnostic for Model/Realization/result
/// drift, shape mismatch, stale spatial resources, or any snapshot invariant
/// failure.
pub fn snapshot_fixed_reference_fsi_solution_v1(
    context: &ValidatedFixedSpatialContextV1<'_>,
    solution: &ResolvedFixedReferenceFsiSolution2d,
) -> Result<FixedReferenceFsiSnapshotSetV1, Diagnostic> {
    let model = context.model_reference();
    let realization = context.realization();
    let mesh = context.mesh();
    if model.model() != solution.model()
        || model.semantic_revision() != solution.semantic_revision()
        || realization.realization_revision() != solution.realization_revision()
        || mesh.artifact_reference()? != solution.mesh_artifact()
        || &realization.plan()? != solution.realization_plan()
    {
        return Err(eqiora_core::Diagnostic::error(
            eqiora_core::diagnostic::codes::INVALID_ARTIFACT,
            "accepted FSI solution identity, plan, or mesh differs from the exact fixed-spatial context",
        ));
    }

    let vector_shape = DiscreteFieldShape::Vector {
        components: NonZeroU32::new(2).expect("two is non-zero"),
    };
    let vertex_count = mesh.mesh().vertices().len();
    let cell_count = mesh
        .mesh()
        .entity_count(mesh.dimension())
        .ok_or_else(|| artifact_error("FSI mesh has no top-dimensional cell stratum"))?;

    let mut fluid_velocity_vertices = vec![0.0; vertex_count * 2];
    for &vertex in solution.fluid_velocity_vertices() {
        let coefficient = solution
            .fluid_velocity_coefficient(vertex)
            .ok_or_else(|| artifact_error("accepted fluid velocity coefficient is missing"))?;
        write_vector_coefficient(
            &mut fluid_velocity_vertices,
            vertex.index(),
            coefficient,
            "fluid velocity vertex",
        )?;
    }
    let fluid_vertex = discrete_field(
        mesh,
        DiscreteFieldAssociation::Vertex,
        vector_shape,
        fluid_velocity_vertices,
    )?;

    let mut fluid_velocity_bubbles = vec![0.0; cell_count * 2];
    for (&cell, coefficient) in solution
        .fluid_velocity_cells()
        .iter()
        .zip(solution.fluid_velocity_bubble_coefficients())
    {
        write_vector_coefficient(
            &mut fluid_velocity_bubbles,
            cell.index(),
            *coefficient,
            "fluid velocity bubble cell",
        )?;
    }
    let fluid_bubble = discrete_field(
        mesh,
        DiscreteFieldAssociation::Cell,
        vector_shape,
        fluid_velocity_bubbles,
    )?;

    let mut pressure = vec![0.0; vertex_count];
    for (&vertex, &coefficient) in solution
        .fluid_pressure_vertices()
        .iter()
        .zip(solution.fluid_pressure_coefficients())
    {
        *pressure
            .get_mut(vertex.index())
            .ok_or_else(|| artifact_error("fluid pressure vertex is outside the exact mesh"))? =
            coefficient;
    }
    let pressure = discrete_field(
        mesh,
        DiscreteFieldAssociation::Vertex,
        DiscreteFieldShape::Scalar,
        pressure,
    )?;

    let mut solid_velocity = vec![0.0; vertex_count * 2];
    for &vertex in solution.solid_velocity_vertices() {
        let coefficient = solution
            .solid_velocity_coefficient(vertex)
            .ok_or_else(|| artifact_error("accepted solid velocity coefficient is missing"))?;
        write_vector_coefficient(
            &mut solid_velocity,
            vertex.index(),
            coefficient,
            "solid velocity vertex",
        )?;
    }
    let solid_velocity = discrete_field(
        mesh,
        DiscreteFieldAssociation::Vertex,
        vector_shape,
        solid_velocity,
    )?;

    let mut displacement = vec![0.0; vertex_count * 2];
    for &vertex in solution.solid_displacement_vertices() {
        let coefficient = solution
            .solid_displacement_coefficient(vertex)
            .ok_or_else(|| artifact_error("accepted solid displacement coefficient is missing"))?;
        write_vector_coefficient(
            &mut displacement,
            vertex.index(),
            coefficient,
            "solid displacement vertex",
        )?;
    }
    let displacement = discrete_field(
        mesh,
        DiscreteFieldAssociation::Vertex,
        vector_shape,
        displacement,
    )?;

    let fields = solution.fields();
    let mut blocks = BTreeMap::from([
        (
            fields.fluid_velocity().erase(),
            vec![fluid_vertex, fluid_bubble],
        ),
        (fields.fluid_pressure().erase(), vec![pressure]),
        (fields.solid_velocity().erase(), vec![solid_velocity]),
        (fields.solid_displacement().erase(), vec![displacement]),
    ]);
    let mut snapshots = [
        fields.fluid_velocity(),
        fields.fluid_pressure(),
        fields.solid_velocity(),
        fields.solid_displacement(),
    ]
    .into_iter()
    .map(|field| FieldSnapshotEnvelopeV1::new(context, field, &blocks[&field.erase()]))
    .collect::<Result<Vec<_>, _>>()?;
    snapshots.sort_by_key(|snapshot| snapshot.field().ulid());
    for field_blocks in blocks.values_mut() {
        field_blocks.sort_by_key(|block| match block.association() {
            DiscreteFieldAssociation::Vertex => 0,
            DiscreteFieldAssociation::Cell => 1,
        });
    }
    validate_interface_trace(mesh, solution, &blocks)?;
    Ok(FixedReferenceFsiSnapshotSetV1 { snapshots, blocks })
}

fn discrete_field(
    mesh: &SimplicialMeshEnvelopeV1,
    association: DiscreteFieldAssociation,
    shape: DiscreteFieldShape,
    values: Vec<f64>,
) -> Result<DiscreteFieldEnvelopeV1, Diagnostic> {
    let payload = DiscreteFieldPayload::new(mesh.mesh(), association, shape, values)
        .map_err(|error| artifact_error(error.message()))?;
    DiscreteFieldEnvelopeV1::from_payload(mesh, &payload)
}

fn write_vector_coefficient(
    values: &mut [f64],
    entity: usize,
    coefficient: [f64; 2],
    label: &str,
) -> Result<(), Diagnostic> {
    let start = entity
        .checked_mul(2)
        .ok_or_else(|| artifact_error(format!("{label} index overflows")))?;
    let end = start
        .checked_add(2)
        .ok_or_else(|| artifact_error(format!("{label} extent overflows")))?;
    values
        .get_mut(start..end)
        .ok_or_else(|| artifact_error(format!("{label} is outside the exact mesh")))?
        .copy_from_slice(&coefficient);
    Ok(())
}

fn validate_interface_trace(
    mesh: &SimplicialMeshEnvelopeV1,
    solution: &ResolvedFixedReferenceFsiSolution2d,
    blocks: &BTreeMap<eqiora_core::RawId, Vec<DiscreteFieldEnvelopeV1>>,
) -> Result<(), Diagnostic> {
    let fields = solution.fields();
    let fluid = vertex_block(blocks, fields.fluid_velocity(), "fluid velocity")?;
    let solid = vertex_block(blocks, fields.solid_velocity(), "solid velocity")?;
    for &facet in solution.interface_facets() {
        let vertices = mesh
            .mesh()
            .incidence(MeshEntity::new(mesh.dimension() - 1, facet.index()), 0)
            .ok_or_else(|| artifact_error("FSI interface facet has no vertex incidence"))?;
        for incidence in vertices {
            let start = incidence
                .entity
                .index()
                .checked_mul(2)
                .ok_or_else(|| artifact_error("FSI trace vertex offset overflows"))?;
            let end = start
                .checked_add(2)
                .ok_or_else(|| artifact_error("FSI trace vertex extent overflows"))?;
            if fluid.values().get(start..end) != solid.values().get(start..end)
                || fluid.values().get(start..end).is_none()
            {
                return Err(artifact_error(
                    "persisted FSI velocity blocks differ on the exact trace quotient",
                ));
            }
        }
    }
    Ok(())
}

fn vertex_block<'a>(
    blocks: &'a BTreeMap<eqiora_core::RawId, Vec<DiscreteFieldEnvelopeV1>>,
    field: eqiora_core::Id<eqiora_core::entity::kinds::Field>,
    label: &str,
) -> Result<&'a DiscreteFieldEnvelopeV1, Diagnostic> {
    blocks
        .get(&field.erase())
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|block| block.association() == DiscreteFieldAssociation::Vertex)
        })
        .ok_or_else(|| artifact_error(format!("FSI projection omitted the {label} vertex block")))
}

fn artifact_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(eqiora_core::diagnostic::codes::INVALID_ARTIFACT, message)
}
