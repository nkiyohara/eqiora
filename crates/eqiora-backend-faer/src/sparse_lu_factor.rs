use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_execution::DeploymentBinding;
use eqiora_solver::{CanonicalCsrSystemView, SolverPlan, SolverProvider};
use faer::dyn_stack::{MemBuffer, MemStack};
use faer::sparse::linalg::lu::{LuRef, NumericLu, SymbolicLu, factorize_symbolic_lu};
use faer::sparse::{SparseRowMat, SymbolicSparseRowMat};
use faer::{Conj, Mat, Par};
use sha2::{Digest, Sha256};

use crate::{FAER_ADAPTER_VERSION, FAER_VERSION};

pub(super) const STRUCTURE_DOMAIN: &[u8] = b"eqiora.faer-sparse-lu-reuse.structure/v1\0";
pub(super) const COEFFICIENT_DOMAIN: &[u8] = b"eqiora.faer-sparse-lu-reuse.coefficients/v1\0";
const POLICY_DOMAIN: &[u8] = b"eqiora.faer-sparse-lu-reuse.policy/v1\0";
const SYMBOLIC_DOMAIN: &[u8] = b"eqiora.faer-sparse-lu-reuse.symbolic/v1\0";
const NUMERIC_DOMAIN: &[u8] = b"eqiora.faer-sparse-lu-reuse.numeric/v1\0";

#[derive(Debug, Clone, Copy)]
pub(super) struct IdentitySet {
    pub(super) structure: [u8; 32],
    pub(super) coefficients: [u8; 32],
    pub(super) policy: [u8; 32],
    pub(super) symbolic: [u8; 32],
    pub(super) numeric: [u8; 32],
}

/// Owned faer symbolic state for one exact canonical CSR structure.
#[derive(Debug)]
pub(super) struct SparseLuSymbolicFactor {
    factor: SymbolicLu<usize>,
}

/// Owned faer numeric state produced under one compatible symbolic factor.
#[derive(Debug)]
pub(super) struct SparseLuNumericFactor {
    factor: NumericLu<usize, f64>,
}

pub(super) fn factor_symbolic(
    system: &CanonicalCsrSystemView,
) -> Result<SparseLuSymbolicFactor, Diagnostic> {
    let symbolic_row = SymbolicSparseRowMat::<usize>::new_checked(
        system.rows(),
        system.columns(),
        system.row_offsets().to_vec(),
        None,
        system.column_indices().to_vec(),
    );
    let symbolic_column = symbolic_row
        .to_col_major()
        .map_err(|error| solve_failed(format!("faer CSR structure conversion failed: {error}")))?;
    let factor = factorize_symbolic_lu(symbolic_column.as_ref(), Default::default())
        .map_err(|error| solve_failed(format!("faer symbolic LU failed: {error}")))?;
    Ok(SparseLuSymbolicFactor { factor })
}

pub(super) fn factor_numeric(
    symbolic: &SparseLuSymbolicFactor,
    system: &CanonicalCsrSystemView,
) -> Result<SparseLuNumericFactor, Diagnostic> {
    let symbolic_row = SymbolicSparseRowMat::<usize>::new_checked(
        system.rows(),
        system.columns(),
        system.row_offsets().to_vec(),
        None,
        system.column_indices().to_vec(),
    );
    let row_matrix = SparseRowMat::<usize, f64>::new(symbolic_row, system.values().to_vec());
    let column_matrix = row_matrix
        .to_col_major()
        .map_err(|error| solve_failed(format!("faer CSR conversion failed: {error}")))?;

    let parallelism = Par::Seq;
    let mut factor = NumericLu::<usize, f64>::new();
    let scratch = symbolic
        .factor
        .factorize_numeric_lu_scratch::<f64>(parallelism, Default::default());
    let mut buffer = MemBuffer::try_new(scratch)
        .map_err(|error| solve_failed(format!("faer numeric LU workspace failed: {error}")))?;
    symbolic
        .factor
        .factorize_numeric_lu(
            &mut factor,
            column_matrix.as_ref(),
            parallelism,
            MemStack::new(&mut buffer),
            Default::default(),
        )
        .map_err(|error| solve_failed(format!("faer numeric LU failed: {error}")))?;
    Ok(SparseLuNumericFactor { factor })
}

