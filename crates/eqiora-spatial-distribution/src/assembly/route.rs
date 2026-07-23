use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use eqiora_assembly::{AssemblyPlan, AssemblyTargetId, DofId};
use eqiora_core::Diagnostic;
use eqiora_distributed::PartitionId;
use sha2::{Digest, Sha256};

use crate::{DistributedMeshLayout, DistributedMeshLayoutIdentityV1};

use super::codec::{WireReader, hash_f64, hash_usize, invalid, push_usize, reserve, target_sizes};
use super::ownership::{
    AdmittedRowOwnership, AdmittedRowOwnershipIdentityV1, LocalAssemblyProjection,
    validate_ownership,
};

const PLAN_IDENTITY_DOMAIN_V1: &[u8] = b"eqiora.distributed-assembly-plan/v1\0";
const ROUTE_IDENTITY_DOMAIN_V1: &[u8] = b"eqiora.distributed-assembly-route/v1\0";
const LOCAL_ROUTE_ADMISSION_DOMAIN_V1: &[u8] =
    b"eqiora.distributed-assembly-local-route-admission/v1\0";
const ROUTE_DESCRIPTOR_WIRE_MAGIC_V1: &[u8; 8] = b"EQARDS01";
const ROUTE_WIRE_MAGIC_V1: &[u8; 8] = b"EQAROW01";
const LOCAL_ROUTE_ADMISSION_WIRE_MAGIC_V1: &[u8; 8] = b"EQALRA01";

/// Transport-neutral identity of one admitted route plan, including payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DistributedAssemblyPlanIdentityV1(pub(super) [u8; 32]);

impl DistributedAssemblyPlanIdentityV1 {
    /// Fixed-size bytes used for execution-group agreement.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity of one row route's complete semantic payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssemblyRowRouteIdentityV1([u8; 32]);

impl AssemblyRowRouteIdentityV1 {
    /// Fixed-size route identity bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity of one producer's proof that its exact local routes and the
/// collective row owners agree with its actual equation support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalRouteAdmissionIdentityV1([u8; 32]);

impl LocalRouteAdmissionIdentityV1 {
    /// Fixed-size bytes used by transport agreement and the final receipt.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Payload-bound metadata for one packet/target/row route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssemblyRowRouteDescriptorV1 {
    pub(super) packet: usize,
    pub(super) producer: PartitionId,
    pub(super) target: AssemblyTargetId,
    pub(super) target_size: usize,
    pub(super) row: DofId,
    pub(super) destination: PartitionId,
    pub(super) entry_count: usize,
    pub(super) identity: AssemblyRowRouteIdentityV1,
}

impl AssemblyRowRouteDescriptorV1 {
    /// Exact encoded length of one route descriptor.
    pub const ENCODED_LEN: usize = 96;

    /// Global logical packet index controlling the canonical numerical fold.
    #[must_use]
    pub const fn packet(self) -> usize {
        self.packet
    }

    /// Unique cell owner that evaluated the packet.
    #[must_use]
    pub const fn producer(self) -> PartitionId {
        self.producer
    }

    /// Ordered assembly target.
    #[must_use]
    pub const fn target(self) -> AssemblyTargetId {
        self.target
    }

    /// Complete square target dimension.
    #[must_use]
    pub const fn target_size(self) -> usize {
        self.target_size
    }

    /// Global equation carried by this route.
    #[must_use]
    pub const fn row(self) -> DofId {
        self.row
    }

    /// Unique row owner receiving the route.
    #[must_use]
    pub const fn destination(self) -> PartitionId {
        self.destination
    }

    /// Number of canonical column deltas in the payload.
    #[must_use]
    pub const fn entry_count(self) -> usize {
        self.entry_count
    }

    /// Identity over metadata, entries, and right-hand-side payload.
    #[must_use]
    pub const fn identity(self) -> AssemblyRowRouteIdentityV1 {
        self.identity
    }

