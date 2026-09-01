use super::*;
use std::ops::ControlFlow;

// Keep the lifecycle private until another method family proves a durable
// outcome type; the standard ControlFlow shape is intentionally explicit here.
#[allow(clippy::type_complexity)]
fn advance_prepared_actions<S: Clone, P, E>(
    initial: S,
    maximum_actions: usize,
    output_actions: &[usize],
    prepare: impl FnOnce(&S) -> Result<P, E>,
    mut advance: impl FnMut(&P, &S) -> Result<S, E>,
    mut stop_at_boundary: impl FnMut(usize, &S) -> bool,
) -> Result<ControlFlow<(usize, S), Vec<(usize, S)>>, E> {
    let prepared = prepare(&initial)?;
    let mut accepted = initial;
    if stop_at_boundary(0, &accepted) {
        return Ok(ControlFlow::Break((0, accepted)));
    }
    let mut outputs = Vec::with_capacity(output_actions.len());
    for accepted_actions in 1..=maximum_actions {
        let candidate = advance(&prepared, &accepted)?;
        accepted = candidate;
        if output_actions.binary_search(&accepted_actions).is_ok() {
            outputs.push((accepted_actions, accepted.clone()));
        }
        if stop_at_boundary(accepted_actions, &accepted) {
            return Ok(ControlFlow::Break((accepted_actions, accepted)));
        }
    }
    Ok(ControlFlow::Continue(outputs))
}

impl CommonFsiRunRequest {
    pub fn from_steps(
        plan: CommonFsiPlan,
        state: CommonState,
        steps: usize,
        output_steps: Vec<usize>,
    ) -> Result<Self, Diagnostic> {
        let accepted_steps = NonZeroUsize::new(steps)
            .ok_or_else(|| invalid("FSI Run steps must be strictly positive"))?;
        Self::new(plan, state, accepted_steps, output_steps)
    }

    pub fn from_times(
        plan: CommonFsiPlan,
        state: CommonState,
        until_s: f64,
        output_times_s: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        let step_s = plan.temporal().step().value();
        let accepted = exact_grid_index(state.time_s(), until_s, step_s, "until_s")?;
        let outputs = output_times_s
            .into_iter()
            .map(|time| exact_grid_index(state.time_s(), time, step_s, "output_times_s"))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            plan,
            state,
            NonZeroUsize::new(accepted)
                .ok_or_else(|| invalid("FSI Run horizon contains no accepted step"))?,
            outputs,
        )
    }

    pub(super) fn new(
        plan: CommonFsiPlan,
        state: CommonState,
        accepted_steps: NonZeroUsize,
        output_steps: Vec<usize>,
    ) -> Result<Self, Diagnostic> {
        if state.state_space_identity() != plan.state_space_identity() {
            return Err(invalid(
                "FSI Run State belongs to an incompatible common state space",
            ));
        }
        if output_steps.is_empty()
            || output_steps.windows(2).any(|pair| pair[0] >= pair[1])
            || output_steps
                .iter()
                .any(|step| *step == 0 || *step > accepted_steps.get())
        {
            return Err(invalid(
                "FSI output_steps must be nonempty, strictly increasing, and within the horizon",
            ));
        }
        let mut bytes = Vec::new();
        push_framed(&mut bytes, plan.identity().as_bytes());
        push_framed(&mut bytes, state.identity().as_bytes());
        bytes.extend_from_slice(&(accepted_steps.get() as u64).to_be_bytes());
        for output in &output_steps {
            bytes.extend_from_slice(&(*output as u64).to_be_bytes());
        }
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-fsi-run-request/v1\0".as_slice(),
                bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            plan,
            state,
            accepted_steps,
            output_steps,
            identity,
        })
    }

    #[must_use]
    pub const fn plan(&self) -> &CommonFsiPlan {
        &self.plan
    }
    #[must_use]
    pub const fn state(&self) -> &CommonState {
        &self.state
    }
    #[must_use]
    pub const fn accepted_steps(&self) -> NonZeroUsize {
        self.accepted_steps
    }
    #[must_use]
    pub fn output_steps(&self) -> &[usize] {
        &self.output_steps
    }
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Authenticate this exact Run once, then advance its accepted actions.
    ///
    /// The callback is observed only before action one and after a successful
    /// action. Returning `true` stops with the last accepted State.
    #[allow(clippy::type_complexity)]
    pub fn advance_accepted_actions(
        &self,
        backend: &dyn LinearSolverBackend,
        stop_at_boundary: impl FnMut(usize, &CommonState) -> bool,
    ) -> Result<ControlFlow<(usize, CommonState), Vec<(usize, CommonState)>>, Diagnostic> {
        advance_prepared_actions(
            self.state.clone(),
            self.accepted_steps.get(),
            &self.output_steps,
            |state| self.plan.prepare_execution(state, backend),
            |prepared, state| prepared.advance(state),
            stop_at_boundary,
        )
    }
}

