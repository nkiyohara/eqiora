//! The ordinary 2D fixture enters through exact Model and Realization admission.

use eqiora_compiler::compile;
use eqiora_core::{DimExponents, DynQuantity};
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_realization::{
    CoupledFieldwiseRealizationRequest, DiscretizationMethod, MeshArtifactReference, MeshKind,
    RealizationCapabilities, RealizationRevision, SemanticRevision, SpatialDimensionSupport,
    TargetCapabilities, VectorLayoutKind, resolve_coupled_fieldwise,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{ScalarType, SolverCapabilities, SolverCapability};

use crate::canonical_fsi::lower_fixed_reference_fsi_cartesian_2d;
use crate::fsi::{
    FinalizedResolvedFixedReferenceFsiStep2d, FixedReferenceFsiScaleProfile2d,
    finalize_resolved_fixed_reference_fsi_step_2d_with_assembly, fixed_reference_fsi_plan_2d,
    fixed_reference_fsi_requirements_2d,
};

use super::*;

const SOURCE: &str =
    include_str!("../../../../../verify/fsi/fixed-reference-monolithic-step-2d/models/direct.eqi");

pub(super) fn finalize(
    problem: &Fixture,
    config: FixedReferenceFsiStepConfig<2>,
    assembly: &dyn AssemblyBackend,
) -> Result<FinalizedResolvedFixedReferenceFsiStep2d, Diagnostic> {
    let material = config.material();
    let mut source = SOURCE.to_owned();
    for (declaration, value) in [
        (
            "parameter fluid_density: kg / m ^ 3 = 2;",
            material.fluid_density(),
        ),
        (
            "parameter fluid_viscosity: kg / (m * s) = 0.5;",
            material.fluid_dynamic_viscosity(),
        ),
        (
            "parameter solid_density: kg / m ^ 3 = 3;",
            material.solid_density(),
        ),
        (
            "parameter solid_mu: kg / (m * s ^ 2) = 4;",
            material.solid_shear_modulus(),
        ),
        (
            "parameter solid_lambda: kg / (m * s ^ 2) = 2;",
            material.solid_first_lame_parameter(),
        ),
    ] {
        assert_eq!(source.matches(declaration).count(), 1);
        let prefix = declaration.split_once(" = ").unwrap().0;
        source = source.replace(declaration, &format!("{prefix} = {value};"));
    }
    let (transaction, model, _) = compile("resolved-fsi-test.eqi", &source)
        .unwrap()
        .remove(0)
        .into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let model = lower_fixed_reference_fsi_cartesian_2d(&program).unwrap();
    let quantity =
        |value, exponents| DynQuantity::new(value, DimExponents::from_integers(exponents).unwrap());
    let scales = config.scale();
    let scales = FixedReferenceFsiScaleProfile2d::new(
        quantity(scales.length(), [0, 1, 0, 0, 0, 0, 0]),
        quantity(scales.velocity(), [0, 1, -1, 0, 0, 0, 0]),
        quantity(scales.pressure(), [1, -1, -2, 0, 0, 0, 0]),
    )
    .unwrap();
    let mesh_reference = MeshArtifactReference::from_sha256([7; 32]);
    let plan = fixed_reference_fsi_plan_2d(
        &model,
        mesh_reference,
        quantity(config.time_step(), [0, 0, 1, 0, 0, 0, 0]),
        scales,
        reference_solver().plan(),
    )
    .unwrap();
    let solver = SolverCapabilities::exact([SolverCapability {
        algorithm: LinearSolver::MinimumResidual,
        operator_properties: LinearOperatorProperties::SymmetricIndefinite,
        preconditioner: PreconditionerPolicy::Identity,
        reduction: ReductionPolicy::Reproducible,
        scalar_type: ScalarType::F64,
    }])
    .unwrap();
    let capabilities = RealizationCapabilities::cartesian_product(
        [DiscretizationMethod::ContinuousGalerkin],
        [(
            MeshKind::ImportedAffineSimplicial,
            SpatialDimensionSupport::exact(NonZeroUsize::new(2).unwrap()),
        )],
        [VectorLayoutKind::Replicated],
        solver,
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::new(1).unwrap()),
    )
    .unwrap();
    let resolved = resolve_coupled_fieldwise(
        &CoupledFieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(1),
            plan,
        ),
        fixed_reference_fsi_requirements_2d(&model),
        &capabilities,
    )
    .unwrap();
    finalize_resolved_fixed_reference_fsi_step_2d_with_assembly(
        &model,
        &resolved,
        mesh_reference,
        &problem.mesh,
        &problem.partition,
        &problem.previous,
        assembly,
    )
}

#[derive(Debug)]
pub(super) struct PressureNullspaceBackend {
    pub(super) pressure: std::ops::Range<usize>,
}

impl AssemblyBackend for PressureNullspaceBackend {
    fn assemble(
        &self,
        plan: &AssemblyPlan,
        work: &dyn eqiora_assembly::AssemblyWork,
    ) -> Result<AssemblyResult, Diagnostic> {
        let (mut systems, report) = REFERENCE_ASSEMBLY_BACKEND
            .assemble(plan, work)?
            .into_parts();
        let system = &systems[0];
        let n = system.matrix().rows();
        assert!(self.pressure.len() > 1 && self.pressure.end <= n);
        let mut values = vec![0.0; n * n];
        for row in 0..n {
            for column in 0..n {
                if !self.pressure.contains(&row) && !self.pressure.contains(&column) {
                    values[row * n + column] = system.matrix().entry(row, column).unwrap();
                }
            }
        }
        // A chain Laplacian retains nonempty symmetric pressure rows but annihilates constants.
        // Remove pressure/velocity coupling so the full operator has that same null vector.
        for row in self.pressure.start..self.pressure.end - 1 {
            values[row * n + row] += 1.0;
            values[(row + 1) * n + row + 1] += 1.0;
            values[row * n + row + 1] -= 1.0;
            values[(row + 1) * n + row] -= 1.0;
        }
        let local = LocalContribution::new(n, n, values, system.rhs().to_vec())?;
        let map = AssemblyMap::new(
            (0..n).map(|index| Some(DofId::new(index))).collect(),
            (0..n)
                .map(|index| LocalUnknown::Free(DofId::new(index)))
                .collect(),
        )?;
        let mut assembler = eqiora_assembly::CooAssembler::new(n)?;
        assembler.scatter(&map, &local)?;
        systems[0] = assembler.finish()?;
        AssemblyResult::from_complete_systems(
            plan,
            systems,
            work.packet_count(),
            report.execution(),
        )
    }
}
