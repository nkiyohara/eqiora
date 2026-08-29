//! Model-first admission for non-spatial canonical explicit ODEs.

use std::sync::Arc;

use eqiora_artifact::{CanonicalModelArtifact, ModelEnvelope, TimeLoweringEnvelopeV1};
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id};
use eqiora_runtime::{CpuProgram, FirstOrderProgram};
use eqiora_schema::kernel::{ActivationKind, KernelNode};
use eqiora_sem::KernelProgram;
use eqiora_time::{
    InitialConditionPolicy, TimeBackendIdentity, TimeEquationClass, TimeMethod, TimePlan,
    TimeProblem, TimeSolution,
};
use sha2::{Digest, Sha256};

mod state_artifact;

/// One exact Field-bound absolute tolerance for Tsitouras 5(4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommonTsitourasTolerance {
    field: Id<kinds::Field>,
    value: f64,
}

impl CommonTsitourasTolerance {
    /// Construct one positive finite coherent-SI tolerance.
    pub fn new(field: Id<kinds::Field>, value: f64) -> Result<Self, Diagnostic> {
        require_positive(value, "Tsitouras45 absolute tolerances")?;
        Ok(Self { field, value })
    }

    /// Exact canonical Field receiving this tolerance.
    #[must_use]
    pub const fn field(self) -> Id<kinds::Field> {
        self.field
    }

    /// Positive coherent-SI tolerance value.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.value
    }
}

/// Closed adaptive Tsitouras 5(4) request before Model admission.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonTsitouras45 {
    initial_step_s: f64,
    relative_tolerance: f64,
    absolute_tolerances: Vec<CommonTsitourasTolerance>,
}

impl CommonTsitouras45 {
    /// Construct finite positive adaptive controls with no duplicate Field.
    pub fn new(
        initial_step_s: f64,
        relative_tolerance: f64,
        mut absolute_tolerances: Vec<CommonTsitourasTolerance>,
    ) -> Result<Self, Diagnostic> {
        require_positive(initial_step_s, "Tsitouras45 initial_step_s")?;
        require_positive(relative_tolerance, "Tsitouras45 relative_tolerance")?;
        if absolute_tolerances.is_empty() {
            return Err(invalid(
                "Tsitouras45 requires one exact Field-bound absolute tolerance per state",
            ));
        }
        absolute_tolerances.sort_by_key(|entry| entry.field().ulid().to_string());
        if absolute_tolerances
            .windows(2)
            .any(|pair| pair[0].field() == pair[1].field())
        {
            return Err(invalid(
                "Tsitouras45 absolute tolerances contain a duplicate exact Field",
            ));
        }
        Ok(Self {
            initial_step_s,
            relative_tolerance,
            absolute_tolerances,
        })
    }

    /// Initial adaptive step-size guess in coherent SI seconds.
    #[must_use]
    pub const fn initial_step_s(&self) -> f64 {
        self.initial_step_s
    }

    /// Relative local-error tolerance.
    #[must_use]
    pub const fn relative_tolerance(&self) -> f64 {
        self.relative_tolerance
    }

    /// Canonically Field-ordered absolute tolerances.
    #[must_use]
    pub fn absolute_tolerances(&self) -> &[CommonTsitourasTolerance] {
        &self.absolute_tolerances
    }
}

/// Opaque no-Mesh Plan for one structurally proven explicit ODE.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonOdePlan {
    model: Arc<ModelEnvelope>,
    program: FirstOrderProgram,
    temporal: CommonTsitouras45,
    ordered_absolute_tolerances: Vec<f64>,
    field_dimensions: Vec<DimExponents>,
    identity: String,
    lowering_digest: String,
    model_id: String,
    model_digest: String,
    model_revision: u64,
    state_space_identity: String,
    backend: TimeBackendIdentity,
}

impl CommonOdePlan {
    pub(crate) fn model_artifact(&self) -> &ModelEnvelope {
        &self.model
    }

