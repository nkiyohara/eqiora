//! **eqiora-distributed** — backend-neutral distributed algebra contracts.
//!
//! Unique ownership, local/ghost layouts, halo plans, distributed operator
//! shards, and collective policy are independent of transport. The
//! `LoopbackExecutor` is an executable one-process protocol oracle, not an MPI
//! or multi-node support claim.

use std::num::NonZeroUsize;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_solver::{
    CanonicalCsrAgreementFingerprintV1, CanonicalCsrSystemView, DiagonalAvailability,
    LinearOperatorProperties, LinearSolver, PreconditionerPolicy, ReductionPolicy, ScalarType,
    SolveReport, SolverPlan,
};
use sha2::{Digest, Sha256};

const PARTITION_AGREEMENT_DOMAIN_V1: &[u8] = b"eqiora.partition-agreement/v1\0";
const DISTRIBUTED_LAYOUT_AGREEMENT_DOMAIN_V1: &[u8] = b"eqiora.distributed-layout-agreement/v1\0";
const DISTRIBUTED_ADMISSION_DOMAIN_V1: &[u8] = b"eqiora.distributed-admission/v1\0";

/// One execution-group partition index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartitionId(usize);

impl PartitionId {
    /// Construct a zero-based partition index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Zero-based partition index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Global algebraic vector space before distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalVectorSpace {
    dimension: NonZeroUsize,
    scalar_type: ScalarType,
}

impl GlobalVectorSpace {
    /// Construct a nonempty global vector space.
    #[must_use]
    pub const fn new(dimension: NonZeroUsize, scalar_type: ScalarType) -> Self {
        Self {
            dimension,
            scalar_type,
        }
    }

    /// Number of global algebraic entries.
    #[must_use]
    pub const fn dimension(self) -> NonZeroUsize {
        self.dimension
    }

    /// Scalar representation of every entry.
    #[must_use]
    pub const fn scalar_type(self) -> ScalarType {
        self.scalar_type
    }
}

/// Unique owner of every global degree of freedom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    space: GlobalVectorSpace,
    count: NonZeroUsize,
    owners: Vec<PartitionId>,
}

impl Partition {
    /// Validate an explicit unique-owner map.
    ///
    /// Every global entry has exactly one owner by construction. V0 also
    /// requires every declared partition to own at least one entry.
    ///
    /// # Errors
    /// Returns `EQ0807` for a length mismatch, out-of-range owner, empty
    /// partition, or a partition count that cannot be indexed by `usize`.
    pub fn new(
        space: GlobalVectorSpace,
        count: NonZeroUsize,
        owners: Vec<PartitionId>,
    ) -> Result<Self, Diagnostic> {
        if owners.len() != space.dimension.get() {
            return Err(invalid_realization(format!(
                "partition owner map has {} entries for global dimension {}",
                owners.len(),
                space.dimension
            )));
        }
        if count.get() > owners.len() {
            return Err(invalid_realization(format!(
                "{} partitions cannot each own an entry in dimension {}",
                count, space.dimension
            )));
        }
        if let Some(owner) = owners.iter().find(|owner| owner.0 >= count.get()) {
            return Err(invalid_realization(format!(
                "partition owner {} is outside 0..{}",
                owner.0, count
            )));
        }
        let mut owned_counts = Vec::new();
        owned_counts
            .try_reserve_exact(count.get())
            .map_err(|_| invalid_realization("could not reserve partition owner counts"))?;
        owned_counts.resize(count.get(), 0_usize);
        for owner in &owners {
            owned_counts[owner.0] += 1;
        }
        if let Some(empty) = owned_counts.iter().position(|owned| *owned == 0) {
            return Err(invalid_realization(format!(
                "partition {empty} owns no global entries"
            )));
        }
        Ok(Self {
            space,
            count,
            owners,
        })
    }

    /// Deterministic balanced contiguous ownership.
    ///
    /// # Errors
    /// Returns `EQ0807` when more partitions than global entries are
    /// requested.
    pub fn balanced_contiguous(
        space: GlobalVectorSpace,
        count: NonZeroUsize,
    ) -> Result<Self, Diagnostic> {
        if count.get() > space.dimension.get() {
            return Err(invalid_realization(format!(
                "{} partitions cannot each own an entry in dimension {}",
                count, space.dimension
            )));
        }
        let dimension = space.dimension.get();
        let base = dimension / count.get();
        let longer_partitions = dimension % count.get();
        let longer_extent = (base + 1) * longer_partitions;
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(dimension)
            .map_err(|_| invalid_realization("could not reserve balanced partition owner map"))?;
        for global in 0..dimension {
            let owner = if global < longer_extent {
                global / (base + 1)
            } else {
                longer_partitions + (global - longer_extent) / base
            };
            owners.push(PartitionId(owner));
        }
        Self::new(space, count, owners)
    }

    /// Global vector space being partitioned.
    #[must_use]
    pub const fn space(&self) -> GlobalVectorSpace {
        self.space
    }

    /// Execution-group partition count.
    #[must_use]
    pub const fn count(&self) -> NonZeroUsize {
        self.count
    }

    /// Unique owner of one global index.
    #[must_use]
    pub fn owner(&self, global: usize) -> Option<PartitionId> {
        self.owners.get(global).copied()
    }

    /// Complete owner map in ascending global-index order.
    #[must_use]
    pub fn owners(&self) -> &[PartitionId] {
        &self.owners
    }

    /// Ascending global indices uniquely owned by one declared partition.
    ///
    /// An out-of-range partition yields an empty iterator.
    pub fn owned_indices(&self, partition: PartitionId) -> impl Iterator<Item = usize> + '_ {
        self.owners
            .iter()
            .enumerate()
            .filter_map(move |(global, owner)| (*owner == partition).then_some(global))
    }
}

/// Fixed-size L2 identity for one complete unique-owner partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartitionAgreementIdentityV1([u8; 32]);

impl PartitionAgreementIdentityV1 {
    /// Raw SHA-256 bytes for fixed-size collective comparison.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Fixed-size L2 identity for derived local layouts and halo exchanges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DistributedLayoutAgreementIdentityV1([u8; 32]);

impl DistributedLayoutAgreementIdentityV1 {
    /// Raw SHA-256 bytes for fixed-size collective comparison.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Fixed-size L2 identity for collective system/partition/plan admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DistributedAdmissionFingerprintV1([u8; 32]);

impl DistributedAdmissionFingerprintV1 {
    /// Raw SHA-256 bytes for fixed-size collective comparison.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Owned and ordered ghost indices visible to one partition.
#[derive(Debug, PartialEq, Eq)]
pub struct LocalLayout {
    partition: PartitionId,
    owned: Vec<usize>,
    ghosts: Vec<usize>,
}

impl LocalLayout {
    fn reserved(
        partition: &Partition,
        id: PartitionId,
        owned_count: usize,
        ghost_candidate_count: usize,
    ) -> Result<Self, Diagnostic> {
        if id.0 >= partition.count.get() {
            return Err(invalid_realization(format!(
                "local layout partition {} is outside 0..{}",
                id.0, partition.count
            )));
        }
        Ok(Self {
            partition: id,
            owned: realization_vector(owned_count, "local owned-index map")?,
            ghosts: realization_vector(ghost_candidate_count, "local ghost candidates")?,
        })
    }

