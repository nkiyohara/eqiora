//! Pure occurrence-level normalization of typed physical connection fragments.
//!
//! Expansion supplies exact endpoint identities and already typed fragments.
//! This module closes their topology before any graph identity, provenance,
//! source diagnostic, or transaction is constructed.

use core::fmt;

use crate::connection_sets::{
    ConnectionFragment, ConnectionSetError, ConnectionSetLimits, normalize_connection_sets,
};
use crate::identity::{FullElaborationIdentity, InstancePath};

/// One physical endpoint in the completely expanded occurrence tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OccurrencePhysicalEndpoint {
    identity: FullElaborationIdentity,
    exposure_candidate: bool,
    owned: bool,
}

impl OccurrencePhysicalEndpoint {
    pub(super) const fn new(
        identity: FullElaborationIdentity,
        exposure_candidate: bool,
        owned: bool,
    ) -> Self {
        Self {
            identity,
            exposure_candidate,
            owned,
        }
    }
}

/// One explicit, typed fragment plus the occurrence that declared it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OccurrenceConnectionFragment {
    topology: ConnectionFragment<FullElaborationIdentity>,
    declaration_instance_path: InstancePath,
}

impl OccurrenceConnectionFragment {
    pub(super) const fn new(
        topology: ConnectionFragment<FullElaborationIdentity>,
        declaration_instance_path: InstancePath,
    ) -> Self {
        Self {
            topology,
            declaration_instance_path,
        }
    }

    pub(super) const fn topology(&self) -> &ConnectionFragment<FullElaborationIdentity> {
        &self.topology
    }

    pub(super) const fn declaration_instance_path(&self) -> &InstancePath {
        &self.declaration_instance_path
    }
}

/// Semantic topology of one final canonical physical Connection.
///
/// Input order and fragment indices are deliberately absent. The owner path
/// is the least common ancestor of contributing fragment-owner occurrences.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct OccurrenceConnectionTopology {
    retained_members: Box<[FullElaborationIdentity]>,
    eliminated_exposures: Box<[FullElaborationIdentity]>,
    owner_instance_path: InstancePath,
}

impl OccurrenceConnectionTopology {
    pub(super) fn retained_members(&self) -> &[FullElaborationIdentity] {
        &self.retained_members
    }

    pub(super) fn eliminated_exposures(&self) -> &[FullElaborationIdentity] {
        &self.eliminated_exposures
    }

    pub(super) const fn owner_instance_path(&self) -> &InstancePath {
        &self.owner_instance_path
    }
}

/// Traversal-local provenance witness for one semantic topology.
///
/// These indices refer to caller input order and must never enter a canonical
/// Connection identity or topology comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OccurrenceConnectionWitness {
    contributing_fragment_indices: Box<[usize]>,
    lca_owner_candidate_fragment_indices: Box<[usize]>,
}

impl OccurrenceConnectionWitness {
    pub(super) fn contributing_fragment_indices(&self) -> &[usize] {
        &self.contributing_fragment_indices
    }

    pub(super) fn lca_owner_candidate_fragment_indices(&self) -> &[usize] {
        &self.lca_owner_candidate_fragment_indices
    }
}

/// One canonical topology paired with its non-semantic input witness.
#[derive(Clone, Debug)]
pub(super) struct OccurrenceConnectionSet {
    topology: OccurrenceConnectionTopology,
    witness: OccurrenceConnectionWitness,
}

impl OccurrenceConnectionSet {
    pub(super) const fn topology(&self) -> &OccurrenceConnectionTopology {
        &self.topology
    }

    pub(super) const fn witness(&self) -> &OccurrenceConnectionWitness {
        &self.witness
    }
}

/// Deterministic projection from one eliminated exposure to its final set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ExposureConnectionSetWitness {
    exposure: FullElaborationIdentity,
    connection_set_index: usize,
}