    /// Encode fixed-size payload-bound metadata for inventory collectives.
    ///
    /// # Errors
    /// Returns `EQ0806` if an index is not representable by portable `u64`.
    pub fn to_bytes(self) -> Result<[u8; 96], Diagnostic> {
        let mut bytes = Vec::with_capacity(Self::ENCODED_LEN);
        bytes.extend_from_slice(ROUTE_DESCRIPTOR_WIRE_MAGIC_V1);
        push_usize(&mut bytes, self.packet)?;
        push_usize(&mut bytes, self.producer.index())?;
        push_usize(&mut bytes, self.target.index())?;
        push_usize(&mut bytes, self.target_size)?;
        push_usize(&mut bytes, self.row.index())?;
        push_usize(&mut bytes, self.destination.index())?;
        push_usize(&mut bytes, self.entry_count)?;
        bytes.extend_from_slice(&self.identity.0);
        Ok(bytes
            .try_into()
            .expect("route descriptor encoding has a fixed-length shape"))
    }

    /// Decode fixed-size route metadata against one concrete target plan.
    ///
    /// Layout producer/owner checks occur when the descriptor enters a sealed
    /// [`DistributedAssemblyRoutePlanV1`]. Payload identity is checked when
    /// the corresponding [`AssemblyRowRouteV1`] enters a local inventory or
    /// destination inbox.
    ///
    /// # Errors
    /// Returns `EQ0806` for invalid magic, target, or trailing data.
    pub fn from_bytes(plan: &AssemblyPlan, bytes: &[u8]) -> Result<Self, Diagnostic> {
        let mut reader = WireReader::new(bytes, ROUTE_DESCRIPTOR_WIRE_MAGIC_V1)?;
        let packet = reader.usize()?;
        let producer = PartitionId::new(reader.usize()?);
        let target = plan
            .target_id(reader.usize()?)
            .ok_or_else(|| invalid("assembly route descriptor names a target outside the plan"))?;
        let target_size = reader.usize()?;
        let row = DofId::new(reader.usize()?);
        let destination = PartitionId::new(reader.usize()?);
        let entry_count = reader.usize()?;
        let identity = AssemblyRowRouteIdentityV1(reader.array_32()?);
        reader.finish()?;
        Ok(Self {
            packet,
            producer,
            target,
            target_size,
            row,
            destination,
            entry_count,
            identity,
        })
    }