    fn canonicalize(
        &mut self,
        partition: &Partition,
        owned_count: usize,
        ghost_candidate_count: usize,
    ) -> Result<(), Diagnostic> {
        if self.owned.len() != owned_count || self.ghosts.len() != ghost_candidate_count {
            return Err(invalid_realization(
                "pre-counted local layout shape changed during derivation",
            ));
        }
        if self
            .owned
            .iter()
            .any(|global| partition.owner(*global) != Some(self.partition))
            || self.owned.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid_realization(
                "local owned-index map contradicts unique ordered ownership",
            ));
        }
        self.ghosts.sort_unstable();
        self.ghosts.dedup();
        if let Some(global) = self
            .ghosts
            .iter()
            .find(|global| partition.owner(**global).is_none())
        {
            return Err(invalid_realization(format!(
                "ghost index {global} is outside the global vector space"
            )));
        }
        if let Some(global) = self
            .ghosts
            .iter()
            .find(|global| partition.owner(**global) == Some(self.partition))
        {
            return Err(invalid_realization(format!(
                "owned index {global} cannot also be a ghost"
            )));
        }
        Ok(())
    }

    /// Partition whose local vector uses this layout.
    #[must_use]
    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    /// Globally indexed entries uniquely owned here, in ascending order.
    #[must_use]
    pub fn owned(&self) -> &[usize] {
        &self.owned
    }

    /// Read-only cached global entries owned elsewhere, in ascending order.
    #[must_use]
    pub fn ghosts(&self) -> &[usize] {
        &self.ghosts
    }
}

/// One ordered owner-to-receiver halo transfer.
#[derive(Debug, PartialEq, Eq)]
pub struct HaloExchange {
    owner: PartitionId,
    receiver: PartitionId,
    indices: Vec<usize>,
}

impl HaloExchange {
    /// Source partition that uniquely owns every transferred entry.
    #[must_use]
    pub const fn owner(&self) -> PartitionId {
        self.owner
    }

    /// Partition caching the entries as ghosts.
    #[must_use]
    pub const fn receiver(&self) -> PartitionId {
        self.receiver
    }

    /// Ascending global indices transferred between this peer pair.
    #[must_use]
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }
}

/// Deterministic halo communication derived from operator sparsity.
#[derive(Debug, PartialEq, Eq)]
pub struct HaloPlan {
    exchanges: Vec<HaloExchange>,
}

impl HaloPlan {
    fn derive(partition: &Partition, layouts: &[LocalLayout]) -> Result<Self, Diagnostic> {
        if layouts.len() != partition.count.get()
            || layouts
                .iter()
                .enumerate()
                .any(|(index, layout)| layout.partition.index() != index)
        {
            return Err(invalid_realization(
                "halo derivation requires one ordered layout per partition",
            ));
        }

        let triple_count = layouts.iter().try_fold(0_usize, |count, layout| {
            checked_count_add(count, layout.ghosts.len(), "halo transfer triples")
        })?;
        let mut triples = realization_vector(triple_count, "halo transfer triples")?;
        for layout in layouts {
            for &global in &layout.ghosts {
                let owner = partition
                    .owner(global)
                    .ok_or_else(|| invalid_realization("halo ghost has no unique owner"))?;
                triples.push((owner, layout.partition, global));
            }
        }
        triples.sort_unstable();
        if triples.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid_realization(
                "halo derivation produced a duplicate owner/receiver/index transfer",
            ));
        }

        let exchange_count = triples
            .windows(2)
            .filter(|pair| (pair[0].0, pair[0].1) != (pair[1].0, pair[1].1))
            .count()
            .checked_add(usize::from(!triples.is_empty()))
            .ok_or_else(|| invalid_realization("halo exchange count overflowed"))?;
        let mut exchanges = realization_vector(exchange_count, "halo exchange table")?;
        let mut first = 0;
        while first < triples.len() {
            let (owner, receiver, _) = triples[first];
            let mut end = first + 1;
            while end < triples.len() && (triples[end].0, triples[end].1) == (owner, receiver) {
                end += 1;
            }
            let mut indices = realization_vector(end - first, "halo exchange indices")?;
            for &(_, _, global) in &triples[first..end] {
                indices.push(global);
            }
            exchanges.push(HaloExchange {
                owner,
                receiver,
                indices,
            });
            first = end;
        }
        Ok(Self { exchanges })
    }

    /// Peer exchanges in `(owner, receiver)` order.
    #[must_use]
    pub fn exchanges(&self) -> &[HaloExchange] {
        &self.exchanges
    }
}

#[derive(Debug, PartialEq)]
struct LocalCsrStorage {
    partition: PartitionId,
    row_offsets: Vec<usize>,
    column_indices: Vec<usize>,
    values: Vec<f64>,
}

/// One partition's canonical owned rows for an already assembled linear system.
///
/// Rows and columns retain global indices. This is the transport-neutral seam
/// for promoting an accepted owner-row assembly without choosing another
/// partition or rebuilding shards from a complete matrix.
#[derive(Debug, PartialEq)]
pub struct OwnedLinearSystemShard {
    partition: PartitionId,
    global_dimension: NonZeroUsize,
    rows: Vec<usize>,
    row_offsets: Vec<usize>,
    column_indices: Vec<usize>,
    values: Vec<f64>,
    right_hand_side: Vec<f64>,
}

impl OwnedLinearSystemShard {
    /// Validate one compact owned-row CSR shard.
    ///
    /// # Errors
    /// Returns `EQ0807` for noncanonical rows/CSR, an out-of-range index,
    /// shape disagreement, or non-finite numerical content.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        partition: PartitionId,
        global_dimension: NonZeroUsize,
        rows: Vec<usize>,
        row_offsets: Vec<usize>,
        column_indices: Vec<usize>,
        values: Vec<f64>,
        right_hand_side: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        let expected_row_offsets = rows.len().checked_add(1).ok_or_else(|| {
            invalid_realization("owned linear-system shard row count exceeds addressable storage")
        })?;
        if rows.windows(2).any(|pair| pair[0] >= pair[1])
            || rows.iter().any(|row| *row >= global_dimension.get())
            || right_hand_side.len() != rows.len()
            || right_hand_side.iter().any(|value| !value.is_finite())
            || row_offsets.len() != expected_row_offsets
            || row_offsets.first() != Some(&0)
            || row_offsets.last() != Some(&column_indices.len())
            || column_indices.len() != values.len()
            || row_offsets.windows(2).any(|pair| pair[0] > pair[1])
            || column_indices
                .iter()
                .any(|column| *column >= global_dimension.get())
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(invalid_realization(
                "owned linear-system shard has noncanonical shape, indices, or values",
            ));
        }
        for row in 0..rows.len() {
            if column_indices[row_offsets[row]..row_offsets[row + 1]]
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(invalid_realization(
                    "owned linear-system shard columns must be strictly ordered within every row",
                ));
            }
        }
        Ok(Self {
            partition,
            global_dimension,
            rows,
            row_offsets,
            column_indices,
            values,
            right_hand_side,
        })
    }

    /// Owning execution-group partition.
    #[must_use]
    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    /// Strictly increasing owned global rows.
    #[must_use]
    pub fn rows(&self) -> &[usize] {
        &self.rows
    }
}

/// Borrowed pairing of one canonical local layout and its owned-row CSR data.
///
/// [`DistributedCsr`] owns each [`LocalLayout`] exactly once. This view keeps
/// shard action convenient without cloning that identity-bearing layout into
/// the numerical storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalCsrShard<'a> {
    layout: &'a LocalLayout,
    storage: &'a LocalCsrStorage,
}

