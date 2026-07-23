use std::fmt;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_solver::ExecutionReport;

use crate::{AssemblyDelta, AssemblyMap, CooAssembler, LinearSystem, LocalContribution};

/// Identity of the ordered logical entity set addressed by assembly packets.
///
/// A content-bound identity lets a placement backend prove that packet index
/// `i` names the same entity as its ownership layout. `Unbound` is explicit
/// and remains valid for reference or threaded local assembly, but spatially
/// distributed backends must reject it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssemblyPacketSetIdentityV1 {
    /// No external content identity is attached to this local operation.
    Unbound,
    /// SHA-256 identity of the authenticated ordered entity set.
    ContentSha256([u8; 32]),
}

impl AssemblyPacketSetIdentityV1 {
    /// Explicitly declare an operation with no externally comparable packet
    /// set identity.
    #[must_use]
    pub const fn unbound() -> Self {
        Self::Unbound
    }

    /// Bind an already authenticated content digest.
    #[must_use]
    pub const fn from_sha256(bytes: [u8; 32]) -> Self {
        Self::ContentSha256(bytes)
    }

    /// Content digest when this packet set is externally bound.
    #[must_use]
    pub const fn sha256(self) -> Option<[u8; 32]> {
        match self {
            Self::Unbound => None,
            Self::ContentSha256(bytes) => Some(bytes),
        }
    }
}

/// Typed ordinal of one output system within an [`AssemblyPlan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssemblyTargetId(usize);

impl AssemblyTargetId {
    /// Zero-based target ordinal.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// One nonempty square algebraic system produced by assembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssemblyTarget {
    size: usize,
}

impl AssemblyTarget {
    /// Construct one square target with `size` equations and unknowns.
    ///
    /// # Errors
    /// Returns `EQ0806` when `size` is zero.
    pub fn new(size: usize) -> Result<Self, Diagnostic> {
        if size == 0 {
            return Err(assembly_failed(
                "an assembly target requires at least one equation",
            ));
        }
        Ok(Self { size })
    }

    /// Equation and unknown count of this square target.
    #[must_use]
    pub const fn size(self) -> usize {
        self.size
    }
}

/// Ordered output shape for one assembly operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyPlan {
    targets: Vec<AssemblyTarget>,
}

impl AssemblyPlan {
    /// Construct a nonempty ordered target plan.
    ///
    /// # Errors
    /// Returns `EQ0806` when no output target is declared.
    pub fn new(targets: Vec<AssemblyTarget>) -> Result<Self, Diagnostic> {
        if targets.is_empty() {
            return Err(assembly_failed(
                "an assembly plan requires at least one target",
            ));
        }
        Ok(Self { targets })
    }

    /// Number of output systems.
    #[must_use]
    pub fn target_count(&self) -> usize {
        self.targets.len()
    }

    /// Obtain the plan-scoped typed ID for one target ordinal.
    #[must_use]
    pub fn target_id(&self, index: usize) -> Option<AssemblyTargetId> {
        (index < self.targets.len()).then_some(AssemblyTargetId(index))
    }

    /// Target shape for a valid plan-scoped ID.
    #[must_use]
    pub fn target(&self, id: AssemblyTargetId) -> Option<AssemblyTarget> {
        self.targets.get(id.0).copied()
    }
}

/// One local-to-global map addressed to a specific assembly target.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetAssemblyMap {
    target: AssemblyTargetId,
    map: AssemblyMap,
}

impl TargetAssemblyMap {
    /// Bind one map to a target obtained from [`AssemblyPlan::target_id`].
    #[must_use]
    pub const fn new(target: AssemblyTargetId, map: AssemblyMap) -> Self {
        Self { target, map }
    }

    /// Destination target.
    #[must_use]
    pub const fn target(&self) -> AssemblyTargetId {
        self.target
    }

    /// Local-to-global map for this target.
    #[must_use]
    pub const fn map(&self) -> &AssemblyMap {
        &self.map
    }
}

/// One pure local contribution and its one-or-more algebraic projections.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyPacket {
    local: LocalContribution,
    mappings: Vec<TargetAssemblyMap>,
}

/// One plan-validated packet-local delta addressed to its target ordinal.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetAssemblyDelta {
    target: AssemblyTargetId,
    delta: AssemblyDelta,
}

impl TargetAssemblyDelta {
    /// Destination target in the plan used for projection.
    #[must_use]
    pub const fn target(&self) -> AssemblyTargetId {
        self.target
    }

    /// Canonical additive global rows for this target.
    #[must_use]
    pub const fn delta(&self) -> &AssemblyDelta {
        &self.delta
    }
}

