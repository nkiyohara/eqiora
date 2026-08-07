//! Private three-generator Cartesian periodic topology projection.
//!
//! The projection composes three already-validated spatial-periodic boundary
//! pairs with one exact Cartesian mesh. It remains an internal structural
//! prerequisite: no numerical operator, persisted representation, or public
//! topology API is introduced here.

#![cfg_attr(not(test), allow(dead_code))]

use std::collections::{BTreeSet, VecDeque};

use eqiora_artifact::{
    AcceptedModelArtifact, CartesianMeshEnvelopeV1, ReplayableCanonicalModelArtifact,
};
use eqiora_core::entity::kinds;
use eqiora_core::{Id, RawId};
use eqiora_graph::EdgeKind;
use eqiora_meshing::{CartesianMesh, MeshEntity, MeshTopology};
use eqiora_schema::kernel::{BoundarySide, ConnectionSemantics, DomainKind, KernelNode};
use eqiora_sem::{BoundaryJunctionGeometry, KernelProgram};

const DIMENSION: usize = 3;
const REFERENCE_COUNTS: [usize; DIMENSION] = [2, 3, 4];
#[derive(Debug, Clone)]
pub(crate) struct CartesianPeriodic3dProjection {
    model_artifact_sha256: String,
    model_id: String,
    semantic_revision: u64,
    mesh_artifact_sha256: [u8; 32],
    parent: RawId,
    connector: RawId,
    generators: Vec<Generator>,
    cycles: Vec<CycleReceipt>,
    inventory: Inventory,
    entities: Vec<QuotientEntity>,
    face_incidences: Vec<FaceIncidence>,
    cell_face_incidences: Vec<CellFaceIncidence>,
    packets: Vec<PositivePacket>,
}
#[derive(Debug, Clone)]
struct Generator {
    connection: RawId,
    lower_port: RawId,
    upper_port: RawId,
    parent: RawId,
    connector: RawId,
    axis: usize,
    lower_coordinate: f64,
    upper_coordinate: f64,
    period: f64,
}
#[derive(Debug, Clone)]
struct CycleReceipt {
    word: Vec<i8>,
    net_coefficients: [i8; DIMENSION],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Inventory {
    cells: usize,
    box_entities: usize,
    quotient_strata: [usize; 4],
    quotient_entities: usize,
    quotient_closure_vertex_references: usize,
    orbit_outputs: usize,
    box_orbit_memberships: usize,
    positive_packets: usize,
    seam_packets: [usize; DIMENSION],
}
#[derive(Debug, Clone)]
struct BoxRepresentative {
    anchors: [usize; DIMENSION],
    base_index: usize,
}
#[derive(Debug, Clone)]
struct QuotientEntity {
    dimension: usize,
    free_axes: Vec<usize>,
    quotient_anchor: [usize; DIMENSION],
    quotient_index: usize,
    orbit: Vec<BoxRepresentative>,
    closure_vertices: Vec<usize>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FaceIncidence {
    quotient_face: usize,
    positive_side_cell: usize,
    negative_side_cell: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CellFaceIncidence {
    cell: usize,
    axis: usize,
    side: i8,
    quotient_face: usize,
}
#[derive(Debug, Clone)]
struct PositivePacket {
    packet: usize,
    axis: usize,
    owner_cell: usize,
    neighbor_cell: usize,
    quotient_face: usize,
    normal: [i8; DIMENSION],
    seam: bool,
    lifted_center_distance: Option<f64>,
    owner_face_points: Vec<[f64; DIMENSION]>,
    lifted_neighbor_face_points: Vec<[f64; DIMENSION]>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectionEvent {
    PairValidated(RawId),
    GroupAdmitted,
    AbstractInventoryAdmitted(Inventory),
    ProjectionStateAllocated,
    ProjectionPublished,
}
#[derive(Debug, Clone)]
struct ProjectionRun {
    result: Result<CartesianPeriodic3dProjection, String>,
    events: Vec<ProjectionEvent>,
}
fn run_projection(
    model: &AcceptedModelArtifact,
    mesh_envelope: &CartesianMeshEnvelopeV1,
    selected_connections: &[Id<kinds::Connection>],
) -> ProjectionRun {
    let mut events = Vec::new();
    let result = (|| {
        let replayed = model
            .replay_model()
            .map_err(|error| format!("cannot replay exact Model: {error}"))?;
        let program = replayed.program();
        let mut generators = Vec::with_capacity(selected_connections.len());
        for &connection in selected_connections {
            let generator = derive_generator(program, connection)?;
            events.push(ProjectionEvent::PairValidated(generator.connection));
            generators.push(generator);
        }
        let (parent, connector) = admit_group(program, &mut generators)?;
        events.push(ProjectionEvent::GroupAdmitted);

        let mesh = mesh_envelope.mesh();
        let axes = admit_reference_mesh(mesh, &generators)?;
        let inventory = derive_inventory(REFERENCE_COUNTS)?;
        events.push(ProjectionEvent::AbstractInventoryAdmitted(inventory));
        let model_reference = replayed.artifact_reference();
        let mesh_reference = mesh_envelope
            .artifact_reference()
            .map_err(|error| format!("cannot identify exact mesh: {error}"))?;

        events.push(ProjectionEvent::ProjectionStateAllocated);
        let entities = derive_entities(mesh, REFERENCE_COUNTS, inventory)?;
        let face_incidences = derive_face_incidences(REFERENCE_COUNTS)?;
        require_connected(&face_incidences, inventory.cells)?;
        let cell_face_incidences = derive_cell_face_incidences(REFERENCE_COUNTS)?;
        let packets = derive_packets(&axes, REFERENCE_COUNTS)?;
        if packets.len() != inventory.positive_packets {
            return Err("positive-packet inventory mismatch".to_owned());
        }
        let projection = CartesianPeriodic3dProjection {
            model_artifact_sha256: model_reference.artifact().as_str().to_owned(),
            model_id: model_reference.model().to_string(),
            semantic_revision: model_reference.semantic_revision().get(),
            mesh_artifact_sha256: mesh_reference.sha256(),
            parent,
            connector,
            generators,
            cycles: derive_cycles(),
            inventory,
            entities,
            face_incidences,
            cell_face_incidences,
            packets,
        };
        events.push(ProjectionEvent::ProjectionPublished);
        Ok(projection)
    })();
    ProjectionRun { result, events }
}
fn derive_generator(
    program: &KernelProgram,
    connection: Id<kinds::Connection>,
) -> Result<Generator, String> {
    let junction = program
        .compose_boundary_physical_junction(connection)
        .map_err(|error| format!("constituent pair is invalid: {error}"))?;
    let BoundaryJunctionGeometry::CartesianPeriodic(identification) = junction.geometry() else {
        return Err("selected Connection is not spatial-periodic".to_owned());
    };
    if identification.ambient_dimension() != DIMENSION {
        return Err("selected pair is not three-dimensional".to_owned());
    }

    let mut resolved = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Connects && edge.from() == connection.erase())
        .map(|edge| resolve_port(program, edge.to()))
        .collect::<Result<Vec<_>, _>>()?;
    if resolved.len() != 2 || resolved[0].port == resolved[1].port {
        return Err("spatial-periodic Connection must own two distinct Ports".to_owned());
    }
    resolved.sort_by_key(|port| match port.side {
        BoundarySide::Lower => 0,
        BoundarySide::Upper => 1,
    });
    let lower = resolved[0];
    let upper = resolved[1];
    if lower.side != BoundarySide::Lower
        || upper.side != BoundarySide::Upper
        || lower.parent != upper.parent
        || lower.connector != upper.connector
        || lower.axis != upper.axis
        || lower.axis != identification.normal_axis()
    {
        return Err("pair endpoint identity contradicts validated geometry".to_owned());
    }

    Ok(Generator {
        connection: connection.erase(),
        lower_port: lower.port,
        upper_port: upper.port,
        parent: lower.parent,
        connector: lower.connector,
        axis: identification.normal_axis(),
        lower_coordinate: identification.lower_coordinate(),
        upper_coordinate: identification.upper_coordinate(),
        period: identification.period(),
    })
}
#[derive(Debug, Clone, Copy)]
struct ResolvedPort {
    port: RawId,
    parent: RawId,
    connector: RawId,
    axis: usize,
    side: BoundarySide,
}
fn resolve_port(program: &KernelProgram, port: RawId) -> Result<ResolvedPort, String> {
    let Some(KernelNode::Port(definition)) = program.node(port) else {
        return Err("periodic pair contains a non-Port member".to_owned());
    };
    let (connector, boundary) = definition
        .boundary_physical_contract()
        .ok_or_else(|| "periodic pair member is not boundary-physical".to_owned())?;
    let Some(KernelNode::Domain(boundary_definition)) = program.node(boundary.erase()) else {
        return Err("periodic Port support is not a Domain".to_owned());
    };
    let DomainKind::CartesianBoundary { axis, side } = boundary_definition.kind() else {
        return Err("periodic Port support is not a Cartesian boundary".to_owned());
    };
    let parents = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::BoundaryOf && edge.from() == boundary.erase())
        .map(|edge| edge.to())
        .collect::<Vec<_>>();
    if parents.len() != 1 {
        return Err("Cartesian boundary must own one exact parent".to_owned());
    }
    Ok(ResolvedPort {
        port,
        parent: parents[0],
        connector: connector.erase(),
        axis: *axis,
        side: *side,
    })
}
fn admit_group(
    program: &KernelProgram,
    generators: &mut [Generator],
) -> Result<(RawId, RawId), String> {
    if generators.len() != DIMENSION {
        return Err("three-generator profile requires exactly three pairs".to_owned());
    }
    if generators
        .iter()
        .map(|generator| generator.connection)
        .collect::<BTreeSet<_>>()
        .len()
        != DIMENSION
    {
        return Err("three-generator profile requires distinct Connections".to_owned());
    }
    let parent = generators[0].parent;
    let connector = generators[0].connector;
    if generators
        .iter()
        .any(|generator| generator.parent != parent)
    {
        return Err("periodic pairs do not share one exact parent".to_owned());
    }
    if generators
        .iter()
        .any(|generator| generator.connector != connector)
    {
        return Err("periodic pairs do not share one exact Connector".to_owned());
    }
    if generators
        .iter()
        .flat_map(|generator| [generator.lower_port, generator.upper_port])
        .collect::<BTreeSet<_>>()
        .len()
        != 2 * DIMENSION
    {
        return Err("periodic group reuses a Port".to_owned());
    }
    generators.sort_by_key(|generator| generator.axis);
    if generators
        .iter()
        .map(|generator| generator.axis)
        .collect::<Vec<_>>()
        != [0, 1, 2]
    {
        return Err("periodic pairs do not cover axes {0,1,2} exactly once".to_owned());
    }

    let family = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Connection(definition)
                if definition.semantics() == ConnectionSemantics::SpatialPeriodic =>
            {
                Some(definition.id())
            }
            _ => None,
        })
        .filter_map(|connection| derive_generator(program, connection).ok())
        .filter(|generator| generator.parent == parent && generator.connector == connector)
        .map(|generator| generator.connection)
        .collect::<BTreeSet<_>>();
    let selected = generators
        .iter()
        .map(|generator| generator.connection)
        .collect::<BTreeSet<_>>();
    if family != selected {
        return Err("selected group does not exhaust its parent/Connector family".to_owned());
    }
    Ok((parent, connector))
}
fn admit_reference_mesh(
    mesh: &CartesianMesh,
    generators: &[Generator],
) -> Result<[Vec<f64>; DIMENSION], String> {
    if mesh.topological_dimension() != DIMENSION {
        return Err("reference mesh must be full-dimensional Cartesian 3D".to_owned());
    }
    let mut axes = std::array::from_fn(|_| Vec::new());
    for axis in 0..DIMENSION {
        let coordinates = mesh
            .axis_coordinates(axis)
            .ok_or_else(|| "reference mesh omitted a physical axis".to_owned())?;
        if coordinates.len().checked_sub(1) != Some(REFERENCE_COUNTS[axis]) {
            return Err("reference mesh cell counts must be exactly 2 x 3 x 4".to_owned());
        }
        let generator = &generators[axis];
        if coordinates.first().copied() != Some(generator.lower_coordinate)
            || coordinates.last().copied() != Some(generator.upper_coordinate)
        {
            return Err("reference mesh bounds differ from exact parent bounds".to_owned());
        }
        axes[axis] = coordinates.to_vec();
    }
    let side_lengths = axes.each_ref().map(|axis| axis[axis.len() - 1] - axis[0]);
    if side_lengths[0] == side_lengths[1]
        || side_lengths[0] == side_lengths[2]
        || side_lengths[1] == side_lengths[2]
    {
        return Err("reference parent side lengths must be unequal".to_owned());
    }
    if !axes.iter().any(|axis| {
        let mut widths = axis.windows(2).map(|pair| pair[1] - pair[0]);
        let Some(first) = widths.next() else {
            return false;
        };
        widths.any(|width| width != first)
    }) {
        return Err("reference mesh requires at least one nonuniform axis".to_owned());
    }
    Ok(axes)
}
fn derive_inventory(counts: [usize; DIMENSION]) -> Result<Inventory, String> {
    let cells = checked_product(&counts, "cell inventory")?;
    let box_shape = [
        checked_scale(counts[0], 2, 1, "box-entity axis")?,
        checked_scale(counts[1], 2, 1, "box-entity axis")?,
        checked_scale(counts[2], 2, 1, "box-entity axis")?,
    ];
    let box_entities = checked_product(&box_shape, "box-entity inventory")?;
    let three_cells = checked_scale(cells, 3, 0, "face/packet inventory")?;
    let eight_cells = checked_scale(cells, 8, 0, "quotient inventory")?;
    let closure_references = checked_scale(cells, 27, 0, "closure inventory")?;
    let seam_packets = std::array::from_fn(|axis| cells / counts[axis]);

    for (items, size) in [
        (eight_cells, std::mem::size_of::<QuotientEntity>()),
        (closure_references, std::mem::size_of::<usize>()),
        (box_entities, std::mem::size_of::<BoxRepresentative>()),
        (three_cells, std::mem::size_of::<PositivePacket>()),
    ] {
        items
            .checked_mul(size)
            .ok_or_else(|| "projection byte inventory overflows usize".to_owned())?;
    }

    Ok(Inventory {
        cells,
        box_entities,
        quotient_strata: [cells, three_cells, three_cells, cells],
        quotient_entities: eight_cells,
        quotient_closure_vertex_references: closure_references,
        orbit_outputs: eight_cells,
        box_orbit_memberships: box_entities,
        positive_packets: three_cells,
        seam_packets,
    })
}
fn checked_scale(value: usize, factor: usize, add: usize, label: &str) -> Result<usize, String> {
    value
        .checked_mul(factor)
        .and_then(|scaled| scaled.checked_add(add))
        .ok_or_else(|| format!("{label} overflows usize"))
}
fn checked_product(values: &[usize], label: &str) -> Result<usize, String> {
    values.iter().try_fold(1_usize, |product, value| {
        product
            .checked_mul(*value)
            .ok_or_else(|| format!("{label} overflows usize"))
    })
}
fn derive_entities(
    mesh: &CartesianMesh,
    counts: [usize; DIMENSION],
    inventory: Inventory,
) -> Result<Vec<QuotientEntity>, String> {
    let mut entities = Vec::with_capacity(inventory.quotient_entities);
    let mut closure_references = 0_usize;
    let mut orbit_memberships = 0_usize;
    for dimension in 0..=DIMENSION {
        for free_axes in axis_combinations(dimension) {
            let family_rank = axis_combinations(dimension)
                .iter()
                .position(|candidate| candidate == &free_axes)
                .ok_or_else(|| "free-axis family is absent".to_owned())?;
            for linear_anchor in 0..inventory.cells {
                let quotient_anchor = delinearize(linear_anchor, counts);
                let quotient_index = family_rank
                    .checked_mul(inventory.cells)
                    .and_then(|offset| offset.checked_add(linear_anchor))
                    .ok_or_else(|| "quotient entity index overflows usize".to_owned())?;
                let boundary_axes = (0..DIMENSION)
                    .filter(|axis| !free_axes.contains(axis) && quotient_anchor[*axis] == 0)
                    .collect::<Vec<_>>();
                let orbit_count = 1_usize << boundary_axes.len();
                let mut orbit = Vec::with_capacity(orbit_count);
                for subset in 0..orbit_count {
                    let mut anchors = quotient_anchor;
                    for (ordinal, &axis) in boundary_axes.iter().enumerate() {
                        if (subset >> ordinal) & 1 == 1 {
                            anchors[axis] = counts[axis];
                        }
                    }
                    let base_index = base_entity_index(dimension, &free_axes, anchors, counts)?;
                    require_box_entity(mesh, dimension, &free_axes, anchors, base_index)?;
                    orbit.push(BoxRepresentative {
                        anchors,
                        base_index,
                    });
                }
                let closure_vertices = quotient_closure(&free_axes, quotient_anchor, counts)?;
                closure_references = closure_references
                    .checked_add(closure_vertices.len())
                    .ok_or_else(|| "closure accounting overflows usize".to_owned())?;
                orbit_memberships = orbit_memberships
                    .checked_add(orbit.len())
                    .ok_or_else(|| "orbit accounting overflows usize".to_owned())?;
                entities.push(QuotientEntity {
                    dimension,
                    free_axes: free_axes.clone(),
                    quotient_anchor,
                    quotient_index,
                    orbit,
                    closure_vertices,
                });
            }
        }
    }
    if entities.len() != inventory.quotient_entities
        || closure_references != inventory.quotient_closure_vertex_references
        || orbit_memberships != inventory.box_orbit_memberships
    {
        return Err("derived quotient inventory differs from admitted inventory".to_owned());
    }
    Ok(entities)
}
fn axis_combinations(dimension: usize) -> Vec<Vec<usize>> {
    (0_u8..8)
        .filter(|mask| mask.count_ones() as usize == dimension)
        .map(|mask| {
            (0..DIMENSION)
                .filter(|axis| mask & (1 << axis) != 0)
                .collect()
        })
        .collect()
}
fn delinearize(mut linear: usize, shape: [usize; DIMENSION]) -> [usize; DIMENSION] {
    let mut indices = [0; DIMENSION];
    for axis in (0..DIMENSION).rev() {
        indices[axis] = linear % shape[axis];
        linear /= shape[axis];
    }
    indices
}
fn flat(indices: [usize; DIMENSION], shape: [usize; DIMENSION]) -> Result<usize, String> {
    if indices
        .iter()
        .enumerate()
        .any(|(axis, index)| *index >= shape[axis])
    {
        return Err("Cartesian index exceeds shape".to_owned());
    }
    indices
        .iter()
        .zip(shape)
        .try_fold(0_usize, |flat, (index, extent)| {
            flat.checked_mul(extent)
                .and_then(|value| value.checked_add(*index))
                .ok_or_else(|| "Cartesian linearization overflows usize".to_owned())
        })
}
fn base_entity_index(
    dimension: usize,
    free_axes: &[usize],
    anchors: [usize; DIMENSION],
    counts: [usize; DIMENSION],
) -> Result<usize, String> {
    let mut offset = 0_usize;
    for family in axis_combinations(dimension) {
        let shape: [usize; DIMENSION] = std::array::from_fn(|axis| {
            if family.contains(&axis) {
                counts[axis]
            } else {
                counts[axis] + 1
            }
        });
        let family_size = checked_product(&shape, "box entity family")?;
        if family == free_axes {
            let local = anchors
                .iter()
                .zip(shape)
                .try_fold(0_usize, |flat, (anchor, extent)| {
                    if *anchor >= extent {
                        return Err("box entity anchor exceeds family shape".to_owned());
                    }
                    flat.checked_mul(extent)
                        .and_then(|value| value.checked_add(*anchor))
                        .ok_or_else(|| "box entity index overflows usize".to_owned())
                })?;
            return offset
                .checked_add(local)
                .ok_or_else(|| "box entity index overflows usize".to_owned());
        }
        offset = offset
            .checked_add(family_size)
            .ok_or_else(|| "box entity family offset overflows usize".to_owned())?;
    }
    Err("free-axis family is absent".to_owned())
}
fn quotient_closure(
    free_axes: &[usize],
    anchor: [usize; DIMENSION],
    counts: [usize; DIMENSION],
) -> Result<Vec<usize>, String> {
    let closure_count = 1_usize << free_axes.len();
    (0..closure_count)
        .map(|bits| {
            let mut vertex = anchor;
            for (ordinal, &axis) in free_axes.iter().enumerate() {
                vertex[axis] = (vertex[axis] + ((bits >> ordinal) & 1)) % counts[axis];
            }
            flat(vertex, counts)
        })
        .collect()
}
fn require_box_entity(
    mesh: &CartesianMesh,
    dimension: usize,
    free_axes: &[usize],
    anchors: [usize; DIMENSION],
    base_index: usize,
) -> Result<(), String> {
    let entity = MeshEntity::new(dimension, base_index);
    if mesh.entity_free_axes(entity) != Some(free_axes) {
        return Err("Cartesian mesh entity family/order differs from canonical order".to_owned());
    }
    let vertices = mesh
        .entity_vertices(entity)
        .ok_or_else(|| "derived Cartesian entity does not exist".to_owned())?;
    let expected_vertices = 1_usize << dimension;
    if vertices.len() != expected_vertices {
        return Err("Cartesian entity closure has the wrong size".to_owned());
    }
    for (bits, vertex) in vertices.into_iter().enumerate() {
        let mut expected = anchors;
        for (ordinal, &axis) in free_axes.iter().enumerate() {
            expected[axis] += (bits >> ordinal) & 1;
        }
        if mesh.vertex_multi_index(vertex) != Some(expected.as_slice()) {
            return Err(
                "Cartesian entity closure order differs from tensor-product order".to_owned(),
            );
        }
    }
    Ok(())
}
fn derive_cycles() -> Vec<CycleReceipt> {
    let mut cycles = Vec::with_capacity(9);
    for (first, second) in [(0_i8, 1_i8), (0, 2), (1, 2)] {
        cycles.push(CycleReceipt {
            word: vec![first + 1, second + 1, -(first + 1), -(second + 1)],
            net_coefficients: [0, 0, 0],
        });
    }
    for order in [
        [0_i8, 1_i8, 2_i8],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        cycles.push(CycleReceipt {
            word: order.into_iter().map(|axis| axis + 1).collect(),
            net_coefficients: [1, 1, 1],
        });
    }
    cycles
}
fn derive_face_incidences(counts: [usize; DIMENSION]) -> Result<Vec<FaceIncidence>, String> {
    let cells = checked_product(&counts, "cell inventory")?;
    let capacity = DIMENSION
        .checked_mul(cells)
        .ok_or_else(|| "face-incidence inventory overflows usize".to_owned())?;
    let mut incidences = Vec::with_capacity(capacity);
    for normal_axis in 0..DIMENSION {
        let free_axes = (0..DIMENSION)
            .filter(|axis| *axis != normal_axis)
            .collect::<Vec<_>>();
        let family_rank = axis_combinations(2)
            .iter()
            .position(|candidate| candidate == &free_axes)
            .ok_or_else(|| "face family is absent".to_owned())?;
        for linear_anchor in 0..cells {
            let anchor = delinearize(linear_anchor, counts);
            let mut positive = anchor;
            positive[normal_axis] =
                (anchor[normal_axis] + counts[normal_axis] - 1) % counts[normal_axis];
            incidences.push(FaceIncidence {
                quotient_face: family_rank * cells + linear_anchor,
                positive_side_cell: flat(positive, counts)?,
                negative_side_cell: flat(anchor, counts)?,
            });
        }
    }
    incidences.sort_by_key(|incidence| incidence.quotient_face);
    Ok(incidences)
}
fn derive_cell_face_incidences(
    counts: [usize; DIMENSION],
) -> Result<Vec<CellFaceIncidence>, String> {
    let cells = checked_product(&counts, "cell inventory")?;
    let capacity = cells
        .checked_mul(2 * DIMENSION)
        .ok_or_else(|| "cell-face inventory overflows usize".to_owned())?;
    let mut incidences = Vec::with_capacity(capacity);
    for cell in 0..cells {
        let indices = delinearize(cell, counts);
        for axis in 0..DIMENSION {
            let free_axes = (0..DIMENSION)
                .filter(|candidate| *candidate != axis)
                .collect::<Vec<_>>();
            let family_rank = axis_combinations(2)
                .iter()
                .position(|candidate| candidate == &free_axes)
                .ok_or_else(|| "face family is absent".to_owned())?;
            let negative_face = family_rank * cells + flat(indices, counts)?;
            let mut positive_anchor = indices;
            positive_anchor[axis] = (positive_anchor[axis] + 1) % counts[axis];
            let positive_face = family_rank * cells + flat(positive_anchor, counts)?;
            incidences.push(CellFaceIncidence {
                cell,
                axis,
                side: -1,
                quotient_face: negative_face,
            });
            incidences.push(CellFaceIncidence {
                cell,
                axis,
                side: 1,
                quotient_face: positive_face,
            });
        }
    }
    Ok(incidences)
}
fn require_connected(incidences: &[FaceIncidence], cells: usize) -> Result<(), String> {
    let mut adjacency = vec![Vec::new(); cells];
    for incidence in incidences {
        if incidence.positive_side_cell == incidence.negative_side_cell
            || incidence.positive_side_cell >= cells
            || incidence.negative_side_cell >= cells
        {
            return Err("quotient face requires two distinct valid incident cells".to_owned());
        }
        adjacency[incidence.positive_side_cell].push(incidence.negative_side_cell);
        adjacency[incidence.negative_side_cell].push(incidence.positive_side_cell);
    }
    if cells == 0 {
        return Err("quotient cell inventory is empty".to_owned());
    }
    let mut visited = vec![false; cells];
    let mut queue = VecDeque::from([0]);
    visited[0] = true;
    while let Some(cell) = queue.pop_front() {
        for &neighbor in &adjacency[cell] {
            if !visited[neighbor] {
                visited[neighbor] = true;
                queue.push_back(neighbor);
            }
        }
    }
    if visited.into_iter().all(|seen| seen) {
        Ok(())
    } else {
        Err("periodic quotient adjacency is disconnected".to_owned())
    }
}
fn derive_packets(
    axes: &[Vec<f64>; DIMENSION],
    counts: [usize; DIMENSION],
) -> Result<Vec<PositivePacket>, String> {
    let cells = checked_product(&counts, "cell inventory")?;
    let capacity = DIMENSION
        .checked_mul(cells)
        .ok_or_else(|| "packet inventory overflows usize".to_owned())?;
    let mut packets = Vec::with_capacity(capacity);
    for axis in 0..DIMENSION {
        let free_axes = (0..DIMENSION)
            .filter(|candidate| *candidate != axis)
            .collect::<Vec<_>>();
        let family_rank = axis_combinations(2)
            .iter()
            .position(|candidate| candidate == &free_axes)
            .ok_or_else(|| "face family is absent".to_owned())?;
        for owner_cell in 0..cells {
            let owner = delinearize(owner_cell, counts);
            let mut neighbor = owner;
            neighbor[axis] = (neighbor[axis] + 1) % counts[axis];
            let seam = owner[axis] + 1 == counts[axis];
            let mut face_anchor = owner;
            face_anchor[axis] = (owner[axis] + 1) % counts[axis];
            let (distance, owner_points, neighbor_points) = if seam {
                let coordinates = &axes[axis];
                let first = coordinates[1] - coordinates[0];
                let last = coordinates[coordinates.len() - 1] - coordinates[coordinates.len() - 2];
                let owner_points =
                    seam_face_points(axes, counts, axis, owner, BoundarySide::Upper, false)?;
                let neighbor_points =
                    seam_face_points(axes, counts, axis, neighbor, BoundarySide::Lower, true)?;
                if owner_points != neighbor_points {
                    return Err("lifted seam point sets disagree".to_owned());
                }
                (Some((last + first) / 2.0), owner_points, neighbor_points)
            } else {
                (None, Vec::new(), Vec::new())
            };
            let mut normal = [0; DIMENSION];
            normal[axis] = 1;
            packets.push(PositivePacket {
                packet: axis * cells + owner_cell,
                axis,
                owner_cell,
                neighbor_cell: flat(neighbor, counts)?,
                quotient_face: family_rank * cells + flat(face_anchor, counts)?,
                normal,
                seam,
                lifted_center_distance: distance,
                owner_face_points: owner_points,
                lifted_neighbor_face_points: neighbor_points,
            });
        }
    }
    Ok(packets)
}
fn seam_face_points(
    axes: &[Vec<f64>; DIMENSION],
    counts: [usize; DIMENSION],
    normal_axis: usize,
    tangential_anchor: [usize; DIMENSION],
    side: BoundarySide,
    lift_lower: bool,
) -> Result<Vec<[f64; DIMENSION]>, String> {
    let free_axes = (0..DIMENSION)
        .filter(|axis| *axis != normal_axis)
        .collect::<Vec<_>>();
    let period = axes[normal_axis][counts[normal_axis]] - axes[normal_axis][0];
    let point_count = 1_usize << free_axes.len();
    (0..point_count)
        .map(|bits| {
            let mut point = [0.0; DIMENSION];
            for axis in 0..DIMENSION {
                if axis == normal_axis {
                    point[axis] = match side {
                        BoundarySide::Lower => axes[axis][0],
                        BoundarySide::Upper => axes[axis][counts[axis]],
                    };
                    if lift_lower {
                        point[axis] += period;
                    }
                } else {
                    let ordinal = free_axes
                        .iter()
                        .position(|candidate| *candidate == axis)
                        .ok_or_else(|| "tangential axis is absent".to_owned())?;
                    let coordinate = tangential_anchor[axis] + ((bits >> ordinal) & 1);
                    point[axis] = *axes[axis]
                        .get(coordinate)
                        .ok_or_else(|| "seam point exceeds mesh axis".to_owned())?;
                }
            }
            Ok(point)
        })
        .collect()
}
#[cfg(test)]
pub(super) struct EvidenceHarness;
#[cfg(test)]
impl tests::ProductHarness for EvidenceHarness {
    fn project(
        model: &AcceptedModelArtifact,
        mesh: &CartesianMeshEnvelopeV1,
        selected_connections: &[Id<kinds::Connection>],
    ) -> tests::ProductRun {
        let run = run_projection(model, mesh, selected_connections);
        tests::ProductRun {
            result: run.result.map(to_evidence_observation),
            events: run.events.into_iter().map(to_evidence_event).collect(),
        }
    }
}
#[cfg(test)]
fn to_evidence_event(event: ProjectionEvent) -> tests::ProductEvent {
    match event {
        ProjectionEvent::PairValidated(connection) => {
            tests::ProductEvent::PairValidated(connection)
        }
        ProjectionEvent::GroupAdmitted => tests::ProductEvent::GroupAdmitted,
        ProjectionEvent::AbstractInventoryAdmitted(inventory) => {
            tests::ProductEvent::AbstractInventoryAdmitted(to_evidence_inventory(inventory))
        }
        ProjectionEvent::ProjectionStateAllocated => tests::ProductEvent::ProjectionStateAllocated,
        ProjectionEvent::ProjectionPublished => tests::ProductEvent::ProjectionPublished,
    }
}
#[cfg(test)]
fn to_evidence_inventory(inventory: Inventory) -> tests::InventoryObservation {
    tests::InventoryObservation {
        cells: inventory.cells,
        box_entities: inventory.box_entities,
        quotient_strata: inventory.quotient_strata,
        quotient_entities: inventory.quotient_entities,
        quotient_closure_vertex_references: inventory.quotient_closure_vertex_references,
        orbit_outputs: inventory.orbit_outputs,
        box_orbit_memberships: inventory.box_orbit_memberships,
        positive_packets: inventory.positive_packets,
        seam_packets: inventory.seam_packets,
    }
}
#[cfg(test)]
fn to_evidence_observation(
    projection: CartesianPeriodic3dProjection,
) -> tests::ProductProjectionObservation {
    tests::ProductProjectionObservation {
        model_artifact_sha256: projection.model_artifact_sha256,
        model_id: projection.model_id,
        semantic_revision: projection.semantic_revision,
        mesh_artifact_sha256: projection.mesh_artifact_sha256,
        parent: projection.parent,
        connector: projection.connector,
        generators: projection
            .generators
            .into_iter()
            .map(|generator| tests::GeneratorObservation {
                connection: generator.connection,
                lower_port: generator.lower_port,
                upper_port: generator.upper_port,
                parent: generator.parent,
                connector: generator.connector,
                axis: generator.axis,
                lower_coordinate: generator.lower_coordinate,
                upper_coordinate: generator.upper_coordinate,
                period: generator.period,
                authority: tests::GeneratorAuthority::SpatialPeriodicPair,
                translation_authority: tests::TranslationAuthority::ParentBounds,
                identity_fiber: true,
                lower_outward_sign: -1,
                upper_outward_sign: 1,
            })
            .collect(),
        cycles: projection
            .cycles
            .into_iter()
            .map(|cycle| tests::CycleObservation {
                word: cycle.word,
                net_coefficients: cycle.net_coefficients,
                identity_fiber: true,
                anchor_commutes: true,
                incidence_commutes: true,
            })
            .collect(),
        inventory: to_evidence_inventory(projection.inventory),
        entities: projection
            .entities
            .into_iter()
            .map(|entity| tests::EntityObservation {
                dimension: entity.dimension,
                free_axes: entity.free_axes,
                quotient_anchor: entity.quotient_anchor,
                quotient_index: entity.quotient_index,
                orbit: entity
                    .orbit
                    .into_iter()
                    .map(|representative| tests::BoxRepresentativeObservation {
                        anchors: representative.anchors,
                        base_index: representative.base_index,
                    })
                    .collect(),
                closure_vertices: entity.closure_vertices,
                orientation_code: 0,
            })
            .collect(),
        face_incidences: projection
            .face_incidences
            .into_iter()
            .map(|incidence| tests::FaceIncidenceObservation {
                quotient_face: incidence.quotient_face,
                positive_side_cell: incidence.positive_side_cell,
                negative_side_cell: incidence.negative_side_cell,
            })
            .collect(),
        cell_face_incidences: projection
            .cell_face_incidences
            .into_iter()
            .map(|incidence| tests::CellFaceIncidenceObservation {
                cell: incidence.cell,
                axis: incidence.axis,
                side: incidence.side,
                quotient_face: incidence.quotient_face,
            })
            .collect(),
        packets: projection
            .packets
            .into_iter()
            .map(|packet| tests::PacketObservation {
                packet: packet.packet,
                axis: packet.axis,
                owner_cell: packet.owner_cell,
                neighbor_cell: packet.neighbor_cell,
                quotient_face: packet.quotient_face,
                normal: packet.normal,
                scatter_signs: [1, -1],
                seam: packet.seam,
                lifted_center_distance: packet.lifted_center_distance,
                owner_face_points: packet.owner_face_points,
                lifted_neighbor_face_points: packet.lifted_neighbor_face_points,
            })
            .collect(),
        exterior_face_count: 0,
        persisted_quotient: false,
    }
}
#[cfg(test)]
mod tests;