impl CommonTransientRunRequest {
    /// Canonicalize a step-count horizon and explicit accepted-step outputs.
    pub fn from_steps(
        plan: CommonTransientFlowPlan,
        state: CommonState,
        steps: usize,
        output_steps: Vec<usize>,
    ) -> Result<Self, Diagnostic> {
        let accepted_steps = NonZeroUsize::new(steps)
            .ok_or_else(|| invalid("transient Run steps must be strictly positive"))?;
        Self::new(plan, state, accepted_steps, output_steps)
    }

    /// Canonicalize an exact Backward-Euler time horizon and output-time grid.
    pub fn from_times(
        plan: CommonTransientFlowPlan,
        state: CommonState,
        until_s: f64,
        output_times_s: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        if !until_s.is_finite() || until_s <= state.time_s() {
            return Err(invalid(
                "transient Run until_s must be finite and later than State.time_s",
            ));
        }
        if output_times_s.is_empty()
            || output_times_s.iter().any(|value| !value.is_finite())
            || output_times_s.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid(
                "output_times_s must be finite, nonempty, and strictly increasing",
            ));
        }
        let step_s = plan.temporal().step().value();
        let accepted_steps = exact_grid_index(state.time_s(), until_s, step_s, "until_s")?;
        let output_steps = output_times_s
            .into_iter()
            .map(|time| exact_grid_index(state.time_s(), time, step_s, "output_times_s"))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            plan,
            state,
            NonZeroUsize::new(accepted_steps)
                .ok_or_else(|| invalid("transient Run horizon contains no accepted step"))?,
            output_steps,
        )
    }

    pub(super) fn new(
        plan: CommonTransientFlowPlan,
        state: CommonState,
        accepted_steps: NonZeroUsize,
        output_steps: Vec<usize>,
    ) -> Result<Self, Diagnostic> {
        if state.state_space_identity() != plan.state_space_identity() {
            return Err(invalid(
                "transient Run State belongs to a different exact common state space",
            ));
        }
        if output_steps.is_empty()
            || output_steps.windows(2).any(|pair| pair[0] >= pair[1])
            || output_steps
                .iter()
                .any(|step| *step == 0 || *step > accepted_steps.get())
        {
            return Err(invalid(
                "output_steps must be nonempty, strictly increasing accepted-step indices within the inclusive horizon",
            ));
        }
        let mut bytes = Vec::new();
        push_framed(&mut bytes, plan.identity().as_bytes());
        push_framed(&mut bytes, state.identity().as_bytes());
        let accepted_steps_u64 = u64::try_from(accepted_steps.get())
            .map_err(|_| invalid("transient Run horizon exceeds canonical u64 identity range"))?;
        bytes.extend_from_slice(&accepted_steps_u64.to_be_bytes());
        for step in &output_steps {
            let step = u64::try_from(*step).map_err(|_| {
                invalid("transient Run output index exceeds canonical u64 identity range")
            })?;
            bytes.extend_from_slice(&step.to_be_bytes());
        }
        bytes.extend_from_slice(&1_u64.to_be_bytes());
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-transient-run-request/v1\0".as_slice(),
                bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            plan,
            state,
            accepted_steps,
            output_steps,
            identity,
        })
    }

    #[must_use]
    pub const fn plan(&self) -> &CommonTransientFlowPlan {
        &self.plan
    }
    #[must_use]
    pub const fn state(&self) -> &CommonState {
        &self.state
    }
    #[must_use]
    pub const fn accepted_steps(&self) -> NonZeroUsize {
        self.accepted_steps
    }
    #[must_use]
    pub fn output_steps(&self) -> &[usize] {
        &self.output_steps
    }
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Authenticate this exact Run once, then advance its accepted actions.
    ///
    /// The callback is observed only before action one and after a successful
    /// action. Returning `true` stops with the last accepted State.
    #[allow(clippy::type_complexity)]
    pub fn advance_accepted_actions(
        &self,
        backend: &dyn LinearSolverBackend,
        stop_at_boundary: impl FnMut(usize, &CommonState) -> bool,
    ) -> Result<ControlFlow<(usize, CommonState), Vec<(usize, CommonState)>>, Diagnostic> {
        advance_prepared_actions(
            self.state.clone(),
            self.accepted_steps.get(),
            &self.output_steps,
            |state| self.plan.prepare_execution(state, backend),
            |prepared, state| prepared.advance(state),
            stop_at_boundary,
        )
    }
}