impl AssemblyPacket {
    /// Construct a validated packet and canonicalize mappings by target ID.
    ///
    /// Target bounds are checked against the concrete plan during scatter.
    ///
    /// # Errors
    /// Returns `EQ0806` for no mappings, duplicate targets, or local/map shape
    /// mismatch.
    pub fn new(
        local: LocalContribution,
        mut mappings: Vec<TargetAssemblyMap>,
    ) -> Result<Self, Diagnostic> {
        if mappings.is_empty() {
            return Err(assembly_failed(
                "an assembly packet requires at least one target mapping",
            ));
        }
        mappings.sort_by_key(|mapping| mapping.target);
        for pair in mappings.windows(2) {
            if pair[0].target == pair[1].target {
                return Err(assembly_failed(format!(
                    "assembly packet maps target {} more than once",
                    pair[0].target.0
                )));
            }
        }
        for mapping in &mappings {
            if mapping.map.equations().len() != local.rows()
                || mapping.map.unknowns().len() != local.columns()
            {
                return Err(assembly_failed(format!(
                    "target {} map is {}x{} but local contribution is {}x{}",
                    mapping.target.0,
                    mapping.map.equations().len(),
                    mapping.map.unknowns().len(),
                    local.rows(),
                    local.columns()
                )));
            }
        }
        Ok(Self { local, mappings })
    }

    /// Anonymous local matrix and right-hand side.
    #[must_use]
    pub const fn local(&self) -> &LocalContribution {
        &self.local
    }

    /// Canonically target-ordered mappings.
    #[must_use]
    pub fn mappings(&self) -> &[TargetAssemblyMap] {
        &self.mappings
    }

    /// Project every target mapping through one concrete plan.
    ///
    /// All target ordinals, dimensions, global degrees of freedom, fixed
    /// values, and projected arithmetic are checked before any accumulator is
    /// mutated. Returned deltas retain the packet's canonical target order.
    ///
    /// # Errors
    /// Returns `EQ0806` for a target outside the plan or any mapping/projection
    /// failure.
    pub fn project(&self, plan: &AssemblyPlan) -> Result<Vec<TargetAssemblyDelta>, Diagnostic> {
        let mut projected = Vec::with_capacity(self.mappings.len());
        for mapping in &self.mappings {
            let target = plan.target(mapping.target).ok_or_else(|| {
                assembly_failed(format!(
                    "assembly packet references target {} outside plan count {}",
                    mapping.target.0,
                    plan.target_count()
                ))
            })?;
            projected.push(TargetAssemblyDelta {
                target: mapping.target,
                delta: AssemblyDelta::from_local(target.size, &mapping.map, &self.local)?,
            });
        }
        Ok(projected)
    }
}

/// Indexed pure local work evaluated by an assembly backend.
pub trait AssemblyWork: fmt::Debug + Sync {
    /// Identity of the ordered entity set addressed by packet indices.
    fn packet_set_identity(&self) -> AssemblyPacketSetIdentityV1;

    /// Stable logical packet count for this assembly operation.
    fn packet_count(&self) -> usize;

    /// Evaluate one stable logical packet index without global side effects.
    ///
    /// # Errors
    /// Returns a numerical diagnostic from local geometry, coefficients,
    /// quadrature, or packet validation.
    fn evaluate(&self, packet_index: usize) -> Result<AssemblyPacket, Diagnostic>;
}

/// Ergonomic [`AssemblyWork`] backed by one immutable indexed closure.
pub struct IndexedAssemblyWork<F> {
    packet_set: AssemblyPacketSetIdentityV1,
    packet_count: usize,
    evaluate: F,
}

impl<F> IndexedAssemblyWork<F> {
    /// Bind a stable packet count to an indexed evaluator.
    #[must_use]
    pub const fn new(packet_count: usize, evaluate: F) -> Self {
        Self {
            packet_set: AssemblyPacketSetIdentityV1::Unbound,
            packet_count,
            evaluate,
        }
    }

    /// Bind an authenticated ordered packet set to an indexed evaluator.
    #[must_use]
    pub const fn for_packet_set(
        packet_set: AssemblyPacketSetIdentityV1,
        packet_count: usize,
        evaluate: F,
    ) -> Self {
        Self {
            packet_set,
            packet_count,
            evaluate,
        }
    }
}

impl<F> fmt::Debug for IndexedAssemblyWork<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexedAssemblyWork")
            .field("packet_count", &self.packet_count)
            .finish_non_exhaustive()
    }
}

