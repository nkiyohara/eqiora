use eqiora_core::Diagnostic;

use crate::allocation::{checked_count_add, realization_vector};
use crate::error::invalid_realization;
use crate::partition::{Partition, PartitionId};

/// Owned and ordered ghost indices visible to one partition.
#[derive(Debug, PartialEq, Eq)]
pub struct LocalLayout {
    partition: PartitionId,
    owned: Vec<usize>,
    ghosts: Vec<usize>,
}

impl LocalLayout {
    pub(crate) fn reserved(
        partition: &Partition,
        id: PartitionId,
        owned_count: usize,
        ghost_candidate_count: usize,
    ) -> Result<Self, Diagnostic> {
        if id.index() >= partition.count().get() {
            return Err(invalid_realization(format!(
                "local layout partition {} is outside 0..{}",
                id.index(),
                partition.count()
            )));
        }
        Ok(Self {
            partition: id,
            owned: realization_vector(owned_count, "local owned-index map")?,
            ghosts: realization_vector(ghost_candidate_count, "local ghost candidates")?,
        })
    }

    pub(crate) fn canonicalize(
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

    pub(crate) fn push_owned(&mut self, global: usize) {
        self.owned.push(global);
    }

    pub(crate) fn push_ghost_candidate(&mut self, global: usize) {
        self.ghosts.push(global);
    }

    pub(crate) fn extend_owned(&mut self, globals: &[usize]) {
        self.owned.extend_from_slice(globals);
    }

    pub(crate) fn extend_ghost_candidates(&mut self, globals: impl IntoIterator<Item = usize>) {
        self.ghosts.extend(globals);
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        partition: PartitionId,
        owned: Vec<usize>,
        ghosts: Vec<usize>,
    ) -> Self {
        Self {
            partition,
            owned,
            ghosts,
        }
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
    pub(crate) fn derive(
        partition: &Partition,
        layouts: &[LocalLayout],
    ) -> Result<Self, Diagnostic> {
        if layouts.len() != partition.count().get()
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