fn exact_grid_index(
    start_s: f64,
    target_s: f64,
    step_s: f64,
    label: &str,
) -> Result<usize, Diagnostic> {
    if !target_s.is_finite() || target_s <= start_s {
        return Err(invalid(format!(
            "{label} values must be finite and later than State.time_s"
        )));
    }
    let raw = (target_s - start_s) / step_s;
    if !raw.is_finite() || raw < 1.0 || raw.fract() != 0.0 || raw > usize::MAX as f64 {
        return Err(invalid(format!(
            "{label} values must align exactly to the Plan Backward-Euler grid"
        )));
    }
    let index = raw as usize;
    let reconstructed = start_s + step_s * index as f64;
    if reconstructed.to_bits() != target_s.to_bits() {
        return Err(invalid(format!(
            "{label} values must align exactly to the Plan Backward-Euler grid"
        )));
    }
    Ok(index)
}

impl CommonState {
    pub(super) fn new(
        state_space_identity: String,
        time_s: f64,
        model: Arc<ModelEnvelope>,
        resources: Arc<NativeMeshResources>,
        kind: CommonStateKind,
    ) -> Result<Self, Diagnostic> {
        Self::new_with_boundary_forces(
            state_space_identity,
            time_s,
            model,
            resources,
            kind,
            Vec::new(),
        )
    }