    fn key(self) -> RouteKey {
        RouteKey {
            target: self.target,
            row: self.row,
            packet: self.packet,
        }
    }
}

/// One canonical row payload ready for transport to its admitted owner.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyRowRouteV1 {
    pub(super) descriptor: AssemblyRowRouteDescriptorV1,
    pub(super) entries: Vec<(DofId, f64)>,
    pub(super) rhs: f64,
}

impl AssemblyRowRouteV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        packet: usize,
        producer: PartitionId,
        target: AssemblyTargetId,
        target_size: usize,
        row: DofId,
        destination: PartitionId,
        entries: Vec<(DofId, f64)>,
        rhs: f64,
    ) -> Result<Self, Diagnostic> {
        validate_route_values(target_size, row, &entries, rhs)?;
        let address = RouteAddress {
            packet,
            producer,
            target,
            target_size,
            row,
            destination,
        };
        let identity = route_identity(address, &entries, rhs)?;
        Ok(Self {
            descriptor: AssemblyRowRouteDescriptorV1 {
                packet,
                producer,
                target,
                target_size,
                row,
                destination,
                entry_count: entries.len(),
                identity,
            },
            entries,
            rhs,
        })
    }

    /// Payload-bound route descriptor used to seal the global route plan.
    #[must_use]
    pub const fn descriptor(&self) -> AssemblyRowRouteDescriptorV1 {
        self.descriptor
    }

    /// Canonically ascending global-column deltas.
    #[must_use]
    pub fn entries(&self) -> &[(DofId, f64)] {
        &self.entries
    }

    /// Additive right-hand-side delta.
    #[must_use]
    pub const fn rhs(&self) -> f64 {
        self.rhs
    }

    /// Encode a bounded, endian-stable payload for byte transports.
    ///
    /// # Errors
    /// Returns `EQ0806` if an index is not representable by portable `u64`,
    /// arithmetic overflows, allocation fails, or internal integrity is lost.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Diagnostic> {
        self.validate_integrity()?;
        let entry_bytes = self
            .entries
            .len()
            .checked_mul(16)
            .ok_or_else(|| invalid("assembly route wire length overflows usize"))?;
        let capacity = 72_usize
            .checked_add(entry_bytes)
            .ok_or_else(|| invalid("assembly route wire length overflows usize"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| invalid("could not reserve assembly route wire payload"))?;
        bytes.extend_from_slice(ROUTE_WIRE_MAGIC_V1);
        push_usize(&mut bytes, self.descriptor.packet)?;
        push_usize(&mut bytes, self.descriptor.producer.index())?;
        push_usize(&mut bytes, self.descriptor.target.index())?;
        push_usize(&mut bytes, self.descriptor.target_size)?;
        push_usize(&mut bytes, self.descriptor.row.index())?;
        push_usize(&mut bytes, self.descriptor.destination.index())?;
        push_usize(&mut bytes, self.entries.len())?;
        for (column, value) in &self.entries {
            push_usize(&mut bytes, column.index())?;
            bytes.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        bytes.extend_from_slice(&self.rhs.to_bits().to_be_bytes());
        debug_assert_eq!(bytes.len(), capacity);
        Ok(bytes)
    }

    /// Decode and structurally validate one complete route payload.
    ///
    /// Exact producer, destination, and inventory membership are validated by
    /// [`DistributedAssemblyRoutePlanV1::fold_inbox`].
    ///
    /// # Errors
    /// Returns `EQ0806` for a truncated, trailing, noncanonical, non-finite,
    /// oversized, or otherwise malformed payload.
    pub fn from_bytes(plan: &AssemblyPlan, bytes: &[u8]) -> Result<Self, Diagnostic> {
        decode_route_fields(bytes, |index| {
            plan.target_id(index)
                .ok_or_else(|| invalid("assembly route names a target outside the plan"))
        })
    }

    pub(super) fn canonical_key(&self) -> RouteKey {
        self.descriptor.key()
    }

    fn validate_integrity(&self) -> Result<(), Diagnostic> {
        validate_route_values(
            self.descriptor.target_size,
            self.descriptor.row,
            &self.entries,
            self.rhs,
        )?;
        if self.descriptor.entry_count != self.entries.len()
            || self.descriptor.identity
                != route_identity(
                    RouteAddress {
                        packet: self.descriptor.packet,
                        producer: self.descriptor.producer,
                        target: self.descriptor.target,
                        target_size: self.descriptor.target_size,
                        row: self.descriptor.row,
                        destination: self.descriptor.destination,
                    },
                    &self.entries,
                    self.rhs,
                )?
        {
            return Err(invalid(
                "assembly route payload does not match its descriptor",
            ));
        }
        Ok(())
    }
}

/// Globally sealed inventory of every payload-bound row route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedAssemblyRoutePlanV1 {
    pub(super) layout: DistributedMeshLayoutIdentityV1,
    pub(super) row_ownership: AdmittedRowOwnershipIdentityV1,
    pub(super) packet_count: usize,
    pub(super) partition_count: NonZeroUsize,
    pub(super) target_ids: Vec<AssemblyTargetId>,
    pub(super) target_sizes: Vec<usize>,
    pub(super) row_owners: Vec<Vec<PartitionId>>,
    pub(super) descriptors: Vec<AssemblyRowRouteDescriptorV1>,
    pub(super) identity: DistributedAssemblyPlanIdentityV1,
}

