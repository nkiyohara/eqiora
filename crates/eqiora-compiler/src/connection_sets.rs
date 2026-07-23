//! Bounded normalization of undirected connection fragments.
//!
//! Source `connect` declarations generate an equivalence relation; they are
//! not themselves canonical graph Connections. This module computes the
//! maximal sets without knowing anything about source syntax, hierarchy,
//! packages, physical families, or kernel IDs. Callers type-check fragments
//! before admission and retain provenance outside this topology contract.

use core::fmt;

/// Independent resource limits for one connection-set normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectionSetLimits {
    /// Maximum admitted source fragments.
    pub max_fragments: usize,
    /// Maximum memberships summed across all fragments.
    pub max_memberships: usize,
    /// Maximum distinct endpoint identities.
    pub max_endpoints: usize,
    /// Maximum normalized maximal sets.
    pub max_sets: usize,
    /// Maximum members admitted by one source fragment.
    pub max_members_per_fragment: usize,
    /// Maximum members in one normalized maximal set.
    pub max_members_per_set: usize,
}

impl Default for ConnectionSetLimits {
    fn default() -> Self {
        Self {
            max_fragments: 1_000_000,
            max_memberships: 4_000_000,
            max_endpoints: 4_000_000,
            max_sets: 1_000_000,
            max_members_per_fragment: 65_536,
            max_members_per_set: 65_536,
        }
    }
}

/// One already type-checked undirected connection fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionFragment<I> {
    members: Box<[I]>,
}

impl<I: Ord> ConnectionFragment<I> {
    /// Construct one canonical fragment.
    ///
    /// Member order is erased. Repeating a member inside one declaration is a
    /// source error rather than an idempotent second fragment.
    pub fn try_new(
        input_members: impl IntoIterator<Item = I>,
        limits: ConnectionSetLimits,
    ) -> Result<Self, ConnectionSetError> {
        let mut members = Vec::new();
        for member in input_members {
            if members.len() >= limits.max_members_per_fragment {
                return Err(ConnectionSetError::LimitExceeded {
                    resource: "members in one connection fragment",
                    observed: members.len().saturating_add(1),
                    limit: limits.max_members_per_fragment,
                });
            }
            members
                .try_reserve(1)
                .map_err(|_| ConnectionSetError::Allocation {
                    resource: "connection fragment members",
                })?;
            members.push(member);
        }
        if members.len() < 2 {
            return Err(ConnectionSetError::TooFewMembers {
                found: members.len(),
            });
        }
        members.sort_unstable();
        if members.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ConnectionSetError::DuplicateMember);
        }
        Ok(Self {
            members: members.into_boxed_slice(),
        })
    }

    pub fn members(&self) -> &[I] {
        &self.members
    }
}

/// One maximal normalized connection set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalConnectionSet<I> {
    members: Box<[I]>,
}

impl<I> CanonicalConnectionSet<I> {
    /// Exact members in ascending identity order.
    pub fn members(&self) -> &[I] {
        &self.members
    }
}

/// A canonical partition plus a non-semantic witness for each input fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionSetNormalization<I> {
    sets: Box<[CanonicalConnectionSet<I>]>,
    fragment_sets: Box<[usize]>,
}

impl<I> ConnectionSetNormalization<I> {
    /// Maximal sets ordered lexicographically by their exact member sequences.
    pub fn sets(&self) -> &[CanonicalConnectionSet<I>] {
        &self.sets
    }

    /// For each input fragment, the index of its maximal set.
    ///
    /// This array follows input order and is only a caller-side provenance
    /// witness. It must never enter semantic identity.
    pub fn fragment_sets(&self) -> &[usize] {
        &self.fragment_sets
    }
}

