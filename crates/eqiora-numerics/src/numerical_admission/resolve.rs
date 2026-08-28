use super::*;

pub fn resolve_common_plan(
    model: &ModelEnvelope,
    owner: AuthenticatedCommonMesh,
    spatial: impl Into<CommonSpatialRequest>,
    solve: CommonSolvePolicy,
    scaling: Option<IncompressibleScalingRequest2d>,
    temporal: Option<CommonBackwardEuler>,
    stokes_backend: &dyn LinearSolverBackend,
) -> Result<ResolvedCommonPlan, Diagnostic> {
    let recognized = RecognizedNativeAdmission::recognize(model, owner)?;
    let spatial = spatial.into();
    let requested_linear = match solve {
        CommonSolvePolicy::Linear(linear) | CommonSolvePolicy::Newton { linear, .. } => linear,
    };
    if requested_linear.algorithm() != LinearSolver::ConjugateGradient
        || requested_linear.preconditioner() != PreconditionerPolicy::Identity
        || requested_linear.reduction() != ReductionPolicy::Reproducible
    {
        return Err(invalid(
            "common Linear request must contain identity-preconditioned reproducible controls",
        ));
    }
    match recognized.capability {
        NativeCapability::ScalarElliptic => {
            let CommonSolvePolicy::Linear(solve) = solve else {
                return Err(invalid(
                    "scalar-elliptic mathematics requires Linear solve policy",
                ));
            };
            if temporal.is_some() {
                return Err(invalid(
                    "steady scalar-elliptic mathematics does not admit a temporal policy",
                ));
            }
            if scaling.is_some() {
                return Err(invalid(
                    "scalar-elliptic Model mathematics does not admit incompressible-flow scaling",
                ));
            }
            let CommonSpatialRequest::Uniform(spatial) = spatial else {
                return Err(invalid(
                    "scalar-elliptic mathematics does not admit Domain-scoped spatial policies",
                ));
            };
            let spatial = match spatial {
                CommonSpatialPolicy::Q1 => NativeSpatialPolicy::ScalarQ1,
                CommonSpatialPolicy::CellCenteredTpfa => NativeSpatialPolicy::ScalarTpfa,
                CommonSpatialPolicy::MiniP1 => {
                    return Err(invalid(
                        "scalar-elliptic Model mathematics is incompatible with MINI/P1",
                    ));
                }
                CommonSpatialPolicy::CellCentered => {
                    return Err(invalid(
                        "scalar-elliptic Model mathematics is incompatible with incompressible CellCentered",
                    ));
                }
                CommonSpatialPolicy::P1 => {
                    return Err(invalid(
                        "scalar-elliptic Model mathematics is incompatible with simplex P1",
                    ));
                }
            };
            let linear = NativeLinearPolicy::exact(solve, &REFERENCE_LINEAR_SOLVER)?;
            let admission = recognized.complete(spatial, linear, None, None)?;
            CommonScalarPlan::from_admission(model, admission).map(|plan| ResolvedCommonPlan {
                kind: ResolvedCommonPlanKind::Scalar(Box::new(plan)),
            })
        }
        NativeCapability::IsotropicElasticity => {
            let CommonSolvePolicy::Linear(solve) = solve else {
                return Err(invalid(
                    "linear-elasticity mathematics requires Linear solve policy",
                ));
            };
            if temporal.is_some() {
                return Err(invalid(
                    "steady linear-elasticity mathematics does not admit a temporal policy",
                ));
            }
            if scaling.is_some() {
                return Err(invalid(
                    "linear-elasticity mathematics does not admit incompressible-flow scaling",
                ));
            }
            let CommonSpatialRequest::Uniform(spatial) = spatial else {
                return Err(invalid(
                    "linear-elasticity mathematics does not admit Domain-scoped spatial policies",
                ));
            };
            if spatial != CommonSpatialPolicy::Q1 {
                return Err(invalid(
                    "linear-elasticity mathematics requires the admitted Cartesian Q1 policy",
                ));
            }
            let linear = NativeLinearPolicy::exact(solve, &REFERENCE_LINEAR_SOLVER)?;
            let admission =
                recognized.complete(NativeSpatialPolicy::ElasticityQ1, linear, None, None)?;
            CommonElasticityPlan::from_admission(model, admission).map(|plan| ResolvedCommonPlan {
                kind: ResolvedCommonPlanKind::Elasticity(Box::new(plan)),
            })
        }
        NativeCapability::SteadyIncompressibleStokes => {
            let CommonSolvePolicy::Linear(solve) = solve else {
                return Err(invalid(
                    "steady-Stokes mathematics requires Linear solve policy",
                ));
            };
            if temporal.is_some() {
                return Err(invalid(
                    "steady-Stokes mathematics does not admit a temporal policy",
                ));
            }
            let CommonSpatialRequest::Uniform(spatial) = spatial else {
                return Err(invalid(
                    "steady-Stokes mathematics does not admit Domain-scoped spatial policies",
                ));
            };
            if spatial != CommonSpatialPolicy::MiniP1 {
                return Err(invalid(
                    "steady-Stokes Model mathematics requires the admitted MINI/P1 policy",
                ));
            }
            let RecognizedNativeModel::Stokes(binding) = &recognized.recognized else {
                unreachable!("steady-Stokes capability recognition returns a Stokes binding")
            };
            let scaling = binding.resolve_incompressible_scaling(model, scaling)?;
            let effective_solve = SolverPlan::new(
                LinearSolver::SparseLu,
                solve.relative_tolerance(),
                solve.absolute_tolerance(),
                solve.maximum_iterations(),
            )?
            .with_reduction(ReductionPolicy::Fast);
            let linear = NativeLinearPolicy::exact(effective_solve, stokes_backend)?;
            let admission = recognized.complete(
                NativeSpatialPolicy::StokesMiniP1(scaling.scales()),
                linear,
                None,
                None,
            )?;
            CommonSteadyStokesPlan::from_admission(model, admission, scaling).map(|plan| {
                ResolvedCommonPlan {
                    kind: ResolvedCommonPlanKind::SteadyStokes(Box::new(plan)),
                }
            })
        }
        NativeCapability::TransientIncompressibleFlow => {
            let CommonSpatialRequest::Uniform(spatial) = spatial else {
                return Err(invalid(
                    "transient-flow mathematics does not admit Domain-scoped spatial policies",
                ));
            };
            let CommonSolvePolicy::Newton { nonlinear, linear } = solve else {
                return Err(invalid(
                    "transient incompressible-flow mathematics requires Newton(linear=...) policy",
                ));
            };
            let temporal = temporal.ok_or_else(|| {
                invalid("transient incompressible-flow mathematics requires BackwardEuler")
            })?;
            let (geometry, mesh, correspondence, _) =
                resource_artifact_digests(&recognized.resources)?;
            let scaling = resolve_complete_manual_incompressible_scaling_2d(
                scaling,
                model.digest()?,
                geometry,
                correspondence,
                mesh,
            )?;
            let (algorithm, reduction) = match spatial {
                CommonSpatialPolicy::MiniP1 => (LinearSolver::SparseLu, ReductionPolicy::Fast),
                CommonSpatialPolicy::CellCentered => (
                    LinearSolver::BiConjugateGradientStabilized,
                    ReductionPolicy::Reproducible,
                ),
                CommonSpatialPolicy::Q1 | CommonSpatialPolicy::CellCenteredTpfa => {
                    return Err(invalid(
                        "transient incompressible-flow mathematics requires MINI/P1 or CellCentered",
                    ));
                }
                CommonSpatialPolicy::P1 => {
                    return Err(invalid(
                        "transient incompressible-flow mathematics does not admit standalone P1",
                    ));
                }
            };
            let effective_linear = SolverPlan::new(
                algorithm,
                linear.relative_tolerance(),
                linear.absolute_tolerance(),
                linear.maximum_iterations(),
            )?
            .with_preconditioner(PreconditionerPolicy::Identity)
            .with_reduction(reduction);
            let linear_backend: &dyn LinearSolverBackend = match spatial {
                CommonSpatialPolicy::MiniP1 => stokes_backend,
                CommonSpatialPolicy::CellCentered => &REFERENCE_LINEAR_SOLVER,
                CommonSpatialPolicy::Q1 | CommonSpatialPolicy::CellCenteredTpfa => unreachable!(),
                CommonSpatialPolicy::P1 => unreachable!(),
            };
            let linear = NativeLinearPolicy::exact(effective_linear, linear_backend)?;
            let native_spatial = match spatial {
                CommonSpatialPolicy::MiniP1 => {
                    NativeSpatialPolicy::TransientMiniP1(scaling.scales())
                }
                CommonSpatialPolicy::CellCentered => {
                    NativeSpatialPolicy::TransientCellCentered(scaling.scales())
                }
                CommonSpatialPolicy::Q1 | CommonSpatialPolicy::CellCenteredTpfa => unreachable!(),
                CommonSpatialPolicy::P1 => unreachable!(),
            };
            let admission =
                recognized.complete(native_spatial, linear, Some(temporal), Some(nonlinear))?;
            CommonTransientFlowPlan::from_admission(model, admission, scaling, temporal, nonlinear)
                .map(|plan| ResolvedCommonPlan {
                    kind: ResolvedCommonPlanKind::TransientFlow(Box::new(plan)),
                })
        }
        NativeCapability::FixedReferenceFsi => {
            let CommonSolvePolicy::Linear(linear) = solve else {
                return Err(invalid(
                    "fixed-reference FSI mathematics requires Linear solve policy",
                ));
            };
            let temporal = temporal
                .ok_or_else(|| invalid("fixed-reference FSI mathematics requires BackwardEuler"))?;
            let CommonSpatialRequest::Scoped(bindings) = spatial else {
                return Err(invalid(
                    "fixed-reference FSI mathematics requires exact Domain-scoped spatial policies",
                ));
            };
            let RecognizedNativeModel::Fsi(canonical) = &recognized.recognized else {
                unreachable!("FSI capability owns recognized FSI meaning")
            };
            let expected = BTreeMap::from([
                (canonical.fluid().domain(), CommonSpatialPolicy::MiniP1),
                (canonical.solid().domain(), CommonSpatialPolicy::P1),
            ]);
            let mut actual = BTreeMap::new();
            for binding in bindings {
                if binding.model() != &model.digest()? {
                    return Err(invalid(
                        "FSI scoped spatial policy carries a foreign or stale exact Model reference",
                    ));
                }
                if actual
                    .insert(binding.domain().erase(), binding.policy())
                    .is_some()
                {
                    return Err(invalid("FSI scoped spatial policy repeats one DomainRef"));
                }
            }
            if actual != expected {
                return Err(invalid(
                    "FSI scoped spatial policies must completely and exclusively bind MiniP1 to fluid and P1 to solid",
                ));
            }
            let effective_linear = SolverPlan::new(
                LinearSolver::MinimumResidual,
                linear.relative_tolerance(),
                linear.absolute_tolerance(),
                linear.maximum_iterations(),
            )?
            .with_preconditioner(PreconditionerPolicy::Identity)
            .with_reduction(ReductionPolicy::Reproducible);
            REFERENCE_LINEAR_SOLVER.capabilities().require_problem(
                effective_linear,
                ScalarType::F64,
                LinearOperatorProperties::SymmetricIndefinite,
            )?;
            CommonFsiPlan::from_recognized(model, recognized, scaling, temporal, effective_linear)
                .map(|plan| ResolvedCommonPlan {
                    kind: ResolvedCommonPlanKind::Fsi(Box::new(plan)),
                })
        }
    }
}
