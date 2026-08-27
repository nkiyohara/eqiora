//! Private Model-driven admission of exact common Mesh resources and numerical policy.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use crate::canonical::{
    lower_scalar_elliptic_cartesian_with_resources, recognize_scalar_elliptic_geometry_mathematics,
};
use crate::canonical_stokes::{
    IncompressibleScalingRequest2d, ResolvedIncompressibleScaling2d,
    recognize_steady_incompressible_stokes_geometry_mathematics,
    solve_resolved_steady_stokes_geometry_mini_2d,
};
use crate::fluid::{IncompressibleFlowScaleProfile2d, SteadyStokesGeometryBinding2d};
use crate::scalar::{
    ResolvedScalarEllipticCartesianSolution, ScalarEllipticCartesianModel,
    solve_scalar_elliptic_cartesian_fem, solve_scalar_elliptic_cartesian_fvm,
};
use eqiora_artifact::{
    CanonicalModelArtifact, CartesianMeshEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    MeshProductionLineageEnvelopeV1, ModelEnvelope, RealizationEnvelopeV2,
    SimplicialMeshEnvelopeV1,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_io_gmsh::{GmshImportLimits, GmshSimplexImporter, GmshSimplicialImport};
use eqiora_meshing::{MeshEntity, MeshTopology, QuadratureRule};
use eqiora_realization::{
    DiscretizationMethod, FieldwiseRealizationRequest, MeshKind, RealizationCapabilities,
    RealizationRevision, ResolvedFieldwiseRealization, SemanticRevision, Space, SpaceFamily,
    SpatialDimensionSupport, TargetCapabilities, VectorLayoutKind, resolve_fieldwise,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    ExecutionProvider, LinearOperatorProperties, LinearSolveRequest, LinearSolver,
    LinearSolverBackend, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, ScalarType, SolverCapabilities, SolverCapability, SolverPlan,
    SolverProvider,
};
use sha2::{Digest, Sha256};

const APPLICATION_REALIZATION_REVISION: u64 = 134;
const POLICY_DOMAIN: &[u8] = b"eqiora.private-native-numerical-admission/v1\0";
type TaggedMeshAssignments = (BTreeMap<u32, Vec<usize>>, BTreeMap<u32, Vec<usize>>);

/// Closed spatial choice requested from the Model-first common resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonSpatialPolicy {
    Q1,
    CellCenteredTpfa,
    MiniP1,
}

/// Opaque result of Model-first common numerical resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCommonPlan {
    kind: ResolvedCommonPlanKind,
}

#[derive(Debug, Clone, PartialEq)]
enum ResolvedCommonPlanKind {
    Scalar(Box<CommonScalarPlan>),
    SteadyStokes(Box<CommonSteadyStokesPlan>),
}

impl ResolvedCommonPlan {
    /// Project one already-resolved Plan without reopening capability selection.
    pub fn project<T>(
        self,
        scalar: impl FnOnce(CommonScalarPlan) -> T,
        steady_stokes: impl FnOnce(CommonSteadyStokesPlan) -> T,
    ) -> T {
        match self.kind {
            ResolvedCommonPlanKind::Scalar(plan) => scalar(*plan),
            ResolvedCommonPlanKind::SteadyStokes(plan) => steady_stokes(*plan),
        }
    }
}

/// Opaque common scalar Plan owning authenticated Model, Mesh, and policy state.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonScalarPlan {
    admission: NativeNumericalAdmission,
    identity: String,
    model_id: String,
    model_revision: u64,
    geometry_digest: String,
    mesh_digest: String,
    correspondence_digest: String,
    production_digest: String,
    field_id: String,
    cells: [usize; 2],
}

/// Opaque steady-Stokes Plan owning one authenticated exact-cylinder occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonSteadyStokesPlan {
    admission: NativeNumericalAdmission,
    binding: SteadyStokesGeometryBinding2d,
    resolved: ResolvedFieldwiseRealization,
    realization: RealizationEnvelopeV2,
    scaling: ResolvedIncompressibleScaling2d,
    identity: String,
    model_id: String,
    model_revision: u64,
    geometry_digest: String,
    mesh_digest: String,
    correspondence_digest: String,
    production_digest: String,
    velocity_field_id: String,
    pressure_field_id: String,
    velocity_space: Space,
    pressure_space: Space,
}

/// Resolve Model mathematics first, then admit the requested numerical policies.
pub fn resolve_common_plan(
    model: &ModelEnvelope,
    owner: AuthenticatedCommonMesh,
    spatial: CommonSpatialPolicy,
    solve: SolverPlan,
    stokes_backend: &dyn LinearSolverBackend,
) -> Result<ResolvedCommonPlan, Diagnostic> {
    let recognized = RecognizedNativeAdmission::recognize(model, owner)?;
    if solve.algorithm() != LinearSolver::ConjugateGradient
        || solve.preconditioner() != PreconditionerPolicy::Identity
        || solve.reduction() != ReductionPolicy::Reproducible
    {
        return Err(invalid(
            "common Linear request must contain identity-preconditioned reproducible controls",
        ));
    }
    match recognized.capability {
        NativeCapability::ScalarElliptic => {
            let spatial = match spatial {
                CommonSpatialPolicy::Q1 => NativeSpatialPolicy::ScalarQ1,
                CommonSpatialPolicy::CellCenteredTpfa => NativeSpatialPolicy::ScalarTpfa,
                CommonSpatialPolicy::MiniP1 => {
                    return Err(invalid(
                        "scalar-elliptic Model mathematics is incompatible with MINI/P1",
                    ));
                }
            };
            let linear = NativeLinearPolicy::exact(solve, &REFERENCE_LINEAR_SOLVER)?;
            let admission = recognized.complete(spatial, linear)?;
            CommonScalarPlan::from_admission(model, admission).map(|plan| ResolvedCommonPlan {
                kind: ResolvedCommonPlanKind::Scalar(Box::new(plan)),
            })
        }
        NativeCapability::SteadyIncompressibleStokes => {
            if spatial != CommonSpatialPolicy::MiniP1 {
                return Err(invalid(
                    "steady-Stokes Model mathematics requires the admitted MINI/P1 policy",
                ));
            }
            let RecognizedNativeModel::Stokes(binding) = &recognized.recognized else {
                unreachable!("steady-Stokes capability recognition returns a Stokes binding")
            };
            let scaling = binding
                .resolve_incompressible_scaling(model, None::<IncompressibleScalingRequest2d>)?;
            let effective_solve = SolverPlan::new(
                LinearSolver::SparseLu,
                solve.relative_tolerance(),
                solve.absolute_tolerance(),
                solve.maximum_iterations(),
            )?
            .with_reduction(ReductionPolicy::Fast);
            let linear = NativeLinearPolicy::exact(effective_solve, stokes_backend)?;
            let admission =
                recognized.complete(NativeSpatialPolicy::StokesMiniP1(scaling.scales()), linear)?;
            CommonSteadyStokesPlan::from_admission(model, admission, scaling).map(|plan| {
                ResolvedCommonPlan {
                    kind: ResolvedCommonPlanKind::SteadyStokes(Box::new(plan)),
                }
            })
        }
    }
}

