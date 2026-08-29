use super::solver_planning::{
    resolve_fixed_reference_fsi, resolve_reference_spd, resolve_stokes_mini, resolve_transient_flow,
};
use super::spatial_planning::{
    require_fixed_reference_fsi, resolve_elasticity, resolve_scalar, resolve_stokes,
    resolve_transient,
};
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
            let spatial = resolve_scalar(spatial)?;
            let linear = resolve_reference_spd(solve)?;
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
            let spatial = resolve_elasticity(spatial)?;
            let linear = resolve_reference_spd(solve)?;
            let admission = recognized.complete(spatial, linear, None, None)?;
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
            let spatial = resolve_stokes(spatial)?;
            let RecognizedNativeModel::Stokes(binding) = &recognized.recognized else {
                unreachable!("steady-Stokes capability recognition returns a Stokes binding")
            };
            let scaling = binding.resolve_incompressible_scaling(model, scaling)?;
            let linear = resolve_stokes_mini(solve, stokes_backend)?;
            let admission =
                recognized.complete(spatial.with_scaling(scaling.scales()), linear, None, None)?;
            CommonSteadyStokesPlan::from_admission(model, admission, scaling).map(|plan| {
                ResolvedCommonPlan {
                    kind: ResolvedCommonPlanKind::SteadyStokes(Box::new(plan)),
                }
            })
        }
        NativeCapability::TransientIncompressibleFlow => {
            let spatial = resolve_transient(spatial)?;
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
            let linear = resolve_transient_flow(linear, spatial, stokes_backend)?;
            let native_spatial = spatial.with_scaling(scaling.scales());
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
            let RecognizedNativeModel::Fsi(canonical) = &recognized.recognized else {
                unreachable!("FSI capability owns recognized FSI meaning")
            };
            require_fixed_reference_fsi(model, canonical, spatial)?;
            let effective_linear = resolve_fixed_reference_fsi(linear)?;
            CommonFsiPlan::from_recognized(model, recognized, scaling, temporal, effective_linear)
                .map(|plan| ResolvedCommonPlan {
                    kind: ResolvedCommonPlanKind::Fsi(Box::new(plan)),
                })
        }
    }
}
