//! Private exact-source mesh-family admission for the DFG cylinder galleries.
//!
//! This module composes existing Geometry, bounded Gmsh import, Mesh,
//! correspondence, and chordal-realization owners. It adds no public mesher,
//! artifact family, scientific mesh choice, time method, or result claim.

#![cfg_attr(not(test), allow(dead_code))]

use eqiora_artifact::{
    AcceptedCircularHoleChordalRealizationV1, GeometryMeshCorrespondenceEnvelopeV1,
    SimplicialMeshEnvelopeV1,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_io_gmsh::{GmshImportLimits, GmshSimplexImporter};
use eqiora_meshing::MeshQualityGate;

mod identity;
mod validate;

use identity::*;

const FIXTURE_MINIMUM_MEAN_RATIO: f64 = 1.0e-8;

#[derive(Clone)]
struct CylinderMeshMemberInput {
    provider: ProviderFamilyIdentity,
    provider_seed: u64,
    ordinal: usize,
    requested_max_boundary_error_m: f64,
    max_segments: usize,
    msh_bytes: Vec<u8>,
}

#[derive(Clone)]
struct PreparedCylinderMeshMember {
    provider: ProviderFamilyIdentity,
    provider_seed: u64,
    ordinal: usize,
    accepted: AcceptedCircularHoleChordalRealizationV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    vertex_count: usize,
    cell_count: usize,
    max_cylinder_chord: f64,
    max_triangle_diameter: f64,
    canonical_topology: CanonicalTopology,
}

impl PreparedCylinderMeshMember {
    fn from_input(
        source: &CanonicalGeometryV1,
        input: CylinderMeshMemberInput,
    ) -> Result<Self, Diagnostic> {
        validate::validate_exact_source(source)?;
        input.provider.revalidate()?;

        let quality_gate = MeshQualityGate::new(FIXTURE_MINIMUM_MEAN_RATIO)?;
        let reference = AcceptedCircularHoleChordalRealizationV1::from_reference(
            source,
            input.requested_max_boundary_error_m,
            input.max_segments,
            quality_gate,
        )?;
        let mesh = GmshSimplexImporter::new(2, quality_gate, GmshImportLimits::default())?
            .import_bytes(&input.msh_bytes)?;
        let mesh = SimplicialMeshEnvelopeV1::from_mesh(&mesh)?;
        let correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(
            reference.realized_geometry(),
            &mesh,
        )?;
        let accepted = reference.bind_conforming_mesh(&mesh, &correspondence)?;
        accepted.revalidate()?;

        let vertex_count = mesh.mesh().vertices().len();
        let cell_count = mesh.mesh().cells().len();
        let max_cylinder_chord = validate::max_cylinder_chord(&accepted)?;
        let max_triangle_diameter = validate::max_triangle_diameter(mesh.mesh())?;
        let canonical_topology = validate::canonical_topology(mesh.mesh())?;
        validate::validate_named_partition(&accepted)?;

        Ok(Self {
            provider: input.provider,
            provider_seed: input.provider_seed,
            ordinal: input.ordinal,
            accepted,
            correspondence,
            vertex_count,
            cell_count,
            max_cylinder_chord,
            max_triangle_diameter,
            canonical_topology,
        })
    }

    fn nearest_mesh_vertex(&self, point: [f64; 2]) -> [f64; 2] {
        self.accepted
            .mesh()
            .mesh()
            .vertices()
            .iter()
            .filter(|vertex| {
                [
                    identity::normalized_bits(vertex[0]),
                    identity::normalized_bits(vertex[1]),
                ] != [
                    identity::normalized_bits(point[0]),
                    identity::normalized_bits(point[1]),
                ]
            })
            .min_by(|left, right| {
                validate::squared_distance(left, point)
                    .total_cmp(&validate::squared_distance(right, point))
            })
            .map(|vertex| [vertex[0], vertex[1]])
            .expect("an accepted Mesh has vertices")
    }
}

#[derive(Clone)]
struct CylinderMeshFamilyInput {
    benchmark: CylinderBenchmark,
    source: CanonicalGeometryV1,
    primary: Vec<PreparedCylinderMeshMember>,
    bias: PreparedCylinderMeshMember,
    probes: ProbeInventoryIdentity,
    time_family: Option<TimeFamilyInput>,
    space_time_cells: Vec<SpaceTimeCellInput>,
}

#[derive(Clone)]
struct AcceptedCylinderMeshMember {
    provider: ProviderFamilyIdentity,
    provider_seed: u64,
    ordinal: usize,
    accepted: AcceptedCircularHoleChordalRealizationV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    vertex_count: usize,
    cell_count: usize,
    max_cylinder_chord: f64,
    max_triangle_diameter: f64,
    canonical_topology: CanonicalTopology,
    identity: SpatialMemberIdentity,
}

impl AcceptedCylinderMeshMember {
    fn revalidate(&self) -> Result<(), Diagnostic> {
        validate::revalidate_member(self)?;
        if validate::spatial_member_identity(self)? != self.identity {
            return Err(invalid("cylinder mesh member identity differs from replay"));
        }
        Ok(())
    }

    const fn accepted(&self) -> &AcceptedCircularHoleChordalRealizationV1 {
        &self.accepted
    }

    const fn max_cylinder_chord(&self) -> f64 {
        self.max_cylinder_chord
    }

    const fn max_triangle_diameter(&self) -> f64 {
        self.max_triangle_diameter
    }

    const fn canonical_topology(&self) -> &CanonicalTopology {
        &self.canonical_topology
    }
}

#[derive(Clone)]
struct AcceptedCylinderMeshFamily {
    benchmark: CylinderBenchmark,
    source: CanonicalGeometryV1,
    primary: Vec<AcceptedCylinderMeshMember>,
    bias: AcceptedCylinderMeshMember,
    probes: ProbeInventoryIdentity,
    spatial_identity: SpatialFamilyIdentity,
    time_family: Option<TimeFamilyIdentity>,
    space_time_cells: Vec<SpaceTimeCellIdentity>,
}

impl AcceptedCylinderMeshFamily {
    fn revalidate(&self) -> Result<(), Diagnostic> {
        validate::revalidate_family(self)
    }

    const fn source(&self) -> &CanonicalGeometryV1 {
        &self.source
    }

    fn primary_members(&self) -> &[AcceptedCylinderMeshMember] {
        &self.primary
    }

    const fn bias_member(&self) -> &AcceptedCylinderMeshMember {
        &self.bias
    }

    const fn probe_inventory(&self) -> &ProbeInventoryIdentity {
        &self.probes
    }

    const fn time_family(&self) -> Option<&TimeFamilyIdentity> {
        self.time_family.as_ref()
    }

    fn space_time_cells(&self) -> &[SpaceTimeCellIdentity] {
        &self.space_time_cells
    }
}

fn admit_family(input: CylinderMeshFamilyInput) -> Result<AcceptedCylinderMeshFamily, Diagnostic> {
    validate::admit_family(input)
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

#[cfg(test)]
mod tests;
