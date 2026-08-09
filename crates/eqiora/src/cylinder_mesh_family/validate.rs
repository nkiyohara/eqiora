//! Replay and relation checks for the private cylinder mesh family.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_artifact::ArtifactDigest;
use eqiora_core::Diagnostic;
use eqiora_geometry::{EDGE_DIMENSION, FACE_DIMENSION};
use eqiora_meshing::{MeshEntity, MeshTopology, SimplicialMesh};

use super::*;

const EXACT_SOURCE_SHA256: [u8; 32] = [
    0xb0, 0x01, 0x23, 0x47, 0x2a, 0x59, 0x6e, 0x82, 0x89, 0x82, 0x0c, 0xab, 0xae, 0xe2, 0x0d, 0x52,
    0xcd, 0xf8, 0x1b, 0x55, 0x72, 0xfa, 0x9c, 0xe5, 0x8f, 0xf1, 0x7c, 0xda, 0xa0, 0x00, 0x46, 0xd9,
];
const MINIMUM_FAMILY_MEMBERS: usize = 3;
const MAXIMUM_FAMILY_MEMBERS: usize = 8;

pub(super) fn validate_exact_source(source: &CanonicalGeometryV1) -> Result<(), Diagnostic> {
    if source.digest_bytes() != EXACT_SOURCE_SHA256 {
        return Err(invalid(
            "cylinder mesh family requires the exact DFG source",
        ));
    }
    Ok(())
}

pub(super) fn admit_family(
    input: CylinderMeshFamilyInput,
) -> Result<AcceptedCylinderMeshFamily, Diagnostic> {
    validate_exact_source(&input.source)?;
    input.probes.revalidate()?;
    if !input.probes.is_exact_dfg() {
        return Err(invalid(
            "cylinder mesh family requires the exact ordered DFG probes",
        ));
    }

    let mut primary = Vec::new();
    primary
        .try_reserve_exact(input.primary.len())
        .map_err(|_| invalid("primary cylinder mesh family allocation exceeds capacity"))?;
    for member in input.primary {
        revalidate_prepared_member(&member)?;
        let identity = prepared_member_identity(&member)?;
        primary.push(AcceptedCylinderMeshMember {
            provider: member.provider,
            provider_seed: member.provider_seed,
            ordinal: member.ordinal,
            accepted: member.accepted,
            correspondence: member.correspondence,
            vertex_count: member.vertex_count,
            cell_count: member.cell_count,
            max_cylinder_chord: member.max_cylinder_chord,
            max_triangle_diameter: member.max_triangle_diameter,
            canonical_topology: member.canonical_topology,
            identity,
        });
    }

    revalidate_prepared_member(&input.bias)?;
    let bias_identity = prepared_member_identity(&input.bias)?;
    let bias = AcceptedCylinderMeshMember {
        provider: input.bias.provider,
        provider_seed: input.bias.provider_seed,
        ordinal: input.bias.ordinal,
        accepted: input.bias.accepted,
        correspondence: input.bias.correspondence,
        vertex_count: input.bias.vertex_count,
        cell_count: input.bias.cell_count,
        max_cylinder_chord: input.bias.max_cylinder_chord,
        max_triangle_diameter: input.bias.max_triangle_diameter,
        canonical_topology: input.bias.canonical_topology,
        identity: bias_identity,
    };

    validate_spatial_family(&input.source, &primary, &bias)?;
    let spatial_identity = spatial_family_identity(&input.source, &input.probes, &primary, &bias)?;
    let time_family = match input.time_family {
        Some(time) => Some(admit_time_family(time)?),
        None => None,
    };
    let space_time_cells = admit_cells(
        input.benchmark,
        &primary,
        time_family.as_ref(),
        input.space_time_cells,
    )?;

    let accepted = AcceptedCylinderMeshFamily {
        benchmark: input.benchmark,
        source: input.source,
        primary,
        bias,
        probes: input.probes,
        spatial_identity,
        time_family,
        space_time_cells,
    };
    revalidate_family(&accepted)?;
    Ok(accepted)
}

