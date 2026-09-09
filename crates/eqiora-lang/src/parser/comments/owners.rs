//! Source interval lookup for the parser's nested declaration owners.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use crate::ast::TextRange;

pub(in crate::parser) struct OwnerIndex<'a> {
    ranges: &'a [TextRange],
    ordered: Vec<usize>,
    pub(super) parents: Vec<usize>,
    starts: BTreeMap<u32, usize>,
    ends: BTreeMap<u32, usize>,
}

impl<'a> OwnerIndex<'a> {
    pub(in crate::parser) fn new(ranges: &'a [TextRange]) -> Self {
        let root = ranges.len() - 1;
        let mut ordered: Vec<_> = (0..root).collect();
        ordered.sort_by_key(|index| {
            (
                ranges[*index].start(),
                Reverse(ranges[*index].end()),
                *index,
            )
        });
        let mut parents = vec![root; ranges.len()];
        let mut stack = vec![root];
        for &index in &ordered {
            while stack.len() > 1 && !contains(ranges[*stack.last().unwrap()], ranges[index]) {
                stack.pop();
            }
            parents[index] = *stack.last().unwrap();
            stack.push(index);
        }
        let mut starts = BTreeMap::new();
        let mut ends = BTreeMap::new();
        for (index, range) in ranges[..root].iter().enumerate() {
            starts.entry(range.start()).or_insert(index);
            ends.entry(range.end()).or_insert(index);
        }
        Self {
            ranges,
            ordered,
            parents,
            starts,
            ends,
        }
    }

    pub(in crate::parser) fn containing(&self, range: TextRange) -> usize {
        let root = self.ranges.len() - 1;
        let before = self
            .ordered
            .partition_point(|index| self.ranges[*index].start() <= range.start());
        let mut owner = before
            .checked_sub(1)
            .map_or(root, |index| self.ordered[index]);
        while owner != root && !contains(self.ranges[owner], range) {
            owner = self.parents[owner];
        }
        owner
    }

    pub(super) fn starting_at(&self, position: u32) -> Option<usize> {
        self.starts.get(&position).copied()
    }

    pub(super) fn ending_at(&self, position: u32) -> Option<usize> {
        self.ends.get(&position).copied()
    }
}

fn contains(owner: TextRange, range: TextRange) -> bool {
    owner.start() <= range.start() && range.end() <= owner.end()
}
