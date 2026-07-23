//! Interior cuts for transparent physical exposure occurrences.
//!
//! A cut follows original connection fragments from one exposure, but only
//! fragments declared at or below that exposure's component occurrence. It
//! therefore crosses nested forwarding boundaries while never crossing the
//! parent fragment that connects the exposure to the outside.

use std::collections::VecDeque;

use eqiora_core::Diagnostic;

use crate::identity::{FullElaborationIdentity, InstancePath};

use super::hierarchy_error;
use super::occurrence_connections::OccurrenceConnectionFragment;

pub(super) struct ExposureCutIndex {
    endpoints: Box<[FullElaborationIdentity]>,
    fragment_memberships: Box<[(FullElaborationIdentity, usize)]>,
    endpoint_generation: Box<[usize]>,
    fragment_generation: Box<[usize]>,
    queue: VecDeque<FullElaborationIdentity>,
    generation: usize,
    traversal_memberships: usize,
}

impl ExposureCutIndex {
    pub(super) fn new(
        endpoints: impl IntoIterator<Item = FullElaborationIdentity>,
        endpoint_count: usize,
        fragments: &[OccurrenceConnectionFragment],
    ) -> Result<Self, Diagnostic> {
        let mut endpoint_order = Vec::new();
        endpoint_order
            .try_reserve_exact(endpoint_count)
            .map_err(|_| hierarchy_error("cannot reserve physical cut endpoints"))?;
        endpoint_order.extend(endpoints);
        endpoint_order.sort_unstable();
        if endpoint_order.len() != endpoint_count
            || endpoint_order.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(hierarchy_error(
                "physical cut endpoint inventory is not exact and unique",
            ));
        }

        let membership_count = fragments.iter().try_fold(0_usize, |count, fragment| {
            count.checked_add(fragment.topology().members().len())
        });
        let membership_count = membership_count
            .ok_or_else(|| hierarchy_error("physical cut fragment membership count overflows"))?;
        let mut fragment_memberships = Vec::new();
        fragment_memberships
            .try_reserve_exact(membership_count)
            .map_err(|_| hierarchy_error("cannot reserve physical cut fragment index"))?;
        for (fragment_index, fragment) in fragments.iter().enumerate() {
            fragment_memberships.extend(
                fragment
                    .topology()
                    .members()
                    .iter()
                    .map(|identity| (*identity, fragment_index)),
            );
        }
        fragment_memberships.sort_unstable();

        let mut endpoint_generation = Vec::new();
        endpoint_generation
            .try_reserve_exact(endpoint_order.len())
            .map_err(|_| hierarchy_error("cannot reserve physical cut endpoint marks"))?;
        endpoint_generation.resize(endpoint_order.len(), 0);
        let mut fragment_generation = Vec::new();
        fragment_generation
            .try_reserve_exact(fragments.len())
            .map_err(|_| hierarchy_error("cannot reserve physical cut fragment marks"))?;
        fragment_generation.resize(fragments.len(), 0);
        let mut queue = VecDeque::new();
        queue
            .try_reserve_exact(endpoint_order.len())
            .map_err(|_| hierarchy_error("cannot reserve physical cut traversal queue"))?;
        Ok(Self {
            endpoints: endpoint_order.into_boxed_slice(),
            fragment_memberships: fragment_memberships.into_boxed_slice(),
            endpoint_generation: endpoint_generation.into_boxed_slice(),
            fragment_generation: fragment_generation.into_boxed_slice(),
            queue,
            generation: 0,
            traversal_memberships: 0,
        })
    }

    pub(super) fn derive(
        &mut self,
        exposure: FullElaborationIdentity,
        occurrence_path: &InstancePath,
        retained_members: &[FullElaborationIdentity],
        fragments: &[OccurrenceConnectionFragment],
        max_traversal_memberships: usize,
    ) -> Result<Vec<FullElaborationIdentity>, Diagnostic> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| hierarchy_error("physical cut traversal generation overflows"))?;
        self.queue.clear();
        let exposure_index = self.endpoint_index(exposure)?;
        self.endpoint_generation[exposure_index] = self.generation;
        self.queue.push_back(exposure);

        while let Some(endpoint) = self.queue.pop_front() {
            let (start, end) = self.fragment_membership_range(endpoint);
            for membership_index in start..end {
                self.charge_traversal(1, max_traversal_memberships)?;
                let fragment_index = self.fragment_memberships[membership_index].1;
                if self.fragment_generation[fragment_index] == self.generation {
                    continue;
                }
                let fragment = &fragments[fragment_index];
                if !fragment
                    .declaration_instance_path()
                    .segments()
                    .starts_with(occurrence_path.segments())
                {
                    continue;
                }
                self.fragment_generation[fragment_index] = self.generation;
                self.charge_traversal(
                    fragment.topology().members().len(),
                    max_traversal_memberships,
                )?;
                for member in fragment.topology().members() {
                    let endpoint_index = self.endpoint_index(*member)?;
                    if self.endpoint_generation[endpoint_index] != self.generation {
                        self.endpoint_generation[endpoint_index] = self.generation;
                        self.queue.push_back(*member);
                    }
                }
            }
        }

        let mut interior = Vec::new();
        interior
            .try_reserve_exact(retained_members.len())
            .map_err(|_| hierarchy_error("cannot reserve physical exposure cut"))?;
        for member in retained_members {
            let endpoint_index = self.endpoint_index(*member)?;
            if self.endpoint_generation[endpoint_index] == self.generation {
                interior.push(*member);
            }
        }
        Ok(interior)
    }

    fn charge_traversal(&mut self, memberships: usize, limit: usize) -> Result<(), Diagnostic> {
        self.traversal_memberships = self
            .traversal_memberships
            .checked_add(memberships)
            .ok_or_else(|| hierarchy_error("physical cut traversal work overflows usize"))?;
        if self.traversal_memberships > limit {
            return Err(hierarchy_error(format!(
                "physical exposure cut traversal totals {} memberships, exceeding the {limit} limit",
                self.traversal_memberships
            )));
        }
        Ok(())
    }

    fn endpoint_index(&self, identity: FullElaborationIdentity) -> Result<usize, Diagnostic> {
        self.endpoints.binary_search(&identity).map_err(|_| {
            hierarchy_error(format!(
                "physical cut references unknown endpoint {identity}"
            ))
        })
    }

    fn fragment_membership_range(&self, identity: FullElaborationIdentity) -> (usize, usize) {
        let start = self
            .fragment_memberships
            .partition_point(|(candidate, _)| *candidate < identity);
        let end = self
            .fragment_memberships
            .partition_point(|(candidate, _)| *candidate <= identity);
        (start, end)
    }
}