pub(super) fn revalidate_family(family: &AcceptedCylinderMeshFamily) -> Result<(), Diagnostic> {
    validate_exact_source(&family.source)?;
    family.probes.revalidate()?;
    if !family.probes.is_exact_dfg() {
        return Err(invalid(
            "accepted cylinder probe inventory differs from the exact source",
        ));
    }
    for member in &family.primary {
        member.revalidate()?;
    }
    family.bias.revalidate()?;
    validate_spatial_family(&family.source, &family.primary, &family.bias)?;
    let spatial = spatial_family_identity(
        &family.source,
        &family.probes,
        &family.primary,
        &family.bias,
    )?;
    if spatial != family.spatial_identity {
        return Err(invalid(
            "cylinder spatial-family identity differs from replay",
        ));
    }
    if let Some(time) = &family.time_family {
        revalidate_time_family(time)?;
    }
    revalidate_cells(
        family.benchmark,
        &family.primary,
        family.time_family.as_ref(),
        &family.space_time_cells,
    )
}

fn revalidate_prepared_member(member: &PreparedCylinderMeshMember) -> Result<(), Diagnostic> {
    member.provider.revalidate()?;
    revalidate_member_parts(
        &member.accepted,
        &member.correspondence,
        member.vertex_count,
        member.cell_count,
        member.max_cylinder_chord,
        member.max_triangle_diameter,
        &member.canonical_topology,
    )
}

pub(super) fn revalidate_member(member: &AcceptedCylinderMeshMember) -> Result<(), Diagnostic> {
    member.provider.revalidate()?;
    revalidate_member_parts(
        &member.accepted,
        &member.correspondence,
        member.vertex_count,
        member.cell_count,
        member.max_cylinder_chord,
        member.max_triangle_diameter,
        &member.canonical_topology,
    )
}

fn revalidate_member_parts(
    accepted: &AcceptedCircularHoleChordalRealizationV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    vertex_count: usize,
    cell_count: usize,
    chord: f64,
    diameter: f64,
    topology: &CanonicalTopology,
) -> Result<(), Diagnostic> {
    accepted.revalidate()?;
    validate_exact_source(accepted.source())?;
    if correspondence != accepted.correspondence() {
        return Err(invalid("cylinder member retains a stale correspondence"));
    }
    let mesh = accepted.mesh().mesh();
    if vertex_count != mesh.vertices().len()
        || cell_count != mesh.cells().len()
        || !(1..=1_000_000).contains(&vertex_count)
        || !(1..=2_000_000).contains(&cell_count)
    {
        return Err(invalid(
            "cylinder member Mesh counts differ from replay or exceed bounds",
        ));
    }
    if max_cylinder_chord(accepted)?.to_bits() != chord.to_bits()
        || max_triangle_diameter(mesh)?.to_bits() != diameter.to_bits()
        || canonical_topology(mesh)? != *topology
    {
        return Err(invalid(
            "cylinder member geometric observations differ from replay",
        ));
    }
    validate_named_partition(accepted)
}

