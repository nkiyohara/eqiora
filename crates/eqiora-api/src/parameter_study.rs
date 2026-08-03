//! Bounded serial composition of accepted differentiable evaluations.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};

use crate::{
    DifferentiableEvaluation, DifferentiableParameterPoint, DifferentiableProgram,
    DifferentiableProgramIdentity,
};

const MINIMUM_POINT_COUNT: usize = 2;
const MAXIMUM_POINT_COUNT: usize = 64;

/// One exact program-agnostic coordinate in a Parameter study inventory.
#[derive(Debug, Clone, Copy)]
pub struct ParameterStudyPointKey {
    parameter: Id<kinds::Parameter>,
    value_bits: u64,
}

impl ParameterStudyPointKey {
    fn new(parameter: Id<kinds::Parameter>, value: f64) -> Self {
        Self {
            parameter,
            value_bits: value.to_bits(),
        }
    }

    /// Exact varying Parameter identity.
    #[must_use]
    pub const fn parameter(&self) -> Id<kinds::Parameter> {
        self.parameter
    }

    /// Exact binary64 coordinate value.
    #[must_use]
    pub const fn value(&self) -> f64 {
        f64::from_bits(self.value_bits)
    }
}

impl PartialEq for ParameterStudyPointKey {
    fn eq(&self, other: &Self) -> bool {
        self.parameter == other.parameter && self.value_bits == other.value_bits
    }
}

impl Eq for ParameterStudyPointKey {}

impl Hash for ParameterStudyPointKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.parameter.hash(state);
        self.value_bits.hash(state);
    }
}

impl PartialOrd for ParameterStudyPointKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ParameterStudyPointKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.parameter
            .ulid()
            .cmp(&other.parameter.ulid())
            .then_with(|| self.value().total_cmp(&other.value()))
    }
}

/// One immutable program-bound, default-anchored Parameter study plan.
#[derive(Debug, Clone)]
pub struct ParameterStudyPlan {
    program: Box<DifferentiableProgram>,
    varying_parameter: Id<kinds::Parameter>,
    varying_index: usize,
    point_keys: Vec<ParameterStudyPointKey>,
}

impl ParameterStudyPlan {
    /// Plan one bounded study without numerically evaluating its alternate points.
    ///
    /// The exact accepted program default is the fixed base. `values` replaces
    /// only `varying_parameter` and is canonicalized by binary64 total order.
    ///
    /// # Errors
    /// Returns a diagnostic when the Parameter is not a program input, the
    /// inventory is outside the 2--64 bound, a value is non-finite or repeated
    /// by exact bits, or the inventory omits the exact default anchor.
    pub fn new(
        program: &DifferentiableProgram,
        varying_parameter: Id<kinds::Parameter>,
        values: &[f64],
    ) -> Result<Self, Diagnostic> {
        let Some(varying_index) = program
            .identity()
            .inputs()
            .iter()
            .position(|parameter| *parameter == varying_parameter)
        else {
            return Err(invalid_study(
                "Parameter study varying Parameter is not an input of the retained program",
            ));
        };
        if !(MINIMUM_POINT_COUNT..=MAXIMUM_POINT_COUNT).contains(&values.len()) {
            return Err(invalid_study(format!(
                "Parameter study inventory must contain between {MINIMUM_POINT_COUNT} and {MAXIMUM_POINT_COUNT} values"
            )));
        }

        let mut seen = HashSet::with_capacity(values.len());
        let mut point_keys = Vec::with_capacity(values.len());
        for &value in values {
            if !value.is_finite() {
                return Err(invalid_study(
                    "Parameter study inventory values must all be finite",
                ));
            }
            if !seen.insert(value.to_bits()) {
                return Err(invalid_study(
                    "Parameter study inventory contains an exact-bit duplicate",
                ));
            }
            point_keys.push(ParameterStudyPointKey::new(varying_parameter, value));
        }

        let default_bits = program.default_point().values()[varying_index].to_bits();
        if !seen.contains(&default_bits) {
            return Err(invalid_study(
                "Parameter study inventory must contain the retained program's exact default anchor",
            ));
        }
        point_keys.sort_unstable();

        Ok(Self {
            program: Box::new(program.clone()),
            varying_parameter,
            varying_index,
            point_keys,
        })
    }