/// Owned remapping from one accepted shard to its rectangular execution view.
///
/// The accepted shard remains the sole source of rows, values, and global
/// layout identity. This capture owns only the backend-independent local
/// column ordinals and the exactly permuted coefficients needed to present a
/// canonical rectangular sparse execution view.
#[derive(Debug, PartialEq)]
pub struct LocalCsrExecutionCapture<'a> {
    shard: LocalCsrShard<'a>,
    local_column_indices: Vec<usize>,
    local_values: Vec<f64>,
}

impl LocalCsrExecutionCapture<'_> {
    /// Borrow the rectangular CSR view represented by this capture.
    ///
    /// Columns address a local input vector ordered as all owned entries in
    /// [`LocalLayout::owned`] order followed by all ghosts in
    /// [`LocalLayout::ghosts`] order.
    #[must_use]
    pub fn view(&self) -> LocalCsrExecutionView<'_> {
        LocalCsrExecutionView {
            partition: self.shard.layout.partition,
            owned_global_indices: &self.shard.layout.owned,
            ghost_global_indices: &self.shard.layout.ghosts,
            row_offsets: &self.shard.storage.row_offsets,
            column_indices: &self.local_column_indices,
            values: &self.local_values,
        }
    }
}

/// Borrowed rectangular CSR with deterministic partition-local columns.
///
/// Its rows are the shard's owned global rows in ascending order. Its input
/// columns use the canonical `[owned | ghost]` layout described by
/// [`LocalCsrExecutionCapture::view`]. This is an execution projection and
/// deliberately carries no independent algebraic identity. Entries within
/// each row are ordered by the local column ordinal; coefficients are copied
/// only to preserve their exact pairing through that deterministic
/// permutation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalCsrExecutionView<'a> {
    partition: PartitionId,
    owned_global_indices: &'a [usize],
    ghost_global_indices: &'a [usize],
    row_offsets: &'a [usize],
    column_indices: &'a [usize],
    values: &'a [f64],
}

impl LocalCsrExecutionView<'_> {
    /// Partition whose owned rows form this view.
    #[must_use]
    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    /// Number of owned rows.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.owned_global_indices.len()
    }

    /// Number of local input columns: owned entries followed by ghosts.
    #[must_use]
    pub const fn columns(&self) -> usize {
        self.owned_global_indices.len() + self.ghost_global_indices.len()
    }

    /// Ascending global rows and leading local columns owned by this partition.
    #[must_use]
    pub const fn owned_global_indices(&self) -> &[usize] {
        self.owned_global_indices
    }

    /// Ascending global indices represented by trailing local columns.
    #[must_use]
    pub const fn ghost_global_indices(&self) -> &[usize] {
        self.ghost_global_indices
    }

    /// CSR row offsets for the owned rows.
    #[must_use]
    pub const fn row_offsets(&self) -> &[usize] {
        self.row_offsets
    }

    /// CSR column ordinals into the canonical local `[owned | ghost]` input.
    #[must_use]
    pub const fn column_indices(&self) -> &[usize] {
        self.column_indices
    }

    /// CSR values borrowed from the capture-owned exact permutation.
    #[must_use]
    pub const fn values(&self) -> &[f64] {
        self.values
    }
}

impl<'a> LocalCsrShard<'a> {
    /// Owned/ghost vector layout used by this shard.
    #[must_use]
    pub const fn layout(&self) -> &LocalLayout {
        self.layout
    }

    /// Whether two views borrow the exact same sealed layout and CSR storage.
    ///
    /// This process-local authority check is intentionally distinct from
    /// structural equality and from durable algebraic identity. A repeated
    /// solver action can prove it received the admitted objects in constant
    /// time without walking resident coefficients; an equal clone is not the
    /// same origin.
    #[must_use]
    pub fn same_origin(self, other: Self) -> bool {
        std::ptr::eq(self.layout, other.layout) && std::ptr::eq(self.storage, other.storage)
    }

    /// Capture a backend-independent rectangular CSR execution projection.
    ///
    /// Every global column is deterministically remapped into a local input
    /// ordered as `[owned | ghost]`. Rows, values, and layout identity remain
    /// derived from this accepted shard; the capture owns only the local
    /// remap and its exact coefficient permutation.
    ///
    /// # Errors
    /// Returns `EQ0807` if the shard and layout disagree, the CSR shape is not
    /// canonical, or a global column is absent from both owned and ghost maps.
    pub fn capture_execution(self) -> Result<LocalCsrExecutionCapture<'a>, Diagnostic> {
        let expected_offsets = self.layout.owned.len().checked_add(1).ok_or_else(|| {
            invalid_realization("local CSR execution row-offset count overflowed")
        })?;
        let local_columns = self
            .layout
            .owned
            .len()
            .checked_add(self.layout.ghosts.len())
            .ok_or_else(|| invalid_realization("local CSR execution column count overflowed"))?;
        if self.storage.partition != self.layout.partition
            || self.layout.owned.is_empty()
            || self.layout.owned.windows(2).any(|pair| pair[0] >= pair[1])
            || self.layout.ghosts.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .layout
                .owned
                .iter()
                .any(|global| self.layout.ghosts.binary_search(global).is_ok())
            || self.storage.row_offsets.len() != expected_offsets
            || self.storage.row_offsets.first() != Some(&0)
            || self.storage.row_offsets.last() != Some(&self.storage.column_indices.len())
            || self
                .storage
                .row_offsets
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self.storage.column_indices.len() != self.storage.values.len()
            || self.storage.values.iter().any(|value| !value.is_finite())
        {
            return Err(invalid_realization(
                "local CSR execution capture has noncanonical layout or shape",
            ));
        }
        for local_row in 0..self.layout.owned.len() {
            let start = self.storage.row_offsets[local_row];
            let end = self.storage.row_offsets[local_row + 1];
            if self.storage.column_indices[start..end]
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(invalid_realization(
                    "local CSR execution columns must be globally ordered within every row",
                ));
            }
        }

        let mut local_column_indices = realization_vector(
            self.storage.column_indices.len(),
            "local CSR execution column remap",
        )?;
        let mut local_values = realization_vector(
            self.storage.values.len(),
            "local CSR execution coefficient permutation",
        )?;
        let maximum_row_entries = self
            .storage
            .row_offsets
            .windows(2)
            .map(|range| range[1] - range[0])
            .max()
            .unwrap_or(0);
        let mut row_entries =
            realization_vector(maximum_row_entries, "local CSR execution row permutation")?;
        for local_row in 0..self.layout.owned.len() {
            row_entries.clear();
            let start = self.storage.row_offsets[local_row];
            let end = self.storage.row_offsets[local_row + 1];
            for (&global, &value) in self.storage.column_indices[start..end]
                .iter()
                .zip(&self.storage.values[start..end])
            {
                let local = match self.layout.owned.binary_search(&global) {
                    Ok(owned) => owned,
                    Err(_) => {
                        let ghost = self.layout.ghosts.binary_search(&global).map_err(|_| {
                            invalid_realization(format!(
                                "local CSR execution column {global} is neither owned nor ghosted"
                            ))
                        })?;
                        self.layout.owned.len().checked_add(ghost).ok_or_else(|| {
                            invalid_realization("local CSR execution ghost ordinal overflowed")
                        })?
                    }
                };
                if local >= local_columns {
                    return Err(invalid_realization(
                        "local CSR execution remap exceeds its rectangular column extent",
                    ));
                }
                row_entries.push((local, value));
            }
            row_entries.sort_unstable_by_key(|(column, _)| *column);
            if row_entries.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
                return Err(invalid_realization(
                    "local CSR execution remap produced duplicate local columns",
                ));
            }
            local_column_indices.extend(row_entries.iter().map(|(column, _)| *column));
            local_values.extend(row_entries.iter().map(|(_, value)| *value));
        }
        Ok(LocalCsrExecutionCapture {
            shard: self,
            local_column_indices,
            local_values,
        })
    }

    /// Apply owned rows from owned and already-exchanged ghost values.
    ///
    /// Inputs and output use the exact ascending orders in [`LocalLayout`].
    ///
    /// # Errors
    /// Returns `EQ0802` for shape/non-finite values or an internally missing
    /// local/ghost column.
    pub fn apply(
        &self,
        owned: &[f64],
        ghosts: &[f64],
        output: &mut [f64],
    ) -> Result<(), Diagnostic> {
        if owned.len() != self.layout.owned.len()
            || ghosts.len() != self.layout.ghosts.len()
            || output.len() != self.layout.owned.len()
            || owned.iter().chain(ghosts).any(|value| !value.is_finite())
        {
            return Err(solve_failed(
                "local CSR input/output shapes or values contradict its layout",
            ));
        }
        for (local_row, value) in output.iter_mut().enumerate() {
            let mut sum = 0.0;
            for entry in
                self.storage.row_offsets[local_row]..self.storage.row_offsets[local_row + 1]
            {
                let global = self.storage.column_indices[entry];
                let input = match self.layout.owned.binary_search(&global) {
                    Ok(local) => owned[local],
                    Err(_) => {
                        let local = self.layout.ghosts.binary_search(&global).map_err(|_| {
                            solve_failed(format!(
                                "local CSR column {global} is neither owned nor ghosted"
                            ))
                        })?;
                        ghosts[local]
                    }
                };
                sum = finite_sum(sum, self.storage.values[entry] * input)?;
            }
            *value = sum;
        }
        Ok(())
    }

    /// Write diagonal entries for the owned rows in local-layout order.
    ///
    /// # Errors
    /// Returns `EQ0802` when `output` does not match the owned-row extent.
    pub fn diagonal(&self, output: &mut [f64]) -> Result<DiagonalAvailability, Diagnostic> {
        if output.len() != self.layout.owned.len() {
            return Err(solve_failed(
                "local CSR diagonal output must match its owned-row extent",
            ));
        }
        for (local, (&global, value)) in self.layout.owned.iter().zip(output).enumerate() {
            let start = self.storage.row_offsets[local];
            let end = self.storage.row_offsets[local + 1];
            let Ok(offset) = self.storage.column_indices[start..end].binary_search(&global) else {
                return Ok(DiagonalAvailability::Unavailable);
            };
            *value = self.storage.values[start + offset];
        }
        Ok(DiagonalAvailability::Available)
    }
}

