//! Physical P1 mass/stiffness assembly for the prescribed-solid step.

use eqiora_assembly::{
    AssemblyBackend, AssemblyMap, AssemblyPacket, AssemblyPacketSetIdentityV1, AssemblyPlan,
    AssemblyReport, AssemblyResult, AssemblyTarget, AssemblyTargetId, AssemblyWork, CsrMatrix,
    DofId, LinearSystem, LocalContribution, LocalUnknown, TargetAssemblyMap,
};
use eqiora_core::Diagnostic;
use eqiora_meshing::{
    MeshEntity, MeshGeometry, MeshTopology, QuadratureRule, simplex_duffy_gauss_legendre,
};

use super::contract::{PrescribedDynamicSolidContract, invalid};
use crate::simplicial_solid_element::p1_solid_element_matrices;

const DIMENSION: usize = 3;

#[derive(Debug)]
pub(super) struct AssembledPhysicalOperators {
    mass: CsrMatrix,
    stiffness: CsrMatrix,
    report: AssemblyReport,
}

impl AssembledPhysicalOperators {
    pub(super) const fn mass(&self) -> &CsrMatrix {
        &self.mass
    }

    pub(super) const fn stiffness(&self) -> &CsrMatrix {
        &self.stiffness
    }

    pub(super) fn into_parts(self) -> (CsrMatrix, CsrMatrix, AssemblyReport) {
        (self.mass, self.stiffness, self.report)
    }
}

pub(super) fn assemble_physical_operators(
    contract: &PrescribedDynamicSolidContract,
    backend: &dyn AssemblyBackend,
) -> Result<AssembledPhysicalOperators, Diagnostic> {
    let prepared = PreparedPhysicalAssembly::new(contract)?;
    let result = backend.assemble(&prepared.plan, &prepared)?;
    prepared.finish(result)
}

#[derive(Debug)]
struct PreparedPhysicalAssembly<'a> {
    contract: &'a PrescribedDynamicSolidContract,
    quadrature: QuadratureRule,
    plan: AssemblyPlan,
    mass_target: AssemblyTargetId,
    stiffness_target: AssemblyTargetId,
    cell_count: usize,
    full_size: usize,
}

impl<'a> PreparedPhysicalAssembly<'a> {
    fn new(contract: &'a PrescribedDynamicSolidContract) -> Result<Self, Diagnostic> {
        let cell_count = contract.mesh().entity_count(DIMENSION).ok_or_else(|| {
            invalid("prescribed dynamic-solid mesh omits its tetrahedral cell stratum")
        })?;
        let full_size = contract
            .mesh()
            .vertices()
            .len()
            .checked_mul(DIMENSION)
            .ok_or_else(|| {
                invalid("prescribed dynamic-solid full operator width overflows usize")
            })?;
        let plan = AssemblyPlan::new(vec![
            AssemblyTarget::new(full_size)?,
            AssemblyTarget::new(full_size)?,
        ])?;
        let mass_target = plan
            .target_id(0)
            .expect("two-target physical assembly owns mass target");
        let stiffness_target = plan
            .target_id(1)
            .expect("two-target physical assembly owns stiffness target");
        Ok(Self {
            contract,
            quadrature: simplex_duffy_gauss_legendre(DIMENSION, 3)?,
            plan,
            mass_target,
            stiffness_target,
            cell_count,
            full_size,
        })
    }

    fn finish(self, result: AssemblyResult) -> Result<AssembledPhysicalOperators, Diagnostic> {
        let (systems, report) = result.into_parts();
        if report.packet_count() != self.cell_count || report.target_count() != 2 {
            return Err(invalid(
                "prescribed dynamic-solid assembly evidence differs from its exact cell/two-operator inventory",
            ));
        }
        let systems: [LinearSystem; 2] = systems.try_into().map_err(|systems: Vec<_>| {
            invalid(format!(
                "prescribed dynamic-solid assembly returned {} systems instead of mass and stiffness",
                systems.len()
            ))
        })?;
        let [mass, stiffness] = systems;
        for (name, system) in [("mass", &mass), ("stiffness", &stiffness)] {
            if system.matrix().rows() != self.full_size
                || system.matrix().columns() != self.full_size
                || system.rhs().iter().any(|value| *value != 0.0)
            {
                return Err(invalid(format!(
                    "prescribed dynamic-solid {name} assembly differs from its exact physical operator shape"
                )));
            }
        }
        Ok(AssembledPhysicalOperators {
            mass: mass.matrix().clone(),
            stiffness: stiffness.matrix().clone(),
            report,
        })
    }
}

impl AssemblyWork for PreparedPhysicalAssembly<'_> {
    fn packet_set_identity(&self) -> AssemblyPacketSetIdentityV1 {
        AssemblyPacketSetIdentityV1::Unbound
    }

    fn packet_count(&self) -> usize {
        self.cell_count
    }

    fn evaluate(&self, packet_index: usize) -> Result<AssemblyPacket, Diagnostic> {
        if packet_index >= self.cell_count {
            return Err(invalid(format!(
                "prescribed dynamic-solid packet {packet_index} is outside cell count {}",
                self.cell_count
            )));
        }
        let cell = MeshEntity::new(DIMENSION, packet_index);
        let geometry = self.contract.mesh().geometry_map(cell).ok_or_else(|| {
            invalid(format!(
                "prescribed dynamic-solid tetrahedron {packet_index} has no affine geometry"
            ))
        })?;
        let vertices = self.contract.mesh().entity_vertices(cell).ok_or_else(|| {
            invalid(format!(
                "prescribed dynamic-solid tetrahedron {packet_index} has no vertex closure"
            ))
        })?;
        let physical = p1_solid_element_matrices::<DIMENSION>(
            &geometry,
            &self.quadrature,
            self.contract.density(),
            self.contract.shear_modulus(),
            self.contract.first_lame_parameter(),
        )?;
        let local_size = physical.local_size();
        let columns = local_size
            .checked_mul(2)
            .ok_or_else(|| invalid("prescribed dynamic-solid block-local width overflows usize"))?;
        let mut matrix = vec![0.0; local_size * columns];
        for row in 0..local_size {
            for column in 0..local_size {
                matrix[row * columns + column] = physical.mass()[row * local_size + column];
                matrix[row * columns + local_size + column] =
                    physical.stiffness()[row * local_size + column];
            }
        }
        let local = LocalContribution::new(local_size, columns, matrix, vec![0.0; local_size])?;
        let equations = local_dofs(&vertices);
        let physical_unknowns = equations
            .iter()
            .map(|dof| LocalUnknown::Free(dof.expect("every full physical row owns a DOF")))
            .collect::<Vec<_>>();
        let fixed_zero = vec![LocalUnknown::Fixed(0.0); local_size];
        let mass_map = AssemblyMap::new(
            equations.clone(),
            physical_unknowns
                .iter()
                .copied()
                .chain(fixed_zero.iter().copied())
                .collect(),
        )?;
        let stiffness_map = AssemblyMap::new(
            equations,
            fixed_zero.into_iter().chain(physical_unknowns).collect(),
        )?;
        AssemblyPacket::new(
            local,
            vec![
                TargetAssemblyMap::new(self.mass_target, mass_map),
                TargetAssemblyMap::new(self.stiffness_target, stiffness_map),
            ],
        )
    }
}

fn local_dofs(vertices: &[MeshEntity]) -> Vec<Option<DofId>> {
    vertices
        .iter()
        .flat_map(|vertex| {
            (0..DIMENSION)
                .map(move |component| Some(DofId::new(vertex.index() * DIMENSION + component)))
        })
        .collect()
}