    pub(super) fn new_with_boundary_forces(
        state_space_identity: String,
        time_s: f64,
        model: Arc<ModelEnvelope>,
        resources: Arc<NativeMeshResources>,
        kind: CommonStateKind,
        mut named_boundary_forces_on_domain: Vec<(String, [f64; 2])>,
    ) -> Result<Self, Diagnostic> {
        if !time_s.is_finite() || time_s < 0.0 || time_s.to_bits() == (-0.0_f64).to_bits() {
            return Err(invalid("State time_s must be finite and non-negative"));
        }
        named_boundary_forces_on_domain.sort_by(|left, right| left.0.cmp(&right.0));
        if named_boundary_forces_on_domain
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
            || named_boundary_forces_on_domain.iter().any(|(name, force)| {
                name.is_empty() || force.iter().any(|value| !value.is_finite())
            })
        {
            return Err(invalid(
                "State named boundary forces must have unique non-empty names and finite components",
            ));
        }
        let mut bytes = Vec::new();
        push_framed(&mut bytes, state_space_identity.as_bytes());
        bytes.extend_from_slice(&time_s.to_bits().to_be_bytes());
        match &kind {
            CommonStateKind::MiniP1(state) => {
                push_framed(&mut bytes, b"mini-p1/backward-euler/no-extra-history/v1");
                for value in state
                    .velocity()
                    .vertex_values()
                    .iter()
                    .flatten()
                    .chain(state.velocity().cell_bubble_values().iter().flatten())
                    .chain(state.pressure().vertex_values())
                {
                    bytes.extend_from_slice(&value.to_bits().to_be_bytes());
                }
                match state.pressure_reference() {
                    SteadyStokesPressureReference2d::ZeroIntegral { multiplier } => {
                        push_framed(&mut bytes, b"zero-integral");
                        bytes.extend_from_slice(&multiplier.to_bits().to_be_bytes());
                    }
                    SteadyStokesPressureReference2d::BoundaryTraction => {
                        push_framed(&mut bytes, b"boundary-traction");
                    }
                }
            }
            CommonStateKind::CellCentered(state) => {
                push_framed(
                    &mut bytes,
                    b"cell-centered/backward-euler/bdf1-previous-accepted-face-volume-flux/v1",
                );
                for value in state
                    .velocity()
                    .values()
                    .iter()
                    .flatten()
                    .chain(state.pressure().values())
                    .chain(std::iter::once(&state.gauge_multiplier()))
                    .chain(state.previous_face_volume_fluxes())
                {
                    bytes.extend_from_slice(&value.to_bits().to_be_bytes());
                }
            }
            CommonStateKind::Fsi {
                state, pressure, ..
            } => {
                push_framed(
                    &mut bytes,
                    b"fixed-reference-fsi/mini-p1+p1/backward-euler/v1",
                );
                for value in state
                    .vertex_velocity()
                    .iter()
                    .flatten()
                    .chain(state.fluid_cell_bubble_velocity().iter().flatten())
                    .chain(pressure.iter())
                    .chain(state.solid_displacement().iter().flatten())
                {
                    bytes.extend_from_slice(&value.to_bits().to_be_bytes());
                }
            }
        }
        for (name, force) in &named_boundary_forces_on_domain {
            push_framed(&mut bytes, name.as_bytes());
            for component in force {
                bytes.extend_from_slice(&component.to_bits().to_be_bytes());
            }
        }
        let identity = hex_bytes(&Sha256::digest(
            [b"eqiora.common-state/v1\0".as_slice(), bytes.as_slice()].concat(),
        ));
        Ok(Self {
            state_space_identity,
            identity,
            time_s,
            model,
            resources,
            kind,
            named_boundary_forces_on_domain,
        })
    }

    /// Exact identity of Model, Mesh, complete fields/spaces, gauge, layout, and history schema.
    #[must_use]
    pub fn state_space_identity(&self) -> &str {
        &self.state_space_identity
    }

    /// Content identity of this exact accepted state occurrence.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Exact coherent-SI model time.
    #[must_use]
    pub const fn time_s(&self) -> f64 {
        self.time_s
    }

    /// Intrinsic-2D force exerted on the fluid domain by one named boundary.
    #[must_use]
    pub fn named_boundary_force_on_domain(&self, name: &str) -> Option<[f64; 2]> {
        self.named_boundary_forces_on_domain
            .iter()
            .find_map(|(candidate, force)| (candidate == name).then_some(*force))
    }

    #[must_use]
    pub fn velocity_vertex_values(&self) -> Option<&[[f64; 2]]> {
        match &self.kind {
            CommonStateKind::MiniP1(state) => Some(state.velocity().vertex_values()),
            CommonStateKind::CellCentered(_) => None,
            CommonStateKind::Fsi { state, .. } => Some(state.vertex_velocity()),
        }
    }

    #[must_use]
    pub fn velocity_cell_values(&self) -> &[[f64; 2]] {
        match &self.kind {
            CommonStateKind::MiniP1(state) => state.velocity().cell_bubble_values(),
            CommonStateKind::CellCentered(state) => state.velocity().values(),
            CommonStateKind::Fsi { state, .. } => state.fluid_cell_bubble_velocity(),
        }
    }

