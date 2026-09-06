//! Deterministic quotient layout and reduced/full assembly maps.

use eqiora_assembly::{AssemblyMap, DofId};
use eqiora_core::Diagnostic;
use eqiora_meshing::{MeshEntity, MeshTopology, SimplicialMesh, VertexId};

use super::contract::FixedReferenceFsiBoundary;
use super::invalid;
use super::partition::FixedReferenceFsiPartition;
use super::{fluid_local_size, solid_local_size};
use crate::constrained_dofs::ConstrainedDofLayout;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FsiLayout<const D: usize = 2> {
    constraints: ConstrainedDofLayout,
    reduced_vertex_velocity: Vec<[Option<DofId>; D]>,
    reduced_bubble_offset: usize,
    reduced_pressure_offset: usize,
    reduced_size: usize,
    full_bubble_offset: usize,
    full_pressure_offset: usize,
    full_size: usize,
    pressure_vertices: Vec<VertexId>,
    pressure_position: Vec<Option<usize>>,
    fixed_velocity: Vec<[Option<f64>; D]>,
}

type ReconstructedFsiFields<const D: usize> = (Vec<[f64; D]>, Vec<[f64; D]>, Vec<f64>);

impl<const D: usize> FsiLayout<D> {
    pub(crate) fn new(
        mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition<D>,
        boundary: &FixedReferenceFsiBoundary<D>,
    ) -> Result<Self, Diagnostic> {
        if let Some(prescribed) = boundary.prepared_current_quotient() {
            return Self::with_prescribed_velocity(mesh, partition, prescribed);
        }
        let mut prescribed = vec![[None; D]; mesh.vertices().len()];
        for vertex in boundary.fixed_zero_velocity_vertices() {
            let values = prescribed.get_mut(vertex.index()).ok_or_else(|| {
                invalid("fixed-reference FSI boundary vertex is outside the mesh revision")
            })?;
            if values.iter().any(Option::is_some) {
                return Err(invalid(
                    "fixed-reference FSI boundary inventory contains a duplicate vertex",
                ));
            }
            *values = [Some(0.0); D];
        }
        Self::with_prescribed_velocity(mesh, partition, &prescribed)
    }

