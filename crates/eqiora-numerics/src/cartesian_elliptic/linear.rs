//! Field-major Cartesian assembly of checked linear scalar equations.

use std::sync::Arc;

use eqiora_assembly::{AssemblyBackend, AssemblyReport, LinearSystem};
use eqiora_core::{Diagnostic, RawId, ValueType};
use eqiora_meshing::{CartesianMesh, MeshEntity, MeshGeometry, MeshTopology, QuadratureRule};
use eqiora_realization::{Target, VectorLayoutKind};
use eqiora_schema::kernel::BoundarySide;
use eqiora_solver::{
    CanonicalCsrSystemView, LinearOperatorProperties, LinearSolveRequest, SolveReport,
};

use super::{CartesianBoundaryValue, CartesianQ1Field};
use crate::constrained_dofs::ConstrainedDofLayout;
use crate::finalized_spatial::FinalizedLinearCore;
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CartesianLinearSolution {
    pub(crate) fields: Vec<(RawId, ValueType, CartesianQ1Field)>,
    pub(crate) solve_report: SolveReport,
    pub(crate) assembly_report: AssemblyReport,
}

#[cfg(test)]
mod tests;

impl CartesianLinearAssembly {
    /// Validate the complete algebraic solution before publishing any Field.
    pub(crate) fn solve(
        self,
        request: LinearSolveRequest<'_>,
        target: Target,
    ) -> Result<CartesianLinearSolution, Diagnostic> {
        let canonical = Arc::new(CanonicalCsrSystemView::new(
            &self.system,
            LinearOperatorProperties::General,
        )?);
        let core = FinalizedLinearCore::new(
            request.plan(),
            VectorLayoutKind::Replicated,
            target,
            canonical,
        );
        let solution = request.solve(&core.linear_problem()?)?;
        core.validate_solution(&solution)?;
        let (algebraic, solve_report) = solution.into_parts();
        let values = self.constraints.lift(&algebraic)?;
        let vertices = self.mesh.entity_count(0).expect("Cartesian vertices");
        if self.fields.len().checked_mul(vertices) != Some(values.len()) {
            return Err(super::invalid(
                "linear solution does not cover the complete Field inventory",
            ));
        }
        let fields = self
            .fields
            .into_iter()
            .zip(values.chunks_exact(vertices))
            .map(|((field, value_type), values)| {
                Ok((
                    field,
                    value_type,
                    CartesianQ1Field::new(self.mesh.clone(), values.to_vec())?,
                ))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        Ok(CartesianLinearSolution {
            fields,
            solve_report,
            assembly_report: self.report,
        })
    }

    pub(crate) fn assemble<B>(
        form: &CompiledLinearBlockForm,
        mesh: &CartesianMesh,
        quadrature: &QuadratureRule,
        backend: &dyn AssemblyBackend,
        boundary: &B,
    ) -> Result<Self, Diagnostic>
    where
        B: Fn(RawId, usize, BoundarySide, &[f64]) -> CartesianBoundaryValue + ?Sized,
    {
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
        let mut natural = Vec::new();
        for (index, (field, _)) in form.fields().iter().enumerate() {
            let boundary =
                |axis, side, coordinates: &[f64]| boundary(*field, axis, side, coordinates);
            fixed.extend(super::essential_fem_values(mesh, &boundary)?);
            natural.extend(super::natural_fem_facets(mesh, &boundary)?.into_iter().map(
                |(local, facet_vertices)| {
                    let globals = facet_vertices
                        .into_iter()
                        .map(|vertex| index * vertices + vertex.index())
                        .collect::<Vec<_>>();
                    (local, globals)
                },
            ));
        }
        let constraints = ConstrainedDofLayout::new(fixed)?;
        if constraints.free_count() == 0 {
            return Err(super::invalid("linear block requires unconstrained DOFs"));
        }
        let cells = mesh.entity_count(dimension).expect("Cartesian cells");
        let packets = cells
            .checked_add(natural.len())
            .ok_or_else(|| super::invalid("linear block packet count overflows usize"))?;
        let (system, full_system, report) = constraints.assemble(backend, packets, |index| {
            if index >= cells {
                return Ok(natural[index - cells].clone());
            }
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