/// One square CSR operator lowered into owned-row shards and a halo plan.
#[derive(Debug, PartialEq)]
pub struct DistributedCsr {
    partition: Partition,
    layouts: Vec<LocalLayout>,
    shards: Vec<LocalCsrStorage>,
    halo: HaloPlan,
}

impl DistributedCsr {
    /// Partition a finite square global CSR artifact.
    ///
    /// This is an explicit realization copy: each shard retains only its
    /// owned rows, with global column indices used to derive ghost storage.
    ///
    /// # Errors
    /// Returns `EQ0807` for malformed CSR structure, non-finite values, or a
    /// non-`f64` global space.
    pub fn from_global_csr(
        partition: Partition,
        row_offsets: &[usize],
        column_indices: &[usize],
        values: &[f64],
    ) -> Result<Self, Diagnostic> {
        let dimension = partition.space.dimension.get();
        if partition.space.scalar_type != ScalarType::F64 {
            return Err(invalid_realization(
                "distributed CSR v0 executes only f64 vectors",
            ));
        }
        validate_csr(dimension, row_offsets, column_indices, values)?;

        #[derive(Debug, Clone, Copy, Default)]
        struct ShardShape {
            owned_rows: usize,
            nonzeros: usize,
            ghost_candidates: usize,
        }

        let mut shapes = realization_vector(partition.count.get(), "local CSR shape table")?;
        shapes.resize(partition.count.get(), ShardShape::default());
        for row in 0..dimension {
            let owner = partition.owners[row].index();
            let row_nonzeros = row_offsets[row + 1] - row_offsets[row];
            shapes[owner].owned_rows =
                checked_count_add(shapes[owner].owned_rows, 1, "local owned-row count")?;
            shapes[owner].nonzeros = checked_count_add(
                shapes[owner].nonzeros,
                row_nonzeros,
                "local CSR nonzero count",
            )?;
            for &column in &column_indices[row_offsets[row]..row_offsets[row + 1]] {
                if partition.owners[column].index() != owner {
                    shapes[owner].ghost_candidates = checked_count_add(
                        shapes[owner].ghost_candidates,
                        1,
                        "local ghost-candidate count",
                    )?;
                }
            }
        }

        let mut layouts = realization_vector(partition.count.get(), "local layout table")?;
        for id in 0..partition.count.get() {
            let id = PartitionId(id);
            let shape = shapes[id.index()];
            layouts.push(LocalLayout::reserved(
                &partition,
                id,
                shape.owned_rows,
                shape.ghost_candidates,
            )?);
        }
        for row in 0..dimension {
            let owner = partition.owners[row];
            let layout = &mut layouts[owner.index()];
            layout.owned.push(row);
            for &column in &column_indices[row_offsets[row]..row_offsets[row + 1]] {
                if partition.owners[column] != owner {
                    layout.ghosts.push(column);
                }
            }
        }
        for (layout, shape) in layouts.iter_mut().zip(&shapes) {
            layout.canonicalize(&partition, shape.owned_rows, shape.ghost_candidates)?;
        }

        let mut shards = realization_vector(layouts.len(), "local CSR shard table")?;
        for (layout, shape) in layouts.iter().zip(&shapes) {
            let offset_count = shape
                .owned_rows
                .checked_add(1)
                .ok_or_else(|| invalid_realization("local CSR offset count overflowed"))?;
            let mut local_offsets = realization_vector(offset_count, "local CSR row offsets")?;
            let mut local_columns = realization_vector(shape.nonzeros, "local CSR column indices")?;
            let mut local_values = realization_vector(shape.nonzeros, "local CSR values")?;
            local_offsets.push(0);
            for &row in &layout.owned {
                for entry in row_offsets[row]..row_offsets[row + 1] {
                    local_columns.push(column_indices[entry]);
                    local_values.push(values[entry]);
                }
                local_offsets.push(local_columns.len());
            }
            if local_offsets.len() != offset_count
                || local_columns.len() != shape.nonzeros
                || local_values.len() != shape.nonzeros
            {
                return Err(invalid_realization(
                    "pre-counted local CSR shape changed during materialization",
                ));
            }
            shards.push(LocalCsrStorage {
                partition: layout.partition,
                row_offsets: local_offsets,
                column_indices: local_columns,
                values: local_values,
            });
        }
        let halo = HaloPlan::derive(&partition, &layouts)?;
        Ok(Self {
            partition,
            layouts,
            shards,
            halo,
        })
    }