    /// Resolve one exact Model through canonical first-order structural lowering.
    pub fn resolve(
        model: &ModelEnvelope,
        kernel: &KernelProgram,
        temporal: CommonTsitouras45,
        backend: TimeBackendIdentity,
    ) -> Result<Self, Diagnostic> {
        let reference = model.artifact_reference()?;
        let cpu = CpuProgram::lower(kernel).map_err(|diagnostics| {
            diagnostics.into_iter().next().unwrap_or_else(|| {
                invalid("canonical explicit-ODE lowering failed without a diagnostic")
            })
        })?;
        let relations = kernel
            .nodes()
            .filter_map(|node| match node {
                KernelNode::Relation(relation) => Some(relation.id()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let activations = kernel
            .nodes()
            .filter_map(|node| match node {
                KernelNode::Activation(activation) => Some(activation.kind()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if relations.len() != 1
            || activations.len() != 1
            || !matches!(activations[0], ActivationKind::Continuous)
        {
            return Err(invalid(
                "no-Mesh explicit-ODE resolution requires exactly one continuously activated Relation and no other activation family",
            ));
        }
        let program = FirstOrderProgram::lower(&cpu, relations[0])?;
        if program.equation_class() != TimeEquationClass::ExplicitOde {
            return Err(invalid(
                "Tsitouras45 admits only a structurally proven explicit ODE",
            ));
        }
        if program.initial_condition_policy() != InitialConditionPolicy::Provided {
            return Err(invalid(
                "explicit-ODE resolution requires complete Model-owned initial values",
            ));
        }
        let state_fields = program.state_fields();
        let requested = temporal.absolute_tolerances();
        if requested.len() != state_fields.len()
            || requested
                .iter()
                .any(|entry| !state_fields.contains(&entry.field()))
        {
            return Err(invalid(
                "Tsitouras45 absolute tolerances must cover exactly the admitted Model state Fields",
            ));
        }
        let ordered_absolute_tolerances = state_fields
            .iter()
            .map(|field| {
                requested
                    .iter()
                    .find(|entry| entry.field() == *field)
                    .map(|entry| entry.value())
                    .ok_or_else(|| invalid("Tsitouras45 omitted one exact state Field"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let field_dimensions = state_fields
            .iter()
            .map(|field| match kernel.node(field.erase()) {
                Some(KernelNode::Field(definition)) if definition.shape().is_scalar() => {
                    Ok(definition.dimension())
                }
                _ => Err(invalid(
                    "no-Mesh explicit-ODE State admits only exact scalar Model Fields",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let lowering = TimeLoweringEnvelopeV1::from_proof(model, kernel, program.lowering_proof())?;
        let lowering_digest = lowering.digest()?.to_string();
        let model_digest = reference.artifact().to_string();
        let model_id = reference.model().ulid().to_string();
        let model_revision = reference.semantic_revision().get();

        let mut state_space = Vec::new();
        push(&mut state_space, model_id.as_bytes());
        push(&mut state_space, model_digest.as_bytes());
        state_space.extend_from_slice(&model_revision.to_be_bytes());
        push(&mut state_space, lowering_digest.as_bytes());
        for (field, dimension) in state_fields.iter().zip(&field_dimensions) {
            push(&mut state_space, field.ulid().to_string().as_bytes());
            state_space.extend_from_slice(&dimension_bytes(*dimension));
        }
        push(&mut state_space, b"scalar-f64/no-method-history/v1");
        let state_space_identity = digest(b"eqiora.common-ode-state-space/v1\0", &state_space);

        let mut identity = state_space;
        identity.extend_from_slice(&temporal.initial_step_s().to_bits().to_be_bytes());
        identity.extend_from_slice(&temporal.relative_tolerance().to_bits().to_be_bytes());
        for tolerance in &ordered_absolute_tolerances {
            identity.extend_from_slice(&tolerance.to_bits().to_be_bytes());
        }
        push(&mut identity, b"tsitouras45");
        push(&mut identity, backend.id().as_str().as_bytes());
        push(&mut identity, backend.version().as_str().as_bytes());
        push(&mut identity, b"host-serial");
        let identity = digest(b"eqiora.common-ode-plan/v1\0", &identity);

        Ok(Self {
            model: Arc::new(model.clone()),
            program,
            temporal,
            ordered_absolute_tolerances,
            field_dimensions,
            identity,
            lowering_digest,
            model_id,
            model_digest,
            model_revision,
            state_space_identity,
            backend,
        })
    }

    /// Exact Plan identity excluding Run horizon and output schedule.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    #[must_use]
    pub fn model_digest(&self) -> &str {
        &self.model_digest
    }

    #[must_use]
    pub const fn model_revision(&self) -> u64 {
        self.model_revision
    }

    #[must_use]
    pub fn lowering_digest(&self) -> &str {
        &self.lowering_digest
    }

    #[must_use]
    pub const fn temporal(&self) -> &CommonTsitouras45 {
        &self.temporal
    }

    #[must_use]
    pub fn field_ids(&self) -> impl ExactSizeIterator<Item = Id<kinds::Field>> + '_ {
        self.program.state_fields().iter().copied()
    }

    #[must_use]
    pub fn field_dimensions(&self) -> &[DimExponents] {
        &self.field_dimensions
    }

    #[must_use]
    pub fn state_space_identity(&self) -> &str {
        &self.state_space_identity
    }

    /// Exact time backend selected by resolution.
    #[must_use]
    pub const fn backend(&self) -> TimeBackendIdentity {
        self.backend
    }

    /// Construct the exact Model-owned initial state at model time zero.
    pub fn initial_state(&self) -> Result<CommonOdeState, Diagnostic> {
        self.model.artifact_reference()?;
        CommonOdeState::new(self, 0.0, self.program.initial_state().to_vec(), "initial")
    }

    fn problem<'a>(&'a self, state: &CommonOdeState) -> Result<TimeProblem<'a>, Diagnostic> {
        if state.state_space_identity() != self.state_space_identity() {
            return Err(invalid(
                "State belongs to a different exact no-Mesh ODE state space",
            ));
        }
        TimeProblem::new(
            &self.program,
            TimeEquationClass::ExplicitOde,
            InitialConditionPolicy::Provided,
            state.values.clone(),
        )
    }
}

/// Complete accepted scalar state for one no-Mesh ODE state space.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonOdeState {
    state_space_identity: String,
    identity: String,
    model_digest: String,
    time_s: f64,
    field_ids: Vec<Id<kinds::Field>>,
    dimensions: Vec<DimExponents>,
    values: Vec<f64>,
    source_kind: &'static str,
}

impl CommonOdeState {
    fn new(
        plan: &CommonOdePlan,
        time_s: f64,
        values: Vec<f64>,
        source_kind: &'static str,
    ) -> Result<Self, Diagnostic> {
        if !time_s.is_finite()
            || time_s < 0.0
            || time_s.to_bits() == (-0.0_f64).to_bits()
            || values.len() != plan.program.state_fields().len()
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(invalid(
                "no-Mesh ODE State requires finite time and one finite value per exact state Field",
            ));
        }
        let mut bytes = Vec::new();
        push(&mut bytes, plan.state_space_identity().as_bytes());
        bytes.extend_from_slice(&time_s.to_bits().to_be_bytes());
        for value in &values {
            bytes.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        let identity = digest(b"eqiora.common-ode-state/v1\0", &bytes);
        Ok(Self {
            state_space_identity: plan.state_space_identity().to_owned(),
            identity,
            model_digest: plan.model_digest().to_owned(),
            time_s,
            field_ids: plan.field_ids().collect(),
            dimensions: plan.field_dimensions.clone(),
            values,
            source_kind,
        })
    }

    #[must_use]
    pub fn state_space_identity(&self) -> &str {
        &self.state_space_identity
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    #[must_use]
    pub fn model_digest(&self) -> &str {
        &self.model_digest
    }

    #[must_use]
    pub const fn time_s(&self) -> f64 {
        self.time_s
    }

    #[must_use]
    pub fn field_ids(&self) -> &[Id<kinds::Field>] {
        &self.field_ids
    }

    #[must_use]
    pub fn dimensions(&self) -> &[DimExponents] {
        &self.dimensions
    }

    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    #[must_use]
    pub const fn source_kind(&self) -> &'static str {
        self.source_kind
    }
}

/// Exact adaptive horizon/output request over one accepted no-Mesh State.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonOdeRunRequest {
    plan: CommonOdePlan,
    state: CommonOdeState,
    until_s: f64,
    output_times_s: Vec<f64>,
    execution_times_s: Vec<f64>,
    time_plan: TimePlan,
    identity: String,
}

impl CommonOdeRunRequest {
    /// Bind Run-only time controls without changing Plan identity.
    pub fn new(
        plan: CommonOdePlan,
        state: CommonOdeState,
        until_s: f64,
        output_times_s: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        if state.state_space_identity() != plan.state_space_identity() {
            return Err(invalid(
                "State belongs to a different exact no-Mesh ODE state space",
            ));
        }
        if !until_s.is_finite() || until_s <= state.time_s() {
            return Err(invalid(
                "ODE Run until_s must be finite and later than State.time_s",
            ));
        }
        if output_times_s.is_empty()
            || output_times_s
                .iter()
                .any(|time| !time.is_finite() || *time <= state.time_s() || *time > until_s)
            || output_times_s.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid(
                "ODE output_times_s must be finite, nonempty, strictly increasing, later than State.time_s, and within until_s",
            ));
        }
        let mut execution_times_s = output_times_s.clone();
        if execution_times_s
            .last()
            .is_none_or(|time| time.to_bits() != until_s.to_bits())
        {
            execution_times_s.push(until_s);
        }
        let time_plan = TimePlan::new(
            TimeMethod::Tsitouras45,
            state.time_s(),
            plan.temporal.initial_step_s(),
            plan.temporal.relative_tolerance(),
            plan.ordered_absolute_tolerances.clone(),
            execution_times_s.clone(),
        )?;
        time_plan.validate_for(&plan.problem(&state)?)?;
        let mut bytes = Vec::new();
        push(&mut bytes, plan.identity().as_bytes());
        push(&mut bytes, state.identity().as_bytes());
        bytes.extend_from_slice(&until_s.to_bits().to_be_bytes());
        for time in &output_times_s {
            bytes.extend_from_slice(&time.to_bits().to_be_bytes());
        }
        let identity = digest(b"eqiora.common-ode-run-request/v1\0", &bytes);
        Ok(Self {
            plan,
            state,
            until_s,
            output_times_s,
            execution_times_s,
            time_plan,
            identity,
        })
    }

    #[must_use]
    pub const fn plan(&self) -> &CommonOdePlan {
        &self.plan
    }

    #[must_use]
    pub const fn state(&self) -> &CommonOdeState {
        &self.state
    }

    #[must_use]
    pub const fn until_s(&self) -> f64 {
        self.until_s
    }

    #[must_use]
    pub fn output_times_s(&self) -> &[f64] {
        &self.output_times_s
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Reconstruct the backend-neutral problem from exact Plan and State.
    pub fn problem(&self) -> Result<TimeProblem<'_>, Diagnostic> {
        self.plan.problem(&self.state)
    }

    /// Complete internal time execution controls, including an unrequested horizon sample.
    #[must_use]
    pub const fn time_plan(&self) -> &TimePlan {
        &self.time_plan
    }
}

/// Requested accepted States from one completed adaptive ODE Run.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonOdeRunResult {
    request_identity: String,
    states: Vec<CommonOdeState>,
}

impl CommonOdeRunResult {
    /// Reaccept a backend solution against the exact request and discard only
    /// the internal unrequested horizon sample.
    pub fn accept(
        request: &CommonOdeRunRequest,
        solution: TimeSolution,
    ) -> Result<Self, Diagnostic> {
        if solution.report().method() != TimeMethod::Tsitouras45
            || solution.report().backend_identity() != request.plan.backend()
            || solution.report().equation_class() != TimeEquationClass::ExplicitOde
            || solution.report().initial_condition() != InitialConditionPolicy::Provided
            || solution.dimension() != request.plan.field_dimensions.len()
            || solution.times() != request.execution_times_s
        {
            return Err(invalid(
                "adaptive backend result differs from the exact no-Mesh ODE request",
            ));
        }
        let states = request
            .output_times_s
            .iter()
            .enumerate()
            .map(|(sample, &time)| {
                let values = solution
                    .state(sample)
                    .ok_or_else(|| invalid("adaptive backend omitted one requested ODE State"))?;
                CommonOdeState::new(&request.plan, time, values.to_vec(), "result")
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            request_identity: request.identity.clone(),
            states,
        })
    }

    #[must_use]
    pub fn request_identity(&self) -> &str {
        &self.request_identity
    }

    #[must_use]
    pub fn states(&self) -> &[CommonOdeState] {
        &self.states
    }
}

fn require_positive(value: f64, label: &str) -> Result<(), Diagnostic> {
    if !value.is_finite() || value <= 0.0 || value.to_bits() == (-0.0_f64).to_bits() {
        return Err(invalid(format!(
            "{label} must be finite and strictly positive"
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn push(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value);
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let value = Sha256::digest([domain, bytes].concat());
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn dimension_bytes(value: DimExponents) -> [u8; 7] {
    [
        value.mass as u8,
        value.length as u8,
        value.time as u8,
        value.current as u8,
        value.temperature as u8,
        value.amount as u8,
        value.luminous_intensity as u8,
    ]
}

#[cfg(test)]
mod tests {
    use eqiora_artifact::ModelEnvelope;
    use eqiora_core::{DynQuantity, OntologyId};
    use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
    use eqiora_schema::kernel::{
        ActivationDef, ExprDagBuilder, FieldDef, ParameterDef, RelationDef, SymbolRef,
    };
    use eqiora_schema::{Model, ModelView};
    use eqiora_sem::KernelProgram;
    use eqiora_solver::REFERENCE_LINEAR_SOLVER;
    use eqiora_time::TimeBackendIdentity;

    use super::*;

    const DECAY: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;

    fn fixture() -> (ModelEnvelope, KernelProgram) {
        let compiled = eqiora_compiler::compile("decay.eqi", DECAY)
            .unwrap()
            .pop()
            .unwrap();
        let (transaction, model, _) = compiled.into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
        let envelope = ModelEnvelope::from_program(&program).unwrap();
        (envelope, program)
    }

    fn two_state_fixture(
        mass_matrix: bool,
    ) -> (
        ModelEnvelope,
        KernelProgram,
        Id<kinds::Field>,
        Id<kinds::Field>,
    ) {
        let inverse_time = DimExponents {
            time: -1,
            ..DimExponents::DIMENSIONLESS
        };
        let decay = Id::<kinds::Field>::new();
        let integral = Id::<kinds::Field>::new();
        let rate = Id::<kinds::Parameter>::new();
        let relation = Id::<kinds::Relation>::new();
        let continuous = Id::<kinds::Activation>::new();
        let model = OntologyId::<Model>::new();

        let mut expression = ExprDagBuilder::new();
        let decay_derivative = expression.symbol(SymbolRef::Derivative(decay)).unwrap();
        let integral_derivative = expression.symbol(SymbolRef::Derivative(integral)).unwrap();
        let decay_value = expression.symbol(SymbolRef::Field(decay)).unwrap();
        let rate_value = expression.symbol(SymbolRef::Parameter(rate)).unwrap();
        let decay_rate = expression.mul(rate_value, decay_value).unwrap();
        let integral_residual = expression.sub(integral_derivative, decay_rate).unwrap();
        let decay_residual = expression.add(decay_derivative, decay_rate).unwrap();
        let decay_residual = if mass_matrix {
            expression.add(decay_residual, integral_derivative).unwrap()
        } else {
            decay_residual
        };
        let residuals = expression
            .finish([decay_residual, integral_residual])
            .unwrap();

        let nodes = [
            KernelNode::from(
                FieldDef::new(decay, DimExponents::DIMENSIONLESS)
                    .with_initial(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
                    .unwrap(),
            ),
            KernelNode::from(
                FieldDef::new(integral, DimExponents::DIMENSIONLESS)
                    .with_initial(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
                    .unwrap(),
            ),
            KernelNode::from(ParameterDef::new(rate, DynQuantity::new(1.0, inverse_time))),
            KernelNode::from(RelationDef::new(relation, residuals)),
            KernelNode::from(ActivationDef::continuous(continuous)),
        ];
        let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
        let mut transaction = Transaction::new("two-state explicit ODE");
        for node in nodes {
            transaction.push(Op::DefineKernelNode { node });
        }
        for dependency in [decay.erase(), integral.erase(), rate.erase()] {
            transaction.push(Op::Connect {
                from: relation.erase(),
                to: dependency,
                edge: EdgeKind::DependsOn,
            });
        }
        transaction
            .push(Op::Connect {
                from: continuous.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            })
            .push(Op::DefineOntologyView {
                view: ModelView::new(model, members, []).unwrap().into(),
            });
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
        let envelope = ModelEnvelope::from_program(&program).unwrap();
        (envelope, program, decay, integral)
    }

    #[test]
    fn no_mesh_plan_owns_model_initial_state_and_run_only_horizon() {
        let (model, program) = fixture();
        let field = program
            .nodes()
            .find_map(|node| match node {
                KernelNode::Field(field) => Some(field.id()),
                _ => None,
            })
            .unwrap();
        let temporal = CommonTsitouras45::new(
            0.01,
            1.0e-9,
            vec![CommonTsitourasTolerance::new(field, 1.0e-11).unwrap()],
        )
        .unwrap();
        let plan = CommonOdePlan::resolve(
            &model,
            &program,
            temporal,
            TimeBackendIdentity::new("eqiora.test.time", "1"),
        )
        .unwrap();
        let resolved = crate::ResolvedCommonPlan::Ode(Box::new(plan.clone()));
        let bytes = resolved.to_bytes().unwrap();
        assert_eq!(
            crate::ResolvedCommonPlan::from_bytes(
                &bytes,
                &REFERENCE_LINEAR_SOLVER,
                TimeBackendIdentity::new("eqiora.test.time", "1"),
            )
            .unwrap(),
            resolved
        );
        let state = plan.initial_state().unwrap();
        assert_eq!(state.time_s(), 0.0);
        assert_eq!(state.field_ids(), &[field]);
        assert_eq!(state.values(), &[1.0]);
        let state_bytes = state.to_bytes().unwrap();
        assert_eq!(
            CommonOdeState::from_bytes(&state_bytes, &plan).unwrap(),
            state
        );
        let mut noncanonical_state = state_bytes;
        noncanonical_state.push(b'\n');
        assert!(CommonOdeState::from_bytes(&noncanonical_state, &plan).is_err());

        let request =
            CommonOdeRunRequest::new(plan.clone(), state.clone(), 0.2, vec![0.1]).unwrap();
        assert_eq!(request.output_times_s(), &[0.1]);
        assert_eq!(request.time_plan().output_times(), &[0.1, 0.2]);
        assert_eq!(request.plan().identity(), plan.identity());
        assert!(CommonOdeRunRequest::new(plan, state, 0.2, vec![0.0]).is_err());
    }

    #[test]
    fn field_tolerances_are_exact_complete_and_positive() {
        let (model, program) = fixture();
        let field = program
            .nodes()
            .find_map(|node| match node {
                KernelNode::Field(field) => Some(field.id()),
                _ => None,
            })
            .unwrap();
        assert!(CommonTsitourasTolerance::new(field, 0.0).is_err());
        assert!(CommonTsitouras45::new(0.01, 1.0e-9, Vec::new()).is_err());
        let foreign = Id::<kinds::Field>::new();
        let temporal = CommonTsitouras45::new(
            0.01,
            1.0e-9,
            vec![CommonTsitourasTolerance::new(foreign, 1.0e-11).unwrap()],
        )
        .unwrap();
        assert!(
            CommonOdePlan::resolve(
                &model,
                &program,
                temporal,
                TimeBackendIdentity::new("eqiora.test.time", "1"),
            )
            .is_err()
        );
    }

    #[test]
    fn field_tolerances_map_to_canonical_first_order_state_coordinates() {
        let (model, program, decay, integral) = two_state_fixture(false);
        let temporal = CommonTsitouras45::new(
            0.01,
            1.0e-9,
            vec![
                CommonTsitourasTolerance::new(integral, 2.0e-11).unwrap(),
                CommonTsitourasTolerance::new(decay, 1.0e-11).unwrap(),
            ],
        )
        .unwrap();
        let plan = CommonOdePlan::resolve(
            &model,
            &program,
            temporal,
            TimeBackendIdentity::new("eqiora.test.time", "1"),
        )
        .unwrap();

        assert_eq!(plan.field_ids().collect::<Vec<_>>(), [decay, integral]);
        assert_eq!(plan.ordered_absolute_tolerances, [1.0e-11, 2.0e-11]);
        assert_eq!(
            plan.field_dimensions(),
            [DimExponents::DIMENSIONLESS, DimExponents::DIMENSIONLESS]
        );
        let initial = plan.initial_state().unwrap();
        assert_eq!(initial.field_ids(), [decay, integral]);
        assert_eq!(initial.values(), [1.0, 0.0]);
    }

    #[test]
    fn tsitouras_common_plan_rejects_a_structural_mass_matrix() {
        let (model, program, decay, integral) = two_state_fixture(true);
        let temporal = CommonTsitouras45::new(
            0.01,
            1.0e-9,
            vec![
                CommonTsitourasTolerance::new(decay, 1.0e-11).unwrap(),
                CommonTsitourasTolerance::new(integral, 2.0e-11).unwrap(),
            ],
        )
        .unwrap();
        assert!(
            CommonOdePlan::resolve(
                &model,
                &program,
                temporal,
                TimeBackendIdentity::new("eqiora.test.time", "1"),
            )
            .is_err()
        );
    }
}