    #[must_use]
    pub fn pressure_vertex_values(&self) -> Option<&[f64]> {
        match &self.kind {
            CommonStateKind::MiniP1(state) => Some(state.pressure().vertex_values()),
            CommonStateKind::CellCentered(_) => None,
            CommonStateKind::Fsi { pressure, .. } => Some(pressure),
        }
    }

    #[must_use]
    pub fn pressure_cell_values(&self) -> Option<&[f64]> {
        match &self.kind {
            CommonStateKind::MiniP1(_) => None,
            CommonStateKind::CellCentered(state) => Some(state.pressure().values()),
            CommonStateKind::Fsi { .. } => None,
        }
    }

    #[must_use]
    pub fn fsi_solid_displacement_values(&self) -> Option<&[[f64; 2]]> {
        match &self.kind {
            CommonStateKind::Fsi { state, .. } => Some(state.solid_displacement()),
            _ => None,
        }
    }

    #[must_use]
    pub fn fsi_accepted_solution(&self) -> Option<&ResolvedFixedReferenceFsiSolution2d> {
        match &self.kind {
            CommonStateKind::Fsi { accepted, .. } => accepted.as_deref(),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn method_history_values(&self) -> &[f64] {
        match &self.kind {
            CommonStateKind::MiniP1(_) => &[],
            CommonStateKind::CellCentered(state) => state.previous_face_volume_fluxes(),
            CommonStateKind::Fsi { .. } => &[],
        }
    }
}

#[cfg(test)]
mod prepared_action_tests {
    use std::cell::Cell;
    use std::ops::ControlFlow;

    use super::advance_prepared_actions;

    #[test]
    fn prepares_once_and_observes_exact_accepted_boundaries() {
        let prepares = Cell::new(0);
        let advances = Cell::new(0);
        let boundaries = Cell::new(0);
        let outcome = advance_prepared_actions(
            0_u64,
            10,
            &[3, 10],
            |_| {
                prepares.set(prepares.get() + 1);
                Ok::<_, ()>("prepared")
            },
            |prepared, accepted| {
                assert_eq!(*prepared, "prepared");
                advances.set(advances.get() + 1);
                Ok(accepted + 1)
            },
            |_, _| {
                boundaries.set(boundaries.get() + 1);
                false
            },
        )
        .unwrap();

        assert_eq!(prepares.get(), 1);
        assert_eq!(advances.get(), 10);
        assert_eq!(boundaries.get(), 11);
        assert_eq!(outcome, ControlFlow::Continue(vec![(3, 3), (10, 10)]));
    }

    #[test]
    fn cancellation_and_failure_never_publish_a_candidate_state() {
        for cancellation_boundary in [0, 4] {
            let outcome = advance_prepared_actions(
                0_u64,
                10,
                &[10],
                |_| Ok::<_, ()>("prepared"),
                |prepared, accepted| {
                    assert_eq!(*prepared, "prepared");
                    Ok(accepted + 1)
                },
                |boundary, _| boundary == cancellation_boundary,
            )
            .unwrap();
            assert_eq!(
                outcome,
                ControlFlow::Break((cancellation_boundary, cancellation_boundary as u64))
            );
        }

        let last_observed = Cell::new(0_u64);
        let failure = advance_prepared_actions(
            0_u64,
            10,
            &[10],
            |_| Ok::<_, &'static str>("prepared"),
            |prepared, accepted| {
                assert_eq!(*prepared, "prepared");
                if *accepted == 3 {
                    Err("candidate failed")
                } else {
                    Ok(accepted + 1)
                }
            },
            |_, accepted| {
                last_observed.set(*accepted);
                false
            },
        );
        assert_eq!(failure, Err("candidate failed"));
        assert_eq!(last_observed.get(), 3);
    }
}
