//! **eqiora-runtime** — execution paths derived from validated kernel models.
//!
//! The first Rust CPU path lowers every residual DAG to scalar Operator IR and
//! runs it under the reference activation/numerical engine. This deliberately
//! isolates and tests operator lowering before introducing a second scheduler,
//! sparse solver, or target-specific code generator.

use std::collections::BTreeMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath, RawId};
use eqiora_ir::ScalarOperatorIr;
use eqiora_schema::kernel::{ExprDag, KernelNode, SymbolRef};
use eqiora_sem::{ExpressionBackend, Interpreter, KernelProgram, ReferenceConfig, Trajectory};

mod hybrid;
mod implicit_time;
mod time;

pub use hybrid::{CanonicalEventLinearization, CanonicalEventProgram, CanonicalRootSet};
pub use implicit_time::{GeneralImplicitProgram, ImplicitEulerStepLinearization};
pub use time::FirstOrderProgram;

/// A validated kernel model plus independently lowered scalar Operator IR.
#[derive(Debug, Clone, PartialEq)]
pub struct CpuProgram {
    kernel: KernelProgram,
    operators: BTreeMap<RawId, ScalarOperatorIr>,
}

impl CpuProgram {
    /// Lower every Relation in a validated immutable model.
    ///
    /// # Errors
    /// Returns IR diagnostics if a canonical expression cannot be represented
    /// by the scalar CPU instruction set.
    pub fn lower(kernel: &KernelProgram) -> Result<Self, Vec<Diagnostic>> {
        let mut operators = BTreeMap::new();
        let mut diagnostics = Vec::new();
        for node in kernel.nodes() {
            if let KernelNode::Relation(relation) = node {
                match ScalarOperatorIr::lower(relation.residuals()) {
                    Ok(operator) => {
                        operators.insert(relation.id().erase(), operator);
                    }
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
        }
        if diagnostics.is_empty() {
            Ok(Self {
                kernel: kernel.clone(),
                operators,
            })
        } else {
            Err(diagnostics)
        }
    }

    /// Immutable semantic input captured by this CPU program.
    #[must_use]
    pub const fn kernel(&self) -> &KernelProgram {
        &self.kernel
    }

    /// Lowered Relation program.
    #[must_use]
    pub fn operator(&self, relation: RawId) -> Option<&ScalarOperatorIr> {
        self.operators.get(&relation)
    }
}

/// Rust CPU conformance executor.
#[derive(Debug, Default)]
pub struct CpuExecutor {
    interpreter: Interpreter,
}

impl CpuExecutor {
    /// Construct the stateless executor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute lowered Operator IR with the normative activation calendar and
    /// reference numerical controls.
    ///
    /// # Errors
    /// Returns the same execution diagnostics as the reference interpreter,
    /// plus Operator IR input/structure diagnostics.
    pub fn run(
        &self,
        program: &CpuProgram,
        config: ReferenceConfig,
    ) -> Result<Trajectory, Vec<Diagnostic>> {
        self.interpreter.run_with_expression_backend(
            &program.kernel,
            config,
            &CpuExpressionBackend {
                operators: &program.operators,
            },
        )
    }
}

struct CpuExpressionBackend<'a> {
    operators: &'a BTreeMap<RawId, ScalarOperatorIr>,
}

impl ExpressionBackend for CpuExpressionBackend<'_> {
    fn evaluate(
        &self,
        owner: RawId,
        _expression: &ExprDag,
        resolve: &mut dyn FnMut(SymbolRef) -> Option<f64>,
    ) -> Result<Vec<f64>, Diagnostic> {
        let operator = self.operators.get(&owner).ok_or_else(|| {
            Diagnostic::error(
                codes::INVALID_OPERATOR_IR,
                format!("Relation {owner} has no lowered CPU Operator IR"),
            )
            .with_graph_path(relation_path(owner))
        })?;
        let inputs = operator
            .symbols()
            .iter()
            .map(|symbol| {
                resolve(*symbol).ok_or_else(|| {
                    Diagnostic::error(
                        codes::MISSING_EXECUTION_INPUT,
                        format!("no CPU execution value is available for {symbol:?}"),
                    )
                    .with_graph_path(relation_path(owner))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        operator.evaluate(&inputs)
    }
}

fn relation_path(id: RawId) -> GraphPath {
    GraphPath::new(["cpu-program", "relation", &id.to_string()])
}