impl CommonSteadyStokesPlan {
    fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
        scaling: ResolvedIncompressibleScaling2d,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let binding = admission.stokes_binding()?;
        let (resolved, realization, velocity_space, pressure_space) =
            admission.resolve_stokes(&binding)?;
        let (geometry_digest, mesh_digest, correspondence_digest, production_digest) =
            resource_digests(admission.resources())?;
        let mut velocity_field_id = None;
        let mut pressure_field_id = None;
        for field in resolved.plan().spatial().field_spaces() {
            match field.space().family() {
                SpaceFamily::SimplexP1Bubble => {
                    velocity_field_id = Some(field.field().ulid().to_string());
                }
                SpaceFamily::ContinuousLagrange { order } if order == std::num::NonZeroU16::MIN => {
                    pressure_field_id = Some(field.field().ulid().to_string());
                }
                _ => {}
            }
        }
        let (velocity_field_id, pressure_field_id) = velocity_field_id
            .zip(pressure_field_id)
            .ok_or_else(|| invalid("steady-Stokes Plan omitted its MINI/P1 Field identities"))?;
        let realization_digest = realization.digest()?.to_string();
        let mut identity_bytes = Vec::new();
        for value in [
            admission.model_digest(),
            geometry_digest.as_str(),
            mesh_digest.as_str(),
            correspondence_digest.as_str(),
            production_digest.as_str(),
            realization_digest.as_str(),
            admission.policy_identity(),
            "automatic-exact-cylinder-stokes-scaling/v1",
        ] {
            push_framed(&mut identity_bytes, value.as_bytes());
        }
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-steady-stokes-plan/v1\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            binding,
            resolved,
            realization,
            scaling,
            identity,
            model_id: model_reference.model().ulid().to_string(),
            model_revision: model_reference.semantic_revision().get(),
            geometry_digest,
            mesh_digest,
            correspondence_digest,
            production_digest,
            velocity_field_id,
            pressure_field_id,
            velocity_space,
            pressure_space,
        })
    }

    /// Execute solely from the state retained by this Plan.
    pub fn run(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<crate::fluid::SteadyStokesMiniSolution2d, Diagnostic> {
        self.admission.revalidate()?;
        if backend.provider() != self.admission.linear.provider
            || backend.capabilities() != self.admission.linear.capabilities
        {
            return Err(invalid(
                "steady-Stokes execution backend differs from the admitted provider or capabilities",
            ));
        }
        let solution = solve_resolved_steady_stokes_geometry_mini_2d(
            &self.admission.program,
            &self.resolved,
            &self.binding,
            backend,
        )?;
        if solution.scales() != self.scaling.scales() {
            return Err(invalid(
                "steady-Stokes execution changed the Plan-owned effective scaling",
            ));
        }
        Ok(solution)
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
    #[must_use]
    pub const fn model_revision(&self) -> u64 {
        self.model_revision
    }
    #[must_use]
    pub fn model_digest(&self) -> &str {
        self.admission.model_digest()
    }
    #[must_use]
    pub fn geometry_digest(&self) -> &str {
        &self.geometry_digest
    }
    #[must_use]
    pub fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }
    #[must_use]
    pub fn correspondence_digest(&self) -> &str {
        &self.correspondence_digest
    }
    #[must_use]
    pub fn production_digest(&self) -> &str {
        &self.production_digest
    }
    pub fn realization_digest(&self) -> Result<String, Diagnostic> {
        self.realization.digest().map(|digest| digest.to_string())
    }
    #[must_use]
    pub fn velocity_field_id(&self) -> &str {
        &self.velocity_field_id
    }
    #[must_use]
    pub fn pressure_field_id(&self) -> &str {
        &self.pressure_field_id
    }
    #[must_use]
    pub const fn velocity_space(&self) -> Space {
        self.velocity_space
    }
    #[must_use]
    pub const fn pressure_space(&self) -> Space {
        self.pressure_space
    }
    #[must_use]
    pub const fn scales(&self) -> IncompressibleFlowScaleProfile2d {
        self.scaling.scales()
    }
    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
}

