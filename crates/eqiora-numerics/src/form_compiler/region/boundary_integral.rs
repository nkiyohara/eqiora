use eqiora_assembly::LocalContribution;
use eqiora_meshing::{
    AffineGeometryMap, EntityIncidence, GeometryMap, QuadratureRule, ReferenceCellFamily,
    ReferenceTopology,
};

use super::*;

impl BoundRegionForm {
    /// Integrate physical parent-outward flux into the complete parent-cell map.
    /// The caller owns the authenticated facet-to-cell incidence and Domain binding.
    pub(crate) fn evaluate_natural_facet(
        &self,
        field: RawId,
        cell: &AffineGeometryMap,
        facet: (&AffineGeometryMap, EntityIncidence),
        rule: &QuadratureRule,
        datum: impl Fn(&[f64], &[f64]) -> Result<Vec<f64>, Diagnostic>,
    ) -> Result<LocalContribution, Diagnostic> {
        let (facet, incidence) = facet;
        let dimension = self.form.dimension;
        if cell.reference_cell() != self.reference
            || cell.physical_dimension() != dimension
            || facet.physical_dimension() != dimension
            || facet.reference_cell().dimension() + 1 != dimension
            || facet.reference_cell() != rule.reference_cell()
        {
            return Err(invalid(
                "natural flux requires a matching parent-cell and facet geometry",
            ));
        }
        let row = self
            .fields
            .iter()
            .position(|layout| layout.field == field)
            .ok_or_else(|| invalid("natural flux has a foreign tested Field"))?;
        let layout = &self.fields[row];
        let space = super::binding::basis(layout.space, self.reference)?;
        let count = self.fields.last().expect("nonempty bound form").range.end;
        let entries = count
            .checked_mul(count)
            .ok_or_else(|| invalid("natural flux local matrix size overflow"))?;
        let mut rhs = vec![0.0; count];
        let inverse = cell.inverse_jacobian()?;
        let normal = parent_outward_normal(cell, incidence)?;
        for point in rule.points() {
            let mut physical = vec![0.0; dimension];
            facet.map_point(&point.coordinates, &mut physical)?;
            let reference = (0..dimension)
                .map(|i| {
                    (0..dimension)
                        .map(|j| inverse[i * dimension + j] * (physical[j] - cell.origin()[j]))
                        .sum()
                })
                .collect::<Vec<f64>>();
            let test = space.tabulate(&reference)?;
            let value = datum(&physical, &normal)?;
            if value.len() != layout.components || value.iter().any(|value| !value.is_finite()) {
                return Err(invalid(
                    "natural flux datum differs from the exact Field component shape",
                ));
            }
            let weight = point.weight * facet.measure_scale() * self.row_multipliers[row];
            for (node, test) in test.values().iter().enumerate() {
                for (component, value) in value.iter().enumerate() {
                    rhs[layout.range.start + node * layout.components + component] +=
                        weight * test * value;
                }
            }
        }
        LocalContribution::new(count, count, vec![0.0; entries], rhs)
    }
}

fn parent_outward_normal(
    cell: &AffineGeometryMap,
    incidence: EntityIncidence,
) -> Result<Vec<f64>, Diagnostic> {
    let dimension = cell.physical_dimension();
    if dimension == 0 || incidence.entity.dimension() != dimension {
        return Err(invalid(
            "natural flux requires an exact positive-dimensional parent incidence",
        ));
    }
    let topology = ReferenceTopology::new(cell.reference_cell())?;
    let facet = topology
        .entity(dimension - 1, incidence.local_ordinal)
        .ok_or_else(|| invalid("natural flux has a foreign parent facet ordinal"))?;
    let vertices = facet.vertex_ordinals();
    let mut reference_normal = vec![0.0; dimension];
    match cell.reference_cell().family() {
        ReferenceCellFamily::Simplex => {
            let missing = (0..=dimension)
                .find(|vertex| !vertices.contains(vertex))
                .ok_or_else(|| invalid("simplex facet has no opposite parent vertex"))?;
            if missing == 0 {
                reference_normal.fill(1.0);
            } else {
                reference_normal[missing - 1] = -1.0;
            }
        }
        ReferenceCellFamily::Hypercube => {
            let axis = (0..dimension)
                .find(|axis| {
                    vertices
                        .iter()
                        .all(|vertex| (vertex >> axis) & 1 == (vertices[0] >> axis) & 1)
                })
                .ok_or_else(|| invalid("hypercube facet has no fixed parent axis"))?;
            reference_normal[axis] = if (vertices[0] >> axis) & 1 == 0 {
                -1.0
            } else {
                1.0
            };
        }
        ReferenceCellFamily::Point => {
            return Err(invalid(
                "natural flux requires a positive-dimensional parent",
            ));
        }
    }
    // Covectors transform by J^{-T}; the exact reference facet owns the sign.
    let inverse = cell.inverse_jacobian()?;
    let mut normal = (0..dimension)
        .map(|physical| {
            (0..dimension)
                .map(|reference| {
                    inverse[reference * dimension + physical] * reference_normal[reference]
                })
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let norm = normal
        .iter()
        .fold(0.0_f64, |norm, value| norm.hypot(*value));
    if !norm.is_finite() || norm <= 0.0 {
        return Err(invalid("natural flux facet has no finite nonzero normal"));
    }
    for component in &mut normal {
        *component /= norm;
    }
    Ok(normal)
}
