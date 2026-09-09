//! Exact no-Mesh lifecycle for a finite scalar affine physical closure.

use super::*;
use crate::physical_network::{
    ScalarPhysicalAffineProblem, lower_scalar_physical_affine,
    solve_scalar_physical_affine_with_initial_guess,
};
use eqiora_schema::kernel::{KernelNode, SymbolRef};
use eqiora_sem::PhysicalUnknown;
use eqiora_solver::{FixedOrderInnerProduct, ReplicatedLinearExecution, SERIAL_LINEAR_EXECUTION};

/// One exact finite scalar physical Model and its admitted linear policy.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonAlgebraicPlan {
    model: Arc<ModelEnvelope>,
    kernel: KernelProgram,
    problem: ScalarPhysicalAffineProblem,
    pub(super) linear: NativeLinearPolicy,
    symbols: Vec<SymbolRef>,
    dimensions: Vec<DimExponents>,
    identity: String,
    model_id: String,
    model_digest: String,
    model_revision: u64,
}

/// Initial numerical values bound to one exact finite Plan.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonAlgebraicState {
    plan_identity: String,
    identity: String,
    values: Vec<f64>,
}

impl CommonAlgebraicState {
    /// Canonical exact initial-State encoding.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&(
            "eqiora.common-algebraic-state/v1",
            &self.plan_identity,
            &self.identity,
            &self.values,
        ))
        .map_err(|e| invalid(format!("cannot encode finite State: {e}")))
    }
    /// Reauthenticate the complete initial State against its exact Plan.
    pub fn from_bytes(bytes: &[u8], plan: &CommonAlgebraicPlan) -> Result<Self, Diagnostic> {
        let expected = plan.initial_state()?;
        if bytes != expected.to_bytes()? {
            return Err(invalid(
                "finite State bytes differ from exact Plan initial State",
            ));
        }
        Ok(expected)
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }
}