impl ExposureConnectionSetWitness {
    pub(super) const fn exposure(&self) -> FullElaborationIdentity {
        self.exposure
    }

    pub(super) const fn connection_set_index(&self) -> usize {
        self.connection_set_index
    }
}

/// Complete canonical occurrence partition and exposure projection.
#[derive(Clone, Debug)]
pub(super) struct OccurrenceConnectionNormalization {
    sets: Box<[OccurrenceConnectionSet]>,
    exposure_witnesses: Box<[ExposureConnectionSetWitness]>,
}

impl OccurrenceConnectionNormalization {
    pub(super) fn sets(&self) -> &[OccurrenceConnectionSet] {
        &self.sets
    }

    pub(super) fn exposure_witnesses(&self) -> &[ExposureConnectionSetWitness] {
        &self.exposure_witnesses
    }
}

/// Failure before canonical Connection identity construction or graph mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OccurrenceConnectionError {
    DuplicateEndpoint {
        identity: FullElaborationIdentity,
    },
    UnknownEndpoint {
        fragment_index: usize,
        identity: FullElaborationIdentity,
    },
    MissingMembership {
        identity: FullElaborationIdentity,
    },
    MultipleMemberships {
        identity: FullElaborationIdentity,
        first_set: usize,
        second_set: usize,
    },
    UnownedRetainedEndpoint {
        identity: FullElaborationIdentity,
    },
    TooFewRetainedMembers {
        connection_set_index: usize,
        found: usize,
    },
    NoCommonInstanceAncestor {
        connection_set_index: usize,
    },
    MissingLcaOwnerFragment {
        connection_set_index: usize,
        lca_depth: usize,
    },
    InvalidFragment {
        fragment_index: usize,
        source: ConnectionSetError,
    },
    Normalization(ConnectionSetError),
    Allocation {
        resource: &'static str,
    },
}

impl fmt::Display for OccurrenceConnectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateEndpoint { identity } => {
                write!(
                    formatter,
                    "physical endpoint inventory repeats `{identity}`"
                )
            }
            Self::UnknownEndpoint {
                fragment_index,
                identity,
            } => write!(
                formatter,
                "connection fragment {fragment_index} names unknown endpoint `{identity}`"
            ),
            Self::MissingMembership { identity } => write!(
                formatter,
                "physical endpoint `{identity}` belongs to no normalized connection set"
            ),
            Self::MultipleMemberships {
                identity,
                first_set,
                second_set,
            } => write!(
                formatter,
                "physical endpoint `{identity}` belongs to normalized sets {first_set} and {second_set}"
            ),
            Self::UnownedRetainedEndpoint { identity } => write!(
                formatter,
                "retained physical endpoint `{identity}` has no validated owner"
            ),
            Self::TooFewRetainedMembers {
                connection_set_index,
                found,
            } => write!(
                formatter,
                "normalized connection set {connection_set_index} retains {found} members; at least two are required"
            ),
            Self::NoCommonInstanceAncestor {
                connection_set_index,
            } => write!(
                formatter,
                "contributing fragments of normalized connection set {connection_set_index} have no common root occurrence"
            ),
            Self::MissingLcaOwnerFragment {
                connection_set_index,
                lca_depth,
            } => write!(
                formatter,
                "normalized connection set {connection_set_index} has no contributing fragment declared at its depth-{lca_depth} least common ancestor"
            ),
            Self::InvalidFragment {
                fragment_index,
                source,
            } => write!(
                formatter,
                "connection fragment {fragment_index} is invalid: {source}"
            ),
            Self::Normalization(source) => {
                write!(
                    formatter,
                    "cannot normalize occurrence connection sets: {source}"
                )
            }
            Self::Allocation { resource } => write!(formatter, "cannot reserve {resource}"),
        }
    }
}