pub(super) fn solve_factored(
    symbolic: &SparseLuSymbolicFactor,
    numeric: &SparseLuNumericFactor,
    right_hand_side: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    if right_hand_side.len() != symbolic.factor.nrows()
        || symbolic.factor.nrows() != symbolic.factor.ncols()
    {
        return Err(solve_failed(
            "faer sparse LU factors and right-hand side have incompatible dimensions",
        ));
    }
    let parallelism = Par::Seq;
    let mut output = Mat::from_fn(right_hand_side.len(), 1, |row, _| right_hand_side[row]);
    let scratch = symbolic
        .factor
        .solve_in_place_scratch::<f64>(1, parallelism);
    let mut buffer = MemBuffer::try_new(scratch)
        .map_err(|error| solve_failed(format!("faer sparse LU solve workspace failed: {error}")))?;
    LuRef::new_unchecked(&symbolic.factor, &numeric.factor).solve_in_place_with_conj(
        Conj::No,
        output.as_mut(),
        parallelism,
        MemStack::new(&mut buffer),
    );
    let values = output.col_as_slice(0).to_vec();
    if values.iter().any(|value| !value.is_finite()) {
        return Err(solve_failed(
            "faer sparse LU produced a non-finite solution",
        ));
    }
    Ok(values)
}

pub(super) fn identities(
    system: &CanonicalCsrSystemView,
    plan: SolverPlan,
    provider: SolverProvider,
) -> Result<IdentitySet, Diagnostic> {
    let structure = structure_identity(system)?;
    let coefficients = coefficient_identity(system, structure)?;
    let policy = policy_identity(plan, provider)?;
    let symbolic = symbolic_identity(structure, policy);
    let numeric = numeric_identity(symbolic, coefficients);
    Ok(IdentitySet {
        structure,
        coefficients,
        policy,
        symbolic,
        numeric,
    })
}

pub(super) fn binding_shell_equal(left: &DeploymentBinding, right: &DeploymentBinding) -> bool {
    let (Some(left_host), Some(right_host)) = (left.host_executor(), right.host_executor()) else {
        return false;
    };
    left_host.execution_provider() == right_host.execution_provider()
        && left_host.maximum_workers() == right_host.maximum_workers()
        && left_host.solver_capabilities() == right_host.solver_capabilities()
        && left.execution() == right.execution()
        && left.execution_provider() == right.execution_provider()
        && left.verification_provider() == right.verification_provider()
}

fn structure_identity(system: &CanonicalCsrSystemView) -> Result<[u8; 32], Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(STRUCTURE_DOMAIN);
    update_count(&mut hash, system.rows(), "row count")?;
    update_count(&mut hash, system.columns(), "column count")?;
    update_count(&mut hash, system.row_offsets().len(), "row offset count")?;
    for &offset in system.row_offsets() {
        update_count(&mut hash, offset, "row offset")?;
    }
    update_count(
        &mut hash,
        system.column_indices().len(),
        "column index count",
    )?;
    for &column in system.column_indices() {
        update_count(&mut hash, column, "column index")?;
    }
    Ok(hash.finalize().into())
}

fn coefficient_identity(
    system: &CanonicalCsrSystemView,
    structure: [u8; 32],
) -> Result<[u8; 32], Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(COEFFICIENT_DOMAIN);
    hash.update(structure);
    update_count(&mut hash, system.values().len(), "coefficient count")?;
    for &value in system.values() {
        hash.update(normalized_bits(value).to_be_bytes());
    }
    Ok(hash.finalize().into())
}