impl DistributedAssemblyRoutePlanV1 {
    /// Seal the canonical descriptor inventory before any payload is admitted.
    ///
    /// Every descriptor binds its payload identity. Duplicate route keys,
    /// wrong packet producers, targets, target sizes, rows, and destinations
    /// fail before an inbox can be folded.
    ///
    /// # Errors
    /// Returns `EQ0806` for any malformed or duplicate descriptor inventory.
    pub fn seal(
        layout: &DistributedMeshLayout,
        plan: &AssemblyPlan,
        ownership: &AdmittedRowOwnership,
        mut descriptors: Vec<AssemblyRowRouteDescriptorV1>,
    ) -> Result<Self, Diagnostic> {
        validate_ownership(layout, plan, ownership)?;
        let target_sizes = target_sizes(plan)?;
        let target_ids = (0..plan.target_count())
            .map(|index| {
                plan.target_id(index)
                    .ok_or_else(|| invalid("assembly target is unavailable"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        descriptors.sort_by_key(|descriptor| descriptor.key());
        let mut seen = BTreeSet::new();
        for descriptor in &descriptors {
            validate_descriptor(layout, &target_sizes, ownership, *descriptor)?;
            if !seen.insert(descriptor.key()) {
                return Err(invalid(format!(
                    "assembly route ({}, {}, {}) occurs more than once",
                    descriptor.target.index(),
                    descriptor.row.index(),
                    descriptor.packet
                )));
            }
        }
        let row_owners = ownership
            .targets
            .iter()
            .map(|target| target.owners().to_vec())
            .collect::<Vec<_>>();
        let identity = plan_identity(
            layout,
            ownership.identity,
            &target_sizes,
            &row_owners,
            &descriptors,
        )?;
        Ok(Self {
            layout: layout.identity(),
            row_ownership: ownership.identity,
            packet_count: layout.cell_count(),
            partition_count: layout.partition_count(),
            target_ids,
            target_sizes,
            row_owners,
            descriptors,
            identity,
        })
    }

    /// Payload-bound identity for collective plan agreement.
    #[must_use]
    pub const fn identity(&self) -> DistributedAssemblyPlanIdentityV1 {
        self.identity
    }

    /// Canonically ordered global descriptor inventory.
    #[must_use]
    pub fn descriptors(&self) -> &[AssemblyRowRouteDescriptorV1] {
        &self.descriptors
    }

    /// Admit one producer's actual routes and prove its local support agrees
    /// with the collectively selected row owners.
    ///
    /// # Errors
    /// Returns `EQ0806` for a foreign projection/ownership, a collective owner
    /// below no supported producer or above a local candidate, or missing,
    /// duplicate, foreign, or payload-mismatched routes.
    pub fn admit_local_routes(
        &self,
        projection: &LocalAssemblyProjection,
        ownership: &AdmittedRowOwnership,
        routes: &[AssemblyRowRouteV1],
    ) -> Result<LocalRouteAdmissionV1, Diagnostic> {
        ownership.validate_projection(projection)?;
        if ownership.identity != self.row_ownership || projection.layout != self.layout {
            return Err(invalid(
                "local route admission belongs to another route plan or row ownership",
            ));
        }
        let producer = projection.producer;
        if producer.index() >= self.partition_count.get() {
            return Err(invalid("assembly route producer is outside the plan"));
        }
        for (target, candidates) in projection.candidates.iter().enumerate() {
            for (row, candidate) in candidates.iter().copied().enumerate() {
                let owner = ownership.targets[target]
                    .owner(row)
                    .ok_or_else(|| invalid("collective row owner is missing"))?;
                if candidate.is_some_and(|candidate| owner > candidate) {
                    return Err(invalid(format!(
                        "collective owner {} for target {target} row {row} exceeds local supporting producer {}",
                        owner.index(),
                        producer.index()
                    )));
                }
                if owner == producer && candidate != Some(producer) {
                    return Err(invalid(format!(
                        "collective owner {} for target {target} row {row} has no local equation support",
                        producer.index()
                    )));
                }
            }
        }
        let mut actual = Vec::new();
        actual
            .try_reserve_exact(routes.len())
            .map_err(|_| invalid("could not reserve local route validation inventory"))?;
        for route in routes {
            route.validate_integrity()?;
            if route.descriptor.producer != producer {
                return Err(invalid("local route inventory contains a foreign producer"));
            }
            actual.push(route.descriptor);
        }
        actual.sort_by_key(|descriptor| descriptor.key());
        if actual.windows(2).any(|pair| pair[0].key() == pair[1].key()) {
            return Err(invalid("local route inventory contains a duplicate route"));
        }
        let expected = self
            .descriptors
            .iter()
            .copied()
            .filter(|descriptor| descriptor.producer == producer)
            .collect::<Vec<_>>();
        let projected = projection
            .routes(ownership)?
            .iter()
            .map(AssemblyRowRouteV1::descriptor)
            .collect::<Vec<_>>();
        if actual != expected || actual != projected {
            return Err(invalid(
                "local route inventory differs from its projection or sealed global plan",
            ));
        }
        let identity =
            local_route_admission_identity(self.identity, ownership.identity, producer, &expected)?;
        Ok(LocalRouteAdmissionV1 {
            plan: self.identity,
            row_ownership: ownership.identity,
            producer,
            route_count: expected.len(),
            identity,
        })
    }

    /// Validate one destination's unordered payload inbox exactly once.
    ///
    /// # Errors
    /// Returns `EQ0806` for missing, duplicate, foreign, wrong-destination, or
    /// payload-mismatched routes.
    pub(super) fn accept_inbox(
        &self,
        destination: PartitionId,
        mut routes: Vec<AssemblyRowRouteV1>,
    ) -> Result<AcceptedAssemblyInbox, Diagnostic> {
        if destination.index() >= self.partition_count.get() {
            return Err(invalid("assembly inbox destination is outside the plan"));
        }
        for route in &routes {
            route.validate_integrity()?;
            if route.descriptor.destination != destination {
                return Err(invalid(format!(
                    "assembly inbox {} received a route for destination {}",
                    destination.index(),
                    route.descriptor.destination.index()
                )));
            }
        }
        routes.sort_by_key(AssemblyRowRouteV1::canonical_key);
        if routes
            .windows(2)
            .any(|pair| pair[0].canonical_key() == pair[1].canonical_key())
        {
            return Err(invalid("assembly inbox contains a duplicate route"));
        }
        let actual = routes
            .iter()
            .map(AssemblyRowRouteV1::descriptor)
            .collect::<Vec<_>>();
        let expected = self
            .descriptors
            .iter()
            .copied()
            .filter(|descriptor| descriptor.destination == destination)
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(invalid(format!(
                "assembly inbox {} does not exactly match its sealed route inventory",
                destination.index()
            )));
        }
        Ok(AcceptedAssemblyInbox {
            plan: self.identity,
            destination,
            routes,
        })
    }
}

/// Opaque proof that one producer validated both collective row ownership and
/// its complete payload-bound local route inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalRouteAdmissionV1 {
    pub(super) plan: DistributedAssemblyPlanIdentityV1,
    row_ownership: AdmittedRowOwnershipIdentityV1,
    pub(super) producer: PartitionId,
    route_count: usize,
    pub(super) identity: LocalRouteAdmissionIdentityV1,
}

impl LocalRouteAdmissionV1 {
    /// Exact fixed-size wire length.
    pub const ENCODED_LEN: usize = 120;

    /// Producer whose local support and route inventory were admitted.
    #[must_use]
    pub const fn producer(self) -> PartitionId {
        self.producer
    }

    /// Fixed-size proof identity.
    #[must_use]
    pub const fn identity(self) -> LocalRouteAdmissionIdentityV1 {
        self.identity
    }

    /// Encode one fixed-size producer admission for collective exchange.
    ///
    /// # Errors
    /// Returns `EQ0806` if a portable index cannot be represented.
    pub fn to_bytes(self) -> Result<[u8; Self::ENCODED_LEN], Diagnostic> {
        let mut bytes = Vec::with_capacity(Self::ENCODED_LEN);
        bytes.extend_from_slice(LOCAL_ROUTE_ADMISSION_WIRE_MAGIC_V1);
        bytes.extend_from_slice(&self.plan.0);
        bytes.extend_from_slice(&self.row_ownership.0);
        push_usize(&mut bytes, self.producer.index())?;
        push_usize(&mut bytes, self.route_count)?;
        bytes.extend_from_slice(&self.identity.0);
        Ok(bytes
            .try_into()
            .expect("local route admission encoding has a fixed-length shape"))
    }

    /// Decode and validate one producer admission against exact plans.
    ///
    /// # Errors
    /// Returns `EQ0806` for malformed bytes or any plan, producer, route-count,
    /// or proof-identity mismatch.
    pub fn from_bytes(
        route_plan: &DistributedAssemblyRoutePlanV1,
        ownership: &AdmittedRowOwnership,
        bytes: &[u8],
    ) -> Result<Self, Diagnostic> {
        let mut reader = WireReader::new(bytes, LOCAL_ROUTE_ADMISSION_WIRE_MAGIC_V1)?;
        let admission = Self {
            plan: DistributedAssemblyPlanIdentityV1(reader.array_32()?),
            row_ownership: AdmittedRowOwnershipIdentityV1(reader.array_32()?),
            producer: PartitionId::new(reader.usize()?),
            route_count: reader.usize()?,
            identity: LocalRouteAdmissionIdentityV1(reader.array_32()?),
        };
        reader.finish()?;
        validate_local_route_admission(route_plan, ownership, admission)?;
        Ok(admission)
    }
}

/// Exact unordered inbox after validation against a sealed plan.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AcceptedAssemblyInbox {
    pub(super) plan: DistributedAssemblyPlanIdentityV1,
    pub(super) destination: PartitionId,
    pub(super) routes: Vec<AssemblyRowRouteV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RouteKey {
    pub(super) target: AssemblyTargetId,
    pub(super) row: DofId,
    pub(super) packet: usize,
}

#[derive(Debug, Clone, Copy)]
struct RouteAddress {
    pub(super) packet: usize,
    pub(super) producer: PartitionId,
    pub(super) target: AssemblyTargetId,
    pub(super) target_size: usize,
    pub(super) row: DofId,
    pub(super) destination: PartitionId,
}

fn validate_descriptor(
    layout: &DistributedMeshLayout,
    target_sizes: &[usize],
    ownership: &AdmittedRowOwnership,
    descriptor: AssemblyRowRouteDescriptorV1,
) -> Result<(), Diagnostic> {
    if descriptor.packet >= layout.cell_count() {
        return Err(invalid("assembly route names a packet outside the mesh"));
    }
    let expected_producer = layout
        .cell_owner(descriptor.packet)
        .ok_or_else(|| invalid("assembly route packet has no cell owner"))?;
    if descriptor.producer != expected_producer {
        return Err(invalid(format!(
            "assembly packet {} route producer {} differs from cell owner {}",
            descriptor.packet,
            descriptor.producer.index(),
            expected_producer.index()
        )));
    }
    let Some(&target_size) = target_sizes.get(descriptor.target.index()) else {
        return Err(invalid("assembly route names a target outside the plan"));
    };
    if descriptor.target_size != target_size || descriptor.row.index() >= target_size {
        return Err(invalid("assembly route target shape or row is invalid"));
    }
    let destination = ownership
        .owner(descriptor.target, descriptor.row)
        .ok_or_else(|| invalid("assembly route row has no admitted owner"))?;
    if descriptor.destination != destination {
        return Err(invalid(format!(
            "assembly route destination {} differs from row owner {}",
            descriptor.destination.index(),
            destination.index()
        )));
    }
    Ok(())
}

fn validate_route_values(
    target_size: usize,
    row: DofId,
    entries: &[(DofId, f64)],
    rhs: f64,
) -> Result<(), Diagnostic> {
    if target_size == 0 || row.index() >= target_size || !rhs.is_finite() {
        return Err(invalid(
            "assembly route has invalid target, row, or right-hand side",
        ));
    }
    let mut previous = None;
    for (column, value) in entries {
        if column.index() >= target_size || !value.is_finite() {
            return Err(invalid("assembly route has an invalid column or value"));
        }
        if previous.is_some_and(|previous| previous >= *column) {
            return Err(invalid(
                "assembly route columns must be strictly increasing",
            ));
        }
        previous = Some(*column);
    }
    Ok(())
}

fn route_identity(
    address: RouteAddress,
    entries: &[(DofId, f64)],
    rhs: f64,
) -> Result<AssemblyRowRouteIdentityV1, Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(ROUTE_IDENTITY_DOMAIN_V1);
    hash_usize(&mut hash, address.packet)?;
    hash_usize(&mut hash, address.producer.index())?;
    hash_usize(&mut hash, address.target.index())?;
    hash_usize(&mut hash, address.target_size)?;
    hash_usize(&mut hash, address.row.index())?;
    hash_usize(&mut hash, address.destination.index())?;
    hash_usize(&mut hash, entries.len())?;
    for (column, value) in entries {
        hash_usize(&mut hash, column.index())?;
        hash_f64(&mut hash, *value)?;
    }
    hash_f64(&mut hash, rhs)?;
    Ok(AssemblyRowRouteIdentityV1(hash.finalize().into()))
}

fn plan_identity(
    layout: &DistributedMeshLayout,
    ownership: AdmittedRowOwnershipIdentityV1,
    target_sizes: &[usize],
    row_owners: &[Vec<PartitionId>],
    descriptors: &[AssemblyRowRouteDescriptorV1],
) -> Result<DistributedAssemblyPlanIdentityV1, Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(PLAN_IDENTITY_DOMAIN_V1);
    hash.update(layout.identity().as_bytes());
    hash.update(ownership.as_bytes());
    hash_usize(&mut hash, layout.cell_count())?;
    for cell in 0..layout.cell_count() {
        hash_usize(
            &mut hash,
            layout
                .cell_owner(cell)
                .ok_or_else(|| invalid("spatial plan cell has no owner"))?
                .index(),
        )?;
    }
    hash_usize(&mut hash, target_sizes.len())?;
    for (size, owners) in target_sizes.iter().zip(row_owners) {
        hash_usize(&mut hash, *size)?;
        hash_usize(&mut hash, owners.len())?;
        for owner in owners {
            hash_usize(&mut hash, owner.index())?;
        }
    }
    hash_usize(&mut hash, descriptors.len())?;
    for descriptor in descriptors {
        hash_usize(&mut hash, descriptor.packet)?;
        hash_usize(&mut hash, descriptor.producer.index())?;
        hash_usize(&mut hash, descriptor.target.index())?;
        hash_usize(&mut hash, descriptor.target_size)?;
        hash_usize(&mut hash, descriptor.row.index())?;
        hash_usize(&mut hash, descriptor.destination.index())?;
        hash_usize(&mut hash, descriptor.entry_count)?;
        hash.update(descriptor.identity.as_bytes());
    }
    Ok(DistributedAssemblyPlanIdentityV1(hash.finalize().into()))
}

