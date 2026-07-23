use std::num::NonZeroU64;

use eqiora_assembly::CsrMatrix;
use eqiora_backend_cuda::{
    CUDA_BINDING_TOOLKIT, CUDARC_VERSION, CudaDeviceObservation, CudaResidentCsrActionEvidence,
    CudaResidentCsrActionSession, CudaResidentCsrSetupEvidence, CudaRuntime,
};
use eqiora_backend_mpi::{
    MPI_ADAPTER_VERSION, MPI_RS_VERSION, MpiAdmittedExecutionAdapter, MpiExecutionGroup,
    MpiRankDeviceTopologyV1, MpiRankLocalCsrAction, RankLocalDeviceV1,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_distributed::{LocalCsrShard, PartitionId};
use eqiora_execution::{AcceptedLinearExecution, AdmittedExecution, DistributedDeviceTransport};
use eqiora_solver::{ExecutionId, ExecutionProvider, ProviderLibrary};
use sha2::{Digest, Sha256};

use crate::MPI_CUDA_ADAPTER_VERSION;

/// Exact rank-local CUDA action selected by the host-staged composition.
pub const MPI_CUDA_LOCAL_ACTION_EXECUTION: ExecutionId =
    ExecutionId::new("eqiora.mpi-cuda.partition-csr");

const MPI_CUDA_LOCAL_ACTION_LIBRARIES: &[ProviderLibrary] = &[
    ProviderLibrary::new("cuda-binding-toolkit", CUDA_BINDING_TOOLKIT),
    ProviderLibrary::new("cudarc", CUDARC_VERSION),
];

/// Exact declared partition-local CUDA action release used by the MPI composition.
pub const MPI_CUDA_LOCAL_ACTION_EXECUTION_PROVIDER: ExecutionProvider = ExecutionProvider::new(
    MPI_CUDA_LOCAL_ACTION_EXECUTION,
    MPI_CUDA_ADAPTER_VERSION,
    MPI_CUDA_LOCAL_ACTION_LIBRARIES,
);

const COMPOSITE_SUMMARY_DOMAIN_V2: &[u8] = b"eqiora.mpi-cuda-execution-summary/v2\0";

/// Rank-local CUDA observations paired with one common accepted MPI result.
///
/// The topology and common summary agree across all ranks. Setup and action
/// records intentionally remain rank-local because device UUIDs, allocation
/// identities, and rectangular shard shapes differ by partition.
#[derive(Debug, Clone, PartialEq)]
pub struct MpiCudaLinearExecutionEvidence {
    partition: PartitionId,
    topology: MpiRankDeviceTopologyV1,
    transport: DistributedDeviceTransport,
    setup: CudaResidentCsrSetupEvidence,
    actions: Vec<CudaResidentCsrActionEvidence>,
    common_summary: [u8; 32],
}

impl MpiCudaLinearExecutionEvidence {
    /// Rank owning the local CUDA observations.
    #[must_use]
    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    /// Common rank-to-physical-device topology.
    #[must_use]
    pub const fn topology(&self) -> &MpiRankDeviceTopologyV1 {
        &self.topology
    }

    /// Explicit host/device transport; never an inferred fallback.
    #[must_use]
    pub const fn transport(&self) -> DistributedDeviceTransport {
        self.transport
    }

    /// Sole resident matrix setup for this rank.
    #[must_use]
    pub const fn setup(&self) -> &CudaResidentCsrSetupEvidence {
        &self.setup
    }

    /// Dense input/action/output observations in exact call order.
    #[must_use]
    pub fn actions(&self) -> &[CudaResidentCsrActionEvidence] {
        &self.actions
    }

    /// Domain-separated common summary agreed before result publication.
    #[must_use]
    pub const fn common_summary(&self) -> [u8; 32] {
        self.common_summary
    }
}

/// Complete host result accepted by MPI and its inseparable rank-local CUDA
/// composition evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedMpiCudaLinearExecution {
    accepted: AcceptedLinearExecution,
    evidence: MpiCudaLinearExecutionEvidence,
}

impl AcceptedMpiCudaLinearExecution {
    /// Unchanged L2 distributed result and receipt.
    #[must_use]
    pub const fn accepted(&self) -> &AcceptedLinearExecution {
        &self.accepted
    }

    /// Host-staged rank/device/action evidence paired with the result.
    #[must_use]
    pub const fn evidence(&self) -> &MpiCudaLinearExecutionEvidence {
        &self.evidence
    }