fn validate_spatial_family(
    source: &CanonicalGeometryV1,
    primary: &[AcceptedCylinderMeshMember],
    bias: &AcceptedCylinderMeshMember,
) -> Result<(), Diagnostic> {
    if !(MINIMUM_FAMILY_MEMBERS..=MAXIMUM_FAMILY_MEMBERS).contains(&primary.len()) {
        return Err(invalid(
            "primary cylinder mesh family must contain 3..=8 members",
        ));
    }
    let first_provider = &primary[0].provider;
    if first_provider.family_role != ProviderFamilyRole::Primary
        || primary
            .iter()
            .any(|member| &member.provider != first_provider)
        || bias.provider.family_role != ProviderFamilyRole::Bias
    {
        return Err(invalid(
            "cylinder mesh provider-family identity or role is inconsistent",
        ));
    }
    if bias.provider.recipe_template_sha256 == first_provider.recipe_template_sha256 {
        return Err(invalid(
            "bias provider must retain a distinct recipe identity",
        ));
    }

    let mut seeds = BTreeSet::new();
    let mut geometry = BTreeSet::new();
    let mut meshes = BTreeSet::new();
    let mut correspondences = BTreeSet::new();
    let mut bindings = BTreeSet::new();
    for (ordinal, member) in primary.iter().enumerate() {
        if member.ordinal != ordinal
            || member.accepted.source().digest_bytes() != source.digest_bytes()
            || !seeds.insert(member.provider_seed)
            || !geometry.insert(digest(member.accepted.realized_geometry().digest()?))
            || !meshes.insert(digest(member.accepted.mesh().digest()?))
            || !correspondences.insert(digest(member.accepted.correspondence().digest()?))
            || !bindings.insert(digest(member.accepted.envelope().digest()?))
        {
            return Err(invalid(
                "primary cylinder mesh member identity is reused or out of order",
            ));
        }
    }
    for pair in primary.windows(2) {
        let coarse = &pair[0];
        let fine = &pair[1];
        if coarse.accepted.requested_max_boundary_error_m()
            <= fine.accepted.requested_max_boundary_error_m()
            || coarse.accepted.boundary_error_bound_m() < fine.accepted.boundary_error_bound_m()
            || coarse.accepted.circle_segments() >= fine.accepted.circle_segments()
            || coarse.max_cylinder_chord <= fine.max_cylinder_chord
            || coarse.max_triangle_diameter <= fine.max_triangle_diameter
        {
            return Err(invalid(
                "primary cylinder mesh refinement measures are not ordered",
            ));
        }
    }

    let fine = primary.last().expect("family cardinality checked");
    if bias.ordinal != fine.ordinal
        || bias.provider_seed == fine.provider_seed
        || bias.accepted.source().digest_bytes() != source.digest_bytes()
        || bias.accepted.requested_max_boundary_error_m().to_bits()
            != fine.accepted.requested_max_boundary_error_m().to_bits()
        || bias.accepted.boundary_error_bound_m().to_bits()
            != fine.accepted.boundary_error_bound_m().to_bits()
        || bias.accepted.circle_segments() != fine.accepted.circle_segments()
        || digest(bias.accepted.realized_geometry().digest()?)
            != digest(fine.accepted.realized_geometry().digest()?)
        || digest(bias.accepted.mesh().digest()?) == digest(fine.accepted.mesh().digest()?)
        || digest(bias.accepted.correspondence().digest()?)
            == digest(fine.accepted.correspondence().digest()?)
        || digest(bias.accepted.envelope().digest()?) == digest(fine.accepted.envelope().digest()?)
        || bias.canonical_topology == fine.canonical_topology
    {
        return Err(invalid(
            "independent bias member does not satisfy the fine-level relation",
        ));
    }
    Ok(())
}

fn prepared_member_identity(
    member: &PreparedCylinderMeshMember,
) -> Result<SpatialMemberIdentity, Diagnostic> {
    spatial_identity_parts(
        &member.provider,
        member.provider_seed,
        member.ordinal,
        &member.accepted,
        member.max_cylinder_chord,
        member.max_triangle_diameter,
    )
}

pub(super) fn spatial_member_identity(
    member: &AcceptedCylinderMeshMember,
) -> Result<SpatialMemberIdentity, Diagnostic> {
    spatial_identity_parts(
        &member.provider,
        member.provider_seed,
        member.ordinal,
        &member.accepted,
        member.max_cylinder_chord,
        member.max_triangle_diameter,
    )
}

fn spatial_identity_parts(
    provider: &ProviderFamilyIdentity,
    provider_seed: u64,
    ordinal: usize,
    accepted: &AcceptedCircularHoleChordalRealizationV1,
    chord: f64,
    diameter: f64,
) -> Result<SpatialMemberIdentity, Diagnostic> {
    Ok(SpatialMemberIdentity {
        provider: provider.clone(),
        ordinal,
        source_sha256: accepted.source().digest_bytes(),
        realized_geometry_sha256: digest(accepted.realized_geometry().digest()?),
        mesh_sha256: digest(accepted.mesh().digest()?),
        correspondence_sha256: digest(accepted.correspondence().digest()?),
        realization_binding_sha256: digest(accepted.envelope().digest()?),
        requested_boundary_error_bits: normalized_bits(accepted.requested_max_boundary_error_m()),
        accepted_boundary_error_bits: normalized_bits(accepted.boundary_error_bound_m()),
        circle_segments: accepted.circle_segments(),
        max_cylinder_chord_bits: normalized_bits(chord),
        max_triangle_diameter_bits: normalized_bits(diameter),
        provider_seed,
    })
}