    /// Exact immutable differentiable-program identity.
    #[must_use]
    pub fn program_identity(&self) -> &DifferentiableProgramIdentity {
        self.program.identity()
    }

    /// Sole varying Parameter identity.
    #[must_use]
    pub const fn varying_parameter(&self) -> Id<kinds::Parameter> {
        self.varying_parameter
    }

    /// Complete accepted program default retained as the fixed base point.
    #[must_use]
    pub const fn base_point(&self) -> &DifferentiableParameterPoint {
        self.program.default_point()
    }

    /// Canonical total-ordered study inventory.
    #[must_use]
    pub fn point_keys(&self) -> &[ParameterStudyPointKey] {
        &self.point_keys
    }

    /// Execute every point serially through the retained program.
    ///
    /// # Errors
    /// Returns an all-or-nothing terminal report on the first point failure.
    pub fn execute(&self) -> Result<CompleteParameterStudy, ParameterStudyTerminalReport> {
        self.execute_with_cancellation(|| false)
    }

    /// Execute serially with cancellation observed only before or between points.
    ///
    /// Completion wins after the final accepted member; the callback is never
    /// polled during an evaluation or after the final member.
    ///
    /// # Errors
    /// Returns an all-or-nothing terminal report on cancellation or the first
    /// point failure.
    pub fn execute_with_cancellation<F>(
        &self,
        mut should_cancel: F,
    ) -> Result<CompleteParameterStudy, ParameterStudyTerminalReport>
    where
        F: FnMut() -> bool,
    {
        let mut evaluator =
            |program: &DifferentiableProgram, point: &[f64]| program.evaluate(point);
        self.execute_with_evaluator(&mut evaluator, &mut should_cancel)
    }

    fn execute_with_evaluator<E, C>(
        &self,
        evaluator: &mut E,
        should_cancel: &mut C,
    ) -> Result<CompleteParameterStudy, ParameterStudyTerminalReport>
    where
        E: FnMut(
            &DifferentiableProgram,
            &[f64],
        ) -> Result<DifferentiableEvaluation, Vec<Diagnostic>>,
        C: FnMut() -> bool,
    {
        let mut completed_point_keys = Vec::with_capacity(self.point_keys.len());
        let mut members = Vec::with_capacity(self.point_keys.len());

        for key in &self.point_keys {
            if should_cancel() {
                return Err(ParameterStudyTerminalReport::cancelled(
                    self,
                    completed_point_keys,
                ));
            }

            let mut point = self.base_point().values().to_vec();
            point[self.varying_index] = key.value();
            let member = match evaluator(&self.program, &point) {
                Ok(member) => member,
                Err(diagnostics) => {
                    return Err(ParameterStudyTerminalReport::failed(
                        self,
                        completed_point_keys,
                        *key,
                        nonempty_failure_diagnostics(diagnostics),
                    ));
                }
            };
            if let Err(diagnostic) = validate_member(self, key, &member) {
                return Err(ParameterStudyTerminalReport::failed(
                    self,
                    completed_point_keys,
                    *key,
                    vec![diagnostic],
                ));
            }

            completed_point_keys.push(*key);
            members.push(member);
        }

        Ok(CompleteParameterStudy {
            plan: self.clone(),
            members,
        })
    }
}

impl PartialEq for ParameterStudyPlan {
    fn eq(&self, other: &Self) -> bool {
        self.program_identity() == other.program_identity()
            && self.varying_parameter == other.varying_parameter
            && exact_point_eq(self.base_point(), other.base_point())
            && self.point_keys == other.point_keys
    }
}

/// One complete canonical aggregate of accepted program evaluations.
#[derive(Debug, Clone)]
pub struct CompleteParameterStudy {
    plan: ParameterStudyPlan,
    members: Vec<DifferentiableEvaluation>,
}

impl CompleteParameterStudy {
    #[cfg(test)]
    fn from_members(
        plan: &ParameterStudyPlan,
        members: Vec<DifferentiableEvaluation>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if members.len() != plan.point_keys.len() {
            return Err(vec![invalid_study(format!(
                "complete Parameter study requires exactly {} members, found {}",
                plan.point_keys.len(),
                members.len()
            ))]);
        }
        for (key, member) in plan.point_keys.iter().zip(&members) {
            validate_member(plan, key, member).map_err(|diagnostic| vec![diagnostic])?;
        }
        Ok(Self {
            plan: plan.clone(),
            members,
        })
    }