    /// Consume the result without copying its complete solution or traces.
    #[must_use]
    pub fn into_parts(self) -> (AcceptedLinearExecution, MpiCudaLinearExecutionEvidence) {
        (self.accepted, self.evidence)
    }
}

/// L3 seam consuming one exact distributed-CUDA L2 admission.
pub trait MpiCudaAdmittedExecutionAdapter {
    /// Execute host-staged MPI Krylov with one resident CUDA CSR action on
    /// every rank.
    ///
    /// # Errors
    /// Returns a common diagnostic for binding/runtime/device topology,
    /// resident setup, local action, MPI protocol, receipt, or summary drift.
    fn execute_admitted_mpi_cuda(
        &mut self,
        admitted: AdmittedExecution<'_>,
    ) -> Result<AcceptedMpiCudaLinearExecution, Diagnostic>;
}

impl MpiCudaAdmittedExecutionAdapter for MpiExecutionGroup {
    fn execute_admitted_mpi_cuda(
        &mut self,
        admitted: AdmittedExecution<'_>,
    ) -> Result<AcceptedMpiCudaLinearExecution, Diagnostic> {
        let observation = observe_exact_local_device(&admitted);
        self.agree_composed_local_readiness(&unit_result(&observation))?;
        let observation = observation.map_err(|_| {
            invalid("local CUDA discovery failed despite all-rank preparation readiness agreement")
        })?;

        let local_device = RankLocalDeviceV1::new(
            observation.descriptor().id().ordinal(),
            observation.physical_uuid().as_bytes(),
        );
        let topology = self.agree_rank_device_topology(local_device)?;

        let prepared = prepare_action(&admitted, self.partition(), &observation);
        self.agree_composed_local_readiness(&unit_result(&prepared))?;
        let mut action = prepared.map_err(|_| {
            invalid(
                "local CUDA action setup failed despite all-rank preparation readiness agreement",
            )
        })?;

        let accepted = self.execute_admitted_with_local_action(admitted, &mut action)?;
        let partition = self.partition();
        let local_evidence = action.into_evidence().and_then(|(setup, actions)| {
            validate_local_evidence(partition, &topology, &setup, &actions)?;
            Ok((setup, actions))
        });
        self.agree_composed_local_readiness(&unit_result(&local_evidence))?;
        let (setup, actions) = local_evidence.map_err(|_| {
            invalid(
                "local MPI-CUDA evidence failed despite all-rank validation readiness agreement",
            )
        })?;
        let summary = composite_summary(&accepted, &topology, actions.len(), setup.versions());
        self.agree_composed_local_readiness(&unit_result(&summary))?;
        let common_summary = summary.map_err(|_| {
            invalid(
                "local MPI-CUDA summary failed despite all-rank construction readiness agreement",
            )
        })?;
        self.agree_composed_execution_summary(common_summary)?;

        Ok(AcceptedMpiCudaLinearExecution {
            accepted,
            evidence: MpiCudaLinearExecutionEvidence {
                partition,
                topology,
                transport: DistributedDeviceTransport::HostStaged,
                setup,
                actions,
                common_summary,
            },
        })
    }
}

fn observe_exact_local_device(
    admitted: &AdmittedExecution<'_>,
) -> Result<CudaDeviceObservation, Diagnostic> {
    let placement = admitted
        .binding()
        .cuda_partition_placement()
        .ok_or_else(|| invalid("MPI-CUDA execution requires a partition-local CUDA placement"))?;
    if placement.execution_provider() != MPI_CUDA_LOCAL_ACTION_EXECUTION_PROVIDER
        || admitted.binding().distributed_device_transport()
            != Some(DistributedDeviceTransport::HostStaged)
        || placement.device().id().ordinal() != 0
    {
        return Err(invalid(
            "MPI-CUDA execution requires its exact local-action adapter, host-staged transport, and visible device ordinal zero",
        ));
    }
    let mut visible = CudaRuntime.observe()?;
    if visible.len() != 1 {
        return Err(invalid(
            "MPI-CUDA execution v1 requires exactly one visible CUDA device per rank",
        ));
    }
    let observation = visible.pop().expect("length checked");
    if observation.descriptor() != placement.device() {
        return Err(invalid(
            "live CUDA descriptor differs from the admitted partition placement",
        ));
    }
    Ok(observation)
}