impl CommonScalarPlan {
    fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let NativeMeshResources::Cartesian {
            geometry,
            mesh,
            correspondence,
            production,
        } = &admission.resources
        else {
            return Err(invalid(
                "scalar Q1/TPFA common Plan requires an authenticated Cartesian Mesh",
            ));
        };
        let geometry_digest = hex_bytes(&geometry.digest_bytes());
        let mesh_digest = mesh.digest()?.to_string();
        let correspondence_digest = correspondence.digest()?.to_string();
        let production_digest = production.digest()?.to_string();
        let cells = [
            mesh.mesh()
                .axis_cell_count(0)
                .ok_or_else(|| invalid("common Plan Mesh omitted x-axis cells"))?,
            mesh.mesh()
                .axis_cell_count(1)
                .ok_or_else(|| invalid("common Plan Mesh omitted y-axis cells"))?,
        ];
        let RecognizedNativeModel::Scalar(lowered) = &admission.recognized else {
            return Err(invalid(
                "common scalar Plan admitted non-scalar mathematics",
            ));
        };
        let field_id = lowered.field_id().ulid().to_string();
        let mut identity_bytes = Vec::new();
        for value in [
            admission.model_digest(),
            geometry_digest.as_str(),
            mesh_digest.as_str(),
            correspondence_digest.as_str(),
            production_digest.as_str(),
            admission.policy_identity(),
        ] {
            push_framed(&mut identity_bytes, value.as_bytes());
        }
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-scalar-plan/v1\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            identity,
            model_id: model_reference.model().ulid().to_string(),
            model_revision: model_reference.semantic_revision().get(),
            geometry_digest,
            mesh_digest,
            correspondence_digest,
            production_digest,
            field_id,
            cells,
        })
    }

    /// Execute solely from retained Plan state.
    pub fn run(&self) -> Result<ResolvedScalarEllipticCartesianSolution, Diagnostic> {
        self.admission.execute_scalar(&REFERENCE_LINEAR_SOLVER)
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    #[must_use]
    pub const fn model_revision(&self) -> u64 {
        self.model_revision
    }

    #[must_use]
    pub fn model_digest(&self) -> &str {
        self.admission.model_digest()
    }

    #[must_use]
    pub fn geometry_digest(&self) -> &str {
        &self.geometry_digest
    }

    #[must_use]
    pub fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }

    #[must_use]
    pub fn correspondence_digest(&self) -> &str {
        &self.correspondence_digest
    }

    #[must_use]
    pub fn production_digest(&self) -> &str {
        &self.production_digest
    }

    #[must_use]
    pub fn field_id(&self) -> &str {
        &self.field_id
    }

    #[must_use]
    pub const fn cells(&self) -> [usize; 2] {
        self.cells
    }

    #[must_use]
    pub fn spatial(&self) -> CommonSpatialPolicy {
        match self.admission.spatial {
            NativeSpatialPolicy::ScalarQ1 => CommonSpatialPolicy::Q1,
            NativeSpatialPolicy::ScalarTpfa => CommonSpatialPolicy::CellCenteredTpfa,
            NativeSpatialPolicy::StokesMiniP1(_) => {
                unreachable!("common scalar Plan cannot own Stokes policy")
            }
        }
    }

    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeCapability {
    ScalarElliptic,
    SteadyIncompressibleStokes,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum NativeSpatialPolicy {
    ScalarQ1,
    ScalarTpfa,
    StokesMiniP1(IncompressibleFlowScaleProfile2d),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct NativeLinearPolicy {
    solver: SolverPlan,
    provider: SolverProvider,
    capabilities: SolverCapabilities,
    execution: ExecutionProvider,
    workers: NonZeroUsize,
}

impl NativeLinearPolicy {
    pub(super) fn exact(
        solver: SolverPlan,
        backend: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        if solver.relative_tolerance().to_bits() == (-0.0_f64).to_bits()
            || solver.absolute_tolerance().to_bits() == (-0.0_f64).to_bits()
        {
            return Err(invalid(
                "linear policy contains signed-zero tolerance ambiguity",
            ));
        }
        let provider = backend.provider();
        provider.validate()?;
        SERIAL_EXECUTION_PROVIDER.validate()?;
        Ok(Self {
            solver,
            provider,
            capabilities: backend.capabilities(),
            execution: SERIAL_EXECUTION_PROVIDER,
            workers: NonZeroUsize::MIN,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum NativeMeshResources {
    Cartesian {
        geometry: CanonicalGeometryV1,
        mesh: CartesianMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    },
    ReferenceSimplicial {
        geometry: CanonicalGeometryV1,
        mesh: SimplicialMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    },
    GmshSimplicial {
        geometry: CanonicalGeometryV1,
        policy: eqiora_artifact::PlanarMeshQualityV1,
        provider_output: Box<[u8]>,
        mesh: SimplicialMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    },
}

/// Authenticated in-process owner of one exact common Geometry/Mesh occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthenticatedCommonMesh {
    resources: NativeMeshResources,
}

impl AuthenticatedCommonMesh {
    /// Authenticate and own one structured-Cartesian rectangle occurrence.
    pub fn structured_cartesian(
        geometry: CanonicalGeometryV1,
        mesh: CartesianMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let resources = NativeMeshResources::Cartesian {
            geometry,
            mesh,
            correspondence,
            production,
        };
        validate_cartesian_resources(&resources)?;
        Ok(Self { resources })
    }

    /// Authenticate and own one deterministic reference simplicial occurrence.
    pub fn planar_reference(
        geometry: CanonicalGeometryV1,
        mesh: SimplicialMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let resources = NativeMeshResources::ReferenceSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        };
        validate_simplicial_resources(&resources)?;
        Ok(Self { resources })
    }

    /// Re-import and own one exact bounded Gmsh 4.15.2 provider observation.
    pub fn gmsh_4152(
        geometry: CanonicalGeometryV1,
        policy: eqiora_artifact::PlanarMeshQualityV1,
        provider_output: Vec<u8>,
    ) -> Result<Self, Diagnostic> {
        let resources = derive_gmsh_resources(geometry, policy, provider_output)?;
        Ok(Self { resources })
    }
}

impl NativeMeshResources {
    fn geometry(&self) -> &CanonicalGeometryV1 {
        match self {
            Self::Cartesian { geometry, .. }
            | Self::ReferenceSimplicial { geometry, .. }
            | Self::GmshSimplicial { geometry, .. } => geometry,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct NativeNumericalAdmission {
    model: ModelEnvelope,
    model_digest: String,
    program: KernelProgram,
    capability: NativeCapability,
    recognized: RecognizedNativeModel,
    resources: NativeMeshResources,
    spatial: NativeSpatialPolicy,
    linear: NativeLinearPolicy,
    policy_identity: String,
}

#[derive(Debug, Clone, PartialEq)]
enum RecognizedNativeModel {
    Scalar(Box<ScalarEllipticCartesianModel>),
    Stokes(Box<SteadyStokesGeometryBinding2d>),
}

struct RecognizedNativeAdmission {
    model: ModelEnvelope,
    model_digest: String,
    program: KernelProgram,
    capability: NativeCapability,
    recognized: RecognizedNativeModel,
    resources: NativeMeshResources,
}

impl RecognizedNativeAdmission {
    fn recognize(
        model: &ModelEnvelope,
        owner: AuthenticatedCommonMesh,
    ) -> Result<Self, Diagnostic> {
        let resources = owner.resources;
        let program = replay_program(model, resources.geometry())?;
        let capability = recognize_capability(&program)?;
        let recognized = recognize_exact_model(capability, &program, &resources)?;
        let model_digest = model.digest()?.to_string();
        Ok(Self {
            model: model.clone(),
            model_digest,
            program,
            capability,
            recognized,
            resources,
        })
    }

    fn complete(
        self,
        spatial: NativeSpatialPolicy,
        linear: NativeLinearPolicy,
    ) -> Result<NativeNumericalAdmission, Diagnostic> {
        require_policy_compatibility(self.capability, spatial, &linear)?;
        validate_resources(self.capability, spatial, &self.resources)?;
        let policy_identity = policy_identity(spatial, &linear);
        Ok(NativeNumericalAdmission {
            model: self.model,
            model_digest: self.model_digest,
            program: self.program,
            capability: self.capability,
            recognized: self.recognized,
            resources: self.resources,
            spatial,
            linear,
            policy_identity,
        })
    }
}

impl NativeNumericalAdmission {
    pub(super) fn admit(
        model: &ModelEnvelope,
        owner: AuthenticatedCommonMesh,
        spatial: NativeSpatialPolicy,
        linear: NativeLinearPolicy,
    ) -> Result<Self, Diagnostic> {
        RecognizedNativeAdmission::recognize(model, owner)?.complete(spatial, linear)
    }

    pub(super) fn revalidate(&self) -> Result<(), Diagnostic> {
        let replayed = Self::admit(
            &self.model,
            AuthenticatedCommonMesh {
                resources: self.resources.clone(),
            },
            self.spatial,
            self.linear.clone(),
        )?;
        if &replayed != self {
            return Err(invalid(
                "native numerical admission changed during exact internal replay",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) const fn model(&self) -> &ModelEnvelope {
        &self.model
    }

    pub(super) fn model_digest(&self) -> &str {
        &self.model_digest
    }

    pub(super) fn policy_identity(&self) -> &str {
        &self.policy_identity
    }

    #[cfg(test)]
    pub(super) const fn capability(&self) -> NativeCapability {
        self.capability
    }

    pub(super) const fn resources(&self) -> &NativeMeshResources {
        &self.resources
    }

    pub(super) fn stokes_binding(&self) -> Result<SteadyStokesGeometryBinding2d, Diagnostic> {
        let RecognizedNativeModel::Stokes(binding) = &self.recognized else {
            return Err(invalid(
                "native numerical admission does not own recognized steady-Stokes meaning",
            ));
        };
        Ok((**binding).clone())
    }

    pub(super) fn resolve_stokes(
        &self,
        binding: &SteadyStokesGeometryBinding2d,
    ) -> Result<
        (
            ResolvedFieldwiseRealization,
            RealizationEnvelopeV2,
            Space,
            Space,
        ),
        Diagnostic,
    > {
        let NativeSpatialPolicy::StokesMiniP1(scales) = self.spatial else {
            return Err(invalid(
                "steady-Stokes admission has a non-Stokes spatial policy",
            ));
        };
        let (NativeMeshResources::ReferenceSimplicial { mesh, .. }
        | NativeMeshResources::GmshSimplicial { mesh, .. }) = &self.resources
        else {
            return Err(invalid(
                "steady Stokes requires exact supplied simplicial resources",
            ));
        };
        let solver = self.linear.solver;
        let fieldwise = binding.mini_plan(mesh.artifact_reference()?, scales, solver)?;
        let selected_solver = SolverCapabilities::exact([SolverCapability {
            algorithm: solver.algorithm(),
            operator_properties: LinearOperatorProperties::SymmetricIndefinite,
            preconditioner: solver.preconditioner(),
            reduction: solver.reduction(),
            scalar_type: ScalarType::F64,
        }])?;
        let capabilities = RealizationCapabilities::cartesian_product(
            [DiscretizationMethod::ContinuousGalerkin],
            [(
                MeshKind::ImportedAffineSimplicial,
                SpatialDimensionSupport::exact(NonZeroUsize::new(2).expect("two is nonzero")),
            )],
            [VectorLayoutKind::Replicated],
            selected_solver,
            TargetCapabilities::none().with_host_cpu(self.linear.workers),
        )?;
        let resolved = resolve_fieldwise(
            &FieldwiseRealizationRequest::explicit(
                self.program.model(),
                SemanticRevision::new(self.program.revision().0),
                RealizationRevision::new(APPLICATION_REALIZATION_REVISION),
                fieldwise,
            ),
            binding.fieldwise_requirements(),
            &capabilities,
        )?;
        let realization = RealizationEnvelopeV2::from_resolved(
            &self.model,
            &resolved,
            eqiora_artifact::LayoutArtifacts::Replicated,
        )?;
        let mut velocity = None;
        let mut pressure = None;
        for field in resolved.plan().spatial().field_spaces() {
            match field.space().family() {
                SpaceFamily::SimplexP1Bubble if velocity.replace(field.space()).is_none() => {}
                SpaceFamily::ContinuousLagrange { order }
                    if order == std::num::NonZeroU16::MIN
                        && pressure.replace(field.space()).is_none() => {}
                _ => {
                    return Err(invalid(
                        "steady-Stokes resolved space inventory is not MINI/P1",
                    ));
                }
            }
        }
        let (velocity, pressure) = velocity
            .zip(pressure)
            .ok_or_else(|| invalid("steady-Stokes resolved space inventory is incomplete"))?;
        Ok((resolved, realization, velocity, pressure))
    }

    pub(super) fn execute_scalar(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<ResolvedScalarEllipticCartesianSolution, Diagnostic> {
        self.revalidate()?;
        if backend.provider() != self.linear.provider
            || backend.capabilities() != self.linear.capabilities
        {
            return Err(invalid(
                "scalar execution backend differs from admitted provider or capabilities",
            ));
        }
        let NativeMeshResources::Cartesian { mesh, .. } = &self.resources else {
            return Err(invalid(
                "scalar elliptic execution requires Cartesian resources",
            ));
        };
        let RecognizedNativeModel::Scalar(lowered) = &self.recognized else {
            return Err(invalid(
                "native numerical admission does not own recognized scalar-elliptic meaning",
            ));
        };
        let source =
            |coordinates: &[f64]| lowered.source().evaluate(coordinates).unwrap_or(f64::NAN);
        let boundary = |coordinates: &[f64]| {
            lowered
                .essential_boundary_jvp(
                    coordinates,
                    &vec![0.0; coordinates.len()],
                    &vec![0.0; lowered.parameter_fields().len()],
                )
                .map(|value| value.0)
                .unwrap_or(f64::NAN)
        };
        let solve = LinearSolveRequest::new(backend, self.linear.solver);
        match self.spatial {
            NativeSpatialPolicy::ScalarQ1 => {
                let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2)?;
                solve_scalar_elliptic_cartesian_fem(
                    mesh.mesh(),
                    lowered.coefficient(),
                    &source,
                    &boundary,
                    &quadrature,
                    solve,
                )
                .map(ResolvedScalarEllipticCartesianSolution::FiniteElement)
            }
            NativeSpatialPolicy::ScalarTpfa => {
                let cell = QuadratureRule::tensor_product_gauss_legendre(2, 1)?;
                let facet = QuadratureRule::gauss_legendre(1)?;
                solve_scalar_elliptic_cartesian_fvm(
                    mesh.mesh(),
                    lowered.coefficient(),
                    &source,
                    &boundary,
                    &cell,
                    &facet,
                    solve,
                )
                .map(ResolvedScalarEllipticCartesianSolution::FiniteVolume)
            }
            NativeSpatialPolicy::StokesMiniP1(_) => Err(invalid(
                "scalar execution received a steady-Stokes spatial policy",
            )),
        }
    }
}

fn resource_digests(
    resources: &NativeMeshResources,
) -> Result<(String, String, String, String), Diagnostic> {
    let (geometry, mesh, correspondence, production) = match resources {
        NativeMeshResources::Cartesian {
            geometry,
            mesh,
            correspondence,
            production,
        } => (
            geometry,
            mesh.digest()?,
            correspondence.digest()?,
            production.digest()?,
        ),
        NativeMeshResources::ReferenceSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        }
        | NativeMeshResources::GmshSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
            ..
        } => (
            geometry,
            mesh.digest()?,
            correspondence.digest()?,
            production.digest()?,
        ),
    };
    Ok((
        hex_bytes(&geometry.digest_bytes()),
        mesh.to_string(),
        correspondence.to_string(),
        production.to_string(),
    ))
}

fn recognize_capability(program: &KernelProgram) -> Result<NativeCapability, Diagnostic> {
    let scalar = recognize_scalar_elliptic_geometry_mathematics(program);
    let stokes = recognize_steady_incompressible_stokes_geometry_mathematics(program);
    match (scalar, stokes) {
        (Ok(()), Err(_)) => Ok(NativeCapability::ScalarElliptic),
        (Err(_), Ok(())) => Ok(NativeCapability::SteadyIncompressibleStokes),
        (Ok(()), Ok(())) => Err(invalid(
            "Model mathematical meaning is ambiguous across native capabilities",
        )),
        (Err(scalar), Err(stokes)) => Err(invalid(format!(
            "Model mathematical meaning matches no native capability: scalar [{}: {}]; Stokes [{}: {}]",
            scalar.code(),
            scalar.message(),
            stokes.code(),
            stokes.message(),
        ))),
    }
}

fn recognize_exact_model(
    capability: NativeCapability,
    program: &KernelProgram,
    resources: &NativeMeshResources,
) -> Result<RecognizedNativeModel, Diagnostic> {
    match (capability, resources) {
        (
            NativeCapability::ScalarElliptic,
            NativeMeshResources::Cartesian {
                geometry,
                mesh,
                correspondence,
                ..
            },
        ) => {
            lower_scalar_elliptic_cartesian_with_resources(program, geometry, mesh, correspondence)
                .map(Box::new)
                .map(RecognizedNativeModel::Scalar)
        }
        (
            NativeCapability::SteadyIncompressibleStokes,
            NativeMeshResources::ReferenceSimplicial {
                geometry,
                mesh,
                correspondence,
                ..
            }
            | NativeMeshResources::GmshSimplicial {
                geometry,
                mesh,
                correspondence,
                ..
            },
        ) => SteadyStokesGeometryBinding2d::new_authenticated(
            program,
            geometry,
            mesh,
            correspondence,
        )
        .map(Box::new)
        .map(RecognizedNativeModel::Stokes),
        _ => Err(invalid(
            "recognized Model capability and authenticated common Mesh kind are cross-wired",
        )),
    }
}

fn require_policy_compatibility(
    capability: NativeCapability,
    spatial: NativeSpatialPolicy,
    linear: &NativeLinearPolicy,
) -> Result<(), Diagnostic> {
    let (algorithm, properties, preconditioner, reduction) = match (capability, spatial) {
        (
            NativeCapability::ScalarElliptic,
            NativeSpatialPolicy::ScalarQ1 | NativeSpatialPolicy::ScalarTpfa,
        ) => (
            LinearSolver::ConjugateGradient,
            LinearOperatorProperties::SymmetricPositiveDefinite,
            PreconditionerPolicy::Identity,
            ReductionPolicy::Reproducible,
        ),
        (NativeCapability::SteadyIncompressibleStokes, NativeSpatialPolicy::StokesMiniP1(_)) => (
            LinearSolver::SparseLu,
            LinearOperatorProperties::SymmetricIndefinite,
            PreconditionerPolicy::Identity,
            ReductionPolicy::Fast,
        ),
        _ => {
            return Err(invalid(
                "Model capability and spatial policy are cross-wired",
            ));
        }
    };
    if linear.solver.algorithm() != algorithm
        || linear.solver.preconditioner() != preconditioner
        || linear.solver.reduction() != reduction
        || linear.execution != SERIAL_EXECUTION_PROVIDER
        || linear.workers != NonZeroUsize::MIN
    {
        return Err(invalid(
            "linear solver, preconditioner, reduction, or placement is unsupported",
        ));
    }
    linear
        .capabilities
        .require_problem(linear.solver, ScalarType::F64, properties)
}

fn validate_resources(
    capability: NativeCapability,
    spatial: NativeSpatialPolicy,
    resources: &NativeMeshResources,
) -> Result<(), Diagnostic> {
    match (capability, spatial, resources) {
        (
            NativeCapability::ScalarElliptic,
            NativeSpatialPolicy::ScalarQ1 | NativeSpatialPolicy::ScalarTpfa,
            resources @ NativeMeshResources::Cartesian { .. },
        ) => validate_cartesian_resources(resources),
        (
            NativeCapability::SteadyIncompressibleStokes,
            NativeSpatialPolicy::StokesMiniP1(_),
            resources @ (NativeMeshResources::ReferenceSimplicial { .. }
            | NativeMeshResources::GmshSimplicial { .. }),
        ) => validate_simplicial_resources(resources),
        _ => Err(invalid(
            "Model capability, spatial policy, and common Mesh kind are cross-wired",
        )),
    }
}

fn validate_cartesian_resources(resources: &NativeMeshResources) -> Result<(), Diagnostic> {
    let NativeMeshResources::Cartesian {
        geometry,
        mesh,
        correspondence,
        production,
    } = resources
    else {
        return Err(invalid("authenticated owner is not Cartesian"));
    };
    let policy = production
        .cartesian_cells()
        .ok_or_else(|| invalid("Cartesian resource has a non-Cartesian policy"))?;
    correspondence.validate_against_planar_rectangle_v2_cartesian(
        geometry,
        mesh,
        policy.cells(),
    )?;
    production.validate_against_structured_cartesian_v1_resources(
        policy,
        geometry,
        mesh,
        correspondence,
    )?;
    let [nx, ny] = policy.cells();
    if mesh.dimension() != 2
        || mesh.mesh().axis_cell_count(0) != Some(nx)
        || mesh.mesh().axis_cell_count(1) != Some(ny)
    {
        return Err(invalid(
            "Cartesian Mesh topology differs from its exact production policy",
        ));
    }
    Ok(())
}

fn validate_simplicial_resources(resources: &NativeMeshResources) -> Result<(), Diagnostic> {
    match resources {
        NativeMeshResources::ReferenceSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        } => {
            let policy = production.planar_mesh_quality().ok_or_else(|| {
                invalid("reference simplicial resource has a non-planar production policy")
            })?;
            correspondence.validate_against_planar_circular_hole_v2_reference(
                geometry,
                mesh,
                policy.maximum_boundary_error_m(),
                policy.maximum_boundary_facets(),
            )?;
            production.validate_against_planar_circular_hole_reference_v1_resources(
                policy,
                geometry,
                mesh,
                correspondence,
            )?;
        }
        NativeMeshResources::GmshSimplicial {
            geometry,
            policy,
            provider_output,
            ..
        } => {
            let replayed =
                derive_gmsh_resources(geometry.clone(), *policy, provider_output.to_vec())?;
            if &replayed != resources {
                return Err(invalid(
                    "Gmsh common Mesh resources differ from exact provider-output replay",
                ));
            }
        }
        NativeMeshResources::Cartesian { .. } => {
            return Err(invalid("authenticated owner is not simplicial"));
        }
    }
    let mesh = match resources {
        NativeMeshResources::ReferenceSimplicial { mesh, .. }
        | NativeMeshResources::GmshSimplicial { mesh, .. } => mesh,
        NativeMeshResources::Cartesian { .. } => unreachable!("rejected above"),
    };
    if mesh.dimension() != 2 {
        return Err(invalid(
            "steady Stokes requires a two-dimensional common Mesh",
        ));
    }
    Ok(())
}

fn derive_gmsh_resources(
    geometry: CanonicalGeometryV1,
    policy: eqiora_artifact::PlanarMeshQualityV1,
    provider_output: Vec<u8>,
) -> Result<NativeMeshResources, Diagnostic> {
    CanonicalGeometryV1::decode_planar_circular_hole_v2_canonical(
        geometry.canonical_bytes(),
        eqiora_geometry::CanonicalGeometryLimits::default(),
    )
    .map_err(|_| invalid("Gmsh provider observation requires exact planar circular-hole v2"))?;
    let quality = eqiora_meshing::MeshQualityGate::new(policy.minimum_mean_ratio())?;
    let importer = GmshSimplexImporter::new(2, quality, GmshImportLimits::default())?;
    let imported = importer.import_ascii_bytes_with_entities(&provider_output)?;
    let (tagged_facets, tagged_cells) = derive_entity_assignments(&imported)?;
    let expected_tags = [1_u32, 5, 6, 7, 8]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    if tagged_facets
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        != expected_tags
    {
        return Err(invalid(
            "Gmsh provider observation has a foreign boundary entity-tag inventory",
        ));
    }
    if tagged_cells.keys().copied().collect::<Vec<_>>() != [1] {
        return Err(invalid(
            "Gmsh provider observation has a foreign source-face entity-tag inventory",
        ));
    }
    let mut source_edge_facets: [Vec<usize>; 5] = std::array::from_fn(|_| Vec::new());
    for (tag, source_edge) in [(1_u32, 4_usize), (5, 2), (6, 1), (7, 3), (8, 0)] {
        source_edge_facets[source_edge] = tagged_facets
            .get(&tag)
            .expect("exact tag inventory checked")
            .clone();
    }
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(imported.mesh())?;
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_mesh_assignments(
            &geometry,
            &mesh,
            source_edge_facets,
        )?;
    let production = MeshProductionLineageEnvelopeV1::from_gmsh_4152_resources(
        policy,
        &geometry,
        &mesh,
        &correspondence,
    )?;
    Ok(NativeMeshResources::GmshSimplicial {
        geometry,
        policy,
        provider_output: provider_output.into_boxed_slice(),
        mesh,
        correspondence,
        production,
    })
}

fn derive_entity_assignments(
    imported: &GmshSimplicialImport,
) -> Result<TaggedMeshAssignments, Diagnostic> {
    let mesh = imported.mesh();
    let dimension = mesh.topological_dimension();
    let facet_dimension = dimension
        .checked_sub(1)
        .ok_or_else(|| invalid("Gmsh simplex Mesh has no boundary stratum"))?;
    let facet_count = mesh
        .entity_count(facet_dimension)
        .ok_or_else(|| invalid("Gmsh simplex Mesh omitted its facet stratum"))?;
    let mut facet_by_vertices = BTreeMap::new();
    let mut boundary_facets = BTreeSet::new();
    for facet_index in 0..facet_count {
        let facet = MeshEntity::new(facet_dimension, facet_index);
        let mut vertices = mesh
            .entity_vertices(facet)
            .ok_or_else(|| invalid("Gmsh Mesh facet omitted its vertex closure"))?
            .into_iter()
            .map(MeshEntity::index)
            .collect::<Vec<_>>();
        vertices.sort_unstable();
        if facet_by_vertices.insert(vertices, facet_index).is_some() {
            return Err(invalid(
                "Gmsh Mesh has duplicate canonical facet connectivity",
            ));
        }
        let parents = mesh
            .incidence(facet, dimension)
            .ok_or_else(|| invalid("Gmsh Mesh facet omitted parent incidence"))?;
        if parents.len() == 1 {
            boundary_facets.insert(facet_index);
        }
    }
    let mut tagged_facets: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut assigned_facets = BTreeSet::new();
    for block in imported
        .element_blocks()
        .iter()
        .filter(|block| block.dimension() == facet_dimension)
    {
        let facets = tagged_facets.entry(block.entity_tag()).or_default();
        for element in block.elements() {
            let mut vertices = element.clone();
            vertices.sort_unstable();
            let facet = *facet_by_vertices
                .get(&vertices)
                .ok_or_else(|| invalid("Gmsh boundary element is absent from Mesh topology"))?;
            if !boundary_facets.contains(&facet) || !assigned_facets.insert(facet) {
                return Err(invalid(
                    "Gmsh boundary assignment is interior or duplicated",
                ));
            }
            facets.push(facet);
        }
    }
    for facets in tagged_facets.values_mut() {
        facets.sort_unstable();
    }
    if assigned_facets != boundary_facets {
        return Err(invalid(
            "Gmsh entity blocks do not assign every Mesh boundary facet",
        ));
    }

    let mut cell_by_vertices = BTreeMap::new();
    for (cell_index, cell) in mesh.cells().iter().enumerate() {
        let mut vertices = cell.clone();
        vertices.sort_unstable();
        if cell_by_vertices.insert(vertices, cell_index).is_some() {
            return Err(invalid("Gmsh Mesh has duplicate canonical cells"));
        }
    }
    let mut tagged_cells: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut assigned_cells = BTreeSet::new();
    for block in imported
        .element_blocks()
        .iter()
        .filter(|block| block.dimension() == dimension)
    {
        let cells = tagged_cells.entry(block.entity_tag()).or_default();
        for element in block.elements() {
            let mut vertices = element.clone();
            vertices.sort_unstable();
            let cell = *cell_by_vertices
                .get(&vertices)
                .ok_or_else(|| invalid("Gmsh top element is absent from Mesh topology"))?;
            if !assigned_cells.insert(cell) {
                return Err(invalid("Gmsh top cell assignment is duplicated"));
            }
            cells.push(cell);
        }
    }
    for cells in tagged_cells.values_mut() {
        cells.sort_unstable();
    }
    if assigned_cells != (0..mesh.cells().len()).collect() {
        return Err(invalid(
            "Gmsh entity blocks do not assign every Mesh top cell",
        ));
    }
    Ok((tagged_facets, tagged_cells))
}

fn replay_program(
    model: &ModelEnvelope,
    geometry: &CanonicalGeometryV1,
) -> Result<KernelProgram, Diagnostic> {
    let reference = model.artifact_reference()?;
    let (transaction, model_id) = model.to_transaction().map_err(first)?;
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).map_err(first)?;
    let program =
        KernelProgram::from_snapshot_with_geometry(&store.snapshot(), model_id, &[geometry.into()])
            .map_err(first)?;
    if program.model() != reference.model()
        || program.revision().0 != reference.semantic_revision().get()
    {
        return Err(invalid(
            "replayed Model identity differs from exact caller Model",
        ));
    }
    Ok(program)
}

fn policy_identity(spatial: NativeSpatialPolicy, linear: &NativeLinearPolicy) -> String {
    let mut bytes = Vec::new();
    match spatial {
        NativeSpatialPolicy::ScalarQ1 => bytes.extend_from_slice(b"scalar-q1"),
        NativeSpatialPolicy::ScalarTpfa => bytes.extend_from_slice(b"scalar-tpfa"),
        NativeSpatialPolicy::StokesMiniP1(scales) => {
            bytes.extend_from_slice(b"stokes-mini-p1");
            bytes.extend_from_slice(&scales.length().value().to_bits().to_be_bytes());
            bytes.extend_from_slice(&scales.velocity().value().to_bits().to_be_bytes());
            bytes.extend_from_slice(&scales.pressure().value().to_bits().to_be_bytes());
        }
    }
    bytes.extend_from_slice(format!("{:?}", linear.solver.algorithm()).as_bytes());
    bytes.extend_from_slice(format!("{:?}", linear.solver.preconditioner()).as_bytes());
    bytes.extend_from_slice(format!("{:?}", linear.solver.reduction()).as_bytes());
    bytes.extend_from_slice(&linear.solver.relative_tolerance().to_bits().to_be_bytes());
    bytes.extend_from_slice(&linear.solver.absolute_tolerance().to_bits().to_be_bytes());
    bytes.extend_from_slice(&linear.solver.maximum_iterations().get().to_be_bytes());
    push_framed(&mut bytes, linear.provider.id().as_str().as_bytes());
    push_framed(
        &mut bytes,
        linear.provider.implementation_version().as_bytes(),
    );
    for library in linear.provider.libraries() {
        push_framed(&mut bytes, library.name().as_bytes());
        push_framed(&mut bytes, library.version().as_bytes());
    }
    push_framed(&mut bytes, linear.execution.id().as_str().as_bytes());
    push_framed(
        &mut bytes,
        linear.execution.implementation_version().as_bytes(),
    );
    bytes.extend_from_slice(&linear.workers.get().to_be_bytes());
    let digest = Sha256::digest([POLICY_DOMAIN, bytes.as_slice()].concat());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn push_framed(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&value.len().to_be_bytes());
    target.extend_from_slice(value);
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn first(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .next()
        .unwrap_or_else(|| invalid("Model replay failed without a diagnostic"))
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use eqiora_artifact::{
        CartesianMeshCellsV1, GeometryDecoderLimits, GeometryMeshCorrespondenceEnvelopeV1,
        MeshProductionLineageEnvelopeV1, PlanarMeshQualityV1,
    };
    use eqiora_core::{DimExponents, DynQuantity};
    use eqiora_geometry::{
        CadAuthoredGraph, CanonicalGeometryRef, ConstrainedRectangleV1, NamedEntitySet,
        PlanarOperationGraph, PlanarTopologyHandle,
    };
    use eqiora_meshing::{CartesianMesh, MeshEntity, MeshQualityGate};
    use eqiora_solver::{
        BackendId, LinearProblem, LinearSolution, REFERENCE_LINEAR_SOLVER,
        ReplicatedLinearExecution, SolverPlan,
    };

    use super::*;
    use eqiora_compiler::CompiledModel;

    const COMPONENT: &str = r#"
public component PoissonRectangle {
  public support region: volume(ambient_dimension = 2);
  public support left: boundary(parent = region);
  public support right: boundary(parent = region);
  public support bottom: boundary(parent = region);
  public support top: boundary(parent = region);
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  representation space = continuum;
  field potential on region as space: 1 = 0;
  relation balance continuous on region {
    -div(grad(potential))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation left_value continuous on left { trace(potential) = 0; }
  relation right_value continuous on right { trace(potential) = 0; }
  relation bottom_value continuous on bottom { trace(potential) = 0; }
  relation top_value continuous on top { trace(potential) = 0; }
}
"#;

    const STOKES_COMPONENT: &str =
        include_str!("../../eqiora-api/src/steady_stokes/accepted_component.eqi");

    type SupportBinding<'a> = (
        &'a str,
        &'a NamedEntitySet,
        Option<(&'a str, &'a NamedEntitySet)>,
    );

    fn compile_model(
        filename: &str,
        source: &str,
        geometry: &CanonicalGeometryV1,
        model: &str,
        component: &str,
        supports: &[SupportBinding<'_>],
        parameters: &[(&str, DynQuantity)],
    ) -> ModelEnvelope {
        let compiled = CompiledModel::compile_external_component(
            filename,
            source,
            model,
            component,
            CanonicalGeometryRef::from(geometry),
            supports,
            parameters,
        )
        .unwrap();
        let (transaction, model, _) = compiled.into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot_with_geometry(
            &store.snapshot(),
            model,
            &[CanonicalGeometryRef::from(geometry)],
        )
        .unwrap();
        ModelEnvelope::from_program(&program).unwrap()
    }

    #[derive(Debug)]
    struct ResolveOnlyBackend;

    impl LinearSolverBackend for ResolveOnlyBackend {
        fn provider(&self) -> SolverProvider {
            SolverProvider::new(BackendId::new("eqiora.test-resolve-only"), "1", &[])
        }

        fn capabilities(&self) -> SolverCapabilities {
            SolverCapabilities::exact([SolverCapability {
                algorithm: LinearSolver::SparseLu,
                operator_properties: LinearOperatorProperties::SymmetricIndefinite,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Fast,
                scalar_type: ScalarType::F64,
            }])
            .unwrap()
        }

        fn solve_with_execution(
            &self,
            _problem: &LinearProblem<'_>,
            _plan: SolverPlan,
            _execution: &dyn ReplicatedLinearExecution,
        ) -> Result<LinearSolution, Diagnostic> {
            unreachable!("resolution test must not execute")
        }
    }

    #[derive(Debug)]
    struct AlternateScalarBackend;

    impl LinearSolverBackend for AlternateScalarBackend {
        fn provider(&self) -> SolverProvider {
            SolverProvider::new(BackendId::new("eqiora.test-alternate-scalar"), "1", &[])
        }

        fn capabilities(&self) -> SolverCapabilities {
            REFERENCE_LINEAR_SOLVER.capabilities()
        }

        fn solve_with_execution(
            &self,
            _problem: &LinearProblem<'_>,
            _plan: SolverPlan,
            _execution: &dyn ReplicatedLinearExecution,
        ) -> Result<LinearSolution, Diagnostic> {
            unreachable!("provider mismatch must reject before execution")
        }
    }

    fn rectangle() -> CanonicalGeometryV1 {
        let graph = PlanarOperationGraph::new();
        let rectangle = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
        let edges = rectangle.boundaries();
        graph
            .build(
                &rectangle,
                &BTreeMap::from([
                    ("region".to_owned(), vec![rectangle.region().into()]),
                    (
                        "left".to_owned(),
                        vec![PlanarTopologyHandle::from(edges[0])],
                    ),
                    (
                        "right".to_owned(),
                        vec![PlanarTopologyHandle::from(edges[1])],
                    ),
                    (
                        "bottom".to_owned(),
                        vec![PlanarTopologyHandle::from(edges[2])],
                    ),
                    ("top".to_owned(), vec![PlanarTopologyHandle::from(edges[3])]),
                ]),
            )
            .unwrap()
    }

    fn model(geometry: &CanonicalGeometryV1) -> ModelEnvelope {
        scalar_model_from_source(geometry, COMPONENT)
    }

    fn scalar_model_from_source(geometry: &CanonicalGeometryV1, source: &str) -> ModelEnvelope {
        let region = geometry.entity_set("region").unwrap();
        let supports = [
            ("region", region, None),
            (
                "left",
                geometry.entity_set("left").unwrap(),
                Some(("region", region)),
            ),
            (
                "right",
                geometry.entity_set("right").unwrap(),
                Some(("region", region)),
            ),
            (
                "bottom",
                geometry.entity_set("bottom").unwrap(),
                Some(("region", region)),
            ),
            (
                "top",
                geometry.entity_set("top").unwrap(),
                Some(("region", region)),
            ),
        ];
        let parameters = [
            (
                "wave_number",
                DynQuantity::new(
                    std::f64::consts::PI,
                    DimExponents {
                        length: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "source_scale",
                DynQuantity::new(
                    2.0 * std::f64::consts::PI.powi(2),
                    DimExponents {
                        length: -2,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
        ];
        compile_model(
            "poisson-rectangle.eqi",
            source,
            geometry,
            "PoissonRectangleModel",
            "PoissonRectangle",
            &supports,
            &parameters,
        )
    }

    fn resources(geometry: &CanonicalGeometryV1) -> AuthenticatedCommonMesh {
        let cells = CartesianMeshCellsV1::new([2, 3]).unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                geometry,
                cells.cells(),
            )
            .unwrap();
        let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
            cells,
            geometry,
            &mesh,
            &correspondence,
        )
        .unwrap();
        AuthenticatedCommonMesh::structured_cartesian(
            geometry.clone(),
            mesh,
            correspondence,
            production,
        )
        .unwrap()
    }

    fn gmsh_provider_output(
        mesh: &SimplicialMeshEnvelopeV1,
        source_edge_facets: &[Vec<usize>; 5],
    ) -> Vec<u8> {
        let native = mesh.mesh();
        let vertex_count = native.vertices().len();
        let boundary_count = source_edge_facets.iter().map(Vec::len).sum::<usize>();
        let element_count = boundary_count + native.cells().len();
        let mut output = String::from("$MeshFormat\n4.1 0 8\n$EndMeshFormat\n$Nodes\n");
        writeln!(output, "1 {vertex_count} 1 {vertex_count}").unwrap();
        writeln!(output, "2 1 0 {vertex_count}").unwrap();
        for tag in 1..=vertex_count {
            writeln!(output, "{tag}").unwrap();
        }
        for coordinate in native.vertices() {
            writeln!(output, "{:?} {:?} 0", coordinate[0], coordinate[1]).unwrap();
        }
        output.push_str("$EndNodes\n$Elements\n");
        writeln!(output, "6 {element_count} 1 {element_count}").unwrap();
        let mut element_tag = 1;
        for (entity_tag, source_edge) in [(1, 4), (5, 2), (6, 1), (7, 3), (8, 0)] {
            let facets = &source_edge_facets[source_edge];
            writeln!(output, "1 {entity_tag} 1 {}", facets.len()).unwrap();
            for &facet_index in facets {
                let vertices = native
                    .entity_vertices(MeshEntity::new(1, facet_index))
                    .unwrap();
                writeln!(
                    output,
                    "{element_tag} {} {}",
                    vertices[0].index() + 1,
                    vertices[1].index() + 1,
                )
                .unwrap();
                element_tag += 1;
            }
        }
        writeln!(output, "2 1 2 {}", native.cells().len()).unwrap();
        for cell in native.cells() {
            writeln!(
                output,
                "{element_tag} {} {} {}",
                cell[0] + 1,
                cell[1] + 1,
                cell[2] + 1,
            )
            .unwrap();
            element_tag += 1;
        }
        output.push_str("$EndElements\n");
        assert_eq!(element_tag, element_count + 1);
        output.into_bytes()
    }

    fn linear() -> NativeLinearPolicy {
        NativeLinearPolicy::exact(
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-10,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap(),
            &REFERENCE_LINEAR_SOLVER,
        )
        .unwrap()
    }

    fn stokes_geometry() -> CanonicalGeometryV1 {
        let predecessor = CadAuthoredGraph::new(
            ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), 0.0).unwrap(),
            1.0,
            1.0e-10,
        )
        .unwrap();
        let end_cap = predecessor.face_handle("end-cap").unwrap();
        let x_lower = predecessor.face_handle("profile-x-lower").unwrap();
        let x_upper = predecessor.face_handle("profile-x-upper").unwrap();
        let y_lower = predecessor.face_handle("profile-y-lower").unwrap();
        let y_upper = predecessor.face_handle("profile-y-upper").unwrap();
        let graph = predecessor
            .circular_through_cut([0.2, 0.2], 0.05, 1.0e-10)
            .unwrap();
        let cut_wall = graph.face_handle("cut-wall").unwrap();
        graph
            .planar_result()
            .unwrap()
            .with_named_topology(&BTreeMap::from([
                ("fluid".to_owned(), vec![end_cap]),
                ("inlet".to_owned(), vec![x_lower]),
                ("outlet".to_owned(), vec![x_upper]),
                ("walls".to_owned(), vec![y_lower, y_upper]),
                ("cylinder".to_owned(), vec![cut_wall]),
            ]))
            .unwrap()
    }

    fn stokes_model(geometry: &CanonicalGeometryV1) -> ModelEnvelope {
        stokes_model_from_source(geometry, STOKES_COMPONENT)
    }

    fn stokes_model_from_source(geometry: &CanonicalGeometryV1, source: &str) -> ModelEnvelope {
        let fluid = geometry.entity_set("fluid").unwrap();
        let supports = [
            ("fluid", fluid, None),
            (
                "inlet",
                geometry.entity_set("inlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "outlet",
                geometry.entity_set("outlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "walls",
                geometry.entity_set("walls").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "cylinder",
                geometry.entity_set("cylinder").unwrap(),
                Some(("fluid", fluid)),
            ),
        ];
        let parameters = [
            (
                "dynamic_viscosity",
                DynQuantity::new(
                    0.001,
                    DimExponents {
                        mass: 1,
                        length: -1,
                        time: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "zero_pressure",
                DynQuantity::new(
                    0.0,
                    DimExponents {
                        mass: 1,
                        length: -1,
                        time: -2,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "inlet_speed",
                DynQuantity::new(
                    0.3,
                    DimExponents {
                        length: 1,
                        time: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "channel_height",
                DynQuantity::new(
                    0.41,
                    DimExponents {
                        length: 1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
        ];
        compile_model(
            "steady-flow-past-cylinder.eqi",
            source,
            geometry,
            "SteadyFlowPastCylinderModel",
            "SteadyFlowPastCylinder",
            &supports,
            &parameters,
        )
    }

    #[test]
    fn scalar_q1_and_tpfa_consume_one_exact_anisotropic_common_mesh() {
        let geometry = rectangle();
        let model = model(&geometry);
        let exact_owner = resources(&geometry);
        let caller_resources = exact_owner.resources.clone();
        let q1 = NativeNumericalAdmission::admit(
            &model,
            exact_owner.clone(),
            NativeSpatialPolicy::ScalarQ1,
            linear(),
        )
        .unwrap();
        let q1_repeat = NativeNumericalAdmission::admit(
            &model,
            resources(&geometry),
            NativeSpatialPolicy::ScalarQ1,
            linear(),
        )
        .unwrap();
        let tpfa = NativeNumericalAdmission::admit(
            &model,
            exact_owner,
            NativeSpatialPolicy::ScalarTpfa,
            linear(),
        )
        .unwrap();
        let alternate_provider = NativeNumericalAdmission::admit(
            &model,
            resources(&geometry),
            NativeSpatialPolicy::ScalarQ1,
            NativeLinearPolicy::exact(
                SolverPlan::new(
                    LinearSolver::ConjugateGradient,
                    1.0e-10,
                    1.0e-13,
                    NonZeroUsize::new(1000).unwrap(),
                )
                .unwrap(),
                &AlternateScalarBackend,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(q1.model(), &model);
        assert_eq!(q1.model_digest(), model.digest().unwrap().to_string());
        assert_ne!(q1.policy_identity(), tpfa.policy_identity());
        assert_eq!(q1.policy_identity(), q1_repeat.policy_identity());
        assert_ne!(q1.policy_identity(), alternate_provider.policy_identity());
        assert_eq!(q1.resources(), &caller_resources);
        assert_eq!(tpfa.resources(), &caller_resources);
        assert_eq!(q1.resources(), q1_repeat.resources());
        assert!(q1.execute_scalar(&AlternateScalarBackend).is_err());
        assert_eq!(
            q1.execute_scalar(&REFERENCE_LINEAR_SOLVER)
                .unwrap()
                .into_primary_field_values()
                .len(),
            12
        );
        assert_eq!(
            tpfa.execute_scalar(&REFERENCE_LINEAR_SOLVER)
                .unwrap()
                .into_primary_field_values()
                .len(),
            6
        );
    }

    #[test]
    fn common_scalar_plan_owns_exact_lineage_and_executes_without_repeated_inputs() {
        let geometry = rectangle();
        let model = model(&geometry);
        let linear = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(10_000).unwrap(),
        )
        .unwrap();
        let resolve_scalar = |spatial, solve| {
            resolve_common_plan(
                &model,
                resources(&geometry),
                spatial,
                solve,
                &ResolveOnlyBackend,
            )
            .unwrap()
            .project(
                |plan| plan,
                |_| panic!("scalar Model resolved as another capability"),
            )
        };
        let q1 = resolve_scalar(CommonSpatialPolicy::Q1, linear);
        let repeat = resolve_scalar(CommonSpatialPolicy::Q1, linear);
        let tpfa = resolve_scalar(CommonSpatialPolicy::CellCenteredTpfa, linear);
        let alternate_tolerance = resolve_scalar(
            CommonSpatialPolicy::Q1,
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-9,
                1.0e-12,
                NonZeroUsize::new(10_000).unwrap(),
            )
            .unwrap(),
        );

        assert_eq!(q1.identity(), repeat.identity());
        assert_ne!(q1.identity(), tpfa.identity());
        assert_ne!(q1.identity(), alternate_tolerance.identity());
        assert_eq!(q1.model_digest(), model.digest().unwrap().to_string());
        assert_eq!(q1.cells(), [2, 3]);
        assert_eq!(q1.run().unwrap().into_primary_field_values().len(), 12);
        assert_eq!(tpfa.run().unwrap().into_primary_field_values().len(), 6);
        assert!(
            resolve_common_plan(
                &model,
                resources(&geometry),
                CommonSpatialPolicy::Q1,
                SolverPlan::new(
                    LinearSolver::MinimumResidual,
                    1.0e-10,
                    1.0e-12,
                    NonZeroUsize::new(10_000).unwrap(),
                )
                .unwrap(),
                &ResolveOnlyBackend,
            )
            .is_err()
        );
        assert!(
            resolve_common_plan(
                &model,
                resources(&geometry),
                CommonSpatialPolicy::MiniP1,
                linear,
                &ResolveOnlyBackend,
            )
            .is_err()
        );
    }

    #[test]
    fn admission_rejects_policy_and_resource_cross_wires() {
        let geometry = rectangle();
        let model = model(&geometry);
        assert!(
            NativeLinearPolicy::exact(
                SolverPlan::new(
                    LinearSolver::ConjugateGradient,
                    -0.0,
                    1.0e-13,
                    NonZeroUsize::new(1000).unwrap(),
                )
                .unwrap(),
                &REFERENCE_LINEAR_SOLVER,
            )
            .is_err()
        );
        for solver in [
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-10,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap()
            .with_preconditioner(PreconditionerPolicy::Jacobi),
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-10,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap()
            .with_reduction(ReductionPolicy::Fast),
            SolverPlan::new(
                LinearSolver::BiConjugateGradientStabilized,
                1.0e-10,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap(),
        ] {
            assert!(
                NativeNumericalAdmission::admit(
                    &model,
                    resources(&geometry),
                    NativeSpatialPolicy::ScalarQ1,
                    NativeLinearPolicy::exact(solver, &REFERENCE_LINEAR_SOLVER).unwrap(),
                )
                .is_err()
            );
        }
        assert!(
            NativeNumericalAdmission::admit(
                &model,
                resources(&geometry),
                NativeSpatialPolicy::StokesMiniP1(
                    IncompressibleFlowScaleProfile2d::new(
                        DynQuantity::new(
                            1.0,
                            DimExponents {
                                length: 1,
                                ..DimExponents::DIMENSIONLESS
                            }
                        ),
                        DynQuantity::new(
                            1.0,
                            DimExponents {
                                length: 1,
                                time: -1,
                                ..DimExponents::DIMENSIONLESS
                            }
                        ),
                        DynQuantity::new(
                            1.0,
                            DimExponents {
                                mass: 1,
                                length: -1,
                                time: -2,
                                ..DimExponents::DIMENSIONLESS
                            }
                        ),
                    )
                    .unwrap(),
                ),
                linear(),
            )
            .is_err()
        );

        let owner = resources(&geometry);
        let NativeMeshResources::Cartesian {
            geometry: exact_geometry,
            mesh,
            correspondence,
            production,
        } = owner.resources
        else {
            unreachable!()
        };
        let substituted_mesh = CartesianMeshEnvelopeV1::from_mesh(
            &CartesianMesh::from_axes(vec![vec![0.0, 0.25, 1.0], vec![0.0, 0.2, 0.8, 1.0]])
                .unwrap(),
        )
        .unwrap();
        assert!(
            AuthenticatedCommonMesh::structured_cartesian(
                exact_geometry.clone(),
                substituted_mesh,
                correspondence.clone(),
                production.clone(),
            )
            .is_err()
        );
        let mut correspondence_value: serde_json::Value =
            serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
        let frontiers = correspondence_value["frontiers"].as_array_mut().unwrap();
        let left = frontiers[0]["facet_indices"].clone();
        let right = frontiers[1]["facet_indices"].clone();
        frontiers[0]["facet_indices"] = right;
        frontiers[1]["facet_indices"] = left;
        let relabelled = GeometryMeshCorrespondenceEnvelopeV1::from_json(
            &serde_json::to_vec(&correspondence_value).unwrap(),
            GeometryDecoderLimits::default(),
        )
        .unwrap();
        assert!(
            AuthenticatedCommonMesh::structured_cartesian(
                exact_geometry.clone(),
                mesh.clone(),
                relabelled,
                production.clone(),
            )
            .is_err()
        );
        let production_json = String::from_utf8(production.canonical_json().unwrap()).unwrap();
        let provider_mutation = production_json.replace(
            "\"identity\":\"eqiora.structured-cartesian\",\"version\":\"1\"",
            "\"identity\":\"eqiora.gmsh-cli\",\"version\":\"4.15.2\"",
        );
        assert!(MeshProductionLineageEnvelopeV1::from_json(provider_mutation.as_bytes()).is_err());
        let foreign_production_json = production_json.replace(
            &correspondence.digest().unwrap().to_string(),
            &"00".repeat(32),
        );
        let foreign_production =
            MeshProductionLineageEnvelopeV1::from_json(foreign_production_json.as_bytes()).unwrap();
        assert!(
            AuthenticatedCommonMesh::structured_cartesian(
                exact_geometry,
                mesh,
                correspondence,
                foreign_production,
            )
            .is_err()
        );

        let reaction_source = COMPONENT.replace(
            "-div(grad(potential))\n      - source_scale * sin(wave_number * coordinate(0))\n        * sin(wave_number * coordinate(1)) = 0;",
            "potential - 1 = 0;",
        );
        let reaction = scalar_model_from_source(&geometry, &reaction_source);
        let reaction_program = replay_program(&reaction, &geometry).unwrap();
        assert!(recognize_capability(&reaction_program).is_err());

        let stokes_geometry = stokes_geometry();
        let non_stokes_source =
            STOKES_COMPONENT.replace("div(velocity) = 0;", "pressure - zero_pressure = 0;");
        let non_stokes = stokes_model_from_source(&stokes_geometry, &non_stokes_source);
        let non_stokes_program = replay_program(&non_stokes, &stokes_geometry).unwrap();
        assert!(recognize_capability(&non_stokes_program).is_err());

        let foreign = rectangle();
        let mut foreign_resources = resources(&foreign);
        if let NativeMeshResources::Cartesian { geometry, .. } = &mut foreign_resources.resources {
            *geometry = {
                let graph = PlanarOperationGraph::new();
                let rectangle = graph.rectangle([0.0, 2.0], [0.0, 1.0]).unwrap();
                let edges = rectangle.boundaries();
                graph
                    .build(
                        &rectangle,
                        &BTreeMap::from([
                            ("region".to_owned(), vec![rectangle.region().into()]),
                            ("left".to_owned(), vec![edges[0].into()]),
                            ("right".to_owned(), vec![edges[1].into()]),
                            ("bottom".to_owned(), vec![edges[2].into()]),
                            ("top".to_owned(), vec![edges[3].into()]),
                        ]),
                    )
                    .unwrap()
            };
        }
        assert!(
            NativeNumericalAdmission::admit(
                &model,
                foreign_resources,
                NativeSpatialPolicy::ScalarQ1,
                linear(),
            )
            .is_err()
        );
    }

    #[test]
    fn stokes_resolution_consumes_exact_source_owned_common_mesh() {
        let geometry = stokes_geometry();
        let model = stokes_model(&geometry);
        let policy = PlanarMeshQualityV1::new(1.0e-4, 1.0e-5, 50).unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
                &geometry,
                policy.maximum_boundary_error_m(),
                policy.maximum_boundary_facets(),
                MeshQualityGate::new(policy.minimum_mean_ratio()).unwrap(),
            )
            .unwrap();
        let production =
            MeshProductionLineageEnvelopeV1::from_planar_circular_hole_reference_v1_resources(
                policy,
                &geometry,
                &mesh,
                &correspondence,
            )
            .unwrap();
        let correspondence_value: serde_json::Value =
            serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
        let frontiers = correspondence_value["frontiers"].as_array().unwrap();
        let assignment_proof: [Vec<usize>; 5] = std::array::from_fn(|edge| {
            frontiers[edge]["facet_indices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| usize::try_from(value.as_u64().unwrap()).unwrap())
                .collect()
        });
        let provider_output = gmsh_provider_output(&mesh, &assignment_proof);
        let exact_gmsh =
            AuthenticatedCommonMesh::gmsh_4152(geometry.clone(), policy, provider_output.clone())
                .unwrap();
        let mut relabelled_assignments = assignment_proof.clone();
        relabelled_assignments.swap(0, 1);
        let relabelled_output = gmsh_provider_output(&mesh, &relabelled_assignments);
        let relabelled_gmsh =
            AuthenticatedCommonMesh::gmsh_4152(geometry.clone(), policy, relabelled_output)
                .unwrap();
        assert_ne!(exact_gmsh, relabelled_gmsh);
        let NativeMeshResources::GmshSimplicial {
            correspondence: exact_correspondence,
            production: exact_production,
            ..
        } = &exact_gmsh.resources
        else {
            unreachable!("Gmsh factory returns Gmsh resources")
        };
        let NativeMeshResources::GmshSimplicial {
            correspondence: relabelled_correspondence,
            production: relabelled_production,
            ..
        } = &relabelled_gmsh.resources
        else {
            unreachable!("Gmsh factory returns Gmsh resources")
        };
        assert_ne!(exact_correspondence, relabelled_correspondence);
        assert_ne!(exact_production, relabelled_production);
        let malformed_output = provider_output
            .windows(b"1 5 1".len())
            .position(|window| window == b"1 5 1")
            .map(|offset| {
                let mut mutated = provider_output.clone();
                mutated[offset + 2] = b'9';
                mutated
            })
            .unwrap();
        assert!(
            AuthenticatedCommonMesh::gmsh_4152(geometry.clone(), policy, malformed_output).is_err()
        );
        let resources = AuthenticatedCommonMesh::planar_reference(
            geometry,
            mesh.clone(),
            correspondence,
            production,
        )
        .unwrap();
        let solver = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-6,
            1.0e-13,
            NonZeroUsize::new(10_000).unwrap(),
        )
        .unwrap();
        let common = resolve_common_plan(
            &model,
            resources.clone(),
            CommonSpatialPolicy::MiniP1,
            solver,
            &ResolveOnlyBackend,
        )
        .unwrap()
        .project(
            |_| panic!("steady-Stokes Model resolved as another capability"),
            |plan| plan,
        );
        assert_eq!(common.model_digest(), model.digest().unwrap().to_string());
        assert_eq!(common.mesh_digest(), mesh.digest().unwrap().to_string());
        assert_eq!(
            common.scales().length().value().to_bits(),
            0.41_f64.to_bits()
        );
        assert_eq!(
            common.scales().velocity().value().to_bits(),
            0.3_f64.to_bits()
        );
        assert_eq!(
            common.scales().pressure().value().to_bits(),
            (0.001_f64 * 0.3 / 0.41).to_bits()
        );
        assert_eq!(common.linear().algorithm(), LinearSolver::SparseLu);
        assert_eq!(common.linear().reduction(), ReductionPolicy::Fast);
        assert_eq!(
            common.linear().relative_tolerance(),
            solver.relative_tolerance()
        );
        assert_eq!(
            common.linear().absolute_tolerance(),
            solver.absolute_tolerance()
        );
        assert_eq!(
            common.linear().maximum_iterations(),
            solver.maximum_iterations()
        );
        let admission = NativeNumericalAdmission::admit(
            &model,
            resources,
            NativeSpatialPolicy::StokesMiniP1(common.scales()),
            NativeLinearPolicy::exact(common.linear(), &ResolveOnlyBackend).unwrap(),
        )
        .unwrap();
        assert_eq!(
            admission.capability(),
            NativeCapability::SteadyIncompressibleStokes
        );
        assert_eq!(admission.model(), &model);
        let binding = admission.stokes_binding().unwrap();
        let (_resolved, realization, velocity, pressure) =
            admission.resolve_stokes(&binding).unwrap();
        assert_eq!(
            realization.mesh_artifact().unwrap(),
            Some(mesh.digest().unwrap())
        );
        assert_eq!(velocity.family(), SpaceFamily::SimplexP1Bubble);
        assert!(matches!(
            pressure.family(),
            SpaceFamily::ContinuousLagrange { .. }
        ));
    }

    #[test]
    fn registered_model_driven_common_mesh_admission_evidence() {
        scalar_q1_and_tpfa_consume_one_exact_anisotropic_common_mesh();
        admission_rejects_policy_and_resource_cross_wires();
        stokes_resolution_consumes_exact_source_owned_common_mesh();
    }
}