impl<F> AssemblyWork for IndexedAssemblyWork<F>
where
    F: Fn(usize) -> Result<AssemblyPacket, Diagnostic> + Sync,
{
    fn packet_set_identity(&self) -> AssemblyPacketSetIdentityV1 {
        self.packet_set
    }

    fn packet_count(&self) -> usize {
        self.packet_count
    }

    fn evaluate(&self, packet_index: usize) -> Result<AssemblyPacket, Diagnostic> {
        if packet_index >= self.packet_count {
            return Err(assembly_failed(format!(
                "assembly packet {packet_index} is outside work count {}",
                self.packet_count
            )));
        }
        (self.evaluate)(packet_index)
    }
}

/// Evidence for one completely accepted assembly operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssemblyReport {
    execution: ExecutionReport,
    packet_count: usize,
    target_count: usize,
}

impl AssemblyReport {
    /// Placement used to evaluate local packets.
    #[must_use]
    pub const fn execution(self) -> ExecutionReport {
        self.execution
    }

    /// Accepted logical packet count.
    #[must_use]
    pub const fn packet_count(self) -> usize {
        self.packet_count
    }

    /// Finalized output target count.
    #[must_use]
    pub const fn target_count(self) -> usize {
        self.target_count
    }
}

/// Finalized target systems and their exact assembly placement evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyResult {
    systems: Vec<LinearSystem>,
    report: AssemblyReport,
}

impl AssemblyResult {
    /// Admit complete target systems produced by an alternate assembly path.
    ///
    /// This is the construction seam for owner-routed or device assembly after
    /// it has independently proved exact packet coverage and reconstructed
    /// complete canonical systems. It validates output shape but does not
    /// itself attest how packets were evaluated or transported.
    ///
    /// # Errors
    /// Returns `EQ0806` for zero accepted packets, a target-count mismatch, or
    /// a system dimension that contradicts its ordered target.
    pub fn from_complete_systems(
        plan: &AssemblyPlan,
        systems: Vec<LinearSystem>,
        packet_count: usize,
        execution: ExecutionReport,
    ) -> Result<Self, Diagnostic> {
        if packet_count == 0 {
            return Err(assembly_failed(
                "an assembly result requires at least one accepted packet",
            ));
        }
        if systems.len() != plan.target_count() {
            return Err(assembly_failed(format!(
                "assembly result has {} systems for {} planned targets",
                systems.len(),
                plan.target_count()
            )));
        }
        for (index, (system, target)) in systems.iter().zip(&plan.targets).enumerate() {
            if system.matrix().rows() != target.size || system.matrix().columns() != target.size {
                return Err(assembly_failed(format!(
                    "assembly result target {index} is {}x{} but plan requires {}x{}",
                    system.matrix().rows(),
                    system.matrix().columns(),
                    target.size,
                    target.size
                )));
            }
        }
        Ok(Self {
            systems,
            report: AssemblyReport {
                execution,
                packet_count,
                target_count: plan.target_count(),
            },
        })
    }

    /// One system addressed by a target ID from the input plan.
    #[must_use]
    pub fn system(&self, target: AssemblyTargetId) -> Option<&LinearSystem> {
        self.systems.get(target.0)
    }

    /// Ordered finalized systems.
    #[must_use]
    pub fn systems(&self) -> &[LinearSystem] {
        &self.systems
    }

    /// Exact assembly placement and accepted shape.
    #[must_use]
    pub const fn report(&self) -> &AssemblyReport {
        &self.report
    }

    /// Consume the result into ordered systems and its report.
    #[must_use]
    pub fn into_parts(self) -> (Vec<LinearSystem>, AssemblyReport) {
        (self.systems, self.report)
    }
}