fn spatial_family_identity(
    source: &CanonicalGeometryV1,
    probes: &ProbeInventoryIdentity,
    primary: &[AcceptedCylinderMeshMember],
    bias: &AcceptedCylinderMeshMember,
) -> Result<SpatialFamilyIdentity, Diagnostic> {
    let primary_identities = primary
        .iter()
        .map(spatial_member_identity)
        .collect::<Result<Vec<_>, _>>()?;
    let mut refinements = Vec::new();
    refinements
        .try_reserve_exact(primary_identities.len().saturating_sub(1))
        .map_err(|_| invalid("cylinder refinement identity allocation exceeds capacity"))?;
    for pair in primary_identities.windows(2) {
        refinements.push(SpatialRefinementIdentity {
            source_sha256: source.digest_bytes(),
            coarse: pair[0].clone(),
            fine: pair[1].clone(),
        });
    }
    Ok(SpatialFamilyIdentity {
        source_sha256: source.digest_bytes(),
        primary_provider: primary[0].provider.clone(),
        bias_provider: bias.provider.clone(),
        probes: probes.clone(),
        primary: primary_identities,
        refinements,
        bias: spatial_member_identity(bias)?,
    })
}

fn admit_time_family(input: TimeFamilyInput) -> Result<TimeFamilyIdentity, Diagnostic> {
    if !(MINIMUM_FAMILY_MEMBERS..=MAXIMUM_FAMILY_MEMBERS).contains(&input.members.len()) {
        return Err(invalid("cylinder time family must contain 3..=8 members"));
    }
    let method: [u8; 32] = input.members[0]
        .method
        .as_slice()
        .try_into()
        .map_err(|_| invalid("opaque time method identity must contain exactly 32 bytes"))?;
    let mut members = Vec::new();
    members
        .try_reserve_exact(input.members.len())
        .map_err(|_| invalid("time-family identity allocation exceeds capacity"))?;
    for (ordinal, member) in input.members.into_iter().enumerate() {
        let actual_method: [u8; 32] =
            member.method.as_slice().try_into().map_err(|_| {
                invalid("opaque time method identity must contain exactly 32 bytes")
            })?;
        if member.ordinal != ordinal
            || actual_method != method
            || !member.step.is_finite()
            || member.step <= 0.0
            || members.last().is_some_and(|previous: &TimeMemberIdentity| {
                f64::from_bits(previous.step_bits) <= member.step
            })
        {
            return Err(invalid(
                "cylinder time-family identity or ordering is invalid",
            ));
        }
        members.push(TimeMemberIdentity {
            ordinal,
            method,
            step_bits: normalized_bits(member.step),
        });
    }
    Ok(TimeFamilyIdentity { method, members })
}

fn revalidate_time_family(time: &TimeFamilyIdentity) -> Result<(), Diagnostic> {
    if !(MINIMUM_FAMILY_MEMBERS..=MAXIMUM_FAMILY_MEMBERS).contains(&time.members.len())
        || time.members.iter().enumerate().any(|(ordinal, member)| {
            member.ordinal != ordinal
                || member.method != time.method
                || !f64::from_bits(member.step_bits).is_finite()
                || f64::from_bits(member.step_bits) <= 0.0
        })
        || time
            .members
            .windows(2)
            .any(|pair| f64::from_bits(pair[0].step_bits) <= f64::from_bits(pair[1].step_bits))
    {
        return Err(invalid("accepted cylinder time family differs from replay"));
    }
    Ok(())
}