    /// Unique global ownership map.
    #[must_use]
    pub const fn partition(&self) -> &Partition {
        &self.partition
    }

    /// Local layouts in partition-index order.
    #[must_use]
    pub fn layouts(&self) -> &[LocalLayout] {
        &self.layouts
    }

    /// Derived deterministic halo plan.
    #[must_use]
    pub const fn halo(&self) -> &HaloPlan {
        &self.halo
    }

    /// Owned-row shard for one partition.
    #[must_use]
    pub fn shard(&self, partition: PartitionId) -> Option<LocalCsrShard<'_>> {
        let layout = self.layouts.get(partition.index())?;
        let storage = self.shards.get(partition.index())?;
        if layout.partition != partition || storage.partition != partition {
            return None;
        }
        Some(LocalCsrShard { layout, storage })
    }
}

/// Complete algebraic source lowered into one validated distributed layout.
///
/// The complete matrix is represented by its Eqiora-owned CSR agreement
/// identity while [`DistributedCsr`] owns the derived row shards. The finite
/// complete RHS and its rank-local projections are retained here. Mesh,
/// assembly, transport, reconstruction, and solver policy remain outside this
/// contract.
#[derive(Debug, PartialEq)]
pub struct DistributedLinearSystem {
    operator: DistributedCsr,
    complete_right_hand_side: Vec<f64>,
    local_right_hand_sides: Vec<Vec<f64>>,
    properties: LinearOperatorProperties,
    system_identity: CanonicalCsrAgreementFingerprintV1,
    partition_identity: PartitionAgreementIdentityV1,
    layout_identity: DistributedLayoutAgreementIdentityV1,
}

impl DistributedLinearSystem {
    /// Derive distributed shards, layouts, halo, RHS projections, and all L2
    /// identities from one canonical complete view and one owner map.
    ///
    /// # Errors
    /// Returns `EQ0807` when dimensions or scalar types disagree, or when the
    /// derived distributed CSR violates its closed contract.
    pub fn from_complete(
        complete: &CanonicalCsrSystemView,
        partition: Partition,
    ) -> Result<Self, Diagnostic> {
        if partition.space.scalar_type != ScalarType::F64
            || partition.space.dimension.get() != complete.rows()
            || complete.rows() != complete.columns()
        {
            return Err(invalid_realization(format!(
                "complete {}x{} f64 CSR and partition {:?} dimension {} do not agree",
                complete.rows(),
                complete.columns(),
                partition.space.scalar_type,
                partition.space.dimension
            )));
        }
        let operator = DistributedCsr::from_global_csr(
            partition,
            complete.row_offsets(),
            complete.column_indices(),
            complete.values(),
        )?;
        let complete_right_hand_side = realization_copy(
            complete.right_hand_side(),
            "complete distributed right-hand side",
        )?;
        let mut local_right_hand_sides =
            realization_vector(operator.layouts.len(), "rank-local RHS table")?;
        for layout in &operator.layouts {
            let mut local = realization_vector(layout.owned.len(), "one rank-local RHS")?;
            for &global in &layout.owned {
                local.push(complete_right_hand_side[global]);
            }
            local_right_hand_sides.push(local);
        }
        let system_identity = complete.agreement_fingerprint();
        let partition_identity = partition_agreement_identity(operator.partition())?;
        let layout_identity =
            distributed_layout_agreement_identity(system_identity, partition_identity, &operator)?;
        Ok(Self {
            operator,
            complete_right_hand_side,
            local_right_hand_sides,
            properties: complete.properties(),
            system_identity,
            partition_identity,
            layout_identity,
        })
    }

    /// Promote already assembled owned rows into the unique distributed
    /// system for their accepted owner map.
    ///
    /// Every shard is compared bit-for-bit with the complete verifier before
    /// it can enter the operator. The supplied partition must own exactly the
    /// rows carried by the shards; there is no balancing or secondary
    /// partition choice in this constructor.
    ///
    /// # Errors
    /// Returns `EQ0807` for a missing, duplicate, misowned, reordered, or
    /// numerically different shard, or for disagreement with the complete
    /// canonical system.
    pub fn from_owned_shards(
        complete: &CanonicalCsrSystemView,
        partition: Partition,
        mut shards: Vec<OwnedLinearSystemShard>,
    ) -> Result<Self, Diagnostic> {
        if partition.space.scalar_type != ScalarType::F64
            || partition.space.dimension.get() != complete.rows()
            || complete.rows() != complete.columns()
        {
            return Err(invalid_realization(format!(
                "complete {}x{} f64 CSR and owned-row partition {:?} dimension {} do not agree",
                complete.rows(),
                complete.columns(),
                partition.space.scalar_type,
                partition.space.dimension
            )));
        }
        shards.sort_unstable_by_key(|shard| shard.partition.index());
        if shards.len() != partition.count.get() {
            return Err(invalid_realization(format!(
                "owned-row inventory has {} shards for {} partitions",
                shards.len(),
                partition.count
            )));
        }

        for (index, shard) in shards.iter().enumerate() {
            let id = PartitionId(index);
            if shard.partition != id
                || shard.global_dimension != partition.space.dimension
                || !shard.rows.iter().copied().eq(partition.owned_indices(id))
            {
                return Err(invalid_realization(format!(
                    "owned-row shard {index} contradicts the accepted partition",
                )));
            }
            for (local, &global) in shard.rows.iter().enumerate() {
                let complete_range =
                    complete.row_offsets()[global]..complete.row_offsets()[global + 1];
                let local_range = shard.row_offsets[local]..shard.row_offsets[local + 1];
                if shard.column_indices[local_range.clone()]
                    != complete.column_indices()[complete_range.clone()]
                    || !same_f64_bits(
                        &shard.values[local_range],
                        &complete.values()[complete_range],
                    )
                    || shard.right_hand_side[local].to_bits()
                        != complete.right_hand_side()[global].to_bits()
                {
                    return Err(invalid_realization(format!(
                        "owned-row shard {index} row {global} differs from the complete canonical system",
                    )));
                }
            }
        }

        let mut layouts = realization_vector(partition.count.get(), "owned-row layout table")?;
        for shard in &shards {
            let ghost_candidates = shard
                .column_indices
                .iter()
                .filter(|column| partition.owner(**column) != Some(shard.partition))
                .count();
            let mut layout = LocalLayout::reserved(
                &partition,
                shard.partition,
                shard.rows.len(),
                ghost_candidates,
            )?;
            layout.owned.extend_from_slice(&shard.rows);
            layout.ghosts.extend(
                shard
                    .column_indices
                    .iter()
                    .copied()
                    .filter(|column| partition.owner(*column) != Some(shard.partition)),
            );
            layout.canonicalize(&partition, shard.rows.len(), ghost_candidates)?;
            layouts.push(layout);
        }
        let halo = HaloPlan::derive(&partition, &layouts)?;
        let mut local_right_hand_sides =
            realization_vector(shards.len(), "accepted local RHS table")?;
        let mut storage = realization_vector(shards.len(), "accepted local CSR table")?;
        for shard in shards {
            local_right_hand_sides.push(shard.right_hand_side);
            storage.push(LocalCsrStorage {
                partition: shard.partition,
                row_offsets: shard.row_offsets,
                column_indices: shard.column_indices,
                values: shard.values,
            });
        }
        let operator = DistributedCsr {
            partition,
            layouts,
            shards: storage,
            halo,
        };
        let complete_right_hand_side = realization_copy(
            complete.right_hand_side(),
            "complete distributed right-hand side",
        )?;
        let system_identity = complete.agreement_fingerprint();
        let partition_identity = partition_agreement_identity(operator.partition())?;
        let layout_identity =
            distributed_layout_agreement_identity(system_identity, partition_identity, &operator)?;
        Ok(Self {
            operator,
            complete_right_hand_side,
            local_right_hand_sides,
            properties: complete.properties(),
            system_identity,
            partition_identity,
            layout_identity,
        })
    }