/// Backend-neutral indexed assembly execution.
///
/// [`AssemblyWork`] is `Sync` so a backend may evaluate independent packets
/// concurrently. The backend itself is not required to be `Sync`: a physical
/// transport adapter may own one application-serialized collective stream.
/// Concurrent operations use distinct backend instances rather than sharing
/// one mutable transport context implicitly.
pub trait AssemblyBackend: fmt::Debug {
    /// Evaluate, scatter, and finalize one complete assembly operation.
    ///
    /// # Errors
    /// Returns the lowest failing logical packet diagnostic or a structured
    /// plan/scatter/finalization diagnostic. No partial result escapes.
    fn assemble(
        &self,
        plan: &AssemblyPlan,
        work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, Diagnostic>;
}

/// Shared ordered scatter state for assembly backend implementors.
///
/// Backends may evaluate packets under any placement, but must present them
/// here exactly once in increasing logical index order. This type owns the
/// numerical accumulation tree used by both reference and parallel paths.
#[derive(Debug)]
pub struct AssemblyAccumulator {
    plan: AssemblyPlan,
    assemblers: Vec<CooAssembler>,
    next_packet: usize,
}

impl AssemblyAccumulator {
    /// Allocate one deterministic accumulator per planned target.
    ///
    /// # Errors
    /// Propagates invalid target shape as `EQ0806`.
    pub fn new(plan: &AssemblyPlan) -> Result<Self, Diagnostic> {
        let assemblers = plan
            .targets
            .iter()
            .map(|target| CooAssembler::new(target.size))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            plan: plan.clone(),
            assemblers,
            next_packet: 0,
        })
    }

    /// Scatter the next logical packet through the common ordered path.
    ///
    /// # Errors
    /// Returns `EQ0806` for skipped/repeated indices, a target outside the
    /// plan, invalid global DOFs, or non-finite accumulation.
    pub fn scatter_packet(
        mut self,
        packet_index: usize,
        packet: &AssemblyPacket,
    ) -> Result<Self, Diagnostic> {
        if packet_index != self.next_packet {
            return Err(assembly_failed(format!(
                "ordered assembly expected packet {}, received {packet_index}",
                self.next_packet
            )));
        }
        let projected = packet.project(&self.plan)?;
        for target_delta in projected {
            let assembler = self
                .assemblers
                .get_mut(target_delta.target.0)
                .expect("projected target belongs to accumulator plan");
            assembler.scatter_delta(&target_delta.delta)?;
        }
        self.next_packet += 1;
        Ok(self)
    }

    /// Finalize all targets and attach exact execution evidence.
    ///
    /// # Errors
    /// Returns `EQ0806` if any target has an empty structural row.
    pub fn finish(self, execution: ExecutionReport) -> Result<AssemblyResult, Diagnostic> {
        let target_count = self.assemblers.len();
        let systems = self
            .assemblers
            .into_iter()
            .map(CooAssembler::finish)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AssemblyResult {
            systems,
            report: AssemblyReport {
                execution,
                packet_count: self.next_packet,
                target_count,
            },
        })
    }
}

/// Direct increasing-index assembly used as the deterministic oracle.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReferenceAssemblyBackend;

/// Shared reference assembly backend.
pub const REFERENCE_ASSEMBLY_BACKEND: ReferenceAssemblyBackend = ReferenceAssemblyBackend;

