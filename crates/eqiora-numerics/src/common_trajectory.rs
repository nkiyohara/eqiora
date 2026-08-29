//! One native authority for accepted common ODE and spatial trajectories.

use eqiora_core::Diagnostic;
use eqiora_time::{InitialConditionPolicy, TimeEquationClass, TimeMethod, TimeSolution};
use sha2::{Digest, Sha256};

use crate::{
    CommonFsiRunRequest, CommonOdeRunRequest, CommonOdeState, CommonState,
    CommonTransientRunRequest,
};

mod artifact;

/// Accepted output States bound to the complete immutable Run request.
#[derive(Debug, Clone, PartialEq)]
pub enum CommonTrajectory {
    Ode {
        request: Box<CommonOdeRunRequest>,
        states: Vec<CommonOdeState>,
        identity: String,
    },
    TransientFlow {
        request: Box<CommonTransientRunRequest>,
        states: Vec<(usize, CommonState)>,
        identity: String,
    },
    Fsi {
        request: Box<CommonFsiRunRequest>,
        states: Vec<(usize, CommonState)>,
        identity: String,
    },
}

impl CommonTrajectory {
    /// Reaccept one adaptive ODE backend solution against its exact request.
    pub fn accept_ode(
        request: CommonOdeRunRequest,
        solution: TimeSolution,
    ) -> Result<Self, Diagnostic> {
        if solution.report().method() != TimeMethod::Tsitouras45
            || solution.report().backend_identity() != request.plan().backend()
            || solution.report().equation_class() != TimeEquationClass::ExplicitOde
            || solution.report().initial_condition() != InitialConditionPolicy::Provided
            || solution.dimension() != request.plan().field_dimensions().len()
            || solution.times() != request.time_plan().output_times()
        {
            return Err(invalid(
                "adaptive backend result differs from the exact no-Mesh ODE request",
            ));
        }
        let states = request
            .output_times_s()
            .iter()
            .enumerate()
            .map(|(sample, &time)| {
                let values = solution
                    .state(sample)
                    .ok_or_else(|| invalid("adaptive backend omitted one requested ODE State"))?;
                CommonOdeState::new(request.plan(), time, values.to_vec(), "result")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let identity = ode_identity(request.identity(), &states);
        Ok(Self::Ode {
            request: Box::new(request),
            states,
            identity,
        })
    }

    pub(crate) fn accept_ode_states(
        request: CommonOdeRunRequest,
        states: Vec<CommonOdeState>,
    ) -> Result<Self, Diagnostic> {
        if states.len() != request.output_times_s().len()
            || states
                .iter()
                .zip(request.output_times_s())
                .any(|(state, time)| {
                    state.state_space_identity() != request.plan().state_space_identity()
                        || state.time_s().to_bits() != time.to_bits()
                })
        {
            return Err(invalid(
                "accepted ODE Trajectory differs from its exact Run request",
            ));
        }
        let identity = ode_identity(request.identity(), &states);
        Ok(Self::Ode {
            request: Box::new(request),
            states,
            identity,
        })
    }

    /// Accept exact requested transient-flow output steps.
    pub fn accept_transient_flow(
        request: CommonTransientRunRequest,
        states: Vec<(usize, CommonState)>,
    ) -> Result<Self, Diagnostic> {
        validate_spatial(
            request.plan().state_space_identity(),
            request.output_steps(),
            &states,
        )?;
        let identity = spatial_identity(b"transient-flow", request.identity(), &states)?;
        Ok(Self::TransientFlow {
            request: Box::new(request),
            states,
            identity,
        })
    }

    /// Accept exact requested fixed-reference-FSI output steps.
    pub fn accept_fsi(
        request: CommonFsiRunRequest,
        states: Vec<(usize, CommonState)>,
    ) -> Result<Self, Diagnostic> {
        validate_spatial(
            request.plan().state_space_identity(),
            request.output_steps(),
            &states,
        )?;
        let identity = spatial_identity(b"fixed-reference-fsi", request.identity(), &states)?;
        Ok(Self::Fsi {
            request: Box::new(request),
            states,
            identity,
        })
    }

    /// Domain-separated identity of the request and all requested States.
    #[must_use]
    pub fn identity(&self) -> &str {
        match self {
            Self::Ode { identity, .. }
            | Self::TransientFlow { identity, .. }
            | Self::Fsi { identity, .. } => identity,
        }
    }

    /// Exact immutable Run-request identity.
    #[must_use]
    pub fn request_identity(&self) -> &str {
        match self {
            Self::Ode { request, .. } => request.identity(),
            Self::TransientFlow { request, .. } => request.identity(),
            Self::Fsi { request, .. } => request.identity(),
        }
    }

    /// Exact owning Plan identity.
    #[must_use]
    pub fn plan_identity(&self) -> &str {
        match self {
            Self::Ode { request, .. } => request.plan().identity(),
            Self::TransientFlow { request, .. } => request.plan().identity(),
            Self::Fsi { request, .. } => request.plan().identity(),
        }
    }

    /// Requested ODE States, when this is a no-Mesh trajectory.
    #[must_use]
    pub fn ode_states(&self) -> Option<&[CommonOdeState]> {
        match self {
            Self::Ode { states, .. } => Some(states),
            Self::TransientFlow { .. } | Self::Fsi { .. } => None,
        }
    }

    /// Requested step-indexed States, when this is a spatial trajectory.
    #[must_use]
    pub fn spatial_states(&self) -> Option<&[(usize, CommonState)]> {
        match self {
            Self::Ode { .. } => None,
            Self::TransientFlow { states, .. } | Self::Fsi { states, .. } => Some(states),
        }
    }
}

fn validate_spatial(
    state_space_identity: String,
    output_steps: &[usize],
    states: &[(usize, CommonState)],
) -> Result<(), Diagnostic> {
    if states.len() != output_steps.len()
        || states
            .iter()
            .zip(output_steps)
            .any(|((step, state), expected)| {
                step != expected || state.state_space_identity() != state_space_identity
            })
    {
        return Err(invalid(
            "accepted spatial Trajectory differs from its exact Run request",
        ));
    }
    Ok(())
}

fn ode_identity(request_identity: &str, states: &[CommonOdeState]) -> String {
    let mut bytes = Vec::new();
    push(&mut bytes, b"ode");
    push(&mut bytes, request_identity.as_bytes());
    for state in states {
        bytes.extend_from_slice(&state.time_s().to_bits().to_be_bytes());
        push(&mut bytes, state.identity().as_bytes());
    }
    digest(&bytes)
}

fn spatial_identity(
    family: &[u8],
    request_identity: &str,
    states: &[(usize, CommonState)],
) -> Result<String, Diagnostic> {
    let mut bytes = Vec::new();
    push(&mut bytes, family);
    push(&mut bytes, request_identity.as_bytes());
    for (step, state) in states {
        bytes.extend_from_slice(
            &u64::try_from(*step)
                .map_err(|_| invalid("Trajectory output step exceeds canonical u64 range"))?
                .to_be_bytes(),
        );
        push(&mut bytes, state.identity().as_bytes());
    }
    Ok(digest(&bytes))
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest([b"eqiora.common-trajectory/v1\0".as_slice(), bytes].concat())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn push(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value);
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(eqiora_core::diagnostic::codes::INVALID_REALIZATION, message)
}