    fn with_prescribed_velocity(
        mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition<D>,
        prescribed: &[[Option<f64>; D]],
    ) -> Result<Self, Diagnostic> {
        if !matches!(D, 2 | 3) || mesh.topological_dimension() != D {
            return Err(invalid(
                "fixed-reference FSI layout requires dimension two or three matching its mesh",
            ));
        }
        let vertex_count = mesh.vertices().len();
        if prescribed.len() != vertex_count
            || prescribed
                .iter()
                .flatten()
                .flatten()
                .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "fixed-reference FSI prescribed velocity must be finite and match the mesh vertex inventory",
            ));
        }
        let pressure_vertices = partition.fluid_vertices().to_vec();
        let mut pressure_position = vec![None; vertex_count];
        for (position, vertex) in pressure_vertices.iter().enumerate() {
            pressure_position[vertex.index()] = Some(position);
        }
        let full_bubble_offset = checked_mul(vertex_count, D, "full velocity width")?;
        let full_pressure_offset = checked_add(
            full_bubble_offset,
            checked_mul(partition.fluid_cells().len(), D, "full bubble width")?,
            "full pressure offset",
        )?;
        let full_size = checked_add(
            full_pressure_offset,
            pressure_vertices.len(),
            "full FSI width",
        )?;
        let mut fixed = prescribed.iter().flatten().copied().collect::<Vec<_>>();
        fixed.resize(full_size, None);
        let constraints = ConstrainedDofLayout::new(fixed)?;
        let mut reduced_vertex_velocity = vec![[None; D]; vertex_count];
        let free = constraints.free_globals();
        let reduced_bubble_offset = free.partition_point(|&global| global < full_bubble_offset);
        let reduced_pressure_offset = free.partition_point(|&global| global < full_pressure_offset);
        for (index, &global) in free[..reduced_bubble_offset].iter().enumerate() {
            reduced_vertex_velocity[global / D][global % D] = Some(DofId::new(index));
        }
        if constraints.free_count() == 0 || full_size == 0 {
            return Err(invalid("fixed-reference FSI layout may not be empty"));
        }
        Ok(Self {
            reduced_size: constraints.free_count(),
            constraints,
            reduced_vertex_velocity,
            reduced_bubble_offset,
            reduced_pressure_offset,
            full_bubble_offset,
            full_pressure_offset,
            full_size,
            pressure_vertices,
            pressure_position,
            fixed_velocity: prescribed.to_vec(),
        })
    }

    pub(crate) fn fluid_map(
        &self,
        fluid_position: usize,
        vertices: &[MeshEntity],
        reduced: bool,
    ) -> Result<AssemblyMap, Diagnostic> {
        if fluid_position >= (self.full_pressure_offset - self.full_bubble_offset) / D {
            return Err(invalid("cell bubble is outside the resolved layout"));
        }
        let mut globals = Vec::with_capacity(fluid_local_size::<D>());
        self.append_vertex_velocity(vertices, &mut globals)?;
        globals.extend(
            (0..D).map(|component| self.full_bubble_offset + fluid_position * D + component),
        );
        for vertex in vertices {
            let position = self
                .pressure_position
                .get(vertex.index())
                .copied()
                .flatten()
                .ok_or_else(|| invalid("cell vertex has no pressure DOF in the resolved layout"))?;
            globals.push(self.full_pressure_offset + position);
        }
        self.map(&globals, reduced)
    }

    pub(crate) fn solid_map(
        &self,
        vertices: &[MeshEntity],
        reduced: bool,
    ) -> Result<AssemblyMap, Diagnostic> {
        let mut globals = Vec::with_capacity(solid_local_size::<D>());
        self.append_vertex_velocity(vertices, &mut globals)?;
        self.map(&globals, reduced)
    }

    fn map(&self, globals: &[usize], reduced: bool) -> Result<AssemblyMap, Diagnostic> {
        if reduced {
            self.constraints.reduced_map(globals)
        } else {
            self.constraints.full_map(globals)
        }
    }

    fn append_vertex_velocity(
        &self,
        vertices: &[MeshEntity],
        globals: &mut Vec<usize>,
    ) -> Result<(), Diagnostic> {
        if vertices
            .iter()
            .any(|vertex| vertex.index() >= self.fixed_velocity.len())
        {
            return Err(invalid(
                "cell vertex is outside the resolved velocity layout",
            ));
        }
        globals.extend(vertices.iter().flat_map(|vertex| {
            (0..D).map(move |component| self.full_vertex_velocity(vertex.index(), component))
        }));
        Ok(())
    }

    pub(crate) const fn full_vertex_velocity(&self, vertex: usize, component: usize) -> usize {
        vertex * D + component
    }

    pub(crate) const fn reduced_size(&self) -> usize {
        self.reduced_size
    }

    pub(crate) fn reduced_vertex_velocity(&self, vertex: usize, component: usize) -> Option<DofId> {
        self.reduced_vertex_velocity
            .get(vertex)
            .and_then(|components| components.get(component))
            .copied()
            .flatten()
    }

    pub(crate) const fn full_size(&self) -> usize {
        self.full_size
    }

    pub(crate) fn pressure_vertices(&self) -> &[VertexId] {
        &self.pressure_vertices
    }

    pub(crate) fn reduced_pressure_range(&self) -> std::ops::Range<usize> {
        self.reduced_pressure_offset..self.reduced_pressure_offset + self.pressure_vertices.len()
    }

    pub(crate) fn full_pressure_range(&self) -> std::ops::Range<usize> {
        self.full_pressure_offset..self.full_pressure_offset + self.pressure_vertices.len()
    }

    pub(crate) fn fixed_velocity(&self, vertex: usize) -> bool {
        self.fixed_velocity[vertex].iter().any(Option::is_some)
    }

    pub(crate) fn reconstruct_primal(
        &self,
        values: &[f64],
        fluid_cell_count: usize,
    ) -> Result<ReconstructedFsiFields<D>, Diagnostic> {
        if values.len() != self.reduced_size {
            return Err(invalid(
                "fixed-reference FSI solution width differs from its finalized layout",
            ));
        }
        let vertex_velocity = self
            .reduced_vertex_velocity
            .iter()
            .enumerate()
            .map(|(vertex, dofs)| {
                std::array::from_fn(|component| {
                    dofs[component].map_or_else(
                        || {
                            self.fixed_velocity[vertex][component]
                                .expect("eliminated velocity owns a prescribed value")
                        },
                        |dof| values[dof.index()],
                    )
                })
            })
            .collect();
        let fluid_bubbles = (0..fluid_cell_count)
            .map(|cell| {
                std::array::from_fn(|component| {
                    values[self.reduced_bubble_offset + cell * D + component]
                })
            })
            .collect();
        let pressure = values[self.reduced_pressure_offset
            ..self.reduced_pressure_offset + self.pressure_vertices.len()]
            .to_vec();
        Ok((vertex_velocity, fluid_bubbles, pressure))
    }

    pub(crate) fn reconstruct_direction(
        &self,
        values: &[f64],
        fluid_cell_count: usize,
    ) -> Result<ReconstructedFsiFields<D>, Diagnostic> {
        if values.len() != self.reduced_size {
            return Err(invalid(
                "fixed-reference FSI direction width differs from its finalized layout",
            ));
        }
        let vertex_velocity = self
            .reduced_vertex_velocity
            .iter()
            .map(|dofs| {
                std::array::from_fn(|component| {
                    dofs[component].map_or(0.0, |dof| values[dof.index()])
                })
            })
            .collect();
        let fluid_bubbles = (0..fluid_cell_count)
            .map(|cell| {
                std::array::from_fn(|component| {
                    values[self.reduced_bubble_offset + cell * D + component]
                })
            })
            .collect();
        let pressure = values[self.reduced_pressure_offset
            ..self.reduced_pressure_offset + self.pressure_vertices.len()]
            .to_vec();
        Ok((vertex_velocity, fluid_bubbles, pressure))
    }

    pub(crate) fn reconstruct(
        &self,
        values: &[f64],
        fluid_cell_count: usize,
    ) -> Result<ReconstructedFsiFields<D>, Diagnostic> {
        self.reconstruct_primal(values, fluid_cell_count)
    }

    pub(crate) fn reduce(
        &self,
        vertex_velocity: &[[f64; D]],
        bubbles: &[[f64; D]],
        pressure: &[f64],
    ) -> Result<Vec<f64>, Diagnostic> {
        if vertex_velocity.len() != self.reduced_vertex_velocity.len()
            || bubbles.len().checked_mul(D).is_none_or(|width| {
                width != self.reduced_pressure_offset - self.reduced_bubble_offset
            })
            || pressure.len() != self.pressure_vertices.len()
            || vertex_velocity
                .iter()
                .chain(bubbles)
                .flatten()
                .chain(pressure)
                .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "FSI field values must be finite and match the exact reduced layout",
            ));
        }
        for (vertex, value) in vertex_velocity.iter().enumerate() {
            for (component, value) in value.iter().enumerate() {
                if self.fixed_velocity[vertex][component]
                    .is_some_and(|fixed| fixed.to_bits() != value.to_bits())
                {
                    return Err(invalid(
                        "FSI reduced layout requires each eliminated velocity to match its exact prescribed word",
                    ));
                }
            }
        }
        let mut values = vec![0.0; self.reduced_size];
        for (vertex, vector) in vertex_velocity.iter().enumerate() {
            for (component, value) in vector.iter().copied().enumerate() {
                if let Some(dof) = self.reduced_vertex_velocity[vertex][component] {
                    values[dof.index()] = value;
                }
            }
        }
        for (cell, vector) in bubbles.iter().enumerate() {
            for (component, value) in vector.iter().copied().enumerate() {
                values[self.reduced_bubble_offset + cell * D + component] = value;
            }
        }
        values[self.reduced_pressure_offset..].copy_from_slice(pressure);
        Ok(values)
    }

    pub(crate) fn fill_full(
        &self,
        vertex_velocity: &[[f64; D]],
        bubbles: &[[f64; D]],
        pressure: &[f64],
    ) -> Vec<f64> {
        let mut values = vec![0.0; self.full_size];
        for (vertex, vector) in vertex_velocity.iter().enumerate() {
            for component in 0..D {
                values[self.full_vertex_velocity(vertex, component)] = vector[component];
            }
        }
        for (cell, vector) in bubbles.iter().enumerate() {
            for component in 0..D {
                values[self.full_bubble_offset + cell * D + component] = vector[component];
            }
        }
        values[self.full_pressure_offset..self.full_pressure_offset + pressure.len()]
            .copy_from_slice(pressure);
        values
    }
}