    /// Exact plan that owns this aggregate.
    #[must_use]
    pub const fn plan(&self) -> &ParameterStudyPlan {
        &self.plan
    }

    /// Canonical point keys, aligned one-to-one with [`Self::members`].
    #[must_use]
    pub fn point_keys(&self) -> &[ParameterStudyPointKey] {
        self.plan.point_keys()
    }

    /// Exact accepted evaluations in canonical point order.
    #[must_use]
    pub fn members(&self) -> &[DifferentiableEvaluation] {
        &self.members
    }

    /// Resolve one program-agnostic coordinate key within this aggregate.
    #[must_use]
    pub fn evaluation(&self, key: &ParameterStudyPointKey) -> Option<&DifferentiableEvaluation> {
        self.point_keys()
            .binary_search(key)
            .ok()
            .map(|index| &self.members[index])
    }
}

/// Atomic failure or cancellation from one bounded Parameter study execution.
#[derive(Debug, Clone)]
pub struct ParameterStudyTerminalReport {
    plan: Box<ParameterStudyPlan>,
    completed_point_keys: Vec<ParameterStudyPointKey>,
    failed_point_key: Option<ParameterStudyPointKey>,
    diagnostics: Vec<Diagnostic>,
    cancelled: bool,
}

impl ParameterStudyTerminalReport {
    fn failed(
        plan: &ParameterStudyPlan,
        completed_point_keys: Vec<ParameterStudyPointKey>,
        failed_point_key: ParameterStudyPointKey,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            plan: Box::new(plan.clone()),
            completed_point_keys,
            failed_point_key: Some(failed_point_key),
            diagnostics,
            cancelled: false,
        }
    }

    fn cancelled(
        plan: &ParameterStudyPlan,
        completed_point_keys: Vec<ParameterStudyPointKey>,
    ) -> Self {
        Self {
            plan: Box::new(plan.clone()),
            completed_point_keys,
            failed_point_key: None,
            diagnostics: vec![Diagnostic::error(
                codes::EXECUTION_CANCELLED,
                "Parameter study execution was cancelled at a point boundary",
            )],
            cancelled: true,
        }
    }

    /// Exact plan whose execution terminated.
    #[must_use]
    pub fn plan(&self) -> &ParameterStudyPlan {
        self.plan.as_ref()
    }

    /// Canonical accepted-key prefix completed before termination.
    #[must_use]
    pub fn completed_point_keys(&self) -> &[ParameterStudyPointKey] {
        &self.completed_point_keys
    }

    /// Exact point that failed, or `None` for cancellation.
    #[must_use]
    pub const fn failed_point_key(&self) -> Option<&ParameterStudyPointKey> {
        self.failed_point_key.as_ref()
    }

    /// Original point diagnostics, or the one typed cancellation diagnostic.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether execution accepted cooperative cancellation.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

fn validate_member(
    plan: &ParameterStudyPlan,
    key: &ParameterStudyPointKey,
    member: &DifferentiableEvaluation,
) -> Result<(), Diagnostic> {
    if member.identity() != plan.program_identity() {
        return Err(invalid_study(
            "Parameter study member belongs to a different differentiable program",
        ));
    }
    if member.point().inputs() != plan.base_point().inputs() {
        return Err(invalid_study(
            "Parameter study member has a foreign input-coordinate inventory",
        ));
    }

    let mut expected_values = plan.base_point().values().to_vec();
    expected_values[plan.varying_index] = key.value();
    if !exact_value_bits_eq(member.point().values(), &expected_values) {
        return Err(invalid_study(
            "Parameter study member differs from its exact planned point",
        ));
    }
    Ok(())
}

fn exact_point_eq(
    left: &DifferentiableParameterPoint,
    right: &DifferentiableParameterPoint,
) -> bool {
    left.inputs() == right.inputs() && exact_value_bits_eq(left.values(), right.values())
}

fn exact_value_bits_eq(left: &[f64], right: &[f64]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.to_bits() == right.to_bits())
}

fn nonempty_failure_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    if diagnostics.is_empty() {
        vec![invalid_study(
            "Parameter study point evaluation failed without a diagnostic",
        )]
    } else {
        diagnostics
    }
}

fn invalid_study(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_LINEARIZATION, message)
}

#[cfg(test)]
mod tests;