fn admit_cells(
    benchmark: CylinderBenchmark,
    primary: &[AcceptedCylinderMeshMember],
    time: Option<&TimeFamilyIdentity>,
    inputs: Vec<SpaceTimeCellInput>,
) -> Result<Vec<SpaceTimeCellIdentity>, Diagnostic> {
    match (benchmark, time) {
        (CylinderBenchmark::S1, None) if inputs.is_empty() => return Ok(Vec::new()),
        (CylinderBenchmark::S1, _) => {
            return Err(invalid("DFG S1 admits no time family or space-time cells"));
        }
        (CylinderBenchmark::S2, None) if inputs.is_empty() => return Ok(Vec::new()),
        (CylinderBenchmark::S2, None) => {
            return Err(invalid("space-time cells require an accepted time family"));
        }
        (CylinderBenchmark::S2, Some(_)) => {}
    }
    let time = time.expect("matched attached time family");
    let expected_count = primary
        .len()
        .checked_mul(time.members.len())
        .ok_or_else(|| invalid("space-time Cartesian cardinality overflows usize"))?;
    if inputs.len() != expected_count {
        return Err(invalid(
            "space-time association is not the complete Cartesian product",
        ));
    }
    let mut accepted = Vec::new();
    accepted
        .try_reserve_exact(expected_count)
        .map_err(|_| invalid("space-time identity allocation exceeds capacity"))?;
    for (index, input) in inputs.into_iter().enumerate() {
        let spatial_ordinal = index / time.members.len();
        let time_ordinal = index % time.members.len();
        if input.spatial_ordinal != spatial_ordinal || input.time_ordinal != time_ordinal {
            return Err(invalid(
                "space-time association is missing, duplicated, or reordered",
            ));
        }
        accepted.push(SpaceTimeCellIdentity {
            spatial: primary[spatial_ordinal].identity.clone(),
            time: time.members[time_ordinal].clone(),
        });
    }
    Ok(accepted)
}

fn revalidate_cells(
    benchmark: CylinderBenchmark,
    primary: &[AcceptedCylinderMeshMember],
    time: Option<&TimeFamilyIdentity>,
    cells: &[SpaceTimeCellIdentity],
) -> Result<(), Diagnostic> {
    let inputs = cells
        .iter()
        .map(|cell| SpaceTimeCellInput {
            spatial_ordinal: cell.spatial_ordinal(),
            time_ordinal: cell.time_ordinal(),
        })
        .collect();
    let replayed = admit_cells(benchmark, primary, time, inputs)?;
    if replayed != cells {
        return Err(invalid("space-time cell identities differ from replay"));
    }
    Ok(())
}

pub(super) fn max_cylinder_chord(
    accepted: &AcceptedCircularHoleChordalRealizationV1,
) -> Result<f64, Diagnostic> {
    let entities = accepted
        .correspondence()
        .region_entity_set_entities(accepted.realized_geometry(), "cylinder")?;
    if entities.is_empty()
        || entities
            .iter()
            .any(|entity| entity.dimension() != EDGE_DIMENSION)
    {
        return Err(invalid(
            "cylinder correspondence has no complete edge membership",
        ));
    }
    if entities.len() != accepted.circle_segments() {
        return Err(invalid(
            "cylinder correspondence does not contain every realized chord",
        ));
    }
    let radius = accepted
        .source()
        .circular_hole_radius_m()
        .ok_or_else(|| invalid("cylinder source has no exact circular-hole radius"))?;
    let diameter = 2.0 * radius;
    let maximum = diameter * (std::f64::consts::PI / accepted.circle_segments() as f64).sin();
    if !maximum.is_finite() || maximum <= 0.0 {
        return Err(invalid("cylinder chord length must be finite and positive"));
    }
    Ok(maximum)
}

pub(super) fn max_triangle_diameter(mesh: &SimplicialMesh) -> Result<f64, Diagnostic> {
    let mut maximum = 0.0_f64;
    for cell in mesh.cells() {
        let [a, b, c] = cell.as_slice() else {
            return Err(invalid("cylinder Mesh requires affine triangles"));
        };
        for (left, right) in [(*a, *b), (*b, *c), (*c, *a)] {
            let left = &mesh.vertices()[left];
            let right = &mesh.vertices()[right];
            maximum = maximum.max((left[0] - right[0]).hypot(left[1] - right[1]));
        }
    }
    if !maximum.is_finite() || maximum <= 0.0 {
        return Err(invalid("triangle diameter must be finite and positive"));
    }
    Ok(maximum)
}