/// Failure before any canonical graph mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionSetError {
    /// A source fragment did not contain two distinct endpoints.
    TooFewMembers { found: usize },
    /// One source fragment repeated an endpoint.
    DuplicateMember,
    /// One independently bounded resource exceeded its policy.
    LimitExceeded {
        resource: &'static str,
        observed: usize,
        limit: usize,
    },
    /// A resource count overflowed `usize`.
    CountOverflow { resource: &'static str },
    /// A fallible reservation failed.
    Allocation { resource: &'static str },
}

impl fmt::Display for ConnectionSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewMembers { found } => write!(
                formatter,
                "connection fragment has {found} members; at least two are required"
            ),
            Self::DuplicateMember => formatter.write_str("connection fragment repeats a member"),
            Self::LimitExceeded {
                resource,
                observed,
                limit,
            } => write!(
                formatter,
                "{resource} requires {observed}, exceeding the {limit} limit"
            ),
            Self::CountOverflow { resource } => write!(formatter, "{resource} overflows usize"),
            Self::Allocation { resource } => write!(formatter, "cannot reserve {resource}"),
        }
    }
}

/// Normalize typed fragments into pairwise-disjoint maximal sets.
///
/// The member partition is independent of fragment order, member order, and
/// the internal disjoint-set representative. The returned fragment mapping is
/// deliberately separate because it is traversal-local provenance, not
/// topology.
pub fn normalize_connection_sets<I>(
    fragments: &[ConnectionFragment<I>],
    limits: ConnectionSetLimits,
) -> Result<ConnectionSetNormalization<I>, ConnectionSetError>
where
    I: Clone + Ord,
{
    check_limit(
        "connection fragments",
        fragments.len(),
        limits.max_fragments,
    )?;

    let mut membership_count = 0_usize;
    for fragment in fragments {
        membership_count = membership_count.checked_add(fragment.members.len()).ok_or(
            ConnectionSetError::CountOverflow {
                resource: "connection fragment memberships",
            },
        )?;
    }
    check_limit(
        "connection fragment memberships",
        membership_count,
        limits.max_memberships,
    )?;
    if fragments.is_empty() {
        return Ok(ConnectionSetNormalization {
            sets: Box::new([]),
            fragment_sets: Box::new([]),
        });
    }

    let mut endpoints = Vec::new();
    endpoints
        .try_reserve_exact(membership_count)
        .map_err(|_| ConnectionSetError::Allocation {
            resource: "connection endpoint identities",
        })?;
    for fragment in fragments {
        endpoints.extend(fragment.members.iter().cloned());
    }
    endpoints.sort_unstable();
    endpoints.dedup();
    check_limit(
        "distinct connection endpoints",
        endpoints.len(),
        limits.max_endpoints,
    )?;

    let mut parents = Vec::new();
    let mut sizes = Vec::new();
    parents
        .try_reserve_exact(endpoints.len())
        .map_err(|_| ConnectionSetError::Allocation {
            resource: "connection-set parents",
        })?;
    sizes
        .try_reserve_exact(endpoints.len())
        .map_err(|_| ConnectionSetError::Allocation {
            resource: "connection-set sizes",
        })?;
    parents.extend(0..endpoints.len());
    sizes.resize(endpoints.len(), 1_usize);

    for fragment in fragments {
        let first = endpoint_index(&endpoints, &fragment.members[0]);
        for member in &fragment.members[1..] {
            union(
                &mut parents,
                &mut sizes,
                first,
                endpoint_index(&endpoints, member),
            );
        }
    }

    let mut rooted_endpoints = Vec::new();
    rooted_endpoints
        .try_reserve_exact(endpoints.len())
        .map_err(|_| ConnectionSetError::Allocation {
            resource: "normalized endpoint memberships",
        })?;
    for endpoint in 0..endpoints.len() {
        rooted_endpoints.push((find(&mut parents, endpoint), endpoint));
    }
    rooted_endpoints.sort_unstable();

    let mut working_sets = Vec::new();
    working_sets
        .try_reserve_exact(rooted_endpoints.len())
        .map_err(|_| ConnectionSetError::Allocation {
            resource: "normalized connection sets",
        })?;
    let mut start = 0_usize;
    while start < rooted_endpoints.len() {
        let root = rooted_endpoints[start].0;
        let mut end = start + 1;
        while end < rooted_endpoints.len() && rooted_endpoints[end].0 == root {
            end += 1;
        }
        check_limit(
            "members in one normalized connection set",
            end - start,
            limits.max_members_per_set,
        )?;
        let mut members = Vec::new();
        members
            .try_reserve_exact(end - start)
            .map_err(|_| ConnectionSetError::Allocation {
                resource: "canonical connection-set members",
            })?;
        for &(_, endpoint) in &rooted_endpoints[start..end] {
            members.push(endpoints[endpoint].clone());
        }
        working_sets.push(WorkingSet {
            root,
            members: members.into_boxed_slice(),
        });
        start = end;
    }
    check_limit(
        "normalized connection sets",
        working_sets.len(),
        limits.max_sets,
    )?;
    working_sets.sort_unstable_by(|left, right| left.members.cmp(&right.members));

    let mut root_to_set = Vec::new();
    root_to_set
        .try_reserve_exact(working_sets.len())
        .map_err(|_| ConnectionSetError::Allocation {
            resource: "connection root-to-set witness",
        })?;
    let mut sets = Vec::new();
    sets.try_reserve_exact(working_sets.len())
        .map_err(|_| ConnectionSetError::Allocation {
            resource: "canonical connection sets",
        })?;
    for (set, working) in working_sets.into_iter().enumerate() {
        root_to_set.push((working.root, set));
        sets.push(CanonicalConnectionSet {
            members: working.members,
        });
    }
    root_to_set.sort_unstable_by_key(|(root, _)| *root);

    let mut fragment_sets = Vec::new();
    fragment_sets
        .try_reserve_exact(fragments.len())
        .map_err(|_| ConnectionSetError::Allocation {
            resource: "fragment-to-set witness",
        })?;
    for fragment in fragments {
        let endpoint = endpoint_index(&endpoints, &fragment.members[0]);
        let root = find(&mut parents, endpoint);
        let root_index = root_to_set
            .binary_search_by_key(&root, |(candidate, _)| *candidate)
            .expect("every disjoint-set root has one canonical set");
        fragment_sets.push(root_to_set[root_index].1);
    }

    Ok(ConnectionSetNormalization {
        sets: sets.into_boxed_slice(),
        fragment_sets: fragment_sets.into_boxed_slice(),
    })
}