    /// Derived distributed CSR, including partition, shards, and halo plan.
    #[must_use]
    pub const fn operator(&self) -> &DistributedCsr {
        &self.operator
    }

    /// Unique owner map.
    #[must_use]
    pub const fn partition(&self) -> &Partition {
        self.operator.partition()
    }

    /// Complete finite RHS in global-index order.
    #[must_use]
    pub fn complete_right_hand_side(&self) -> &[f64] {
        &self.complete_right_hand_side
    }

    /// Asserted mathematical properties inherited from the complete view.
    #[must_use]
    pub const fn properties(&self) -> LinearOperatorProperties {
        self.properties
    }

    /// Exact complete-system algebraic agreement identity.
    #[must_use]
    pub const fn system_identity(&self) -> CanonicalCsrAgreementFingerprintV1 {
        self.system_identity
    }

    /// Exact owner-map agreement identity.
    #[must_use]
    pub const fn partition_identity(&self) -> PartitionAgreementIdentityV1 {
        self.partition_identity
    }

    /// Exact derived layout/halo agreement identity.
    #[must_use]
    pub const fn layout_identity(&self) -> DistributedLayoutAgreementIdentityV1 {
        self.layout_identity
    }

    /// Confirm that a supplied complete view is the exact captured algebraic
    /// source, including CSR, RHS, and property assertion.
    #[must_use]
    pub fn matches_complete(&self, complete: &CanonicalCsrSystemView) -> bool {
        self.system_identity == complete.agreement_fingerprint()
    }