/// Normalize all typed occurrence fragments exactly once.
///
/// Endpoint and fragment input order affects only witness indices. Final set
/// ordering is derived exclusively from semantic topology.
pub(super) fn normalize_occurrence_connections(
    endpoints: &[OccurrencePhysicalEndpoint],
    fragments: &[OccurrenceConnectionFragment],
    limits: ConnectionSetLimits,
) -> Result<OccurrenceConnectionNormalization, OccurrenceConnectionError> {
    check_limit("physical endpoints", endpoints.len(), limits.max_endpoints)?;
    check_limit(
        "occurrence connection fragments",
        fragments.len(),
        limits.max_fragments,
    )?;

    let endpoint_order = canonical_endpoint_order(endpoints)?;
    reject_duplicate_endpoints(endpoints, &endpoint_order)?;
    let topology_fragments =
        copy_and_validate_fragments(endpoints, &endpoint_order, fragments, limits)?;
    let normalized = normalize_connection_sets(&topology_fragments, limits)
        .map_err(OccurrenceConnectionError::Normalization)?;
    validate_exact_endpoint_membership(endpoints, &endpoint_order, normalized.sets())?;

    let contributors = contributing_fragments(normalized.sets().len(), normalized.fragment_sets())?;
    let mut sets = Vec::new();
    sets.try_reserve_exact(normalized.sets().len())
        .map_err(|_| OccurrenceConnectionError::Allocation {
            resource: "canonical occurrence connection sets",
        })?;
    for (set_index, set) in normalized.sets().iter().enumerate() {
        sets.push(build_connection_set(
            set_index,
            set.members(),
            &contributors[set_index],
            endpoints,
            &endpoint_order,
            fragments,
        )?);
    }
    sets.sort_unstable_by(|left, right| left.topology.cmp(&right.topology));

    let exposure_witnesses = exposure_witnesses(&sets)?;
    Ok(OccurrenceConnectionNormalization {
        sets: sets.into_boxed_slice(),
        exposure_witnesses,
    })
}

fn canonical_endpoint_order(
    endpoints: &[OccurrencePhysicalEndpoint],
) -> Result<Vec<usize>, OccurrenceConnectionError> {
    let mut order = Vec::new();
    order.try_reserve_exact(endpoints.len()).map_err(|_| {
        OccurrenceConnectionError::Allocation {
            resource: "physical endpoint order",
        }
    })?;
    order.extend(0..endpoints.len());
    order.sort_unstable_by_key(|index| endpoints[*index].identity);
    Ok(order)
}

fn reject_duplicate_endpoints(
    endpoints: &[OccurrencePhysicalEndpoint],
    endpoint_order: &[usize],
) -> Result<(), OccurrenceConnectionError> {
    for pair in endpoint_order.windows(2) {
        let identity = endpoints[pair[0]].identity;
        if identity == endpoints[pair[1]].identity {
            return Err(OccurrenceConnectionError::DuplicateEndpoint { identity });
        }
    }
    Ok(())
}

fn copy_and_validate_fragments(
    endpoints: &[OccurrencePhysicalEndpoint],
    endpoint_order: &[usize],
    fragments: &[OccurrenceConnectionFragment],
    limits: ConnectionSetLimits,
) -> Result<Vec<ConnectionFragment<FullElaborationIdentity>>, OccurrenceConnectionError> {
    let mut topology_fragments = Vec::new();
    topology_fragments
        .try_reserve_exact(fragments.len())
        .map_err(|_| OccurrenceConnectionError::Allocation {
            resource: "occurrence connection topology fragments",
        })?;
    for (fragment_index, fragment) in fragments.iter().enumerate() {
        for identity in fragment.topology.members() {
            if endpoint_index(endpoints, endpoint_order, identity).is_none() {
                return Err(OccurrenceConnectionError::UnknownEndpoint {
                    fragment_index,
                    identity: *identity,
                });
            }
        }
        let topology =
            ConnectionFragment::try_new(fragment.topology.members().iter().copied(), limits)
                .map_err(|source| OccurrenceConnectionError::InvalidFragment {
                    fragment_index,
                    source,
                })?;
        topology_fragments.push(topology);
    }
    Ok(topology_fragments)
}