fn checked_add(left: usize, right: usize, name: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| invalid(format!("fixed-reference FSI {name} overflows usize")))
}

fn checked_mul(left: usize, right: usize, name: &'static str) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| invalid(format!("fixed-reference FSI {name} overflows usize")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_assembly::LocalUnknown;
    use eqiora_meshing::{CellId, FacetId, MeshQualityGate};

    #[test]
    fn shared_constraints_preserve_nonzero_component_values_and_exact_maps() {
        let mesh = SimplicialMesh::new(
            2,
            vec![
                vec![0.0, 0.0],
                vec![1.0, 0.0],
                vec![0.0, 1.0],
                vec![1.0, 1.0],
            ],
            vec![vec![0, 1, 2], vec![1, 3, 2]],
            MeshQualityGate::new(0.1).unwrap(),
        )
        .unwrap();
        let interface = (0..mesh.entity_count(1).unwrap())
            .find(|&index| {
                mesh.entity_vertices(MeshEntity::new(1, index))
                    .unwrap()
                    .iter()
                    .all(|vertex| [1, 2].contains(&vertex.index()))
            })
            .unwrap();
        let partition = FixedReferenceFsiPartition::<2>::new(
            &mesh,
            vec![CellId::new(0)],
            vec![CellId::new(1)],
            vec![FacetId::new(interface)],
        )
        .unwrap();
        let prescribed = [[Some(1.25), None], [None; 2], [None; 2], [None, Some(-2.5)]];
        let layout = FsiLayout::with_prescribed_velocity(&mesh, &partition, &prescribed).unwrap();
        let fluid = [0, 1, 2].map(|index| MeshEntity::new(0, index));
        let solid = [1, 3, 2].map(|index| MeshEntity::new(0, index));
        let fluid_map = layout.fluid_map(0, &fluid, true).unwrap();
        assert_eq!(
            fluid_map.equations(),
            &[
                None,
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(6),
                Some(7),
                Some(8),
                Some(9),
                Some(10)
            ]
            .map(|index| index.map(DofId::new))
        );
        assert_eq!(fluid_map.unknowns()[0], LocalUnknown::Fixed(1.25));
        let solid_map = layout.solid_map(&solid, true).unwrap();
        assert_eq!(
            solid_map.equations(),
            &[Some(1), Some(2), Some(5), None, Some(3), Some(4)].map(|index| index.map(DofId::new))
        );
        assert_eq!(solid_map.unknowns()[3], LocalUnknown::Fixed(-2.5));
        let values = (0..layout.reduced_size())
            .map(|index| index as f64 + 10.0)
            .collect::<Vec<_>>();
        let (velocity, bubbles, pressure) = layout.reconstruct_primal(&values, 1).unwrap();
        assert_eq!((velocity[0][0], velocity[3][1]), (1.25, -2.5));
        assert_eq!(
            layout.reduce(&velocity, &bubbles, &pressure).unwrap(),
            values
        );
        let direction = layout.reconstruct_direction(&values, 1).unwrap().0;
        assert_eq!((direction[0][0], direction[3][1]), (0.0, 0.0));
        assert!(layout.fluid_map(1, &fluid, true).is_err());
        assert!(layout.solid_map(&[MeshEntity::new(0, 4)], false).is_err());
    }
}