pub(super) fn canonical_topology(mesh: &SimplicialMesh) -> Result<CanonicalTopology, Diagnostic> {
    let mut coordinate_bits = Vec::new();
    coordinate_bits
        .try_reserve_exact(mesh.vertices().len())
        .map_err(|_| invalid("canonical coordinate allocation exceeds capacity"))?;
    for vertex in mesh.vertices() {
        let [x, y] = vertex.as_slice() else {
            return Err(invalid("cylinder canonical topology requires XY vertices"));
        };
        coordinate_bits.push([normalized_bits(*x), normalized_bits(*y)]);
    }
    coordinate_bits.sort_unstable();
    if coordinate_bits.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid(
            "canonical topology has duplicate normalized coordinates",
        ));
    }
    let ranks = coordinate_bits
        .iter()
        .copied()
        .enumerate()
        .map(|(rank, key)| (key, rank))
        .collect::<BTreeMap<_, _>>();
    let mut triangles = Vec::new();
    triangles
        .try_reserve_exact(mesh.cells().len())
        .map_err(|_| invalid("canonical triangle allocation exceeds capacity"))?;
    for cell in mesh.cells() {
        let [a, b, c] = cell.as_slice() else {
            return Err(invalid("canonical topology requires triangles"));
        };
        let pa = &mesh.vertices()[*a];
        let pb = &mesh.vertices()[*b];
        let pc = &mesh.vertices()[*c];
        let signed = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pb[1] - pa[1]) * (pc[0] - pa[0]);
        if !signed.is_finite() || signed == 0.0 {
            return Err(invalid("canonical topology contains a degenerate triangle"));
        }
        let oriented = if signed > 0.0 {
            [*a, *b, *c]
        } else {
            [*a, *c, *b]
        };
        let mut ranked = [0_usize; 3];
        for (target, vertex) in ranked.iter_mut().zip(oriented) {
            let point = &mesh.vertices()[vertex];
            *target = ranks[&[normalized_bits(point[0]), normalized_bits(point[1])]];
        }
        let rotations = [
            ranked,
            [ranked[1], ranked[2], ranked[0]],
            [ranked[2], ranked[0], ranked[1]],
        ];
        triangles.push(*rotations.iter().min().expect("three rotations"));
    }
    triangles.sort_unstable();
    Ok(CanonicalTopology {
        coordinate_bits,
        triangles,
    })
}

pub(super) fn validate_named_partition(
    accepted: &AcceptedCircularHoleChordalRealizationV1,
) -> Result<(), Diagnostic> {
    let correspondence = accepted.correspondence();
    let geometry = accepted.realized_geometry();
    let mesh = accepted.mesh().mesh();
    let mut covered = BTreeSet::new();
    for name in ["inlet", "outlet", "walls", "cylinder"] {
        let entities = correspondence.region_entity_set_entities(geometry, name)?;
        if entities.is_empty()
            || entities
                .iter()
                .any(|entity| entity.dimension() != EDGE_DIMENSION || !covered.insert(*entity))
        {
            return Err(invalid(
                "cylinder boundary partition is empty or overlapping",
            ));
        }
    }
    let edge_count = mesh
        .entity_count(EDGE_DIMENSION)
        .ok_or_else(|| invalid("cylinder Mesh has no edge stratum"))?;
    let exterior = (0..edge_count)
        .map(|index| MeshEntity::new(EDGE_DIMENSION, index))
        .filter(|entity| mesh.is_boundary_entity(*entity) == Some(true))
        .collect::<BTreeSet<_>>();
    if covered != exterior {
        return Err(invalid(
            "named cylinder boundaries do not cover every exterior facet",
        ));
    }
    let fluid = correspondence.region_entity_set_entities(geometry, "fluid")?;
    let cell_count = mesh
        .entity_count(FACE_DIMENSION)
        .ok_or_else(|| invalid("cylinder Mesh has no face stratum"))?;
    let all_cells = (0..cell_count)
        .map(|index| MeshEntity::new(FACE_DIMENSION, index))
        .collect::<Vec<_>>();
    if fluid != all_cells {
        return Err(invalid("fluid membership is not the complete cell set"));
    }
    Ok(())
}

pub(super) fn squared_distance(vertex: &[f64], point: [f64; 2]) -> f64 {
    (vertex[0] - point[0]).mul_add(
        vertex[0] - point[0],
        (vertex[1] - point[1]) * (vertex[1] - point[1]),
    )
}

fn digest(value: ArtifactDigest) -> [u8; 32] {
    value.sha256_bytes()
}