fn policy_identity(plan: SolverPlan, provider: SolverProvider) -> Result<[u8; 32], Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(POLICY_DOMAIN);
    for value in [
        "SparseLu",
        "Identity",
        "Fast",
        "F64",
        "normal-orientation",
        "Par::Seq",
    ] {
        update_text(&mut hash, value)?;
    }
    hash.update(normalized_bits(plan.relative_tolerance()).to_be_bytes());
    hash.update(normalized_bits(plan.absolute_tolerance()).to_be_bytes());
    update_count(
        &mut hash,
        plan.maximum_iterations().get(),
        "maximum iteration count",
    )?;
    update_provider(&mut hash, provider)?;
    update_text(&mut hash, FAER_ADAPTER_VERSION)?;
    update_text(&mut hash, FAER_VERSION)?;
    update_text(&mut hash, "implementation-dependency-inventory")?;
    update_count(
        &mut hash,
        provider.libraries().len(),
        "provider library count",
    )?;
    for library in provider.libraries() {
        update_text(&mut hash, library.name())?;
        update_text(&mut hash, library.version())?;
    }
    update_text(&mut hash, "faer-0.24.4-colamd-defaults")?;
    update_text(&mut hash, "automatic-supernodal-selection")?;
    update_text(&mut hash, "default-partial-pivot-policy")?;
    Ok(hash.finalize().into())
}

pub(super) fn symbolic_identity(structure: [u8; 32], policy: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(SYMBOLIC_DOMAIN);
    hash.update(structure);
    hash.update(policy);
    hash.finalize().into()
}

pub(super) fn numeric_identity(symbolic: [u8; 32], coefficients: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(NUMERIC_DOMAIN);
    hash.update(symbolic);
    hash.update(coefficients);
    hash.finalize().into()
}

fn update_provider(hash: &mut Sha256, provider: SolverProvider) -> Result<(), Diagnostic> {
    update_text(hash, provider.id().as_str())?;
    update_text(hash, provider.implementation_version())?;
    update_count(hash, provider.libraries().len(), "provider library count")?;
    for library in provider.libraries() {
        update_text(hash, library.name())?;
        update_text(hash, library.version())?;
    }
    Ok(())
}

fn update_count(hash: &mut Sha256, value: usize, name: &str) -> Result<(), Diagnostic> {
    let value = u64::try_from(value)
        .map_err(|_| invalid_realization(format!("faer sparse LU reuse {name} exceeds u64")))?;
    hash.update(value.to_be_bytes());
    Ok(())
}

fn update_text(hash: &mut Sha256, value: &str) -> Result<(), Diagnostic> {
    update_count(hash, value.len(), "text length")?;
    hash.update(value.as_bytes());
    Ok(())
}

pub(super) const fn normalized_bits(value: f64) -> u64 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

#[cfg(test)]
pub(super) fn rhs_omission_mutant_observation() -> (bool, bool) {
    use eqiora_solver::{CompleteCsrStorage, LinearOperatorProperties};

    struct Storage {
        rhs: [f64; 1],
    }

    impl CompleteCsrStorage for Storage {
        fn rows(&self) -> usize {
            1
        }
        fn columns(&self) -> usize {
            1
        }
        fn row_offsets(&self) -> &[usize] {
            &[0, 1]
        }
        fn column_indices(&self) -> &[usize] {
            &[0]
        }
        fn values(&self) -> &[f64] {
            &[4.0]
        }
        fn right_hand_side(&self) -> &[f64] {
            &self.rhs
        }
    }

    let p0 = CanonicalCsrSystemView::new(
        &Storage { rhs: [1.0] },
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )
    .expect("frozen p0 synthetic system is canonical");
    let p1 = CanonicalCsrSystemView::new(
        &Storage { rhs: [2.0] },
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )
    .expect("frozen p1 synthetic system is canonical");
    let baseline = p0.agreement_fingerprint() == p1.agreement_fingerprint();
    let p0_structure = structure_identity(&p0).expect("p0 structure is portable");
    let p1_structure = structure_identity(&p1).expect("p1 structure is portable");
    let p0_coefficients =
        coefficient_identity(&p0, p0_structure).expect("p0 coefficients are portable");
    let p1_coefficients =
        coefficient_identity(&p1, p1_structure).expect("p1 coefficients are portable");
    let rhs_omitting_mutant = p0_structure == p1_structure && p0_coefficients == p1_coefficients;
    (baseline, rhs_omitting_mutant)
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn solve_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
}
