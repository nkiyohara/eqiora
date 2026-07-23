use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_distributed::PartitionId;
use eqiora_meshing::{MeshEntity, MeshTopology};
use sha2::{Digest, Sha256};

const LAYOUT_IDENTITY_DOMAIN_V1: &[u8] = b"eqiora.distributed-mesh-layout/v1\0";

/// Exact content identity of the already authenticated mesh revision.
///
/// L2 binds this fixed-size value but does not authenticate artifact bytes.
/// The ordinary L3 bridge must validate the mesh envelope and its geometry
/// correspondence before constructing a distributed layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshRevisionIdentityV1([u8; 32]);

impl MeshRevisionIdentityV1 {
    /// Bind raw SHA-256 bytes from an already authenticated mesh artifact.
    #[must_use]
    pub const fn from_sha256(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Fixed-size bytes used by transport agreement and derived identities.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Sole explicit ownership input for one canonical top-dimensional cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellOwnershipClaim {
    cell: MeshEntity,
    owner: PartitionId,
}

impl CellOwnershipClaim {
    /// Associate one global mesh cell with its unique execution partition.
    #[must_use]
    pub const fn new(cell: MeshEntity, owner: PartitionId) -> Self {
        Self { cell, owner }
    }

    /// Global top-dimensional mesh entity.
    #[must_use]
    pub const fn cell(self) -> MeshEntity {
        self.cell
    }

    /// Unique cell producer.
    #[must_use]
    pub const fn owner(self) -> PartitionId {
        self.owner
    }
}

/// One canonical owner-to-receiver entity-residency exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityExchange {
    owner: PartitionId,
    receiver: PartitionId,
    dimension: usize,
    entities: Vec<MeshEntity>,
}

impl EntityExchange {
    /// Unique entity owner.
    #[must_use]
    pub const fn owner(&self) -> PartitionId {
        self.owner
    }

    /// Resident partition that sees these entities as ghosts.
    #[must_use]
    pub const fn receiver(&self) -> PartitionId {
        self.receiver
    }