fn validate_exact_endpoint_membership(
    endpoints: &[OccurrencePhysicalEndpoint],
    endpoint_order: &[usize],
    sets: &[crate::connection_sets::CanonicalConnectionSet<FullElaborationIdentity>],
) -> Result<(), OccurrenceConnectionError> {
    let mut memberships = Vec::new();
    memberships
        .try_reserve_exact(endpoints.len())
        .map_err(|_| OccurrenceConnectionError::Allocation {
            resource: "physical endpoint memberships",
        })?;
    memberships.resize(endpoints.len(), None);
    for (set_index, set) in sets.iter().enumerate() {
        for identity in set.members() {
            let index = endpoint_index(endpoints, endpoint_order, identity)
                .expect("fragment endpoints were validated before normalization");
            if let Some(first_set) = memberships[index].replace(set_index) {
                return Err(OccurrenceConnectionError::MultipleMemberships {
                    identity: *identity,
                    first_set,
                    second_set: set_index,
                });
            }
        }
    }
    for index in endpoint_order {
        if memberships[*index].is_none() {
            return Err(OccurrenceConnectionError::MissingMembership {
                identity: endpoints[*index].identity,
            });
        }
    }
    Ok(())
}

fn contributing_fragments(
    set_count: usize,
    fragment_sets: &[usize],
) -> Result<Vec<Vec<usize>>, OccurrenceConnectionError> {
    let mut counts = Vec::new();
    counts
        .try_reserve_exact(set_count)
        .map_err(|_| OccurrenceConnectionError::Allocation {
            resource: "connection contributor counts",
        })?;
    counts.resize(set_count, 0_usize);
    for set in fragment_sets {
        counts[*set] =
            counts[*set]
                .checked_add(1)
                .ok_or(OccurrenceConnectionError::Normalization(
                    ConnectionSetError::CountOverflow {
                        resource: "connection contributors",
                    },
                ))?;
    }

    let mut contributors = Vec::new();
    contributors.try_reserve_exact(set_count).map_err(|_| {
        OccurrenceConnectionError::Allocation {
            resource: "connection contributor sets",
        }
    })?;
    for count in counts {
        let mut set = Vec::new();
        set.try_reserve_exact(count)
            .map_err(|_| OccurrenceConnectionError::Allocation {
                resource: "connection contributor indices",
            })?;
        contributors.push(set);
    }
    for (fragment_index, set) in fragment_sets.iter().copied().enumerate() {
        contributors[set].push(fragment_index);
    }
    Ok(contributors)
}