fn local_route_admission_identity(
    plan: DistributedAssemblyPlanIdentityV1,
    row_ownership: AdmittedRowOwnershipIdentityV1,
    producer: PartitionId,
    descriptors: &[AssemblyRowRouteDescriptorV1],
) -> Result<LocalRouteAdmissionIdentityV1, Diagnostic> {
    let mut hash = Sha256::new();
    hash.update(LOCAL_ROUTE_ADMISSION_DOMAIN_V1);
    hash.update(plan.as_bytes());
    hash.update(row_ownership.as_bytes());
    hash_usize(&mut hash, producer.index())?;
    hash_usize(&mut hash, descriptors.len())?;
    for descriptor in descriptors {
        if descriptor.producer != producer {
            return Err(invalid(
                "local route admission contains a foreign producer descriptor",
            ));
        }
        hash.update(descriptor.identity.as_bytes());
    }
    Ok(LocalRouteAdmissionIdentityV1(hash.finalize().into()))
}

fn validate_local_route_admission(
    route_plan: &DistributedAssemblyRoutePlanV1,
    ownership: &AdmittedRowOwnership,
    admission: LocalRouteAdmissionV1,
) -> Result<(), Diagnostic> {
    if admission.plan != route_plan.identity
        || admission.row_ownership != ownership.identity
        || route_plan.row_ownership != ownership.identity
        || admission.producer.index() >= route_plan.partition_count.get()
    {
        return Err(invalid(
            "local route admission belongs to another plan, ownership, or producer set",
        ));
    }
    let expected = route_plan
        .descriptors
        .iter()
        .copied()
        .filter(|descriptor| descriptor.producer == admission.producer)
        .collect::<Vec<_>>();
    if admission.route_count != expected.len()
        || admission.identity
            != local_route_admission_identity(
                route_plan.identity,
                ownership.identity,
                admission.producer,
                &expected,
            )?
    {
        return Err(invalid(
            "local route admission count or identity differs from the sealed producer inventory",
        ));
    }
    Ok(())
}