    /// Mesh stratum shared by this exchange.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// Strictly increasing global mesh entities.
    #[must_use]
    pub fn entities(&self) -> &[MeshEntity] {
        &self.entities
    }
}

/// Fixed-size identity of exact cell ownership and every derived local view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DistributedMeshLayoutIdentityV1([u8; 32]);

impl DistributedMeshLayoutIdentityV1 {
    /// Raw SHA-256 bytes for fixed-size transport agreement.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntityOwnership {
    owner: PartitionId,
    residents: Vec<PartitionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalStratum {
    owned: Vec<MeshEntity>,
    ghosts: Vec<MeshEntity>,
}

/// Exact cell partition and its deterministically derived entity residency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedMeshLayout {
    mesh: MeshRevisionIdentityV1,
    topological_dimension: usize,
    partition_count: NonZeroUsize,
    cell_owners: Vec<PartitionId>,
    ownership: Vec<Vec<EntityOwnership>>,
    local: Vec<Vec<LocalStratum>>,
    partition_boundary: Vec<Vec<MeshEntity>>,
    exchanges: Vec<EntityExchange>,
    identity: DistributedMeshLayoutIdentityV1,
}

impl DistributedMeshLayout {
    /// Derive every lower-dimensional owner and local view from exact cell claims.
    ///
    /// Claims are canonicalized by global cell index. Every accepted cell must
    /// occur exactly once, each owner must be in range, and every declared
    /// partition must own a cell. For every lower entity, the owner is the
    /// minimum owner in its nonempty incident-cell star.
    ///
    /// # Errors
    /// Returns `EQ0807` for an incomplete or invalid claim set, malformed mesh
    /// incidence, an empty partition, or an unrepresentable portable identity.
    pub fn derive(
        mesh: MeshRevisionIdentityV1,
        topology: &(impl MeshTopology + ?Sized),
        partition_count: NonZeroUsize,
        mut claims: Vec<CellOwnershipClaim>,
    ) -> Result<Self, Diagnostic> {
        let topological_dimension = topology.topological_dimension();
        if topological_dimension == 0 {
            return Err(invalid(
                "distributed spatial ownership requires a positive-dimensional mesh",
            ));
        }
        let mut entity_counts = Vec::with_capacity(topological_dimension + 1);
        for dimension in 0..=topological_dimension {
            let count = topology.entity_count(dimension).ok_or_else(|| {
                invalid(format!(
                    "distributed mesh is missing entity stratum {dimension}"
                ))
            })?;
            if count == 0 {
                return Err(invalid(format!(
                    "distributed mesh entity stratum {dimension} is empty"
                )));
            }
            entity_counts.push(count);
        }
        let cell_count = entity_counts[topological_dimension];
        if claims.len() != cell_count {
            return Err(invalid(format!(
                "distributed cell ownership has {} claims for {cell_count} cells",
                claims.len()
            )));
        }
        claims.sort_by_key(|claim| (claim.cell.dimension(), claim.cell.index()));
        let mut cell_owners = Vec::with_capacity(cell_count);
        let mut cells_per_partition = vec![0_usize; partition_count.get()];
        for (expected, claim) in claims.into_iter().enumerate() {
            if claim.cell.dimension() != topological_dimension || claim.cell.index() != expected {
                return Err(invalid(format!(
                    "distributed cell ownership must name cell {expected} exactly once"
                )));
            }
            if claim.owner.index() >= partition_count.get() {
                return Err(invalid(format!(
                    "cell {expected} owner {} is outside 0..{}",
                    claim.owner.index(),
                    partition_count
                )));
            }
            cells_per_partition[claim.owner.index()] += 1;
            cell_owners.push(claim.owner);
        }
        if let Some(empty) = cells_per_partition.iter().position(|count| *count == 0) {
            return Err(invalid(format!(
                "distributed mesh partition {empty} owns no cell"
            )));
        }

        let mut ownership = Vec::with_capacity(topological_dimension + 1);
        for (dimension, &count) in entity_counts.iter().enumerate() {
            let mut stratum = Vec::with_capacity(count);
            for index in 0..count {
                let entity = MeshEntity::new(dimension, index);
                let residents = if dimension == topological_dimension {
                    vec![cell_owners[index]]
                } else {
                    let incident = topology
                        .incidence(entity, topological_dimension)
                        .ok_or_else(|| invalid("distributed mesh incidence is unavailable"))?;
                    let residents = incident
                        .into_iter()
                        .map(|entry| {
                            cell_owners
                                .get(entry.entity.index())
                                .copied()
                                .ok_or_else(|| {
                                    invalid("distributed mesh incidence names an invalid cell")
                                })
                        })
                        .collect::<Result<BTreeSet<_>, _>>()?;
                    if residents.is_empty() {
                        return Err(invalid(format!(
                            "mesh entity ({dimension}, {index}) has no incident cell owner"
                        )));
                    }
                    residents.into_iter().collect()
                };
                stratum.push(EntityOwnership {
                    owner: residents[0],
                    residents,
                });
            }
            ownership.push(stratum);
        }

        let local = (0..partition_count.get())
            .map(|partition| {
                ownership
                    .iter()
                    .enumerate()
                    .map(|(dimension, stratum)| {
                        let mut owned = Vec::new();
                        let mut ghosts = Vec::new();
                        for (index, entity) in stratum.iter().enumerate() {
                            if entity
                                .residents
                                .binary_search(&PartitionId::new(partition))
                                .is_ok()
                            {
                                let target = MeshEntity::new(dimension, index);
                                if entity.owner == PartitionId::new(partition) {
                                    owned.push(target);
                                } else {
                                    ghosts.push(target);
                                }
                            }
                        }
                        LocalStratum { owned, ghosts }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let partition_boundary = ownership
            .iter()
            .enumerate()
            .map(|(dimension, stratum)| {
                stratum
                    .iter()
                    .enumerate()
                    .filter_map(|(index, entity)| {
                        (entity.residents.len() > 1).then_some(MeshEntity::new(dimension, index))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let exchanges = derive_exchanges(&ownership, partition_count);
        let identity = layout_identity(
            mesh,
            topological_dimension,
            partition_count,
            &entity_counts,
            &ownership,
            &local,
            &partition_boundary,
            &exchanges,
        )?;
        Ok(Self {
            mesh,
            topological_dimension,
            partition_count,
            cell_owners,
            ownership,
            local,
            partition_boundary,
            exchanges,
            identity,
        })
    }

    /// Exact authenticated mesh identity supplied by the owning bridge.
    #[must_use]
    pub const fn mesh(&self) -> MeshRevisionIdentityV1 {
        self.mesh
    }

    /// Topological dimension of the bound mesh.
    #[must_use]
    pub const fn topological_dimension(&self) -> usize {
        self.topological_dimension
    }

    /// Number of execution partitions.
    #[must_use]
    pub const fn partition_count(&self) -> NonZeroUsize {
        self.partition_count
    }

    /// Unique producer of a canonical cell index.
    #[must_use]
    pub fn cell_owner(&self, cell: usize) -> Option<PartitionId> {
        self.cell_owners.get(cell).copied()
    }

    /// Number of top-dimensional cells.
    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.cell_owners.len()
    }

    /// Derived unique owner of one valid mesh entity.
    #[must_use]
    pub fn entity_owner(&self, entity: MeshEntity) -> Option<PartitionId> {
        self.ownership
            .get(entity.dimension())?
            .get(entity.index())
            .map(|entry| entry.owner)
    }

    /// Sorted resident partitions of one valid mesh entity.
    #[must_use]
    pub fn entity_residents(&self, entity: MeshEntity) -> Option<&[PartitionId]> {
        self.ownership
            .get(entity.dimension())?
            .get(entity.index())
            .map(|entry| entry.residents.as_slice())
    }

    /// Ascending entities uniquely owned by one resident partition.
    #[must_use]
    pub fn owned_entities(
        &self,
        partition: PartitionId,
        dimension: usize,
    ) -> Option<&[MeshEntity]> {
        self.local
            .get(partition.index())?
            .get(dimension)
            .map(|stratum| stratum.owned.as_slice())
    }

    /// Ascending resident entities whose unique owner is another partition.
    #[must_use]
    pub fn ghost_entities(
        &self,
        partition: PartitionId,
        dimension: usize,
    ) -> Option<&[MeshEntity]> {
        self.local
            .get(partition.index())?
            .get(dimension)
            .map(|stratum| stratum.ghosts.as_slice())
    }

    /// Ascending entities visible from more than one execution partition.
    #[must_use]
    pub fn partition_boundary_entities(&self, dimension: usize) -> Option<&[MeshEntity]> {
        self.partition_boundary.get(dimension).map(Vec::as_slice)
    }

    /// Canonically grouped entity owner-to-receiver exchanges.
    #[must_use]
    pub fn entity_exchanges(&self) -> &[EntityExchange] {
        &self.exchanges
    }

    /// Agreement identity covering the exact mesh, cell claims, and derivations.
    #[must_use]
    pub const fn identity(&self) -> DistributedMeshLayoutIdentityV1 {
        self.identity
    }
}

fn derive_exchanges(
    ownership: &[Vec<EntityOwnership>],
    partition_count: NonZeroUsize,
) -> Vec<EntityExchange> {
    let mut result = Vec::new();
    for owner in 0..partition_count.get() {
        for receiver in 0..partition_count.get() {
            if owner == receiver {
                continue;
            }
            for (dimension, stratum) in ownership.iter().enumerate() {
                let entities = stratum
                    .iter()
                    .enumerate()
                    .filter_map(|(index, entity)| {
                        (entity.owner == PartitionId::new(owner)
                            && entity
                                .residents
                                .binary_search(&PartitionId::new(receiver))
                                .is_ok())
                        .then_some(MeshEntity::new(dimension, index))
                    })
                    .collect::<Vec<_>>();
                if !entities.is_empty() {
                    result.push(EntityExchange {
                        owner: PartitionId::new(owner),
                        receiver: PartitionId::new(receiver),
                        dimension,
                        entities,
                    });
                }
            }
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn layout_identity(
    mesh: MeshRevisionIdentityV1,
    topological_dimension: usize,
    partition_count: NonZeroUsize,
    entity_counts: &[usize],
    ownership: &[Vec<EntityOwnership>],
    local: &[Vec<LocalStratum>],
    partition_boundary: &[Vec<MeshEntity>],
    exchanges: &[EntityExchange],
) -> Result<DistributedMeshLayoutIdentityV1, Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(LAYOUT_IDENTITY_DOMAIN_V1);
    hash.update(mesh.as_bytes());
    hash_usize(&mut hash, topological_dimension)?;
    hash_usize(&mut hash, partition_count.get())?;
    hash_usize(&mut hash, entity_counts.len())?;
    for count in entity_counts {
        hash_usize(&mut hash, *count)?;
    }
    for stratum in ownership {
        hash_usize(&mut hash, stratum.len())?;
        for entity in stratum {
            hash_usize(&mut hash, entity.owner.index())?;
            hash_usize(&mut hash, entity.residents.len())?;
            for resident in &entity.residents {
                hash_usize(&mut hash, resident.index())?;
            }
        }
    }
    for partition in local {
        for stratum in partition {
            hash_entities(&mut hash, &stratum.owned)?;
            hash_entities(&mut hash, &stratum.ghosts)?;
        }
    }
    for stratum in partition_boundary {
        hash_entities(&mut hash, stratum)?;
    }
    hash_usize(&mut hash, exchanges.len())?;
    for exchange in exchanges {
        hash_usize(&mut hash, exchange.owner.index())?;
        hash_usize(&mut hash, exchange.receiver.index())?;
        hash_usize(&mut hash, exchange.dimension)?;
        hash_entities(&mut hash, &exchange.entities)?;
    }
    Ok(DistributedMeshLayoutIdentityV1(hash.finalize().into()))
}

fn hash_entities(hash: &mut Sha256, entities: &[MeshEntity]) -> Result<(), Diagnostic> {
    hash_usize(hash, entities.len())?;
    for entity in entities {
        hash_usize(hash, entity.dimension())?;
        hash_usize(hash, entity.index())?;
    }
    Ok(())
}

fn hash_usize(hash: &mut Sha256, value: usize) -> Result<(), Diagnostic> {
    let value = u64::try_from(value)
        .map_err(|_| invalid("distributed mesh identity value exceeds portable u64"))?;
    hash.update(value.to_le_bytes());
    Ok(())
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_meshing::{MeshQualityGate, SimplicialMesh};

    fn mesh() -> SimplicialMesh {
        SimplicialMesh::new(
            2,
            vec![
                vec![0.0, 0.0],
                vec![1.0, 0.0],
                vec![0.0, 1.0],
                vec![1.0, 1.0],
            ],
            vec![vec![0, 1, 3], vec![0, 3, 2]],
            MeshQualityGate::new(0.4).unwrap(),
        )
        .unwrap()
    }

    fn layout(claims: Vec<CellOwnershipClaim>) -> Result<DistributedMeshLayout, Diagnostic> {
        DistributedMeshLayout::derive(
            MeshRevisionIdentityV1::from_sha256([7; 32]),
            &mesh(),
            NonZeroUsize::new(2).unwrap(),
            claims,
        )
    }

    #[test]
    fn derives_lower_entity_residency_and_canonical_exchanges() {
        let layout = layout(vec![
            CellOwnershipClaim::new(MeshEntity::new(2, 1), PartitionId::new(1)),
            CellOwnershipClaim::new(MeshEntity::new(2, 0), PartitionId::new(0)),
        ])
        .unwrap();
        let shared_facet = (0..layout.partition_boundary_entities(1).unwrap().len())
            .map(|index| layout.partition_boundary_entities(1).unwrap()[index])
            .find(|facet| layout.entity_residents(*facet).unwrap().len() == 2)
            .unwrap();
        assert_eq!(layout.entity_owner(shared_facet), Some(PartitionId::new(0)));
        assert!(
            layout
                .ghost_entities(PartitionId::new(1), 1)
                .unwrap()
                .contains(&shared_facet)
        );
        assert!(layout.entity_exchanges().iter().any(|exchange| {
            exchange.owner() == PartitionId::new(0)
                && exchange.receiver() == PartitionId::new(1)
                && exchange.entities().contains(&shared_facet)
        }));
    }

    #[test]
    fn claim_order_does_not_change_identity() {
        let forward = layout(vec![
            CellOwnershipClaim::new(MeshEntity::new(2, 0), PartitionId::new(0)),
            CellOwnershipClaim::new(MeshEntity::new(2, 1), PartitionId::new(1)),
        ])
        .unwrap();
        let reverse = layout(vec![
            CellOwnershipClaim::new(MeshEntity::new(2, 1), PartitionId::new(1)),
            CellOwnershipClaim::new(MeshEntity::new(2, 0), PartitionId::new(0)),
        ])
        .unwrap();
        assert_eq!(forward, reverse);
        assert_eq!(forward.identity(), reverse.identity());
    }

    #[test]
    fn rejects_duplicate_missing_wrong_stratum_and_out_of_range_claims() {
        let cases = [
            vec![
                CellOwnershipClaim::new(MeshEntity::new(2, 0), PartitionId::new(0)),
                CellOwnershipClaim::new(MeshEntity::new(2, 0), PartitionId::new(1)),
            ],
            vec![
                CellOwnershipClaim::new(MeshEntity::new(1, 0), PartitionId::new(0)),
                CellOwnershipClaim::new(MeshEntity::new(2, 1), PartitionId::new(1)),
            ],
            vec![
                CellOwnershipClaim::new(MeshEntity::new(2, 0), PartitionId::new(0)),
                CellOwnershipClaim::new(MeshEntity::new(2, 1), PartitionId::new(2)),
            ],
        ];
        for claims in cases {
            assert_eq!(
                layout(claims).unwrap_err().code(),
                codes::INVALID_REALIZATION
            );
        }
    }

    #[test]
    fn rejects_an_empty_cell_partition() {
        assert_eq!(
            layout(vec![
                CellOwnershipClaim::new(MeshEntity::new(2, 0), PartitionId::new(0)),
                CellOwnershipClaim::new(MeshEntity::new(2, 1), PartitionId::new(0)),
            ])
            .unwrap_err()
            .code(),
            codes::INVALID_REALIZATION
        );
    }
}
