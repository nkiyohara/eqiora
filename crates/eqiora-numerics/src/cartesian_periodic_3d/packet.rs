use eqiora_schema::kernel::BoundarySide;

use super::DIMENSION;
use super::quotient::{axis_combinations, checked_product, delinearize, flat};

#[derive(Debug, Clone)]
pub(super) struct PositivePacket {
    pub(super) packet: usize,
    pub(super) axis: usize,
    pub(super) owner_cell: usize,
    pub(super) neighbor_cell: usize,
    pub(super) quotient_face: usize,
    pub(super) normal: [i8; DIMENSION],
    pub(super) seam: bool,
    pub(super) lifted_center_distance: Option<f64>,
    pub(super) owner_face_points: Vec<[f64; DIMENSION]>,
    pub(super) lifted_neighbor_face_points: Vec<[f64; DIMENSION]>,
}

pub(super) fn derive_packets(
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
