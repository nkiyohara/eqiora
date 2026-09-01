use std::cell::Cell;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_solver::{CanonicalCsrSystemView, LinearSolution};
use sha2::{Digest, Sha256};

use crate::{AcceptedLinearExecution, AdmittedExecution, DeploymentBinding};

const STRUCTURE_DOMAIN: &[u8] = b"eqiora.prepared-linear.structure/v1\0";
const COEFFICIENT_DOMAIN: &[u8] = b"eqiora.prepared-linear.coefficients/v1\0";

/// Opaque Eqiora-owned authority for the last accepted reusable candidate.
///
/// The value is ephemeral and has no serializer or raw identity accessor.
/// Keeping it is necessary but not sufficient for reuse: the provider must
/// retain its matching private payload inside the same prepared occurrence.
#[derive(Debug)]
struct PreparedLinearCommit {
    binding: DeploymentBinding,
    structure: [u8; 32],
    coefficients: [u8; 32],
}

/// One ephemeral prepared linear-execution occurrence.
///
/// Execution compares the exact deployment/graph/provider/plan binding and
/// canonical sparse identities before provider mutation. Eqiora acceptance is
/// the only operation that can replace the reusable commit authority.
#[derive(Debug, Default)]
pub struct PreparedLinearExecution {
    accepted: Option<PreparedLinearCommit>,
    _not_sync: Cell<()>,
}

impl PreparedLinearExecution {
    /// Start one empty ephemeral prepared occurrence.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            accepted: None,
            _not_sync: Cell::new(()),
        }
    }

    /// Execute one candidate against the last accepted commit, if any.
    ///
    /// A changed exact binding or sparse structure is foreign to the current
    /// prepared occurrence and is rejected before provider work. A changed
    /// right-hand side does not invalidate factors. The provider callback is
    /// told whether the last accepted finite coefficients remain reusable;
    /// provider-private cold-versus-symbolic mechanics remain private. Its
    /// one-shot acceptance callback must succeed before provider state may be
    /// committed and before this owner replaces its accepted authority.
    ///
    /// # Errors
    /// Returns `EQ0807` when an identity cannot be represented or the candidate
    /// belongs to another binding or sparse structure.
    pub fn execute<'system>(
        &mut self,
        admitted: AdmittedExecution<'system>,
        operation: impl FnOnce(
            &CanonicalCsrSystemView,
            &DeploymentBinding,
            bool,
            &mut dyn FnMut(LinearSolution) -> Result<(), Diagnostic>,
        ) -> Result<(), Diagnostic>,
    ) -> Result<AcceptedLinearExecution, Diagnostic> {
        let next = PreparedLinearCommit::from_admitted(&admitted)?;
        let coefficients_reusable = self.validate_successor(&next)?;
        let system = admitted.system();
        let binding = admitted.binding().clone();
        let mut admission = Some(admitted);
        let mut accepted = None;
        operation(system, &binding, coefficients_reusable, &mut |solution| {
            let admission = admission
                .take()
                .ok_or_else(|| invalid("prepared linear candidate accepted more than once"))?;
            accepted = Some(admission.accept(solution)?);
            Ok(())
        })?;
        let accepted = accepted
            .ok_or_else(|| invalid("prepared linear provider omitted execution acceptance"))?;
        self.accepted = Some(next);
        Ok(accepted)
    }

    fn validate_successor(&self, next: &PreparedLinearCommit) -> Result<bool, Diagnostic> {
        let Some(accepted) = self.accepted.as_ref() else {
            return Ok(false);
        };
        if accepted.binding != next.binding {
            return Err(invalid(
                "prepared linear candidate changed deployment, provider, plan, or portable graph",
            ));
        }
        if accepted.structure != next.structure {
            return Err(invalid(
                "prepared linear candidate changed canonical sparse structure",
            ));
        }
        Ok(accepted.coefficients == next.coefficients)
    }
}

impl PreparedLinearCommit {
    fn from_admitted(admitted: &AdmittedExecution<'_>) -> Result<Self, Diagnostic> {
        let system = admitted.system();
        Ok(Self {
            binding: admitted.binding().clone(),
            structure: structure_identity(system)?,
            coefficients: coefficient_identity(system)?,
        })
    }
}

fn structure_identity(system: &CanonicalCsrSystemView) -> Result<[u8; 32], Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(STRUCTURE_DOMAIN);
    update_usize(&mut hash, system.rows())?;
    update_usize(&mut hash, system.columns())?;
    update_slice(&mut hash, system.row_offsets())?;
    update_slice(&mut hash, system.column_indices())?;
    Ok(hash.finalize().into())
}

fn coefficient_identity(system: &CanonicalCsrSystemView) -> Result<[u8; 32], Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(COEFFICIENT_DOMAIN);
    update_usize(&mut hash, system.values().len())?;
    for value in system.values() {
        let bits = if *value == 0.0 { 0 } else { value.to_bits() };
        hash.update(bits.to_be_bytes());
    }
    Ok(hash.finalize().into())
}

fn update_slice(hash: &mut Sha256, values: &[usize]) -> Result<(), Diagnostic> {
    update_usize(hash, values.len())?;
    for value in values {
        update_usize(hash, *value)?;
    }
    Ok(())
}

fn update_usize(hash: &mut Sha256, value: usize) -> Result<(), Diagnostic> {
    let value = u64::try_from(value)
        .map_err(|_| invalid("prepared linear identity exceeds portable u64 range"))?;
    hash.update(value.to_be_bytes());
    Ok(())
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}