fn prepare_action<'system>(
    admitted: &AdmittedExecution<'system>,
    partition: PartitionId,
    observation: &CudaDeviceObservation,
) -> Result<ResidentCudaAction<'system>, Diagnostic> {
    let placement = admitted
        .binding()
        .cuda_partition_placement()
        .ok_or_else(|| invalid("MPI-CUDA action setup lost its CUDA placement"))?;
    let distributed = admitted
        .distributed_local_action_system()
        .ok_or_else(|| invalid("MPI-CUDA action setup requires sealed distributed algebra"))?;
    let shard = distributed
        .operator()
        .shard(partition)
        .ok_or_else(|| invalid("MPI rank has no admitted CSR shard"))?;
    let capture = shard.capture_execution()?;
    let view = capture.view();
    let matrix = CsrMatrix::from_sorted_csr(
        view.rows(),
        view.columns(),
        try_copy(view.row_offsets(), "local CSR row offsets")?,
        try_copy(view.column_indices(), "local CSR column indices")?,
        try_copy(view.values(), "local CSR values")?,
    )?;
    let session = CudaResidentCsrActionSession::new(
        &matrix,
        observation,
        placement.queue(),
        placement.action_policy(),
    )?;
    let action_limit = admitted
        .solver_plan()
        .maximum_iterations()
        .get()
        .checked_mul(2)
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| invalid("MPI-CUDA local-action evidence bound overflowed"))?;
    ResidentCudaAction::new(shard, view.columns(), session, action_limit)
}

struct ResidentCudaAction<'system> {
    shard: LocalCsrShard<'system>,
    staging: Vec<f64>,
    session: CudaResidentCsrActionSession,
    actions: Vec<CudaResidentCsrActionEvidence>,
    action_limit: usize,
}

impl<'system> ResidentCudaAction<'system> {
    fn new(
        shard: LocalCsrShard<'system>,
        columns: usize,
        session: CudaResidentCsrActionSession,
        action_limit: usize,
    ) -> Result<Self, Diagnostic> {
        let mut staging = Vec::new();
        staging
            .try_reserve_exact(columns)
            .map_err(|_| invalid("could not reserve host-staged MPI-CUDA input"))?;
        staging.resize(columns, 0.0);
        let mut actions = Vec::new();
        actions
            .try_reserve_exact(action_limit)
            .map_err(|_| invalid("could not reserve MPI-CUDA action evidence"))?;
        Ok(Self {
            shard,
            staging,
            session,
            actions,
            action_limit,
        })
    }

    fn into_evidence(
        self,
    ) -> Result<
        (
            CudaResidentCsrSetupEvidence,
            Vec<CudaResidentCsrActionEvidence>,
        ),
        Diagnostic,
    > {
        let observed = usize::try_from(self.session.action_count())
            .map_err(|_| invalid("resident CUDA action count exceeds host usize"))?;
        if observed != self.actions.len() {
            return Err(invalid(
                "resident CUDA session count contradicts its retained action evidence",
            ));
        }
        Ok((self.session.setup_evidence().clone(), self.actions))
    }
}

impl MpiRankLocalCsrAction for ResidentCudaAction<'_> {
    fn apply_owned_rows(
        &mut self,
        shard: LocalCsrShard<'_>,
        owned_input: &[f64],
        ghosts: &[f64],
        owned_output: &mut [f64],
    ) -> Result<(), Diagnostic> {
        if !shard.same_origin(self.shard)
            || owned_input.len() != self.shard.layout().owned().len()
            || ghosts.len() != self.shard.layout().ghosts().len()
            || owned_output.len() != self.shard.layout().owned().len()
            || self.staging.len() != owned_input.len() + ghosts.len()
        {
            return Err(invalid(
                "MPI-CUDA local action differs from its admitted shard or host-staging shape",
            ));
        }
        if self.actions.len() >= self.action_limit {
            return Err(invalid(
                "MPI-CUDA local action exceeded its pre-admitted evidence bound",
            ));
        }
        let owned_end = owned_input.len();
        self.staging[..owned_end].copy_from_slice(owned_input);
        self.staging[owned_end..].copy_from_slice(ghosts);
        let evidence = self.session.apply(&self.staging, owned_output)?;
        self.actions.push(evidence);
        Ok(())
    }
}