fn build_connection_set(
    set_index: usize,
    members: &[FullElaborationIdentity],
    contributing_fragment_indices: &[usize],
    endpoints: &[OccurrencePhysicalEndpoint],
    endpoint_order: &[usize],
    fragments: &[OccurrenceConnectionFragment],
) -> Result<OccurrenceConnectionSet, OccurrenceConnectionError> {
    let mut retained_count = 0_usize;
    let mut exposure_count = 0_usize;
    for identity in members {
        let endpoint = endpoint(endpoints, endpoint_order, identity);
        if endpoint.exposure_candidate && !endpoint.owned {
            exposure_count += 1;
        } else if endpoint.owned {
            retained_count += 1;
        } else {
            return Err(OccurrenceConnectionError::UnownedRetainedEndpoint {
                identity: *identity,
            });
        }
    }
    if retained_count < 2 {
        return Err(OccurrenceConnectionError::TooFewRetainedMembers {
            connection_set_index: set_index,
            found: retained_count,
        });
    }

    let mut retained_members = Vec::new();
    retained_members
        .try_reserve_exact(retained_count)
        .map_err(|_| OccurrenceConnectionError::Allocation {
            resource: "retained connection members",
        })?;
    let mut eliminated_exposures = Vec::new();
    eliminated_exposures
        .try_reserve_exact(exposure_count)
        .map_err(|_| OccurrenceConnectionError::Allocation {
            resource: "eliminated exposure members",
        })?;
    for identity in members {
        let endpoint = endpoint(endpoints, endpoint_order, identity);
        if endpoint.exposure_candidate && !endpoint.owned {
            eliminated_exposures.push(*identity);
        } else {
            retained_members.push(*identity);
        }
    }

    let first_contributor = contributing_fragment_indices
        .first()
        .copied()
        .expect("every normalized connection set has a contributing fragment");
    let lca_depth = fragment_owner_lca_depth(contributing_fragment_indices, fragments);
    if lca_depth == 0 {
        return Err(OccurrenceConnectionError::NoCommonInstanceAncestor {
            connection_set_index: set_index,
        });
    }
    let lca_prefix = &fragments[first_contributor]
        .declaration_instance_path
        .segments()[..lca_depth];
    let mut lca_candidates = Vec::new();
    lca_candidates
        .try_reserve_exact(contributing_fragment_indices.len())
        .map_err(|_| OccurrenceConnectionError::Allocation {
            resource: "LCA owner fragment candidates",
        })?;
    for fragment_index in contributing_fragment_indices {
        if fragments[*fragment_index]
            .declaration_instance_path
            .segments()
            == lca_prefix
        {
            lca_candidates.push(*fragment_index);
        }
    }
    let Some(owner_fragment_index) = lca_candidates.first().copied() else {
        return Err(OccurrenceConnectionError::MissingLcaOwnerFragment {
            connection_set_index: set_index,
            lca_depth,
        });
    };
    let owner_instance_path = fragments[owner_fragment_index]
        .declaration_instance_path
        .clone();
    let contributing_fragment_indices = copy_indices(
        contributing_fragment_indices,
        "contributing fragment indices",
    )?;

    Ok(OccurrenceConnectionSet {
        topology: OccurrenceConnectionTopology {
            retained_members: retained_members.into_boxed_slice(),
            eliminated_exposures: eliminated_exposures.into_boxed_slice(),
            owner_instance_path,
        },
        witness: OccurrenceConnectionWitness {
            contributing_fragment_indices,
            lca_owner_candidate_fragment_indices: lca_candidates.into_boxed_slice(),
        },
    })
}

fn copy_indices(
    indices: &[usize],
    resource: &'static str,
) -> Result<Box<[usize]>, OccurrenceConnectionError> {
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(indices.len())
        .map_err(|_| OccurrenceConnectionError::Allocation { resource })?;
    copied.extend_from_slice(indices);
    Ok(copied.into_boxed_slice())
}

fn fragment_owner_lca_depth(
    contributing_fragment_indices: &[usize],
    fragments: &[OccurrenceConnectionFragment],
) -> usize {
    let first = contributing_fragment_indices
        .first()
        .copied()
        .expect("every normalized connection set has a contributing fragment");
    let first_segments = fragments[first].declaration_instance_path.segments();
    let mut depth = first_segments.len();
    for fragment_index in &contributing_fragment_indices[1..] {
        let segments = fragments[*fragment_index]
            .declaration_instance_path
            .segments();
        depth = depth.min(segments.len());
        let mut shared = 0_usize;
        while shared < depth && first_segments[shared] == segments[shared] {
            shared += 1;
        }
        depth = shared;
        if depth == 0 {
            break;
        }
    }
    depth
}