    /// Borrow one rank-local problem in that layout's explicit owned order.
    ///
    /// # Errors
    /// Returns `EQ0802` for an unknown partition or an internal projection
    /// contradiction.
    pub fn local_problem(
        &self,
        partition: PartitionId,
    ) -> Result<DistributedLinearProblem<'_>, Diagnostic> {
        let right_hand_side = self
            .local_right_hand_sides
            .get(partition.index())
            .ok_or_else(|| solve_failed("distributed system names an unknown partition"))?;
        DistributedLinearProblem::new(&self.operator, partition, right_hand_side, self.properties)
    }

    /// Validate the distributed numerical policy and derive the exact
    /// fixed-size collective admission fingerprint.
    ///
    /// # Errors
    /// Returns `EQ0807` unless the algorithm accepts the asserted operator,
    /// the distributed contract implements the requested preconditioner, and
    /// an available positive Jacobi diagonal exists when requested.
    pub fn admission_fingerprint(
        &self,
        plan: SolverPlan,
    ) -> Result<DistributedAdmissionFingerprintV1, Diagnostic> {
        self.validate_plan(plan)?;
        distributed_admission_fingerprint(
            self.system_identity,
            self.partition_identity,
            self.layout_identity,
            plan,
        )
    }

    fn validate_plan(&self, plan: SolverPlan) -> Result<(), Diagnostic> {
        if !plan.algorithm().accepts(self.properties) {
            return Err(invalid_realization(
                "distributed solver algorithm does not accept the asserted operator properties",
            ));
        }
        match plan.algorithm() {
            LinearSolver::ConjugateGradient => {}
            LinearSolver::MinimumResidual
                if plan.preconditioner() == PreconditionerPolicy::Identity => {}
            LinearSolver::MinimumResidual => {
                return Err(invalid_realization(
                    "distributed MINRES currently admits identity preconditioning only",
                ));
            }
            LinearSolver::BiConjugateGradientStabilized => {
                return Err(invalid_realization(
                    "distributed BiCGSTAB is not implemented",
                ));
            }
        }
        if plan.preconditioner() == PreconditionerPolicy::Jacobi {
            for partition in 0..self.operator.partition.count.get() {
                let shard = self
                    .operator
                    .shard(PartitionId(partition))
                    .ok_or_else(|| invalid_realization("validated partition has no CSR shard"))?;
                let mut diagonal =
                    realization_vector(shard.layout().owned.len(), "Jacobi admission buffer")?;
                diagonal.resize(shard.layout().owned.len(), 0.0);
                if shard.diagonal(&mut diagonal)? != DiagonalAvailability::Available
                    || diagonal
                        .iter()
                        .any(|value| *value <= 0.0 || !value.is_finite())
                {
                    return Err(invalid_realization(format!(
                        "partition {partition} lacks a finite positive Jacobi diagonal"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// One rank-local view of a validated distributed linear problem.
#[derive(Debug)]
pub struct DistributedLinearProblem<'a> {
    operator: &'a DistributedCsr,
    partition: PartitionId,
    right_hand_side: &'a [f64],
    initial_guess: Option<&'a [f64]>,
    properties: LinearOperatorProperties,
}

impl<'a> DistributedLinearProblem<'a> {
    /// Bind local vectors to one immutable distributed operator artifact.
    ///
    /// # Errors
    /// Returns `EQ0802` for an unknown partition, local shape mismatch, or
    /// non-finite right-hand-side value.
    pub fn new(
        operator: &'a DistributedCsr,
        partition: PartitionId,
        right_hand_side: &'a [f64],
        properties: LinearOperatorProperties,
    ) -> Result<Self, Diagnostic> {
        let layout = operator
            .layouts
            .get(partition.index())
            .ok_or_else(|| solve_failed("distributed problem names an unknown partition"))?;
        if right_hand_side.len() != layout.owned.len()
            || right_hand_side.iter().any(|value| !value.is_finite())
        {
            return Err(solve_failed(format!(
                "partition {} right-hand side must contain {} finite owned values",
                partition.index(),
                layout.owned.len()
            )));
        }
        Ok(Self {
            operator,
            partition,
            right_hand_side,
            initial_guess: None,
            properties,
        })
    }

    /// Attach a rank-local initial guess in owned-index order.
    ///
    /// # Errors
    /// Returns `EQ0802` for a local shape mismatch or non-finite value.
    pub fn with_initial_guess(mut self, initial_guess: &'a [f64]) -> Result<Self, Diagnostic> {
        if initial_guess.len() != self.right_hand_side.len()
            || initial_guess.iter().any(|value| !value.is_finite())
        {
            return Err(solve_failed(
                "distributed initial guess must match the local right-hand side and be finite",
            ));
        }
        self.initial_guess = Some(initial_guess);
        Ok(self)
    }

    /// Immutable distributed operator artifact.
    #[must_use]
    pub const fn operator(&self) -> &'a DistributedCsr {
        self.operator
    }

    /// Rank-local partition identity.
    #[must_use]
    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    /// Rank-local right-hand side in owned-index order.
    #[must_use]
    pub const fn right_hand_side(&self) -> &'a [f64] {
        self.right_hand_side
    }

    /// Explicit rank-local initial guess, or `None` for zero.
    #[must_use]
    pub const fn initial_guess(&self) -> Option<&'a [f64]> {
        self.initial_guess
    }

    /// Asserted global operator properties.
    #[must_use]
    pub const fn properties(&self) -> LinearOperatorProperties {
        self.properties
    }
}

/// Rank-local values paired with globally accepted solve evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalLinearSolution {
    partition: PartitionId,
    values: Vec<f64>,
    report: SolveReport,
}

impl LocalLinearSolution {
    /// Pair finite owned values with an accepted global solve report.
    ///
    /// # Errors
    /// Returns `EQ0802` when values contradict the problem's local layout.
    pub fn new(
        problem: &DistributedLinearProblem<'_>,
        values: Vec<f64>,
        report: SolveReport,
    ) -> Result<Self, Diagnostic> {
        if values.len() != problem.right_hand_side.len()
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(solve_failed(
                "distributed solution must contain one finite value per owned row",
            ));
        }
        Ok(Self {
            partition: problem.partition,
            values,
            report,
        })
    }

    /// Rank-local partition identity.
    #[must_use]
    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    /// Accepted values in owned-index order.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Global convergence and execution evidence.
    #[must_use]
    pub const fn report(&self) -> &SolveReport {
        &self.report
    }
}

#[derive(Debug)]
struct LocalValues {
    owned: Vec<f64>,
    ghosts: Vec<Option<f64>>,
}

impl LocalValues {
    fn value(&self, layout: &LocalLayout, global: usize) -> Option<f64> {
        match layout.owned.binary_search(&global) {
            Ok(local) => self.owned.get(local).copied(),
            Err(_) => layout
                .ghosts
                .binary_search(&global)
                .ok()
                .and_then(|local| self.ghosts.get(local).copied().flatten()),
        }
    }
}

/// Single-process protocol oracle for partition, halo, and owned-row action.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoopbackExecutor;

impl LoopbackExecutor {
    /// Compute a distributed dot product from unique-owner contributions.
    ///
    /// Each partition accumulates its owned entries in ascending global-index
    /// order. The loopback collective then combines partials in partition
    /// order. V0 admits only the reproducible policy; `Fast` requires a native
    /// transport collective rather than a misleading loopback implementation.
    ///
    /// # Errors
    /// Returns `EQ0807` for a policy unsupported by this executor, or `EQ0802`
    /// for shape/non-finite inputs or reduction overflow.
    pub fn dot(
        &self,
        partition: &Partition,
        left: &[f64],
        right: &[f64],
        policy: ReductionPolicy,
    ) -> Result<f64, Diagnostic> {
        if policy != ReductionPolicy::Reproducible {
            return Err(invalid_realization(
                "the loopback collective admits only reproducible reductions",
            ));
        }
        let dimension = partition.space.dimension.get();
        if left.len() != dimension
            || right.len() != dimension
            || left.iter().chain(right).any(|value| !value.is_finite())
        {
            return Err(solve_failed(format!(
                "distributed dot operands must each contain {dimension} finite values"
            )));
        }
        let mut partials = solve_vector(partition.count.get(), "loopback dot partials")?;
        partials.resize(partition.count.get(), 0.0);
        for global in 0..dimension {
            let owner = partition.owners[global].index();
            partials[owner] = finite_sum(partials[owner], left[global] * right[global])?;
        }
        partials
            .into_iter()
            .try_fold(0.0, finite_sum)
            .map_err(|_| solve_failed("distributed dot reduction overflowed"))
    }

    /// Apply a distributed CSR plan by splitting owned input, executing the
    /// exact halo plan, applying owned rows, and gathering owned output.
    ///
    /// # Errors
    /// Returns `EQ0802` for shape/non-finite input, a missing halo value, or a
    /// non-finite local action.
    pub fn apply(&self, operator: &DistributedCsr, input: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        let dimension = operator.partition.space.dimension.get();
        if input.len() != dimension || input.iter().any(|value| !value.is_finite()) {
            return Err(solve_failed(format!(
                "distributed input must contain {dimension} finite values"
            )));
        }
        let mut locals = solve_vector(operator.layouts.len(), "loopback local-vector table")?;
        for layout in &operator.layouts {
            let mut owned = solve_vector(layout.owned.len(), "loopback owned values")?;
            for &global in &layout.owned {
                owned.push(input[global]);
            }
            let mut ghosts = solve_vector(layout.ghosts.len(), "loopback ghost values")?;
            ghosts.resize(layout.ghosts.len(), None);
            locals.push(LocalValues { owned, ghosts });
        }
        for exchange in &operator.halo.exchanges {
            for &global in &exchange.indices {
                let owner_layout = operator
                    .layouts
                    .get(exchange.owner.index())
                    .ok_or_else(|| solve_failed("halo plan names an unknown owner layout"))?;
                let value = locals
                    .get(exchange.owner.index())
                    .and_then(|local| local.value(owner_layout, global))
                    .ok_or_else(|| solve_failed("halo plan source is not owner-resident"))?;
                let receiver_layout = operator
                    .layouts
                    .get(exchange.receiver.index())
                    .ok_or_else(|| solve_failed("halo plan names an unknown receiver layout"))?;
                let ghost = receiver_layout
                    .ghosts
                    .binary_search(&global)
                    .map_err(|_| solve_failed("halo plan target is not a declared ghost"))?;
                let receiver = locals
                    .get_mut(exchange.receiver.index())
                    .ok_or_else(|| solve_failed("halo receiver has no local vector"))?;
                receiver.ghosts[ghost] = Some(value);
            }
        }

        let mut output = solve_vector(dimension, "loopback gathered output")?;
        output.resize(dimension, 0.0);
        for ((layout, shard), local) in operator.layouts.iter().zip(&operator.shards).zip(&locals) {
            if shard.partition != layout.partition {
                return Err(solve_failed(
                    "loopback CSR shard contradicts its canonical local layout",
                ));
            }
            for (local_row, &global_row) in layout.owned.iter().enumerate() {
                let mut value = 0.0;
                for entry in shard.row_offsets[local_row]..shard.row_offsets[local_row + 1] {
                    let global_column = shard.column_indices[entry];
                    let input_value = local.value(layout, global_column).ok_or_else(|| {
                        solve_failed(format!(
                            "partition {} lacks required owned/ghost value {global_column}",
                            layout.partition.0
                        ))
                    })?;
                    value += shard.values[entry] * input_value;
                }
                if !value.is_finite() {
                    return Err(solve_failed(format!(
                        "distributed row {global_row} produced a non-finite value"
                    )));
                }
                output[global_row] = value;
            }
        }
        Ok(output)
    }
}

fn validate_csr(
    dimension: usize,
    row_offsets: &[usize],
    column_indices: &[usize],
    values: &[f64],
) -> Result<(), Diagnostic> {
    if dimension
        .checked_add(1)
        .is_none_or(|expected| row_offsets.len() != expected)
        || row_offsets.first() != Some(&0)
        || row_offsets.last() != Some(&column_indices.len())
        || column_indices.len() != values.len()
        || row_offsets
            .windows(2)
            .any(|offsets| offsets[0] > offsets[1])
    {
        return Err(invalid_realization(
            "global CSR offsets, columns, and values have inconsistent shape",
        ));
    }
    if column_indices.iter().any(|column| *column >= dimension)
        || values.iter().any(|value| !value.is_finite())
    {
        return Err(invalid_realization(
            "global CSR contains an out-of-range column or non-finite value",
        ));
    }
    for row in 0..dimension {
        let columns = &column_indices[row_offsets[row]..row_offsets[row + 1]];
        if columns.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(invalid_realization(
                "global CSR columns must be strictly ordered within every row",
            ));
        }
    }
    Ok(())
}