#[derive(Debug)]
struct WorkingSet<I> {
    root: usize,
    members: Box<[I]>,
}

fn check_limit(
    resource: &'static str,
    observed: usize,
    limit: usize,
) -> Result<(), ConnectionSetError> {
    if observed > limit {
        Err(ConnectionSetError::LimitExceeded {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

fn endpoint_index<I: Ord>(endpoints: &[I], endpoint: &I) -> usize {
    endpoints
        .binary_search(endpoint)
        .expect("every fragment member was collected before normalization")
}

fn find(parents: &mut [usize], mut node: usize) -> usize {
    let mut root = node;
    while parents[root] != root {
        root = parents[root];
    }
    while parents[node] != node {
        let next = parents[node];
        parents[node] = root;
        node = next;
    }
    root
}

fn union(parents: &mut [usize], sizes: &mut [usize], left: usize, right: usize) {
    let mut left_root = find(parents, left);
    let mut right_root = find(parents, right);
    if left_root == right_root {
        return;
    }
    if sizes[left_root] < sizes[right_root]
        || (sizes[left_root] == sizes[right_root] && left_root > right_root)
    {
        core::mem::swap(&mut left_root, &mut right_root);
    }
    parents[right_root] = left_root;
    sizes[left_root] = sizes[left_root]
        .checked_add(sizes[right_root])
        .expect("set sizes cannot exceed the pre-counted endpoint count");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fragment(members: &[u32]) -> ConnectionFragment<u32> {
        ConnectionFragment::try_new(members.iter().copied(), ConnectionSetLimits::default())
            .expect("test fragment is valid")
    }

    fn topology(normalized: &ConnectionSetNormalization<u32>) -> Vec<Vec<u32>> {
        normalized
            .sets()
            .iter()
            .map(|set| set.members().to_vec())
            .collect()
    }

    #[test]
    fn nary_and_partitioned_fragments_have_one_topology() {
        let limits = ConnectionSetLimits::default();
        let nary = normalize_connection_sets(&[fragment(&[3, 1, 2])], limits).unwrap();
        let partitioned =
            normalize_connection_sets(&[fragment(&[1, 2]), fragment(&[3, 2])], limits).unwrap();
        assert_eq!(topology(&nary), vec![vec![1, 2, 3]]);
        assert_eq!(topology(&nary), topology(&partitioned));
        assert_eq!(partitioned.fragment_sets(), &[0, 0]);
    }

    #[test]
    fn fragment_and_member_permutations_preserve_topology() {
        let limits = ConnectionSetLimits::default();
        let first = normalize_connection_sets(
            &[fragment(&[9, 7]), fragment(&[2, 1]), fragment(&[8, 9])],
            limits,
        )
        .unwrap();
        let second = normalize_connection_sets(
            &[fragment(&[9, 8]), fragment(&[1, 2]), fragment(&[7, 9])],
            limits,
        )
        .unwrap();
        assert_eq!(topology(&first), topology(&second));
        assert_eq!(topology(&first), vec![vec![1, 2], vec![7, 8, 9]]);
        assert_eq!(first.fragment_sets(), &[1, 0, 1]);
        assert_eq!(second.fragment_sets(), &[1, 0, 1]);
    }

    #[test]
    fn duplicate_fragments_are_topology_idempotent_and_remain_mapped() {
        let normalized = normalize_connection_sets(
            &[fragment(&[1, 2]), fragment(&[2, 1]), fragment(&[1, 2])],
            ConnectionSetLimits::default(),
        )
        .unwrap();
        assert_eq!(topology(&normalized), vec![vec![1, 2]]);
        assert_eq!(normalized.fragment_sets(), &[0, 0, 0]);
    }

    #[test]
    fn empty_input_and_long_iterative_chain_are_supported() {
        let limits = ConnectionSetLimits::default();
        let empty = normalize_connection_sets::<u32>(&[], limits).unwrap();
        assert!(empty.sets().is_empty());
        assert!(empty.fragment_sets().is_empty());

        let fragments = (0..10_000_u32)
            .map(|member| fragment(&[member, member + 1]))
            .collect::<Vec<_>>();
        let normalized = normalize_connection_sets(&fragments, limits).unwrap();
        assert_eq!(normalized.sets().len(), 1);
        assert_eq!(normalized.sets()[0].members().len(), 10_001);
    }

    #[test]
    fn invalid_fragments_fail_before_normalization() {
        let limits = ConnectionSetLimits::default();
        assert_eq!(
            ConnectionFragment::try_new([1_u32], limits),
            Err(ConnectionSetError::TooFewMembers { found: 1 })
        );
        assert_eq!(
            ConnectionFragment::try_new([1_u32, 1], limits),
            Err(ConnectionSetError::DuplicateMember)
        );
        let one_member_limit = ConnectionSetLimits {
            max_members_per_fragment: 1,
            ..limits
        };
        assert!(matches!(
            ConnectionFragment::try_new([1_u32, 2], one_member_limit),
            Err(ConnectionSetError::LimitExceeded {
                resource: "members in one connection fragment",
                observed: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn every_normalization_resource_has_an_independent_limit() {
        let fragments = [fragment(&[1, 2]), fragment(&[2, 3])];
        let default = ConnectionSetLimits::default();
        let cases = [
            (
                ConnectionSetLimits {
                    max_fragments: 1,
                    ..default
                },
                "connection fragments",
            ),
            (
                ConnectionSetLimits {
                    max_memberships: 3,
                    ..default
                },
                "connection fragment memberships",
            ),
            (
                ConnectionSetLimits {
                    max_endpoints: 2,
                    ..default
                },
                "distinct connection endpoints",
            ),
            (
                ConnectionSetLimits {
                    max_sets: 0,
                    ..default
                },
                "normalized connection sets",
            ),
            (
                ConnectionSetLimits {
                    max_members_per_set: 2,
                    ..default
                },
                "members in one normalized connection set",
            ),
        ];
        for (limits, expected_resource) in cases {
            assert!(matches!(
                normalize_connection_sets(&fragments, limits),
                Err(ConnectionSetError::LimitExceeded { resource, .. })
                    if resource == expected_resource
            ));
        }

        let disjoint = [fragment(&[1, 2]), fragment(&[3, 4])];
        let limits = ConnectionSetLimits {
            max_sets: 1,
            ..default
        };
        assert!(matches!(
            normalize_connection_sets(&disjoint, limits),
            Err(ConnectionSetError::LimitExceeded {
                resource: "normalized connection sets",
                ..
            })
        ));
    }
}