fn exposure_witnesses(
    sets: &[OccurrenceConnectionSet],
) -> Result<Box<[ExposureConnectionSetWitness]>, OccurrenceConnectionError> {
    let exposure_count = sets.iter().try_fold(0_usize, |count, set| {
        count
            .checked_add(set.topology.eliminated_exposures.len())
            .ok_or(OccurrenceConnectionError::Normalization(
                ConnectionSetError::CountOverflow {
                    resource: "eliminated exposure witnesses",
                },
            ))
    })?;
    let mut witnesses = Vec::new();
    witnesses.try_reserve_exact(exposure_count).map_err(|_| {
        OccurrenceConnectionError::Allocation {
            resource: "eliminated exposure witnesses",
        }
    })?;
    for (connection_set_index, set) in sets.iter().enumerate() {
        for exposure in &set.topology.eliminated_exposures {
            witnesses.push(ExposureConnectionSetWitness {
                exposure: *exposure,
                connection_set_index,
            });
        }
    }
    witnesses.sort_unstable_by_key(|witness| witness.exposure);
    Ok(witnesses.into_boxed_slice())
}

fn endpoint<'a>(
    endpoints: &'a [OccurrencePhysicalEndpoint],
    endpoint_order: &[usize],
    identity: &FullElaborationIdentity,
) -> &'a OccurrencePhysicalEndpoint {
    let index = endpoint_index(endpoints, endpoint_order, identity)
        .expect("normalized members were validated against the endpoint inventory");
    &endpoints[index]
}

fn endpoint_index(
    endpoints: &[OccurrencePhysicalEndpoint],
    endpoint_order: &[usize],
    identity: &FullElaborationIdentity,
) -> Option<usize> {
    endpoint_order
        .binary_search_by_key(identity, |index| endpoints[*index].identity)
        .ok()
        .map(|ordered| endpoint_order[ordered])
}

