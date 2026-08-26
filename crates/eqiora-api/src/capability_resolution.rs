//! Private Model-driven admission for the currently executable steady spatial policies.

use std::num::NonZeroUsize;

use eqiora_artifact::{
    AcceptedCircularHoleChordalRealizationV1, ArtifactDigest, CartesianMeshEnvelopeV1,
    ModelEnvelope,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::CartesianMesh;
use eqiora_numerics::fluid::IncompressibleFlowScaleProfile2d;
use eqiora_numerics::scalar::lower_scalar_elliptic_cartesian;
use eqiora_realization::DiscretizationMethod;
use eqiora_solver::{
    LinearOperatorProperties, LinearSolver, ScalarType, SolverCapabilities, SolverPlan,
    SolverProvider,
};
#[cfg(test)]
use eqiora_solver::{PreconditionerPolicy, ReductionPolicy};

use crate::ModelDocument;
use crate::spatial::{ScalarEllipticIntent, ScalarEllipticMethod, resource_shape};
use crate::steady_stokes::{recognize_model_and_mesh, reference_intent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeSpatialPolicy {
    Q1 { cells_per_axis: NonZeroUsize },
    CellCenteredTpfa { cells_per_axis: NonZeroUsize },
    MiniP1,
}

impl NativeSpatialPolicy {
    pub(crate) const fn from_scalar_intent(intent: ScalarEllipticIntent) -> Self {
        match intent.method() {
            ScalarEllipticMethod::FiniteElement => Self::Q1 {
                cells_per_axis: intent.cells_per_axis(),
            },
            ScalarEllipticMethod::FiniteVolume => Self::CellCenteredTpfa {
                cells_per_axis: intent.cells_per_axis(),
            },
        }
    }

    const fn method(self) -> DiscretizationMethod {
        match self {
            Self::Q1 { .. } | Self::MiniP1 => DiscretizationMethod::ContinuousGalerkin,
            Self::CellCenteredTpfa { .. } => DiscretizationMethod::CellCenteredFiniteVolume,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum NativeScalingPolicy {
    None,
    Incompressible(IncompressibleFlowScaleProfile2d),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeMesh<'a> {
    Cartesian(&'a CartesianMeshEnvelopeV1),
    CircularHole(&'a AcceptedCircularHoleChordalRealizationV1),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativePlacement {
    HostCpu { workers: NonZeroUsize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecognizedCapability {
    ScalarElliptic,
    SteadyIncompressibleStokes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelAdmissionRoute {
    SelfContainedScalarCandidate,
    GeometryBackedCandidate,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NativeAdmission {
    model: ArtifactDigest,
    mesh: ArtifactDigest,
    capability: RecognizedCapability,
    spatial: NativeSpatialPolicy,
    method: DiscretizationMethod,
    scaling: NativeScalingPolicy,
    solver: SolverPlan,
    provider: SolverProvider,
    capabilities: SolverCapabilities,
    placement: NativePlacement,
}

impl NativeAdmission {
    #[cfg(test)]
    pub(crate) const fn spatial(&self) -> NativeSpatialPolicy {
        self.spatial
    }

    #[cfg(test)]
    pub(crate) const fn method(&self) -> DiscretizationMethod {
        self.method
    }

    #[cfg(test)]
    pub(crate) const fn solver(&self) -> SolverPlan {
        self.solver
    }

    #[cfg(test)]
    pub(crate) const fn scaling(&self) -> NativeScalingPolicy {
        self.scaling
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn admit(
    model: &ModelEnvelope,
    mesh: NativeMesh<'_>,
    spatial: NativeSpatialPolicy,
    scaling: NativeScalingPolicy,
    solver: SolverPlan,
    provider: SolverProvider,
    capabilities: SolverCapabilities,
    placement: NativePlacement,
) -> Result<NativeAdmission, Diagnostic> {
    let solver = canonical_solver(solver)?;
    let route = classify_model(model)?;

    let capability = match route {
        ModelAdmissionRoute::SelfContainedScalarCandidate => {
            recognize_scalar_model(model)?;
            let NativeMesh::Cartesian(mesh) = mesh else {
                return Err(incompatible(
                    "scalar Q1 and TPFA require an exact Cartesian Mesh",
                ));
            };
            let intent = scalar_intent(spatial, placement)?;
            validate_cartesian_mesh(model, mesh, intent)?;
            if scaling != NativeScalingPolicy::None {
                return Err(unsupported(
                    "scalar elliptic resolution does not accept a scaling policy",
                ));
            }
            if solver != scalar_solver()? {
                return Err(unsupported(
                    "scalar Q1 and TPFA admit only the existing CG/Identity/Reproducible policy",
                ));
            }
            capabilities.require_problem(
                solver,
                ScalarType::F64,
                LinearOperatorProperties::SymmetricPositiveDefinite,
            )?;
            RecognizedCapability::ScalarElliptic
        }
        ModelAdmissionRoute::GeometryBackedCandidate => {
            let NativeMesh::CircularHole(accepted) = mesh else {
                return Err(incompatible(
                    "geometry-backed Model admission requires its exact Geometry and affine-simplicial Mesh authority",
                ));
            };
            recognize_model_and_mesh(model, accepted)?;
            if spatial != NativeSpatialPolicy::MiniP1 {
                return Err(unsupported(
                    "steady Stokes admits only MINI/P1 on the accepted affine-simplicial Mesh",
                ));
            }
            let expected = reference_intent()?;
            if scaling != NativeScalingPolicy::Incompressible(expected.scales())
                || solver != expected.solver()
            {
                return Err(unsupported(
                    "steady Stokes admits only the existing scaling and SparseLU/Identity/Fast policy",
                ));
            }
            if placement
                != (NativePlacement::HostCpu {
                    workers: NonZeroUsize::MIN,
                })
            {
                return Err(unsupported(
                    "steady Stokes admits only serial host placement",
                ));
            }
            capabilities.require_problem(
                solver,
                ScalarType::F64,
                LinearOperatorProperties::SymmetricIndefinite,
            )?;
            RecognizedCapability::SteadyIncompressibleStokes
        }
    };
    let mesh_digest = match mesh {
        NativeMesh::Cartesian(mesh) => mesh.digest()?,
        NativeMesh::CircularHole(accepted) => accepted.mesh().digest()?,
    };

    Ok(NativeAdmission {
        model: model.digest()?,
        mesh: mesh_digest,
        capability,
        spatial,
        method: spatial.method(),
        scaling,
        solver,
        provider,
        capabilities,
        placement,
    })
}

fn classify_model(model: &ModelEnvelope) -> Result<ModelAdmissionRoute, Diagnostic> {
    if model.requires_geometry_admission()? {
        // Geometry ownership routes this closed initial matrix to its sole
        // geometry-backed recognizer. The Geometry does not itself select or
        // prove Stokes; the #522 recognizer still decides the Model meaning.
        Ok(ModelAdmissionRoute::GeometryBackedCandidate)
    } else {
        // Self-contained Models enter only the scalar recognizer. A malformed
        // or unsupported Model preserves that recognizer's diagnostic and is
        // never retried as another physical capability.
        Ok(ModelAdmissionRoute::SelfContainedScalarCandidate)
    }
}

fn recognize_scalar_model(model: &ModelEnvelope) -> Result<(), Diagnostic> {
    let bytes = model.canonical_json()?;
    let document = ModelDocument::replay(&bytes).map_err(first_diagnostic)?;
    lower_scalar_elliptic_cartesian(document.program()).map(|_| ())
}

fn validate_cartesian_mesh(
    model: &ModelEnvelope,
    mesh: &CartesianMeshEnvelopeV1,
    intent: ScalarEllipticIntent,
) -> Result<(), Diagnostic> {
    let bytes = model.canonical_json()?;
    let document = ModelDocument::replay(&bytes).map_err(first_diagnostic)?;
    let lowered = lower_scalar_elliptic_cartesian(document.program())?;
    let dimension = NonZeroUsize::new(lowered.dimension())
        .ok_or_else(|| unsupported("scalar elliptic Model has zero spatial dimension"))?;
    resource_shape(intent, dimension).map_err(first_diagnostic)?;
    let extents = vec![intent.cells_per_axis().get(); dimension.get()];
    let expected = CartesianMesh::uniform(lowered.bounds(), &extents)
        .and_then(|mesh| CartesianMeshEnvelopeV1::from_mesh(&mesh))?;
    if mesh != &expected {
        return Err(incompatible(
            "the supplied Cartesian Mesh does not exactly realize the Model and spatial policy",
        ));
    }
    Ok(())
}

fn scalar_intent(
    spatial: NativeSpatialPolicy,
    placement: NativePlacement,
) -> Result<ScalarEllipticIntent, Diagnostic> {
    let NativePlacement::HostCpu { workers } = placement;
    let (method, cells_per_axis) = match spatial {
        NativeSpatialPolicy::Q1 { cells_per_axis } => {
            (ScalarEllipticMethod::FiniteElement, cells_per_axis)
        }
        NativeSpatialPolicy::CellCenteredTpfa { cells_per_axis } => {
            (ScalarEllipticMethod::FiniteVolume, cells_per_axis)
        }
        NativeSpatialPolicy::MiniP1 => {
            return Err(unsupported(
                "MINI/P1 is incompatible with a scalar elliptic Model",
            ));
        }
    };
    Ok(ScalarEllipticIntent::new(
        eqiora_realization::RealizationRevision::new(1),
        method,
        cells_per_axis,
        workers,
    ))
}

pub(crate) fn scalar_solver() -> Result<SolverPlan, Diagnostic> {
    SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-10,
        1.0e-12,
        NonZeroUsize::new(10_000).expect("10,000 is non-zero"),
    )
}

fn canonical_solver(solver: SolverPlan) -> Result<SolverPlan, Diagnostic> {
    SolverPlan::new(
        solver.algorithm(),
        canonical_float(solver.relative_tolerance()),
        canonical_float(solver.absolute_tolerance()),
        solver.maximum_iterations(),
    )
    .map(|canonical| {
        canonical
            .with_preconditioner(solver.preconditioner())
            .with_reduction(solver.reduction())
    })
}

const fn canonical_float(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
pub(crate) const fn linear_solver_name(value: LinearSolver) -> &'static str {
    match value {
        LinearSolver::ConjugateGradient => "conjugate-gradient",
        LinearSolver::MinimumResidual => "minimum-residual",
        LinearSolver::BiConjugateGradientStabilized => "bicgstab",
        LinearSolver::SparseLu => "sparse-lu",
    }
}

#[cfg(test)]
pub(crate) const fn preconditioner_name(value: PreconditionerPolicy) -> &'static str {
    match value {
        PreconditionerPolicy::Identity => "identity",
        PreconditionerPolicy::Jacobi => "jacobi",
    }
}

#[cfg(test)]
pub(crate) const fn reduction_name(value: ReductionPolicy) -> &'static str {
    match value {
        ReductionPolicy::Reproducible => "reproducible",
        ReductionPolicy::Fast => "fast",
    }
}

fn first_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .next()
        .unwrap_or_else(|| unsupported("Model admission failed without a diagnostic"))
}

fn unsupported(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NOT_IMPLEMENTED, message)
}

fn incompatible(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_artifact::ModelDecoderLimits;
    use eqiora_numerics::scalar::lower_scalar_elliptic_cartesian;
    use eqiora_solver::{LinearSolverBackend, REFERENCE_LINEAR_SOLVER, REFERENCE_SOLVER_PROVIDER};

    use crate::steady_stokes::tests::{
        ACCEPTED_COMPONENT_SOURCE, ResolveOnlyBackend, accepted_realization, authored_model,
    };

    const SCALAR_SOURCE: &str = r#"
model native_policy_scalar {
  domain square = box(0, 1, 0, 1);
  domain left = boundary(square, axis = 0, side = lower);
  domain right = boundary(square, axis = 0, side = upper);
  domain bottom = boundary(square, axis = 1, side = lower);
  domain top = boundary(square, axis = 1, side = upper);
  representation scalar_space = continuum;
  field potential on square as scalar_space: 1 = 0;
  parameter source_scale: 1 / m ^ 2 = 1;
  relation balance continuous on square {
    -div(grad(potential)) - source_scale = 0;
  }
  relation left_value continuous on left { trace(potential) = 0; }
  relation right_value continuous on right { trace(potential) = 0; }
  relation bottom_value continuous on bottom { trace(potential) = 0; }
  relation top_value continuous on top { trace(potential) = 0; }
}
"#;

    fn scalar_model_and_mesh(cells: usize) -> (ModelEnvelope, CartesianMeshEnvelopeV1) {
        let document = ModelDocument::compile("native-policy-scalar.eqi", SCALAR_SOURCE).unwrap();
        let lowered = lower_scalar_elliptic_cartesian(document.program()).unwrap();
        let extents = vec![cells; lowered.dimension()];
        let mesh = CartesianMesh::uniform(lowered.bounds(), &extents).unwrap();
        (
            ModelEnvelope::from_program(document.program()).unwrap(),
            CartesianMeshEnvelopeV1::from_mesh(&mesh).unwrap(),
        )
    }

    fn admit_scalar(
        model: &ModelEnvelope,
        mesh: &CartesianMeshEnvelopeV1,
        spatial: NativeSpatialPolicy,
        solver: SolverPlan,
    ) -> Result<NativeAdmission, Diagnostic> {
        admit(
            model,
            NativeMesh::Cartesian(mesh),
            spatial,
            NativeScalingPolicy::None,
            solver,
            REFERENCE_SOLVER_PROVIDER,
            REFERENCE_LINEAR_SOLVER.capabilities(),
            NativePlacement::HostCpu {
                workers: NonZeroUsize::MIN,
            },
        )
    }

    fn admit_stokes(
        model: &ModelEnvelope,
        accepted: &AcceptedCircularHoleChordalRealizationV1,
        spatial: NativeSpatialPolicy,
        solver: SolverPlan,
    ) -> Result<NativeAdmission, Diagnostic> {
        let intent = reference_intent().unwrap();
        admit(
            model,
            NativeMesh::CircularHole(accepted),
            spatial,
            NativeScalingPolicy::Incompressible(intent.scales()),
            solver,
            ResolveOnlyBackend.provider(),
            ResolveOnlyBackend.capabilities(),
            NativePlacement::HostCpu {
                workers: NonZeroUsize::MIN,
            },
        )
    }

    #[test]
    fn solver_identity_normalizes_signed_zero_and_has_native_names() {
        let positive = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-8,
            0.0,
            NonZeroUsize::MIN,
        )
        .unwrap();
        let negative = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-8,
            -0.0,
            NonZeroUsize::MIN,
        )
        .unwrap();
        let positive = canonical_solver(positive).unwrap();
        let negative = canonical_solver(negative).unwrap();
        assert_eq!(positive, negative);
        assert_eq!(negative.absolute_tolerance().to_bits(), 0.0_f64.to_bits());
        assert_eq!(
            linear_solver_name(negative.algorithm()),
            "conjugate-gradient"
        );
        assert_eq!(preconditioner_name(negative.preconditioner()), "identity");
        assert_eq!(reduction_name(negative.reduction()), "reproducible");
    }

    #[test]
    fn one_native_seam_admits_q1_tpfa_and_model_driven_mini_p1() {
        let (scalar, mesh) = scalar_model_and_mesh(8);
        let q1 = admit_scalar(
            &scalar,
            &mesh,
            NativeSpatialPolicy::Q1 {
                cells_per_axis: NonZeroUsize::new(8).unwrap(),
            },
            scalar_solver().unwrap(),
        )
        .unwrap();
        let tpfa = admit_scalar(
            &scalar,
            &mesh,
            NativeSpatialPolicy::CellCenteredTpfa {
                cells_per_axis: NonZeroUsize::new(8).unwrap(),
            },
            scalar_solver().unwrap(),
        )
        .unwrap();

        let stokes_model =
            authored_model(ACCEPTED_COMPONENT_SOURCE, [1.0e-3, 0.0, 0.3, 0.41], false);
        let accepted = accepted_realization();
        let mini = admit_stokes(
            &stokes_model,
            &accepted,
            NativeSpatialPolicy::MiniP1,
            reference_intent().unwrap().solver(),
        )
        .unwrap();

        assert_eq!(q1.method(), DiscretizationMethod::ContinuousGalerkin);
        assert_eq!(
            tpfa.method(),
            DiscretizationMethod::CellCenteredFiniteVolume
        );
        assert_eq!(mini.method(), DiscretizationMethod::ContinuousGalerkin);
        assert_eq!(q1.solver(), tpfa.solver());
        assert_eq!(mini.spatial(), NativeSpatialPolicy::MiniP1);
        assert!(matches!(
            mini.scaling(),
            NativeScalingPolicy::Incompressible(_)
        ));
    }

    #[test]
    fn fresh_and_replayed_model_use_identical_resolution_semantics() {
        let (fresh, mesh) = scalar_model_and_mesh(4);
        let replayed = ModelEnvelope::from_json(
            &fresh.canonical_json().unwrap(),
            ModelDecoderLimits::default(),
        )
        .unwrap();
        let spatial = NativeSpatialPolicy::Q1 {
            cells_per_axis: NonZeroUsize::new(4).unwrap(),
        };
        let fresh = admit_scalar(&fresh, &mesh, spatial, scalar_solver().unwrap()).unwrap();
        let replayed = admit_scalar(&replayed, &mesh, spatial, scalar_solver().unwrap()).unwrap();
        assert_eq!(fresh, replayed);
    }

    #[test]
    fn model_route_precedes_mesh_and_policy_compatibility() {
        let (scalar, cartesian) = scalar_model_and_mesh(4);
        let stokes_model =
            authored_model(ACCEPTED_COMPONENT_SOURCE, [1.0e-3, 0.0, 0.3, 0.41], false);
        let accepted = accepted_realization();
        assert_eq!(
            classify_model(&scalar).unwrap(),
            ModelAdmissionRoute::SelfContainedScalarCandidate
        );
        assert_eq!(
            classify_model(&stokes_model).unwrap(),
            ModelAdmissionRoute::GeometryBackedCandidate
        );

        let scalar_error = admit(
            &scalar,
            NativeMesh::CircularHole(&accepted),
            NativeSpatialPolicy::Q1 {
                cells_per_axis: NonZeroUsize::new(4).unwrap(),
            },
            NativeScalingPolicy::None,
            scalar_solver().unwrap(),
            REFERENCE_SOLVER_PROVIDER,
            REFERENCE_LINEAR_SOLVER.capabilities(),
            NativePlacement::HostCpu {
                workers: NonZeroUsize::MIN,
            },
        )
        .unwrap_err();
        assert!(scalar_error.message().contains("exact Cartesian Mesh"));

        let stokes_error = admit(
            &stokes_model,
            NativeMesh::Cartesian(&cartesian),
            NativeSpatialPolicy::MiniP1,
            NativeScalingPolicy::Incompressible(reference_intent().unwrap().scales()),
            reference_intent().unwrap().solver(),
            ResolveOnlyBackend.provider(),
            ResolveOnlyBackend.capabilities(),
            NativePlacement::HostCpu {
                workers: NonZeroUsize::MIN,
            },
        )
        .unwrap_err();
        assert!(stokes_error.message().contains("Geometry"));
        assert!(!stokes_error.message().contains("scalar"));

        let unsupported_law = ACCEPTED_COMPONENT_SOURCE.replace(
            "div(velocity) = 0;",
            "div(velocity) + zero_pressure / dynamic_viscosity = 0;",
        );
        let malformed_stokes = authored_model(&unsupported_law, [1.0e-3, 0.0, 0.3, 0.41], false);
        let semantic_error = admit_stokes(
            &malformed_stokes,
            &accepted,
            NativeSpatialPolicy::Q1 {
                cells_per_axis: NonZeroUsize::new(4).unwrap(),
            },
            reference_intent().unwrap().solver(),
        )
        .unwrap_err();
        assert!(
            semantic_error.message().contains("Stokes")
                || semantic_error.message().contains("velocity")
        );
        assert!(!semantic_error.message().contains("MINI/P1"));
    }

    #[test]
    fn malformed_model_preserves_its_selected_recognizer_diagnostic() {
        let malformed_source = SCALAR_SOURCE.replace(
            "-div(grad(potential)) - source_scale = 0;",
            "potential = 0;",
        );
        let malformed = ModelDocument::compile("malformed-scalar.eqi", &malformed_source).unwrap();
        let malformed = ModelEnvelope::from_program(malformed.program()).unwrap();
        let (_, mesh) = scalar_model_and_mesh(4);
        let error = admit_scalar(
            &malformed,
            &mesh,
            NativeSpatialPolicy::Q1 {
                cells_per_axis: NonZeroUsize::new(4).unwrap(),
            },
            scalar_solver().unwrap(),
        )
        .unwrap_err();
        assert!(!error.message().contains("Stokes"));
        assert!(!error.message().contains("Geometry"));
    }

    #[test]
    fn cross_wired_solver_and_spatial_policies_reject_without_fallback() {
        let (scalar, mesh) = scalar_model_and_mesh(4);
        let scalar_policy = NativeSpatialPolicy::Q1 {
            cells_per_axis: NonZeroUsize::new(4).unwrap(),
        };
        let wrong_scalar_solvers = [
            SolverPlan::new(
                LinearSolver::SparseLu,
                1.0e-10,
                1.0e-12,
                NonZeroUsize::new(10_000).unwrap(),
            )
            .unwrap(),
            scalar_solver()
                .unwrap()
                .with_preconditioner(PreconditionerPolicy::Jacobi),
            scalar_solver()
                .unwrap()
                .with_reduction(ReductionPolicy::Fast),
        ];
        for solver in wrong_scalar_solvers {
            let error = admit_scalar(&scalar, &mesh, scalar_policy, solver).unwrap_err();
            assert!(error.message().contains("CG/Identity/Reproducible"));
        }

        let scaling_error = admit(
            &scalar,
            NativeMesh::Cartesian(&mesh),
            scalar_policy,
            NativeScalingPolicy::Incompressible(reference_intent().unwrap().scales()),
            scalar_solver().unwrap(),
            REFERENCE_SOLVER_PROVIDER,
            REFERENCE_LINEAR_SOLVER.capabilities(),
            NativePlacement::HostCpu {
                workers: NonZeroUsize::MIN,
            },
        )
        .unwrap_err();
        assert!(
            scaling_error
                .message()
                .contains("does not accept a scaling")
        );

        let stokes_model =
            authored_model(ACCEPTED_COMPONENT_SOURCE, [1.0e-3, 0.0, 0.3, 0.41], false);
        let accepted = accepted_realization();
        let error = admit_stokes(
            &stokes_model,
            &accepted,
            NativeSpatialPolicy::Q1 {
                cells_per_axis: NonZeroUsize::new(4).unwrap(),
            },
            reference_intent().unwrap().solver(),
        )
        .unwrap_err();
        assert!(error.message().contains("MINI/P1"));

        let wrong_stokes_solvers = [
            scalar_solver().unwrap(),
            reference_intent()
                .unwrap()
                .solver()
                .with_preconditioner(PreconditionerPolicy::Jacobi),
            reference_intent()
                .unwrap()
                .solver()
                .with_reduction(ReductionPolicy::Reproducible),
        ];
        for solver in wrong_stokes_solvers {
            let error = admit_stokes(
                &stokes_model,
                &accepted,
                NativeSpatialPolicy::MiniP1,
                solver,
            )
            .unwrap_err();
            assert!(error.message().contains("SparseLU/Identity/Fast"));
        }

        let missing_scaling = admit(
            &stokes_model,
            NativeMesh::CircularHole(&accepted),
            NativeSpatialPolicy::MiniP1,
            NativeScalingPolicy::None,
            reference_intent().unwrap().solver(),
            ResolveOnlyBackend.provider(),
            ResolveOnlyBackend.capabilities(),
            NativePlacement::HostCpu {
                workers: NonZeroUsize::MIN,
            },
        )
        .unwrap_err();
        assert!(missing_scaling.message().contains("existing scaling"));

        let placement_error = admit(
            &stokes_model,
            NativeMesh::CircularHole(&accepted),
            NativeSpatialPolicy::MiniP1,
            NativeScalingPolicy::Incompressible(reference_intent().unwrap().scales()),
            reference_intent().unwrap().solver(),
            ResolveOnlyBackend.provider(),
            ResolveOnlyBackend.capabilities(),
            NativePlacement::HostCpu {
                workers: NonZeroUsize::new(2).unwrap(),
            },
        )
        .unwrap_err();
        assert!(placement_error.message().contains("serial host"));
    }

    #[test]
    fn resource_admission_uses_the_actual_scalar_method() {
        let (model, mesh) = scalar_model_and_mesh(500);
        let tpfa = admit_scalar(
            &model,
            &mesh,
            NativeSpatialPolicy::CellCenteredTpfa {
                cells_per_axis: NonZeroUsize::new(500).unwrap(),
            },
            scalar_solver().unwrap(),
        )
        .unwrap();
        assert_eq!(
            tpfa.method(),
            DiscretizationMethod::CellCenteredFiniteVolume
        );
        let error = admit_scalar(
            &model,
            &mesh,
            NativeSpatialPolicy::Q1 {
                cells_per_axis: NonZeroUsize::new(500).unwrap(),
            },
            scalar_solver().unwrap(),
        )
        .unwrap_err();
        assert!(error.message().contains("before allocation"));
    }
}
