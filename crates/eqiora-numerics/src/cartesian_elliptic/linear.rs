//! Field-major Cartesian assembly of checked linear scalar equations.

use eqiora_assembly::{AssemblyBackend, AssemblyReport, LinearSystem};
use eqiora_core::{Diagnostic, RawId, ValueType};
use eqiora_meshing::{CartesianMesh, MeshEntity, MeshGeometry, MeshTopology, QuadratureRule};

use crate::constrained_dofs::ConstrainedDofLayout;
use crate::form_compiler::linear::CompiledLinearBlockForm;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CartesianLinearAssembly {
    pub(crate) fields: Vec<(RawId, ValueType)>,
    pub(crate) mesh: CartesianMesh,
    pub(crate) constraints: ConstrainedDofLayout,
    pub(crate) system: LinearSystem,
    pub(crate) full_system: LinearSystem,
    pub(crate) report: AssemblyReport,
}

impl CartesianLinearAssembly {
    pub(crate) fn assemble(
        form: &CompiledLinearBlockForm,
        mesh: &CartesianMesh,
        quadrature: &QuadratureRule,
        backend: &dyn AssemblyBackend,
    ) -> Result<Self, Diagnostic> {
        super::validate_problem(mesh, quadrature)?;
        let dimension = mesh.topological_dimension();
        if form.dimension() != dimension {
            return Err(super::invalid("linear block and Mesh dimensions differ"));
        }
        let vertices = mesh.entity_count(0).expect("Cartesian vertices");
        let count = form
            .fields()
            .len()
            .checked_mul(vertices)
            .ok_or_else(|| super::invalid("linear block DOF count overflows usize"))?;
        let mut fixed = Vec::new();
        fixed
            .try_reserve_exact(count)
            .map_err(|_| super::invalid("linear block DOF allocation exceeds capacity"))?;
        for _ in form.fields() {
            for vertex in 0..vertices {
                fixed.push(
                    mesh.is_boundary_entity(MeshEntity::new(0, vertex))
                        .expect("Cartesian vertex boundary classification")
                        .then_some(0.0),
                );
            }
        }
        let constraints = ConstrainedDofLayout::new(fixed)?;
        if constraints.free_count() == 0 {
            return Err(super::invalid("linear block requires unconstrained DOFs"));
        }
        let cells = mesh.entity_count(dimension).expect("Cartesian cells");
        let (system, full_system, report) = constraints.assemble(backend, cells, |index| {
            let cell = MeshEntity::new(dimension, index);
            let geometry = mesh.geometry_map(cell).expect("Cartesian cell geometry");
            let local = form.evaluate(&geometry, quadrature)?;
            let cell_vertices = mesh.entity_vertices(cell).expect("Cartesian cell vertices");
            let local_count = form
                .fields()
                .len()
                .checked_mul(cell_vertices.len())
                .ok_or_else(|| super::invalid("linear block local DOF count overflows usize"))?;
            let mut globals = Vec::new();
            globals.try_reserve_exact(local_count).map_err(|_| {
                super::invalid("linear block local DOF allocation exceeds capacity")
            })?;
            for field in 0..form.fields().len() {
                globals.extend(
                    cell_vertices
                        .iter()
                        .map(|vertex| field * vertices + vertex.index()),
                );
            }
            Ok((local, globals))
        })?;
        Ok(Self {
            fields: form.fields().to_vec(),
            mesh: mesh.clone(),
            constraints,
            system,
            full_system,
            report,
        })
    }
}