impl AssemblyBackend for ReferenceAssemblyBackend {
    fn assemble(
        &self,
        plan: &AssemblyPlan,
        work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, Diagnostic> {
        if work.packet_count() == 0 {
            return Err(assembly_failed(
                "assembly work requires at least one logical packet",
            ));
        }
        let mut accumulator = AssemblyAccumulator::new(plan)?;
        for packet_index in 0..work.packet_count() {
            let packet = work.evaluate(packet_index)?;
            accumulator = accumulator.scatter_packet(packet_index, &packet)?;
        }
        accumulator.finish(ExecutionReport::host_serial())
    }
}

fn assembly_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::ASSEMBLY_FAILED, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DofId, LocalUnknown};

    fn target_map(target: AssemblyTargetId, dof: usize) -> TargetAssemblyMap {
        let dof = DofId::new(dof);
        TargetAssemblyMap::new(
            target,
            AssemblyMap::new(vec![Some(dof)], vec![LocalUnknown::Free(dof)]).unwrap(),
        )
    }

    #[test]
    fn packet_canonicalizes_targets_and_rejects_duplicates() {
        let plan = AssemblyPlan::new(vec![
            AssemblyTarget::new(1).unwrap(),
            AssemblyTarget::new(1).unwrap(),
        ])
        .unwrap();
        let first = plan.target_id(0).unwrap();
        let second = plan.target_id(1).unwrap();
        let local = LocalContribution::new(1, 1, vec![1.0], vec![2.0]).unwrap();
        let packet = AssemblyPacket::new(
            local.clone(),
            vec![target_map(second, 0), target_map(first, 0)],
        )
        .unwrap();
        assert_eq!(packet.mappings()[0].target(), first);
        assert_eq!(packet.mappings()[1].target(), second);
        let projected = packet.project(&plan).unwrap();
        assert_eq!(projected[0].target(), first);
        assert_eq!(projected[1].target(), second);
        assert_eq!(projected[0].delta().target_size(), 1);
        assert_eq!(
            AssemblyPacket::new(local, vec![target_map(first, 0), target_map(first, 0)])
                .unwrap_err()
                .code(),
            codes::ASSEMBLY_FAILED
        );
    }

    #[test]
    fn reference_assembly_preserves_packet_accumulation_order() {
        let plan = AssemblyPlan::new(vec![AssemblyTarget::new(1).unwrap()]).unwrap();
        let target = plan.target_id(0).unwrap();
        let values = [1.0e16, 1.0, -1.0e16];
        let work = IndexedAssemblyWork::new(values.len(), |index| {
            AssemblyPacket::new(
                LocalContribution::new(1, 1, vec![1.0], vec![values[index]])?,
                vec![target_map(target, 0)],
            )
        });
        let result = REFERENCE_ASSEMBLY_BACKEND.assemble(&plan, &work).unwrap();
        assert_eq!(result.system(target).unwrap().matrix().values(), &[3.0]);
        assert_eq!(result.system(target).unwrap().rhs(), &[0.0]);
        assert_eq!(result.report().packet_count(), values.len());
        assert_eq!(result.report().target_count(), 1);
    }

    #[test]
    fn empty_work_and_target_mismatch_fail_without_a_result() {
        let plan = AssemblyPlan::new(vec![AssemblyTarget::new(1).unwrap()]).unwrap();
        let empty = IndexedAssemblyWork::new(0, |_| unreachable!());
        assert_eq!(
            REFERENCE_ASSEMBLY_BACKEND
                .assemble(&plan, &empty)
                .unwrap_err()
                .code(),
            codes::ASSEMBLY_FAILED
        );

        let foreign_plan = AssemblyPlan::new(vec![
            AssemblyTarget::new(1).unwrap(),
            AssemblyTarget::new(1).unwrap(),
        ])
        .unwrap();
        let foreign = foreign_plan.target_id(1).unwrap();
        let work = IndexedAssemblyWork::new(1, |_| {
            AssemblyPacket::new(
                LocalContribution::new(1, 1, vec![1.0], vec![0.0])?,
                vec![target_map(foreign, 0)],
            )
        });
        assert_eq!(
            REFERENCE_ASSEMBLY_BACKEND
                .assemble(&plan, &work)
                .unwrap_err()
                .code(),
            codes::ASSEMBLY_FAILED
        );
    }

    fn diagonal_system(size: usize) -> LinearSystem {
        LinearSystem::new(
            crate::CsrMatrix::from_sorted_csr(
                size,
                size,
                (0..=size).collect(),
                (0..size).collect(),
                vec![1.0; size],
            )
            .unwrap(),
            vec![0.0; size],
        )
        .unwrap()
    }

    #[test]
    fn complete_result_constructor_checks_packet_and_target_shape() {
        let plan = AssemblyPlan::new(vec![
            AssemblyTarget::new(1).unwrap(),
            AssemblyTarget::new(2).unwrap(),
        ])
        .unwrap();
        let execution = ExecutionReport::host_serial();

        for result in [
            AssemblyResult::from_complete_systems(
                &plan,
                vec![diagonal_system(1), diagonal_system(2)],
                0,
                execution,
            ),
            AssemblyResult::from_complete_systems(&plan, vec![diagonal_system(1)], 1, execution),
            AssemblyResult::from_complete_systems(
                &plan,
                vec![diagonal_system(2), diagonal_system(1)],
                1,
                execution,
            ),
        ] {
            assert_eq!(result.unwrap_err().code(), codes::ASSEMBLY_FAILED);
        }

        let accepted = AssemblyResult::from_complete_systems(
            &plan,
            vec![diagonal_system(1), diagonal_system(2)],
            3,
            execution,
        )
        .unwrap();
        assert_eq!(accepted.report().packet_count(), 3);
        assert_eq!(accepted.report().target_count(), 2);
    }

    #[derive(Debug)]
    struct FailingWork;

    impl AssemblyWork for FailingWork {
        fn packet_set_identity(&self) -> AssemblyPacketSetIdentityV1 {
            AssemblyPacketSetIdentityV1::Unbound
        }

        fn packet_count(&self) -> usize {
            4
        }

        fn evaluate(&self, packet_index: usize) -> Result<AssemblyPacket, Diagnostic> {
            Err(assembly_failed(format!("packet {packet_index} failed")))
        }
    }

    #[test]
    fn reference_reports_the_lowest_failing_packet() {
        let plan = AssemblyPlan::new(vec![AssemblyTarget::new(1).unwrap()]).unwrap();
        let diagnostic = REFERENCE_ASSEMBLY_BACKEND
            .assemble(&plan, &FailingWork)
            .unwrap_err();
        assert_eq!(diagnostic.code(), codes::ASSEMBLY_FAILED);
        assert!(diagnostic.message().contains("packet 0 failed"));
    }
}