fn partition_agreement_identity(
    partition: &Partition,
) -> Result<PartitionAgreementIdentityV1, Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(PARTITION_AGREEMENT_DOMAIN_V1);
    hash.update([scalar_tag(partition.space.scalar_type)]);
    update_count(
        &mut hash,
        partition.space.dimension.get(),
        "global dimension",
    )?;
    update_count(&mut hash, partition.count.get(), "partition count")?;
    update_count(&mut hash, partition.owners.len(), "owner count")?;
    for owner in &partition.owners {
        update_count(&mut hash, owner.index(), "owner partition")?;
    }
    Ok(PartitionAgreementIdentityV1(hash.finalize().into()))
}

fn distributed_layout_agreement_identity(
    system: CanonicalCsrAgreementFingerprintV1,
    partition: PartitionAgreementIdentityV1,
    operator: &DistributedCsr,
) -> Result<DistributedLayoutAgreementIdentityV1, Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(DISTRIBUTED_LAYOUT_AGREEMENT_DOMAIN_V1);
    hash.update(system.as_bytes());
    hash.update(partition.as_bytes());
    update_count(&mut hash, operator.layouts.len(), "local-layout count")?;
    for layout in &operator.layouts {
        update_count(&mut hash, layout.partition.index(), "layout partition")?;
        update_indices(&mut hash, &layout.owned, "owned index")?;
        update_indices(&mut hash, &layout.ghosts, "ghost index")?;
    }
    update_count(
        &mut hash,
        operator.halo.exchanges.len(),
        "halo-exchange count",
    )?;
    for exchange in &operator.halo.exchanges {
        update_count(&mut hash, exchange.owner.index(), "halo owner")?;
        update_count(&mut hash, exchange.receiver.index(), "halo receiver")?;
        update_indices(&mut hash, &exchange.indices, "halo index")?;
    }
    Ok(DistributedLayoutAgreementIdentityV1(hash.finalize().into()))
}

fn distributed_admission_fingerprint(
    system: CanonicalCsrAgreementFingerprintV1,
    partition: PartitionAgreementIdentityV1,
    layout: DistributedLayoutAgreementIdentityV1,
    plan: SolverPlan,
) -> Result<DistributedAdmissionFingerprintV1, Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(DISTRIBUTED_ADMISSION_DOMAIN_V1);
    hash.update(system.as_bytes());
    hash.update(partition.as_bytes());
    hash.update(layout.as_bytes());
    hash.update([match plan.algorithm() {
        LinearSolver::ConjugateGradient => 0,
        LinearSolver::BiConjugateGradientStabilized => 1,
        LinearSolver::MinimumResidual => 2,
    }]);
    hash.update([match plan.preconditioner() {
        PreconditionerPolicy::Identity => 0,
        PreconditionerPolicy::Jacobi => 1,
    }]);
    hash.update([match plan.reduction() {
        ReductionPolicy::Reproducible => 0,
        ReductionPolicy::Fast => 1,
    }]);
    hash.update(plan.relative_tolerance().to_bits().to_be_bytes());
    hash.update(plan.absolute_tolerance().to_bits().to_be_bytes());
    update_count(
        &mut hash,
        plan.maximum_iterations().get(),
        "maximum iteration count",
    )?;
    Ok(DistributedAdmissionFingerprintV1(hash.finalize().into()))
}

fn scalar_tag(scalar_type: ScalarType) -> u8 {
    match scalar_type {
        ScalarType::F32 => 0,
        ScalarType::F64 => 1,
    }
}

fn update_indices(
    hash: &mut Sha256,
    indices: &[usize],
    name: &'static str,
) -> Result<(), Diagnostic> {
    update_count(hash, indices.len(), name)?;
    for &index in indices {
        update_count(hash, index, name)?;
    }
    Ok(())
}

fn update_count(hash: &mut Sha256, value: usize, name: &'static str) -> Result<(), Diagnostic> {
    let value = u64::try_from(value)
        .map_err(|_| invalid_realization(format!("distributed {name} exceeds portable u64")))?;
    hash.update(value.to_be_bytes());
    Ok(())
}

fn finite_sum(sum: f64, value: f64) -> Result<f64, Diagnostic> {
    let next = sum + value;
    next.is_finite()
        .then_some(next)
        .ok_or_else(|| solve_failed("distributed reduction overflowed"))
}

fn same_f64_bits(left: &[f64], right: &[f64]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.to_bits() == right.to_bits())
}

fn checked_count_add(
    count: usize,
    increment: usize,
    purpose: &'static str,
) -> Result<usize, Diagnostic> {
    count
        .checked_add(increment)
        .ok_or_else(|| invalid_realization(format!("{purpose} overflowed usize")))
}

fn realization_vector<T>(capacity: usize, purpose: &'static str) -> Result<Vec<T>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| invalid_realization(format!("could not reserve {purpose}")))?;
    Ok(values)
}

fn realization_copy<T: Copy>(source: &[T], purpose: &'static str) -> Result<Vec<T>, Diagnostic> {
    let mut values = realization_vector(source.len(), purpose)?;
    // `realization_vector` reserves the complete extent first, so this copy
    // cannot trigger a hidden capacity growth after admission begins.
    values.extend_from_slice(source);
    Ok(values)
}

fn solve_vector<T>(capacity: usize, purpose: &'static str) -> Result<Vec<T>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| solve_failed(format!("could not reserve {purpose}")))?;
    Ok(values)
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn solve_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
}

#[cfg(test)]
mod allocation_tests {
    use super::*;

    #[test]
    fn impossible_counts_and_capacities_fail_as_diagnostics() {
        assert!(checked_count_add(usize::MAX, 1, "test count").is_err());
        assert!(realization_vector::<u8>(usize::MAX, "test realization").is_err());
        assert!(solve_vector::<u8>(usize::MAX, "test solve").is_err());
    }

    #[test]
    fn execution_capture_rejects_noncanonical_rectangular_shape() {
        let layout = LocalLayout {
            partition: PartitionId::new(0),
            owned: vec![0, 1],
            ghosts: vec![2],
        };
        let storage = LocalCsrStorage {
            partition: PartitionId::new(0),
            row_offsets: vec![0, 1],
            column_indices: vec![0],
            values: vec![1.0],
        };

        assert!(
            LocalCsrShard {
                layout: &layout,
                storage: &storage,
            }
            .capture_execution()
            .is_err()
        );
    }

    #[test]
    fn execution_capture_rejects_column_outside_owned_and_ghost_maps() {
        let layout = LocalLayout {
            partition: PartitionId::new(0),
            owned: vec![1],
            ghosts: vec![0],
        };
        let storage = LocalCsrStorage {
            partition: PartitionId::new(0),
            row_offsets: vec![0, 1],
            column_indices: vec![2],
            values: vec![1.0],
        };

        assert!(
            LocalCsrShard {
                layout: &layout,
                storage: &storage,
            }
            .capture_execution()
            .is_err()
        );
    }
}