pub(super) fn validate_local_route_admissions(
    route_plan: &DistributedAssemblyRoutePlanV1,
    ownership: &AdmittedRowOwnership,
    mut admissions: Vec<LocalRouteAdmissionV1>,
) -> Result<Vec<LocalRouteAdmissionV1>, Diagnostic> {
    admissions.sort_by_key(|admission| admission.producer.index());
    if admissions.len() != route_plan.partition_count.get() {
        return Err(invalid(format!(
            "distributed assembly has {} local route admissions for {} producers",
            admissions.len(),
            route_plan.partition_count
        )));
    }
    for (producer, admission) in admissions.iter().copied().enumerate() {
        if admission.producer != PartitionId::new(producer) {
            return Err(invalid(
                "local route admissions are missing, duplicate, or misaddressed",
            ));
        }
        validate_local_route_admission(route_plan, ownership, admission)?;
    }
    Ok(admissions)
}

fn decode_route_fields(
    bytes: &[u8],
    target_id: impl FnOnce(usize) -> Result<AssemblyTargetId, Diagnostic>,
) -> Result<AssemblyRowRouteV1, Diagnostic> {
    let mut reader = WireReader::new(bytes, ROUTE_WIRE_MAGIC_V1)?;
    let packet = reader.usize()?;
    let producer = PartitionId::new(reader.usize()?);
    let target = target_id(reader.usize()?)?;
    let target_size = reader.usize()?;
    let row = DofId::new(reader.usize()?);
    let destination = PartitionId::new(reader.usize()?);
    let entry_count = reader.usize()?;
    let required = entry_count
        .checked_mul(16)
        .and_then(|value| value.checked_add(8))
        .ok_or_else(|| invalid("assembly route entry count overflows wire length"))?;
    if reader.remaining() != required {
        return Err(invalid(
            "assembly route wire length does not match entry count",
        ));
    }
    let mut entries = reserve(entry_count, "decoded assembly route entries")?;
    for _ in 0..entry_count {
        entries.push((DofId::new(reader.usize()?), reader.f64()?));
    }
    let rhs = reader.f64()?;
    reader.finish()?;
    AssemblyRowRouteV1::new(
        packet,
        producer,
        target,
        target_size,
        row,
        destination,
        entries,
        rhs,
    )
}