impl CommonAlgebraicPlan {
    pub fn resolve(
        model: &ModelEnvelope,
        solve: CommonSolvePolicy,
        backend: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        let CommonSolvePolicy::Linear(request) = solve else {
            return Err(invalid("finite affine Plan requires a linear solve policy"));
        };
        let kernel = model.to_program().map_err(|errors| {
            errors
                .into_iter()
                .next()
                .unwrap_or_else(|| invalid("finite Model replay failed"))
        })?;
        if kernel.nodes().any(|node| {
            matches!(node, KernelNode::Field(_) | KernelNode::ClockDomain(_))
                || matches!(node, KernelNode::Port(port) if port.signal_contract().is_some())
        }) {
            return Err(invalid(
                "finite physical Plan does not admit unresolved Fields or clocked/signal execution",
            ));
        }
        if request.objective().is_some() {
            return Err(invalid(
                "finite affine Plan requires an exact SparseLU/Identity/Fast request",
            ));
        }
        let connection = kernel
            .nodes()
            .filter_map(|node| match node {
                KernelNode::Connection(value) => Some(value.id()),
                _ => None,
            })
            .min_by_key(|id| id.ulid())
            .ok_or_else(|| invalid("finite physical Plan requires a conserving Connection"))?;
        let problem = lower_scalar_physical_affine(&kernel, connection, None)?;
        let composed = problem.composed_system();
        let symbols = composed
            .unknowns()
            .iter()
            .map(|unknown| match unknown {
                PhysicalUnknown::Across(port) => SymbolRef::Across(*port),
                PhysicalUnknown::Through(port) => SymbolRef::Through(*port),
            })
            .collect();
        let dimensions = composed
            .unknown_types()
            .iter()
            .map(|value| value.dimension())
            .collect();
        let physical_ports = kernel
            .nodes()
            .filter(
                |node| matches!(node, KernelNode::Port(port) if port.physical_domain().is_some()),
            )
            .count();
        let relation_count = kernel
            .nodes()
            .filter(|node| matches!(node, KernelNode::Relation(_)))
            .count();
        if physical_ports * 2 != composed.unknowns().len()
            || relation_count != composed.relations().len()
        {
            return Err(invalid(
                "finite Plan requires one complete connected physical closure with no omitted Relations",
            ));
        }
        let linear = solver_planning::resolve_linear(
            request,
            LinearOperatorProperties::General,
            None,
            backend,
        )?;
        if linear.solver.algorithm() != LinearSolver::SparseLu
            || linear.solver.preconditioner() != PreconditionerPolicy::Identity
            || linear.solver.reduction() != ReductionPolicy::Fast
        {
            return Err(invalid(
                "finite affine Plan admits only exact SparseLU/Identity/Fast execution",
            ));
        }
        let reference = model.artifact_reference()?;
        let model_digest = reference.artifact().to_string();
        let mut bytes = model_digest.as_bytes().to_vec();
        push_framed(&mut bytes, backend.provider().id().as_str().as_bytes());
        push_framed(
            &mut bytes,
            backend.provider().implementation_version().as_bytes(),
        );
        bytes.extend_from_slice(&request.relative_tolerance().to_bits().to_be_bytes());
        bytes.extend_from_slice(&request.absolute_tolerance().to_bits().to_be_bytes());
        bytes.extend_from_slice(&(request.maximum_iterations().get() as u64).to_be_bytes());
        let identity = finite_digest(b"eqiora.common-algebraic-plan/v1\0", &bytes);
        Ok(Self {
            model: Arc::new(model.clone()),
            kernel,
            problem,
            linear,
            symbols,
            dimensions,
            identity,
            model_id: reference.model().ulid().to_string(),
            model_digest,
            model_revision: reference.semantic_revision().get(),
        })
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
    pub fn model_digest(&self) -> &str {
        &self.model_digest
    }
    #[must_use]
    pub const fn model_revision(&self) -> u64 {
        self.model_revision
    }
    #[must_use]
    pub fn model_artifact(&self) -> &ModelEnvelope {
        &self.model
    }
    #[must_use]
    pub fn kernel(&self) -> &KernelProgram {
        &self.kernel
    }
    #[must_use]
    pub fn symbols(&self) -> &[SymbolRef] {
        &self.symbols
    }
    #[must_use]
    pub fn dimensions(&self) -> &[DimExponents] {
        &self.dimensions
    }
    #[must_use]
    pub const fn solver_provider(&self) -> SolverProvider {
        self.linear.provider
    }
    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.linear.solver
    }
    pub fn initial_state(&self) -> Result<CommonAlgebraicState, Diagnostic> {
        let values = vec![0.0; self.symbols.len()];
        Ok(CommonAlgebraicState {
            identity: finite_digest(
                b"eqiora.common-algebraic-initial-state/v1\0",
                self.identity.as_bytes(),
            ),
            plan_identity: self.identity.clone(),
            values,
        })
    }
    pub fn run_result(
        &self,
        state: &CommonAlgebraicState,
        backend: &dyn LinearSolverBackend,
    ) -> Result<crate::CommonResult, Diagnostic> {
        if state != &self.initial_state()? || backend.provider() != self.linear.provider {
            return Err(invalid(
                "finite Run requires its exact Plan-bound State and admitted provider",
            ));
        }
        let checked_backend = self.linear.checked_backend(backend)?;
        let solution = solve_scalar_physical_affine_with_initial_guess(
            &self.problem,
            &state.values,
            LinearSolveRequest::new(&checked_backend, self.linear.solver),
        )?;
        crate::CommonResult::from_algebraic(self, state, &solution)
    }
    pub(crate) fn validate_values(&self, values: &[f64], target: f64) -> Result<f64, Diagnostic> {
        let rhs = self.problem.canonical_system().right_hand_side();
        let rhs_norm = SERIAL_LINEAR_EXECUTION
            .inner_product(FixedOrderInnerProduct::new(rhs, rhs)?)?
            .sqrt();
        if self.linear.solver.residual_target(rhs_norm)?.to_bits() != target.to_bits() {
            return Err(invalid(
                "finite Result target differs from exact original right-hand side",
            ));
        }
        let residuals = self.problem.reference_residuals(values)?;
        let norm = SERIAL_LINEAR_EXECUTION
            .inner_product(FixedOrderInnerProduct::new(&residuals, &residuals)?)?
            .sqrt();
        if !norm.is_finite() || norm > target {
            return Err(invalid(
                "finite Result original semantic residual exceeds acceptance target",
            ));
        }
        Ok(norm)
    }
}

fn finite_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
