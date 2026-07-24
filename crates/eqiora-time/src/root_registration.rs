//! Canonical root registration and uncommitted backend proposal contracts.

use crate::diagnostic::{invalid_lowering, time_solve_failed};
use crate::solution::TimeExecutionReport;
use crate::system::RootFunctions;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use std::collections::HashSet;

/// SHA-256 identity of one validated, ordered root-function registration.
///
/// The identity is computed by the artifact layer from canonical model,
/// lowering, and Activation-group content. This L2 type carries the opaque
/// identity through backend execution without depending on a wire format or
/// hashing implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RootRegistrationId([u8; 32]);

impl RootRegistrationId {
    /// Reconstruct an identity already validated or computed by an artifact
    /// boundary.
    #[must_use]
    pub const fn from_sha256(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Complete SHA-256 bytes.
    #[must_use]
    pub const fn as_sha256(self) -> [u8; 32] {
        self.0
    }
}

/// One scalar root function mapped to a complete atomic Activation group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootActivationGroup {
    activations: Vec<Id<kinds::Activation>>,
}

impl RootActivationGroup {
    /// Construct a canonical non-empty, sorted, unique Activation group.
    ///
    /// # Errors
    /// Returns `EQ0705` for an empty group or a repeated Activation.
    pub fn new(mut activations: Vec<Id<kinds::Activation>>) -> Result<Self, Diagnostic> {
        if activations.is_empty() {
            return Err(invalid_lowering(
                "root registration requires a non-empty Activation group",
            ));
        }
        activations.sort_by_key(|activation| activation.erase());
        if activations.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid_lowering(
                "root registration Activation group contains a duplicate",
            ));
        }
        Ok(Self { activations })
    }

    /// Canonical representative used to order root callback slots.
    #[must_use]
    pub fn representative(&self) -> Id<kinds::Activation> {
        self.activations[0]
    }

    /// Complete sorted atomic Activation group.
    #[must_use]
    pub fn activations(&self) -> &[Id<kinds::Activation>] {
        &self.activations
    }
}

/// Canonical callback order and Activation grouping proven by registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootRegistrationProof {
    groups: Vec<RootActivationGroup>,
}

impl RootRegistrationProof {
    /// Canonicalize root groups by representative Activation and prove that
    /// every supplied Activation occurs exactly once.
    ///
    /// # Errors
    /// Returns `EQ0705` for an empty registration or overlapping groups.
    pub fn new(mut groups: Vec<RootActivationGroup>) -> Result<Self, Diagnostic> {
        if groups.is_empty() {
            return Err(invalid_lowering(
                "root registration requires at least one root function",
            ));
        }
        groups.sort_by_key(|group| group.representative().erase());
        let mut seen = HashSet::new();
        if groups
            .iter()
            .flat_map(RootActivationGroup::activations)
            .any(|activation| !seen.insert(*activation))
        {
            return Err(invalid_lowering(
                "root registration Activation groups overlap",
            ));
        }
        Ok(Self { groups })
    }

    /// Number of ordered scalar root callbacks.
    #[must_use]
    pub fn root_count(&self) -> usize {
        self.groups.len()
    }

    /// Canonically ordered root groups.
    #[must_use]
    pub fn groups(&self) -> &[RootActivationGroup] {
        &self.groups
    }

    /// Activation group selected by one backend-local callback index.
    #[must_use]
    pub fn group(&self, root_index: usize) -> Option<&RootActivationGroup> {
        self.groups.get(root_index)
    }
}

/// Root actions bound to the exact registration identity and callback proof.
pub struct RegisteredRootProblem<'a> {
    registration: RootRegistrationId,
    proof: RootRegistrationProof,
    functions: &'a dyn RootFunctions,
}

impl std::fmt::Debug for RegisteredRootProblem<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RegisteredRootProblem")
            .field("registration", &self.registration)
            .field("proof", &self.proof)
            .finish_non_exhaustive()
    }
}

impl<'a> RegisteredRootProblem<'a> {
    /// Bind root actions to one content-addressed registration.
    ///
    /// # Errors
    /// Returns `EQ0705` if callback count differs from the registration proof.
    pub fn new(
        registration: RootRegistrationId,
        proof: RootRegistrationProof,
        functions: &'a dyn RootFunctions,
    ) -> Result<Self, Diagnostic> {
        if functions.count() != proof.root_count() {
            return Err(invalid_lowering(
                "root callback count differs from its registration proof",
            ));
        }
        Ok(Self {
            registration,
            proof,
            functions,
        })
    }

    /// Content identity retained by every accepted proposal.
    #[must_use]
    pub const fn registration(&self) -> RootRegistrationId {
        self.registration
    }

    /// Ordered canonical Activation grouping.
    #[must_use]
    pub const fn proof(&self) -> &RootRegistrationProof {
        &self.proof
    }

    /// Registered callback actions.
    #[must_use]
    pub const fn functions(&self) -> &dyn RootFunctions {
        self.functions
    }
}

/// Backend-localized candidate presented to Eqiora's hybrid scheduler.
#[derive(Debug, Clone, PartialEq)]
pub struct RootProposal {
    registration: RootRegistrationId,
    time: f64,
    root_index: usize,
    state: Vec<f64>,
    report: TimeExecutionReport,
}

impl RootProposal {
    /// Accept one finite localized root candidate from an adapter.
    ///
    /// # Errors
    /// Returns `EQ0802` for invalid time, index, shape, or state values.
    pub fn accepted(
        registration: RootRegistrationId,
        time: f64,
        root_index: usize,
        root_count: usize,
        state: Vec<f64>,
        expected_dimension: usize,
        report: TimeExecutionReport,
    ) -> Result<Self, Diagnostic> {
        if !time.is_finite()
            || root_count == 0
            || root_index >= root_count
            || state.len() != expected_dimension
            || state.iter().any(|value| !value.is_finite())
        {
            return Err(time_solve_failed(
                "time backend returned an invalid root proposal",
            ));
        }
        Ok(Self {
            registration,
            time,
            root_index,
            state,
            report,
        })
    }

    /// Content-addressed root registration that gives `root_index` meaning.
    #[must_use]
    pub const fn registration(&self) -> RootRegistrationId {
        self.registration
    }

    /// Localized candidate model time.
    #[must_use]
    pub const fn time(&self) -> f64 {
        self.time
    }

    /// Backend root-function index; direction/grouping remain uncommitted.
    #[must_use]
    pub const fn root_index(&self) -> usize {
        self.root_index
    }

    /// Pre-event state at the candidate instant.
    #[must_use]
    pub fn state(&self) -> &[f64] {
        &self.state
    }

    /// Adapter/method/equation evidence.
    #[must_use]
    pub const fn report(&self) -> TimeExecutionReport {
        self.report
    }
}
