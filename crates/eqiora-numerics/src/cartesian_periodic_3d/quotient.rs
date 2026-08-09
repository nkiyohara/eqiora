use std::collections::VecDeque;

use eqiora_meshing::{CartesianMesh, MeshEntity};

use super::DIMENSION;

#[derive(Debug, Clone)]
pub(super) struct CycleReceipt {
    pub(super) word: Vec<i8>,
    pub(super) net_coefficients: [i8; DIMENSION],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Inventory {
    pub(super) cells: usize,
    pub(super) box_entities: usize,
    pub(super) quotient_strata: [usize; 4],
    pub(super) quotient_entities: usize,
    pub(super) quotient_closure_vertex_references: usize,
    pub(super) orbit_outputs: usize,
    pub(super) box_orbit_memberships: usize,
    pub(super) positive_packets: usize,
    pub(super) seam_packets: [usize; DIMENSION],
}

#[derive(Debug, Clone)]
pub(super) struct BoxRepresentative {
    pub(super) anchors: [usize; DIMENSION],
    pub(super) base_index: usize,
}

#[derive(Debug, Clone)]
pub(super) struct QuotientEntity {
    pub(super) dimension: usize,
    pub(super) free_axes: Vec<usize>,
    pub(super) quotient_anchor: [usize; DIMENSION],
    pub(super) quotient_index: usize,
    pub(super) orbit: Vec<BoxRepresentative>,
    pub(super) closure_vertices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct FaceIncidence {
    pub(super) quotient_face: usize,
    pub(super) positive_side_cell: usize,
    pub(super) negative_side_cell: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CellFaceIncidence {
    pub(super) cell: usize,
    pub(super) axis: usize,
    pub(super) side: i8,
    pub(super) quotient_face: usize,
}

pub(super) fn derive_inventory(counts: [usize; DIMENSION]) -> Result<Inventory, String> {
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
        (
            three_cells,
            std::mem::size_of::<super::packet::PositivePacket>(),
        ),
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

pub(super) fn checked_product(values: &[usize], label: &str) -> Result<usize, String> {
    values.iter().try_fold(1_usize, |product, value| {
        product
            .checked_mul(*value)
            .ok_or_else(|| format!("{label} overflows usize"))
    })
}

pub(super) fn derive_entities(
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

pub(super) fn axis_combinations(dimension: usize) -> Vec<Vec<usize>> {
    (0_u8..8)
        .filter(|mask| mask.count_ones() as usize == dimension)
        .map(|mask| {
            (0..DIMENSION)
                .filter(|axis| mask & (1 << axis) != 0)
                .collect()
        })
        .collect()
}

pub(super) fn delinearize(mut linear: usize, shape: [usize; DIMENSION]) -> [usize; DIMENSION] {
    let mut indices = [0; DIMENSION];
    for axis in (0..DIMENSION).rev() {
        indices[axis] = linear % shape[axis];
        linear /= shape[axis];
    }
    indices
}

pub(super) fn flat(
    indices: [usize; DIMENSION],
    shape: [usize; DIMENSION],
) -> Result<usize, String> {
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

pub(super) fn derive_cycles() -> Vec<CycleReceipt> {
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

pub(super) fn derive_face_incidences(
    counts: [usize; DIMENSION],
) -> Result<Vec<FaceIncidence>, String> {
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

pub(super) fn derive_cell_face_incidences(
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

pub(super) fn require_connected(incidences: &[FaceIncidence], cells: usize) -> Result<(), String> {
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
