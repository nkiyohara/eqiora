//! Deterministic quotient layout and reduced/full assembly maps.

use eqiora_assembly::{AssemblyMap, DofId, LocalUnknown};
use eqiora_core::Diagnostic;
use eqiora_meshing::{MeshEntity, MeshTopology, SimplicialMesh, VertexId};

use super::contract::FixedReferenceFsiBoundary;
use super::invalid;
use super::partition::FixedReferenceFsiPartition;
use super::{fluid_local_size, solid_local_size};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FsiLayout<const D: usize = 2> {
    reduced_vertex_velocity: Vec<[Option<DofId>; D]>,
    reduced_bubble_offset: usize,
    reduced_pressure_offset: usize,
    reduced_size: usize,
    full_bubble_offset: usize,
    full_pressure_offset: usize,
    full_size: usize,
    pressure_vertices: Vec<VertexId>,
    pressure_position: Vec<Option<usize>>,
    fixed_velocity: Vec<bool>,
}

type ReconstructedFsiFields<const D: usize> = (Vec<[f64; D]>, Vec<[f64; D]>, Vec<f64>);

impl<const D: usize> FsiLayout<D> {
    pub(crate) fn new(
        mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition<D>,
        boundary: &FixedReferenceFsiBoundary<D>,
    ) -> Result<Self, Diagnostic> {
        if !matches!(D, 2 | 3) || mesh.topological_dimension() != D {
            return Err(invalid(
                "fixed-reference FSI layout requires dimension two or three matching its mesh",
            ));
        }
        let vertex_count = mesh.vertices().len();
        let mut fixed_velocity = vec![false; vertex_count];
        for vertex in boundary.fixed_zero_velocity_vertices() {
            let fixed = fixed_velocity.get_mut(vertex.index()).ok_or_else(|| {
                invalid("fixed-reference FSI boundary vertex is outside the mesh revision")
            })?;
            if *fixed {
                return Err(invalid(
                    "fixed-reference FSI boundary inventory contains a duplicate vertex",
                ));
            }
            *fixed = true;
        }
        let mut reduced_vertex_velocity = vec![[None; D]; vertex_count];
        let mut next = 0_usize;
        for (vertex, dofs) in reduced_vertex_velocity.iter_mut().enumerate() {
            if !fixed_velocity[vertex] {
                for dof in dofs {
                    *dof = Some(DofId::new(next));
                    next = checked_add(next, 1, "free shared velocity width")?;
                }
            }
        }
        let reduced_bubble_offset = next;
        next = checked_add(
            next,
            checked_mul(partition.fluid_cells().len(), D, "fluid bubble width")?,
            "velocity/bubble width",
        )?;
        let reduced_pressure_offset = next;
        let pressure_vertices = partition.fluid_vertices().to_vec();
        next = checked_add(next, pressure_vertices.len(), "pressure width")?;
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
        if next == 0 || full_size == 0 {
            return Err(invalid("fixed-reference FSI layout may not be empty"));
        }
        Ok(Self {
            reduced_vertex_velocity,
            reduced_bubble_offset,
            reduced_pressure_offset,
            reduced_size: next,
            full_bubble_offset,
            full_pressure_offset,
            full_size,
            pressure_vertices,
            pressure_position,
            fixed_velocity,
        })
    }

    pub(crate) fn fluid_map(
        &self,
        fluid_position: usize,
        vertices: &[MeshEntity],
        reduced: bool,
    ) -> Result<AssemblyMap, Diagnostic> {
        let local_size = fluid_local_size::<D>();
        let mut equations = Vec::with_capacity(local_size);
        let mut unknowns = Vec::with_capacity(local_size);
        self.append_vertex_velocity(vertices, reduced, &mut equations, &mut unknowns);
        for component in 0..D {
            let index = if reduced {
                self.reduced_bubble_offset + fluid_position * D + component
            } else {
                self.full_bubble_offset + fluid_position * D + component
            };
            equations.push(Some(DofId::new(index)));
            unknowns.push(LocalUnknown::Free(DofId::new(index)));
        }
        for vertex in vertices {
            let position = self.pressure_position[vertex.index()]
                .expect("fluid vertex owns a pressure position");
            let index = if reduced {
                self.reduced_pressure_offset + position
            } else {
                self.full_pressure_offset + position
            };
            equations.push(Some(DofId::new(index)));
            unknowns.push(LocalUnknown::Free(DofId::new(index)));
        }
        AssemblyMap::new(equations, unknowns)
    }

    pub(crate) fn solid_map(
        &self,
        vertices: &[MeshEntity],
        reduced: bool,
    ) -> Result<AssemblyMap, Diagnostic> {
        let local_size = solid_local_size::<D>();
        let mut equations = Vec::with_capacity(local_size);
        let mut unknowns = Vec::with_capacity(local_size);
        self.append_vertex_velocity(vertices, reduced, &mut equations, &mut unknowns);
        AssemblyMap::new(equations, unknowns)
    }

    fn append_vertex_velocity(
        &self,
        vertices: &[MeshEntity],
        reduced: bool,
        equations: &mut Vec<Option<DofId>>,
        unknowns: &mut Vec<LocalUnknown>,
    ) {
        for vertex in vertices {
            for component in 0..D {
                if reduced {
                    match self.reduced_vertex_velocity[vertex.index()][component] {
                        Some(dof) => {
                            equations.push(Some(dof));
                            unknowns.push(LocalUnknown::Free(dof));
                        }
                        None => {
                            equations.push(None);
                            unknowns.push(LocalUnknown::Fixed(0.0));
                        }
                    }
                } else {
                    let dof = DofId::new(self.full_vertex_velocity(vertex.index(), component));
                    equations.push(Some(dof));
                    unknowns.push(LocalUnknown::Free(dof));
                }
            }
        }
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
        self.fixed_velocity[vertex]
    }

    pub(crate) const fn full_bubble_velocity(
        &self,
        fluid_position: usize,
        component: usize,
    ) -> usize {
        self.full_bubble_offset + fluid_position * D + component
    }

    pub(crate) fn full_pressure(&self, vertex: usize) -> usize {
        self.full_pressure_offset
            + self.pressure_position[vertex].expect("fluid vertex owns pressure position")
    }

    pub(crate) fn reconstruct(
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
        if vertex_velocity
            .iter()
            .enumerate()
            .any(|(vertex, value)| self.fixed_velocity[vertex] && *value != [0.0; D])
        {
            return Err(invalid(
                "FSI reduced layout requires exact zero velocity at every eliminated vertex",
            ));
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
