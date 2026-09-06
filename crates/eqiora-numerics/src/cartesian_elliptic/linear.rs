//! Field-major Cartesian assembly of checked linear scalar equations.

use std::collections::{BTreeMap, BTreeSet};
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
use crate::scalar_conservation::ScalarExteriorLaw;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CartesianLinearAssembly {
    pub(crate) fields: Vec<(RawId, ValueType)>,
    pub(crate) mesh: CartesianMesh,
    pub(crate) constraints: ConstrainedDofLayout,
    pub(crate) system: LinearSystem,
    pub(crate) full_system: LinearSystem,
    pub(crate) report: AssemblyReport,
    source_integrals: Vec<f64>,
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
    pub(crate) fn into_single_field_canonical(
        self,
    ) -> Result<
        (
            Arc<CanonicalCsrSystemView>,
            super::FinalizedCartesianFemState,
        ),
        Diagnostic,
    > {
        if self.fields.len() != 1 {
            return Err(super::invalid(
                "scalar differentiation requires exactly one Field",
            ));
        }
        let canonical = Arc::new(CanonicalCsrSystemView::new(
            &self.system,
            LinearOperatorProperties::General,
        )?);
        Ok((
            canonical,
            super::FinalizedCartesianFemState {
                mesh: self.mesh,
                constrained_dofs: self.constraints,
                full_system: self.full_system,
                integrated_source: self.source_integrals[0],
                assembly_report: self.report,
            },
        ))
    }

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

    pub(crate) fn assemble(
        form: &CompiledLinearBlockForm,
        mesh: &CartesianMesh,
        quadrature: &QuadratureRule,
        backend: &dyn AssemblyBackend,
        boundaries: &BTreeMap<(usize, BoundarySide), RawId>,
    ) -> Result<Self, Diagnostic> {
        super::validate_problem(mesh, quadrature)?;
        let dimension = mesh.topological_dimension();
        if form.dimension() != dimension {
            return Err(super::invalid("linear block and Mesh dimensions differ"));
        }
        let domains = boundaries.values().copied().collect::<BTreeSet<_>>();
        if domains.len() != 2 * dimension
            || boundaries.len() != domains.len()
            || (0..dimension).any(|axis| {
                [BoundarySide::Lower, BoundarySide::Upper]
                    .iter()
                    .any(|side| !boundaries.contains_key(&(axis, *side)))
            })
            || form
                .boundary_laws()
                .values()
                .any(|laws| laws.keys().copied().collect::<BTreeSet<_>>() != domains)
        {
            return Err(super::invalid(
                "linear boundary laws differ from the exact Cartesian support mapping",
            ));
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
        let mut natural_integrals = vec![0.0; form.fields().len()];
        for (index, (field, _)) in form.fields().iter().enumerate() {
            let boundary = |axis, side, coordinates: &[f64]| match &form.boundary_laws()[field]
                [&boundaries[&(axis, side)]]
            {
                ScalarExteriorLaw::PrescribedTrace { value, .. } => {
                    CartesianBoundaryValue::Essential(
                        value.evaluate(coordinates).unwrap_or(f64::NAN),
                    )
                }
                ScalarExteriorLaw::PrescribedOutwardFlux { value, .. } => {
                    CartesianBoundaryValue::Natural(value.evaluate(coordinates).unwrap_or(f64::NAN))
                }
                ScalarExteriorLaw::ZeroOutwardFlux { .. } => CartesianBoundaryValue::Natural(0.0),
                ScalarExteriorLaw::Robin { .. } => {
                    unreachable!("linear compiler rejects Robin laws")
                }
            };
            fixed.extend(super::essential_fem_values(mesh, &boundary)?);
            natural.extend(super::natural_fem_facets(mesh, &boundary)?.into_iter().map(
                |(local, facet_vertices)| {
                    natural_integrals[index] += local.rhs().iter().sum::<f64>();
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
        let source_integrals = full_system
            .rhs()
            .chunks_exact(vertices)
            .zip(natural_integrals)
            .map(|(rhs, natural)| rhs.iter().sum::<f64>() - natural)
            .collect::<Vec<_>>();
        if source_integrals.iter().any(|value| !value.is_finite()) {
            return Err(super::invalid("linear source integral is non-finite"));
        }
        Ok(Self {
            fields: form.fields().to_vec(),
            mesh: mesh.clone(),
            constraints,
            system,
            full_system,
            report,
            source_integrals,
        })
    }
}
