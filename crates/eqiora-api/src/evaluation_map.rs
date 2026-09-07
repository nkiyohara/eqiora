//! Ordered serial composition of immutable accepted evaluations.

use std::sync::Arc;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::{
    DifferentiableEvaluation, DifferentiableParameterPoint, DifferentiableProgram,
    DifferentiableProgramIdentity,
};

mod axes;
mod partition;
mod products;

pub use products::{EvaluationMapJvp, EvaluationMapProducts, EvaluationMapVjp};

/// An admitted ordered collection of complete inputs for one immutable program.
///
/// Request position identifies an occurrence. Equal points remain separate
/// occurrences; neither sorting, deduplication nor a default anchor is applied.
#[derive(Debug, Clone)]
pub struct EvaluationMapPlan {
    program: Arc<DifferentiableProgram>,
    points: Arc<[DifferentiableParameterPoint]>,
    estimated_retained_bytes: usize,
}

impl EvaluationMapPlan {
    /// Admit complete points without evaluating any of them.
    ///
    /// Points follow the program's exact ordered Parameter inputs. The caller
    /// shares an already admitted program; mapping does not clone its Model or
    /// Mesh. Empty and singleton collections are permitted.
    ///
    /// `retained_bytes_limit` bounds a conservative estimate of additional
    /// retained numerical storage: point/member records, states, CSR storage,
    /// and dense input/output Jacobians. It excludes the shared program,
    /// deployment metadata, allocator overhead, diagnostics and solver scratch;
    /// it is not a process or peak-memory limit. All products are checked before
    /// copying points or reserving member storage.
    ///
    /// # Errors
    /// Rejects incomplete/nonfinite points, unrepresentable dimensions, or an
    /// estimate above the supplied byte limit. Physical/numerical admission
    /// remains with ordinary evaluation during execution.
    pub fn new(
        program: Arc<DifferentiableProgram>,
        points: &[&[f64]],
        retained_bytes_limit: usize,
    ) -> Result<Self, Diagnostic> {
        for point in points {
            program.validate_map_point(point)?;
        }
        let estimated_retained_bytes =
            retained_bytes(program.map_occurrence_bytes()?, points.len())?;
        if estimated_retained_bytes > retained_bytes_limit {
            return Err(invalid(format!(
                "evaluation map requires an estimated {estimated_retained_bytes} retained numerical bytes, above limit {retained_bytes_limit}"
            )));
        }
        let points = points
            .iter()
            .map(|values| program.map_point(values))
            .collect::<Vec<_>>()
            .into();
        Ok(Self {
            program,
            points,
            estimated_retained_bytes,
        })
    }

    /// Exact program, Model, Plan, input order and output authority.
    #[must_use]
    pub fn program_identity(&self) -> &DifferentiableProgramIdentity {
        self.program.identity()
    }

    /// Complete immutable inputs in request order, including duplicates.
    #[must_use]
    pub fn points(&self) -> &[DifferentiableParameterPoint] {
        &self.points
    }

    /// Admitted additional retained numerical-storage estimate, not peak memory.
    #[must_use]
    pub const fn estimated_retained_bytes(&self) -> usize {
        self.estimated_retained_bytes
    }

    /// Evaluate serially through the retained program's ordinary admission.
    ///
    /// # Errors
    /// Stops at the first failed occurrence, retaining its original diagnostics
    /// and all earlier accepted members in a terminal report, not a complete map.
    pub fn execute(&self) -> Result<CompleteEvaluationMap, EvaluationMapTerminalReport> {
        self.execute_with_cancellation(|| false)
    }

    /// Execute with cooperative cancellation before or between occurrences.
    ///
    /// No poll occurs within an evaluation or after the final accepted member.
    /// An empty map is already complete and never polls cancellation. At a
    /// boundary cancellation marks the next occurrence cancelled, and every
    /// later occurrence not started. A failure inside evaluation wins over a
    /// cancellation that becomes true during that evaluation.
    ///
    /// # Errors
    /// Returns an indexed terminal report on cancellation or first failure.
    pub fn execute_with_cancellation<F>(
        &self,
        mut should_cancel: F,
    ) -> Result<CompleteEvaluationMap, EvaluationMapTerminalReport>
    where
        F: FnMut() -> bool,
    {
        self.execute_with_evaluator(
            &mut |program, point| program.evaluate(point),
            &mut should_cancel,
        )
    }

    fn execute_with_evaluator<E, C>(
        &self,
        evaluator: &mut E,
        should_cancel: &mut C,
    ) -> Result<CompleteEvaluationMap, EvaluationMapTerminalReport>
    where
        E: FnMut(
            &DifferentiableProgram,
            &[f64],
        ) -> Result<DifferentiableEvaluation, Vec<Diagnostic>>,
        C: FnMut() -> bool,
    {
        let mut members = Vec::with_capacity(self.points.len());
        for point in self.points.iter() {
            if should_cancel() {
                return Err(EvaluationMapTerminalReport {
                    plan: self.clone(),
                    members,
                    diagnostics: vec![Diagnostic::error(
                        codes::EXECUTION_CANCELLED,
                        "evaluation map cancelled at an occurrence boundary",
                    )],
                    cancelled: true,
                });
            }
            let member = evaluator(&self.program, point.values()).and_then(|member| {
                validate_member(self, point, &member)
                    .map(|()| member)
                    .map_err(|diagnostic| vec![diagnostic])
            });
            match member {
                Ok(member) => members.push(member),
                Err(mut diagnostics) => {
                    if diagnostics.is_empty() {
                        diagnostics.push(invalid("evaluation failed without a diagnostic"));
                    }
                    return Err(EvaluationMapTerminalReport {
                        plan: self.clone(),
                        members,
                        diagnostics,
                        cancelled: false,
                    });
                }
            }
        }
        // Every position has passed the same member admission used by the
        // private composition falsifier. No partial result has this type.
        Ok(CompleteEvaluationMap {
            plan: self.clone(),
            members,
        })
    }
}