fn check_limit(
    resource: &'static str,
    observed: usize,
    limit: usize,
) -> Result<(), OccurrenceConnectionError> {
    if observed > limit {
        Err(OccurrenceConnectionError::Normalization(
            ConnectionSetError::LimitExceeded {
                resource,
                observed,
                limit,
            },
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(value: u8) -> FullElaborationIdentity {
        FullElaborationIdentity::from_sha256([value; 32])
    }

    fn path(segments: &[&str]) -> InstancePath {
        InstancePath::new(segments.iter().copied()).unwrap()
    }

    fn endpoint_fixture(
        value: u8,
        exposure_candidate: bool,
        owned: bool,
    ) -> OccurrencePhysicalEndpoint {
        OccurrencePhysicalEndpoint::new(identity(value), exposure_candidate, owned)
    }

    fn fragment(
        members: &[u8],
        declaration_instance_path: &[&str],
    ) -> OccurrenceConnectionFragment {
        OccurrenceConnectionFragment::new(
            ConnectionFragment::try_new(
                members.iter().copied().map(identity),
                ConnectionSetLimits::default(),
            )
            .unwrap(),
            path(declaration_instance_path),
        )
    }

    #[test]
    fn transitive_union_retains_semantic_topology_and_all_fragment_witnesses() {
        let endpoints = [
            endpoint_fixture(1, false, true),
            endpoint_fixture(2, false, true),
            endpoint_fixture(3, false, true),
        ];
        let fragments = [
            fragment(&[1, 2], &["root", "left"]),
            fragment(&[2, 3], &["root"]),
        ];

        let normalized = normalize_occurrence_connections(
            &endpoints,
            &fragments,
            ConnectionSetLimits::default(),
        )
        .unwrap();

        assert_eq!(normalized.sets().len(), 1);
        let set = &normalized.sets()[0];
        assert_eq!(
            set.topology().retained_members(),
            &[identity(1), identity(2), identity(3)]
        );
        assert!(set.topology().eliminated_exposures().is_empty());
        assert_eq!(set.topology().owner_instance_path(), &path(&["root"]));
        assert_eq!(set.witness().contributing_fragment_indices(), &[0, 1]);
        assert_eq!(set.witness().lca_owner_candidate_fragment_indices(), &[1]);
    }

    #[test]
    fn only_unowned_exposure_candidates_are_eliminated() {
        let endpoints = [
            endpoint_fixture(1, true, false),
            endpoint_fixture(2, true, true),
            endpoint_fixture(3, false, true),
        ];
        let fragments = [fragment(&[1, 2], &["root"]), fragment(&[1, 3], &["root"])];

        let normalized = normalize_occurrence_connections(
            &endpoints,
            &fragments,
            ConnectionSetLimits::default(),
        )
        .unwrap();
        let topology = normalized.sets()[0].topology();
        assert_eq!(topology.retained_members(), &[identity(2), identity(3)]);
        assert_eq!(topology.eliminated_exposures(), &[identity(1)]);
        assert_eq!(normalized.exposure_witnesses().len(), 1);
        assert_eq!(normalized.exposure_witnesses()[0].exposure(), identity(1));
        assert_eq!(normalized.exposure_witnesses()[0].connection_set_index(), 0);
    }

    #[test]
    fn endpoint_inventory_is_exact_and_closed() {
        let duplicate = [
            endpoint_fixture(1, false, true),
            endpoint_fixture(1, false, true),
        ];
        assert!(matches!(
            normalize_occurrence_connections(
                &duplicate,
                &[fragment(&[1, 2], &["root"])],
                ConnectionSetLimits::default(),
            ),
            Err(OccurrenceConnectionError::DuplicateEndpoint { .. })
        ));

        let endpoints = [
            endpoint_fixture(1, false, true),
            endpoint_fixture(2, false, true),
            endpoint_fixture(3, false, true),
        ];
        assert!(matches!(
            normalize_occurrence_connections(
                &endpoints,
                &[fragment(&[1, 4], &["root"])],
                ConnectionSetLimits::default(),
            ),
            Err(OccurrenceConnectionError::UnknownEndpoint {
                fragment_index: 0,
                identity: unknown,
            }) if unknown == identity(4)
        ));
        assert!(matches!(
            normalize_occurrence_connections(
                &endpoints,
                &[fragment(&[1, 2], &["root"])],
                ConnectionSetLimits::default(),
            ),
            Err(OccurrenceConnectionError::MissingMembership { identity: missing })
                if missing == identity(3)
        ));
    }

    #[test]
    fn retained_endpoints_require_owners() {
        let endpoints = [
            endpoint_fixture(1, false, false),
            endpoint_fixture(2, false, true),
        ];
        assert!(matches!(
            normalize_occurrence_connections(
                &endpoints,
                &[fragment(&[1, 2], &["root"])],
                ConnectionSetLimits::default(),
            ),
            Err(OccurrenceConnectionError::UnownedRetainedEndpoint { identity: value })
                if value == identity(1)
        ));
    }

    #[test]
    fn exposure_elimination_must_leave_two_retained_members() {
        let endpoints = [
            endpoint_fixture(1, true, false),
            endpoint_fixture(2, false, true),
        ];
        assert_eq!(
            normalize_occurrence_connections(
                &endpoints,
                &[fragment(&[1, 2], &["root"])],
                ConnectionSetLimits::default(),
            )
            .unwrap_err(),
            OccurrenceConnectionError::TooFewRetainedMembers {
                connection_set_index: 0,
                found: 1,
            }
        );
    }

    #[test]
    fn a_contributing_fragment_must_own_the_exact_lca() {
        let endpoints = [
            endpoint_fixture(1, false, true),
            endpoint_fixture(2, false, true),
        ];
        assert_eq!(
            normalize_occurrence_connections(
                &endpoints,
                &[
                    fragment(&[1, 2], &["root", "left"]),
                    fragment(&[1, 2], &["root", "right"]),
                ],
                ConnectionSetLimits::default(),
            )
            .unwrap_err(),
            OccurrenceConnectionError::MissingLcaOwnerFragment {
                connection_set_index: 0,
                lca_depth: 1,
            }
        );
    }

    #[test]
    fn redundant_ancestor_fragment_moves_owner_to_fragment_lca_and_preserves_witnesses() {
        let endpoints = [
            endpoint_fixture(1, false, true),
            endpoint_fixture(2, false, true),
        ];
        let fragments = [
            fragment(&[1, 2], &["root", "child"]),
            fragment(&[2, 1], &["root"]),
        ];

        let normalized = normalize_occurrence_connections(
            &endpoints,
            &fragments,
            ConnectionSetLimits::default(),
        )
        .unwrap();

        assert_eq!(normalized.sets().len(), 1);
        let set = &normalized.sets()[0];
        assert_eq!(
            set.topology().retained_members(),
            &[identity(1), identity(2)]
        );
        assert_eq!(set.topology().owner_instance_path(), &path(&["root"]));
        assert_eq!(set.witness().contributing_fragment_indices(), &[0, 1]);
        assert_eq!(set.witness().lca_owner_candidate_fragment_indices(), &[1]);
    }

    #[test]
    fn endpoint_and_fragment_permutations_preserve_topology_and_exposure_projection() {
        let endpoints = vec![
            endpoint_fixture(1, true, false),
            endpoint_fixture(2, false, true),
            endpoint_fixture(3, false, true),
            endpoint_fixture(4, false, true),
            endpoint_fixture(5, false, true),
        ];
        let fragments = vec![
            fragment(&[1, 2], &["root"]),
            fragment(&[1, 3], &["root"]),
            fragment(&[4, 5], &["root", "other"]),
        ];
        let first = normalize_occurrence_connections(
            &endpoints,
            &fragments,
            ConnectionSetLimits::default(),
        )
        .unwrap();

        let mut permuted_endpoints = endpoints;
        permuted_endpoints.reverse();
        let permuted_fragments = vec![
            fragment(&[5, 4], &["root", "other"]),
            fragment(&[3, 1], &["root"]),
            fragment(&[2, 1], &["root"]),
        ];
        let second = normalize_occurrence_connections(
            &permuted_endpoints,
            &permuted_fragments,
            ConnectionSetLimits::default(),
        )
        .unwrap();

        let first_topology = first
            .sets()
            .iter()
            .map(|set| set.topology().clone())
            .collect::<Vec<_>>();
        let second_topology = second
            .sets()
            .iter()
            .map(|set| set.topology().clone())
            .collect::<Vec<_>>();
        assert_eq!(first_topology, second_topology);
        assert_eq!(first.exposure_witnesses(), second.exposure_witnesses());
        assert_ne!(
            first.sets()[0]
                .witness()
                .lca_owner_candidate_fragment_indices(),
            second.sets()[0]
                .witness()
                .lca_owner_candidate_fragment_indices()
        );
    }

    #[test]
    fn disconnected_roots_fail_without_inventing_an_owner_path() {
        let endpoints = [
            endpoint_fixture(1, false, true),
            endpoint_fixture(2, false, true),
        ];
        assert_eq!(
            normalize_occurrence_connections(
                &endpoints,
                &[
                    fragment(&[1, 2], &["first"]),
                    fragment(&[1, 2], &["second"]),
                ],
                ConnectionSetLimits::default(),
            )
            .unwrap_err(),
            OccurrenceConnectionError::NoCommonInstanceAncestor {
                connection_set_index: 0,
            }
        );
    }

    #[test]
    fn endpoint_inventory_limit_fails_before_normalization() {
        let endpoints = [
            endpoint_fixture(1, false, true),
            endpoint_fixture(2, false, true),
        ];
        let limits = ConnectionSetLimits {
            max_endpoints: 1,
            ..ConnectionSetLimits::default()
        };
        assert!(matches!(
            normalize_occurrence_connections(&endpoints, &[], limits),
            Err(OccurrenceConnectionError::Normalization(
                ConnectionSetError::LimitExceeded {
                    resource: "physical endpoints",
                    observed: 2,
                    limit: 1,
                }
            ))
        ));
    }
}