fn validate_local_evidence(
    partition: PartitionId,
    topology: &MpiRankDeviceTopologyV1,
    setup: &CudaResidentCsrSetupEvidence,
    actions: &[CudaResidentCsrActionEvidence],
) -> Result<(), Diagnostic> {
    let device = topology
        .devices()
        .get(partition.index())
        .ok_or_else(|| invalid("MPI-CUDA topology omits the local rank"))?;
    if device.ordinal() != setup.device().id().ordinal()
        || device.physical_identity() != setup.physical_uuid().as_bytes()
        || setup.policy() != eqiora_device::SparseActionPolicy::Deterministic
        || actions.is_empty()
    {
        return Err(invalid(
            "MPI-CUDA setup or action evidence contradicts its agreed local device",
        ));
    }
    for (index, action) in actions.iter().enumerate() {
        let expected = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .and_then(NonZeroU64::new)
            .ok_or_else(|| invalid("MPI-CUDA action ordinal exceeds portable u64"))?;
        if action.ordinal() != expected {
            return Err(invalid(
                "MPI-CUDA action evidence ordinals are not dense and one-based",
            ));
        }
    }
    Ok(())
}

fn composite_summary(
    accepted: &AcceptedLinearExecution,
    topology: &MpiRankDeviceTopologyV1,
    action_count: usize,
    versions: eqiora_backend_cuda::CudaLibraryVersions,
) -> Result<[u8; 32], Diagnostic> {
    let receipt = accepted.receipt();
    let trace = receipt.distributed_trace().ok_or_else(|| {
        invalid("MPI-CUDA accepted result omits the common distributed execution trace")
    })?;
    let action_count = u64::try_from(action_count)
        .map_err(|_| invalid("MPI-CUDA action count exceeds portable u64"))?;
    let mut hash = Sha256::new();
    hash.update(COMPOSITE_SUMMARY_DOMAIN_V2);
    hash.update(receipt.operator().as_bytes());
    hash.update(receipt.output().as_bytes());
    hash.update(trace.partition().as_bytes());
    hash.update(trace.layout().as_bytes());
    hash.update(trace.admission().as_bytes());
    hash.update(topology.fingerprint());
    hash.update([0]); // DistributedDeviceTransport::HostStaged v1.
    hash.update(action_count.to_be_bytes());
    hash.update(versions.driver().to_be_bytes());
    hash.update(versions.cusparse().to_be_bytes());
    hash.update(versions.cublas().unwrap_or(-1).to_be_bytes());
    update_text(&mut hash, "partition-local-action-provider");
    update_execution_provider(&mut hash, MPI_CUDA_LOCAL_ACTION_EXECUTION_PROVIDER);
    update_text(&mut hash, versions.cudarc());
    update_text(&mut hash, versions.binding_toolkit());
    update_text(&mut hash, MPI_ADAPTER_VERSION);
    update_text(&mut hash, MPI_RS_VERSION);
    update_text(&mut hash, MPI_CUDA_ADAPTER_VERSION);
    debug_assert_eq!(versions.cudarc(), CUDARC_VERSION);
    debug_assert_eq!(versions.binding_toolkit(), CUDA_BINDING_TOOLKIT);
    Ok(hash.finalize().into())
}

fn update_text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value.as_bytes());
}

fn update_execution_provider(hash: &mut Sha256, provider: ExecutionProvider) {
    update_text(hash, provider.id().as_str());
    update_text(hash, provider.implementation_version());
    hash.update((provider.libraries().len() as u64).to_be_bytes());
    for library in provider.libraries() {
        update_text(hash, library.name());
        update_text(hash, library.version());
    }
}

fn try_copy<T: Copy>(source: &[T], label: &str) -> Result<Vec<T>, Diagnostic> {
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(source.len())
        .map_err(|_| invalid(format!("could not reserve {label}")))?;
    copied.extend_from_slice(source);
    Ok(copied)
}

fn unit_result<T>(result: &Result<T, Diagnostic>) -> Result<(), Diagnostic> {
    result.as_ref().map(|_| ()).map_err(Clone::clone)
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_hash(provider: ExecutionProvider) -> [u8; 32] {
        let mut hash = Sha256::new();
        update_text(&mut hash, "partition-local-action-provider");
        update_execution_provider(&mut hash, provider);
        hash.finalize().into()
    }

    #[test]
    fn local_action_provider_hash_preserves_the_stable_role_identity() {
        let substituted = ExecutionProvider::new(
            ExecutionId::new("eqiora.test.same-release-different-action"),
            MPI_CUDA_LOCAL_ACTION_EXECUTION_PROVIDER.implementation_version(),
            MPI_CUDA_LOCAL_ACTION_EXECUTION_PROVIDER.libraries(),
        );

        assert_ne!(
            provider_hash(MPI_CUDA_LOCAL_ACTION_EXECUTION_PROVIDER),
            provider_hash(substituted)
        );
    }
}