impl PartialEq for EvaluationMapPlan {
    fn eq(&self, other: &Self) -> bool {
        self.program_identity() == other.program_identity()
            && self.points.len() == other.points.len()
            && self
                .points
                .iter()
                .zip(other.points.iter())
                .all(|(a, b)| a.inputs() == b.inputs() && exact_values(a.values(), b.values()))
    }
}

/// A complete ordered map; every requested occurrence has an accepted member.
#[derive(Debug, Clone)]
pub struct CompleteEvaluationMap {
    plan: EvaluationMapPlan,
    members: Vec<DifferentiableEvaluation>,
}

impl CompleteEvaluationMap {
    #[cfg(test)]
    fn from_members(
        plan: &EvaluationMapPlan,
        members: Vec<DifferentiableEvaluation>,
    ) -> Result<Self, Diagnostic> {
        if members.len() != plan.points.len() {
            return Err(invalid(
                "complete evaluation map requires exactly one member per occurrence",
            ));
        }
        for (point, member) in plan.points.iter().zip(&members) {
            validate_member(plan, point, member)?;
        }
        Ok(Self {
            plan: plan.clone(),
            members,
        })
    }

    /// Exact admitted request owning the collection, including empty outputs.
    #[must_use]
    pub const fn plan(&self) -> &EvaluationMapPlan {
        &self.plan
    }

    /// Accepted members in request order, without coalescing equal points.
    #[must_use]
    pub fn members(&self) -> &[DifferentiableEvaluation] {
        &self.members
    }

    /// Select an occurrence by its request position.
    #[must_use]
    pub fn evaluation(&self, index: usize) -> Option<&DifferentiableEvaluation> {
        self.members.get(index)
    }
}

/// Borrowed state of one request position in a terminal map report.
#[derive(Debug)]
pub enum EvaluationMapOccurrence<'a> {
    /// Ordinary accepted evaluation, still independently inspectable.
    Accepted(&'a DifferentiableEvaluation),
    /// The failed occurrence's original nonempty diagnostics.
    Failed(&'a [Diagnostic]),
    /// Cancellation was observed before this occurrence began.
    Cancelled,
    /// Execution stopped before reaching this later occurrence.
    NotStarted,
}

/// First failure or boundary cancellation, retaining accepted members locally.
///
/// This cannot be converted to a complete map. A dense adapter must propagate
/// this terminal outcome instead of silently stacking its accepted prefix.
#[derive(Debug, Clone)]
pub struct EvaluationMapTerminalReport {
    plan: EvaluationMapPlan,
    members: Vec<DifferentiableEvaluation>,
    diagnostics: Vec<Diagnostic>,
    cancelled: bool,
}

impl EvaluationMapTerminalReport {
    /// Complete planned inventory, including unstarted occurrences.
    #[must_use]
    pub const fn plan(&self) -> &EvaluationMapPlan {
        &self.plan
    }

    /// Accepted prefix, indexed by the same zero-based request positions.
    #[must_use]
    pub fn accepted_members(&self) -> &[DifferentiableEvaluation] {
        &self.members
    }

    /// Position that failed or was cancelled before it began.
    #[must_use]
    pub fn stopped_index(&self) -> usize {
        self.members.len()
    }

    /// Original numerical failure or typed boundary-cancellation diagnostic.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether the terminal occurrence was cancelled rather than failed.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Inspect an exact occurrence; out-of-inventory positions return None.
    #[must_use]
    pub fn occurrence(&self, index: usize) -> Option<EvaluationMapOccurrence<'_>> {
        if index >= self.plan.points.len() {
            return None;
        }
        Some(if let Some(member) = self.members.get(index) {
            EvaluationMapOccurrence::Accepted(member)
        } else if index > self.stopped_index() {
            EvaluationMapOccurrence::NotStarted
        } else if self.cancelled {
            EvaluationMapOccurrence::Cancelled
        } else {
            EvaluationMapOccurrence::Failed(&self.diagnostics)
        })
    }
}

fn validate_member(
    plan: &EvaluationMapPlan,
    point: &DifferentiableParameterPoint,
    member: &DifferentiableEvaluation,
) -> Result<(), Diagnostic> {
    if member.identity() != plan.program_identity() {
        return Err(invalid(
            "evaluation map member belongs to a foreign program or Plan",
        ));
    }
    if member.point().inputs() != point.inputs()
        || !exact_values(member.point().values(), point.values())
    {
        return Err(invalid(
            "evaluation map member differs from its exact requested point",
        ));
    }
    Ok(())
}

fn exact_values(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.to_bits() == b.to_bits())
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_LINEARIZATION, message)
}

fn retained_bytes(per_occurrence: usize, count: usize) -> Result<usize, Diagnostic> {
    per_occurrence
        .checked_mul(count)
        .ok_or_else(|| invalid("evaluation map retained storage estimate overflows usize"))
}

#[cfg(test)]
mod tests;
