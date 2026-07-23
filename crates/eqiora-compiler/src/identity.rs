//! Deterministic identities for compiler-owned component elaboration.
//!
//! This module is public only so a later hierarchy lowerer can be developed
//! independently of the legacy source lowerer. It is not re-exported by the
//! `eqiora` facade and does not itself emit graph transactions.

use core::fmt;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, Entity, EntityKind, Id, OntologyId};
use eqiora_schema::Model;
use sha2::{Digest, Sha256};
use ulid::Ulid;

const MAGIC: &[u8; 8] = b"EQIORAEK";
const MODEL_VIEW_MAGIC: &[u8; 8] = b"EQIORAMV";
const CANONICAL_VERSION: u16 = 1;
const NAMESPACE_FIELD: u8 = 1;
const INSTANCE_PATH_FIELD: u8 = 2;
const DECLARATION_PATH_FIELD: u8 = 3;
const SUBJECT_FIELD: u8 = 4;

const ENTITY_SUBJECT: u8 = 0;
const GENERATED_SUBJECT: u8 = 1;
const BOUNDARY_FAMILY_ENTITY_SUBJECT: u8 = 2;
const BOUNDARY_FAMILY_GENERATED_SUBJECT: u8 = 3;
const RELATION_ACTIVATION_ROLE: u16 = 1;
const ANONYMOUS_CONNECTION_ROLE: u16 = 2;

/// Resource limits for deterministic elaboration identity construction.
///
/// Limits are checked before allocation or canonical encoding. The default
/// values are intentionally finite even though current source models are much
/// smaller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElaborationIdentityLimits {
    /// Maximum fully qualified namespace depth.
    pub max_namespace_segments: usize,
    /// Maximum component instance nesting depth.
    pub max_instance_depth: usize,
    /// Maximum definition-relative declaration depth.
    pub max_declaration_depth: usize,
    /// Maximum UTF-8 bytes in one path segment.
    pub max_segment_bytes: usize,
    /// Maximum UTF-8 bytes summed across one path.
    pub max_path_bytes: usize,
    /// Maximum ports in one anonymous connection identity.
    pub max_anonymous_connection_members: usize,
    /// Maximum bytes in one canonical elaboration key.
    pub max_canonical_key_bytes: usize,
    /// Maximum distinct identities in one staging allocator.
    pub max_staged_identities: usize,
}

impl Default for ElaborationIdentityLimits {
    fn default() -> Self {
        Self {
            max_namespace_segments: 32,
            max_instance_depth: 64,
            max_declaration_depth: 64,
            max_segment_bytes: 1_024,
            max_path_bytes: 16 * 1_024,
            max_anonymous_connection_members: 4_096,
            max_canonical_key_bytes: 64 * 1_024,
            max_staged_identities: 1_000_000,
        }
    }
}

/// A fully qualified component-definition namespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdentityNamespace(Path);

impl IdentityNamespace {
    /// Construct a non-empty namespace from source-declared UTF-8 segments.
    pub fn new<I, S>(segments: I) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::with_limits(segments, ElaborationIdentityLimits::default())
    }

    /// Construct using explicit compilation limits.
    pub fn with_limits<I, S>(
        segments: I,
        limits: ElaborationIdentityLimits,
    ) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        collect_path(
            segments,
            limits.max_namespace_segments,
            limits,
            "identity namespace",
        )
        .map(Self)
    }

    /// Namespace segments in lexical order.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0.segments
    }
}

/// A typed lexical path from the root component instance to one instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstancePath(Path);

impl InstancePath {
    /// Construct a non-empty instance path, including the root instance.
    pub fn new<I, S>(segments: I) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::with_limits(segments, ElaborationIdentityLimits::default())
    }

    /// Construct using explicit compilation limits.
    pub fn with_limits<I, S>(
        segments: I,
        limits: ElaborationIdentityLimits,
    ) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        collect_path(segments, limits.max_instance_depth, limits, "instance path").map(Self)
    }

    /// Instance path segments from root to leaf.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0.segments
    }
}

/// A declaration path relative to its component definition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclarationPath(Path);

impl DeclarationPath {
    /// Construct a non-empty definition-relative declaration path.
    pub fn new<I, S>(segments: I) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::with_limits(segments, ElaborationIdentityLimits::default())
    }

    /// Construct using explicit compilation limits.
    pub fn with_limits<I, S>(
        segments: I,
        limits: ElaborationIdentityLimits,
    ) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        collect_path(
            segments,
            limits.max_declaration_depth,
            limits,
            "declaration path",
        )
        .map(Self)
    }

    /// Declaration path segments from outer to inner declaration.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0.segments
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Path {
    segments: Vec<String>,
    byte_len: usize,
}

/// Reserved compiler-generated identities.
///
/// User source cannot supply an arbitrary role string. New roles require an
/// append-only canonical code in this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum GeneratedRole {
    /// The Activation owned by an elaborated Relation declaration.
    RelationActivation,
}

impl GeneratedRole {
    const fn entity_kind(self) -> EntityKind {
        match self {
            Self::RelationActivation => EntityKind::Activation,
        }
    }

    const fn canonical_code(self) -> u16 {
        match self {
            Self::RelationActivation => RELATION_ACTIVATION_ROLE,
        }
    }
}

/// The complete compiler-owned key for one elaborated kernel entity.
#[derive(Debug, Clone)]
pub struct ElaborationKey {
    namespace: IdentityNamespace,
    instance_path: InstancePath,
    declaration_path: DeclarationPath,
    subject: Subject,
    limits: ElaborationIdentityLimits,
}

impl PartialEq for ElaborationKey {
    fn eq(&self, other: &Self) -> bool {
        self.namespace == other.namespace
            && self.instance_path == other.instance_path
            && self.declaration_path == other.declaration_path
            && self.subject == other.subject
    }
}

impl Eq for ElaborationKey {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Subject {
    Entity(EntityKind),
    Generated(GeneratedRole),
    AnonymousConnection(Box<[FullElaborationIdentity]>),
    BoundaryFamilyEntity {
        kind: EntityKind,
        boundary: FullElaborationIdentity,
    },
    BoundaryFamilyGenerated {
        role: GeneratedRole,
        boundary: FullElaborationIdentity,
    },
}

impl ElaborationKey {
    /// Identify a source declaration of the given kernel entity kind.
    pub fn entity(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        kind: EntityKind,
    ) -> Result<Self, Diagnostic> {
        Self::entity_with_limits(
            namespace,
            instance_path,
            declaration_path,
            kind,
            ElaborationIdentityLimits::default(),
        )
    }

    /// Identify a source declaration using explicit compilation limits.
    pub fn entity_with_limits(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        kind: EntityKind,
        limits: ElaborationIdentityLimits,
    ) -> Result<Self, Diagnostic> {
        entity_code(kind)?;
        let key = Self {
            namespace,
            instance_path,
            declaration_path,
            subject: Subject::Entity(kind),
            limits,
        };
        key.validate_encoded_size()?;
        Ok(key)
    }

    /// Identify a compiler-generated entity at a deterministic declaration
    /// path and reserved role.
    pub fn generated(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        role: GeneratedRole,
    ) -> Result<Self, Diagnostic> {
        Self::generated_with_limits(
            namespace,
            instance_path,
            declaration_path,
            role,
            ElaborationIdentityLimits::default(),
        )
    }

    /// Identify a generated entity using explicit compilation limits.
    pub fn generated_with_limits(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        role: GeneratedRole,
        limits: ElaborationIdentityLimits,
    ) -> Result<Self, Diagnostic> {
        let key = Self {
            namespace,
            instance_path,
            declaration_path,
            subject: Subject::Generated(role),
            limits,
        };
        key.validate_encoded_size()?;
        Ok(key)
    }

    /// Identify one ordinary Port or Relation expanded from a complete-exterior
    /// family declaration for an exact Boundary occurrence.
    ///
    /// The Boundary full identity is a dedicated canonical discriminator. It
    /// is never encoded as a declaration-path segment, source name, or member
    /// ordinal.
    pub fn boundary_family_entity(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        kind: EntityKind,
        boundary: FullElaborationIdentity,
    ) -> Result<Self, Diagnostic> {
        Self::boundary_family_entity_with_limits(
            namespace,
            instance_path,
            declaration_path,
            kind,
            boundary,
            ElaborationIdentityLimits::default(),
        )
    }

    /// Identify one complete-exterior family entity using explicit limits.
    pub fn boundary_family_entity_with_limits(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        kind: EntityKind,
        boundary: FullElaborationIdentity,
        limits: ElaborationIdentityLimits,
    ) -> Result<Self, Diagnostic> {
        if !matches!(kind, EntityKind::Port | EntityKind::Relation) {
            return Err(identity_error(
                "complete-exterior families can generate only Port or Relation entities",
            ));
        }
        let key = Self {
            namespace,
            instance_path,
            declaration_path,
            subject: Subject::BoundaryFamilyEntity { kind, boundary },
            limits,
        };
        key.validate_encoded_size()?;
        Ok(key)
    }

    /// Identify one compiler-generated entity owned by a complete-exterior
    /// family member, such as the Activation of a generated Relation.
    pub fn boundary_family_generated(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        role: GeneratedRole,
        boundary: FullElaborationIdentity,
    ) -> Result<Self, Diagnostic> {
        Self::boundary_family_generated_with_limits(
            namespace,
            instance_path,
            declaration_path,
            role,
            boundary,
            ElaborationIdentityLimits::default(),
        )
    }

    /// Identify one generated complete-exterior family member using explicit
    /// limits.
    pub fn boundary_family_generated_with_limits(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        role: GeneratedRole,
        boundary: FullElaborationIdentity,
        limits: ElaborationIdentityLimits,
    ) -> Result<Self, Diagnostic> {
        let key = Self {
            namespace,
            instance_path,
            declaration_path,
            subject: Subject::BoundaryFamilyGenerated { role, boundary },
            limits,
        };
        key.validate_encoded_size()?;
        Ok(key)
    }

    /// Identify an anonymous Connection by the exact full identities of its
    /// members. Member order is immaterial; duplicates and fewer than two
    /// members fail closed.
    pub fn anonymous_connection(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        members: impl IntoIterator<Item = FullElaborationIdentity>,
    ) -> Result<Self, Diagnostic> {
        Self::anonymous_connection_with_limits(
            namespace,
            instance_path,
            declaration_path,
            members,
            ElaborationIdentityLimits::default(),
        )
    }

    /// Identify an anonymous Connection using explicit compilation limits.
    pub fn anonymous_connection_with_limits(
        namespace: IdentityNamespace,
        instance_path: InstancePath,
        declaration_path: DeclarationPath,
        members: impl IntoIterator<Item = FullElaborationIdentity>,
        limits: ElaborationIdentityLimits,
    ) -> Result<Self, Diagnostic> {
        let mut collected_members = Vec::new();
        for member in members {
            if collected_members.len() >= limits.max_anonymous_connection_members {
                return Err(identity_error(format!(
                    "anonymous connection exceeds the {} member limit",
                    limits.max_anonymous_connection_members
                )));
            }
            collected_members
                .try_reserve(1)
                .map_err(|_| identity_error("cannot reserve anonymous connection identity"))?;
            collected_members.push(member);
        }
        if collected_members.len() < 2 {
            return Err(identity_error(
                "anonymous connection identity requires at least two members",
            ));
        }
        collected_members.sort_unstable();
        if collected_members.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(identity_error(
                "anonymous connection identity repeats a member",
            ));
        }
        let key = Self {
            namespace,
            instance_path,
            declaration_path,
            subject: Subject::AnonymousConnection(collected_members.into_boxed_slice()),
            limits,
        };
        key.validate_encoded_size()?;
        Ok(key)
    }

    /// Kernel entity kind produced by this key.
    #[must_use]
    pub fn entity_kind(&self) -> EntityKind {
        match self.subject {
            Subject::Entity(kind) => kind,
            Subject::Generated(role) => role.entity_kind(),
            Subject::AnonymousConnection(_) => EntityKind::Connection,
            Subject::BoundaryFamilyEntity { kind, .. } => kind,
            Subject::BoundaryFamilyGenerated { role, .. } => role.entity_kind(),
        }
    }

    /// Versioned, length-delimited canonical bytes for this key.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Diagnostic> {
        let namespace_len = encoded_path_len(&self.namespace.0)?;
        let instance_len = encoded_path_len(&self.instance_path.0)?;
        let declaration_len = encoded_path_len(&self.declaration_path.0)?;
        let subject_len = self.subject_payload_len()?;
        let total =
            canonical_total_len([namespace_len, instance_len, declaration_len, subject_len])?;
        if total > self.limits.max_canonical_key_bytes {
            return Err(identity_error(format!(
                "canonical elaboration key requires {total} bytes, exceeding the {} byte limit",
                self.limits.max_canonical_key_bytes
            )));
        }

        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| identity_error("cannot reserve canonical elaboration key"))?;
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&CANONICAL_VERSION.to_be_bytes());
        write_path_field(&mut bytes, NAMESPACE_FIELD, &self.namespace.0)?;
        write_path_field(&mut bytes, INSTANCE_PATH_FIELD, &self.instance_path.0)?;
        write_path_field(&mut bytes, DECLARATION_PATH_FIELD, &self.declaration_path.0)?;
        self.write_subject_field(&mut bytes, subject_len)?;
        debug_assert_eq!(bytes.len(), total);
        Ok(bytes)
    }

    /// Full SHA-256 identity over [`Self::canonical_bytes`].
    pub fn full_identity(&self) -> Result<FullElaborationIdentity, Diagnostic> {
        let canonical = self.canonical_bytes()?;
        Ok(FullElaborationIdentity(Sha256::digest(canonical).into()))
    }

    fn validate_encoded_size(&self) -> Result<(), Diagnostic> {
        validate_path_against_limits(
            &self.namespace.0,
            self.limits.max_namespace_segments,
            self.limits,
            "identity namespace",
        )?;
        validate_path_against_limits(
            &self.instance_path.0,
            self.limits.max_instance_depth,
            self.limits,
            "instance path",
        )?;
        validate_path_against_limits(
            &self.declaration_path.0,
            self.limits.max_declaration_depth,
            self.limits,
            "declaration path",
        )?;
        self.canonical_bytes().map(|_| ())
    }

    fn subject_payload_len(&self) -> Result<usize, Diagnostic> {
        match &self.subject {
            Subject::Entity(kind) => {
                entity_code(*kind)?;
                Ok(3)
            }
            Subject::Generated(_) => Ok(3),
            Subject::AnonymousConnection(members) => checked_add(
                7,
                checked_mul(members.len(), 32, "anonymous member identity bytes")?,
                "anonymous connection subject bytes",
            ),
            Subject::BoundaryFamilyEntity { kind, .. } => {
                entity_code(*kind)?;
                Ok(35)
            }
            Subject::BoundaryFamilyGenerated { .. } => Ok(35),
        }
    }

    fn write_subject_field(
        &self,
        bytes: &mut Vec<u8>,
        payload_len: usize,
    ) -> Result<(), Diagnostic> {
        bytes.push(SUBJECT_FIELD);
        bytes.extend_from_slice(&as_u32(payload_len, "subject byte length")?.to_be_bytes());
        match &self.subject {
            Subject::Entity(kind) => {
                bytes.push(ENTITY_SUBJECT);
                bytes.extend_from_slice(&entity_code(*kind)?.to_be_bytes());
            }
            Subject::Generated(role) => {
                bytes.push(GENERATED_SUBJECT);
                bytes.extend_from_slice(&role.canonical_code().to_be_bytes());
            }
            Subject::AnonymousConnection(members) => {
                bytes.push(GENERATED_SUBJECT);
                bytes.extend_from_slice(&ANONYMOUS_CONNECTION_ROLE.to_be_bytes());
                bytes.extend_from_slice(
                    &as_u32(members.len(), "anonymous member count")?.to_be_bytes(),
                );
                for member in members {
                    bytes.extend_from_slice(member.as_bytes());
                }
            }
            Subject::BoundaryFamilyEntity { kind, boundary } => {
                bytes.push(BOUNDARY_FAMILY_ENTITY_SUBJECT);
                bytes.extend_from_slice(&entity_code(*kind)?.to_be_bytes());
                bytes.extend_from_slice(boundary.as_bytes());
            }
            Subject::BoundaryFamilyGenerated { role, boundary } => {
                bytes.push(BOUNDARY_FAMILY_GENERATED_SUBJECT);
                bytes.extend_from_slice(&role.canonical_code().to_be_bytes());
                bytes.extend_from_slice(boundary.as_bytes());
            }
        }
        Ok(())
    }
}

/// Domain-separated identity of the one root Model ontology view produced by
/// an elaboration.
///
/// A Model view is not a kernel entity and therefore never receives a
/// fabricated [`EntityKind`]. Its canonical domain is separate from
/// [`ElaborationKey`] even when namespace and instance path are identical.
#[derive(Debug, Clone)]
pub struct ModelViewKey {
    namespace: IdentityNamespace,
    root_instance_path: InstancePath,
    limits: ElaborationIdentityLimits,
}

impl PartialEq for ModelViewKey {
    fn eq(&self, other: &Self) -> bool {
        self.namespace == other.namespace && self.root_instance_path == other.root_instance_path
    }
}

impl Eq for ModelViewKey {}

impl ModelViewKey {
    /// Construct the root Model view key with default limits.
    pub fn new(
        namespace: IdentityNamespace,
        root_instance_path: InstancePath,
    ) -> Result<Self, Diagnostic> {
        Self::with_limits(
            namespace,
            root_instance_path,
            ElaborationIdentityLimits::default(),
        )
    }

    /// Construct the root Model view key with explicit compilation limits.
    pub fn with_limits(
        namespace: IdentityNamespace,
        root_instance_path: InstancePath,
        limits: ElaborationIdentityLimits,
    ) -> Result<Self, Diagnostic> {
        validate_path_against_limits(
            &namespace.0,
            limits.max_namespace_segments,
            limits,
            "identity namespace",
        )?;
        validate_path_against_limits(
            &root_instance_path.0,
            limits.max_instance_depth,
            limits,
            "root instance path",
        )?;
        let key = Self {
            namespace,
            root_instance_path,
            limits,
        };
        key.canonical_bytes()?;
        Ok(key)
    }

    /// Versioned, length-delimited canonical bytes in the Model-view domain.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Diagnostic> {
        let namespace_len = encoded_path_len(&self.namespace.0)?;
        let instance_len = encoded_path_len(&self.root_instance_path.0)?;
        let total = canonical_total_len_for(MODEL_VIEW_MAGIC.len(), [namespace_len, instance_len])?;
        if total > self.limits.max_canonical_key_bytes {
            return Err(identity_error(format!(
                "canonical Model view key requires {total} bytes, exceeding the {} byte limit",
                self.limits.max_canonical_key_bytes
            )));
        }

        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| identity_error("cannot reserve canonical Model view key"))?;
        bytes.extend_from_slice(MODEL_VIEW_MAGIC);
        bytes.extend_from_slice(&CANONICAL_VERSION.to_be_bytes());
        write_path_field(&mut bytes, NAMESPACE_FIELD, &self.namespace.0)?;
        write_path_field(&mut bytes, INSTANCE_PATH_FIELD, &self.root_instance_path.0)?;
        debug_assert_eq!(bytes.len(), total);
        Ok(bytes)
    }

    /// Full SHA-256 identity over [`Self::canonical_bytes`].
    pub fn full_identity(&self) -> Result<FullElaborationIdentity, Diagnostic> {
        let canonical = self.canonical_bytes()?;
        Ok(FullElaborationIdentity(Sha256::digest(canonical).into()))
    }
}

/// Full content identity retained alongside every projected graph identifier.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FullElaborationIdentity([u8; 32]);

impl FullElaborationIdentity {
    /// Reconstruct from an exact SHA-256 digest, primarily for decoded
    /// provenance and structural identity composition.
    #[must_use]
    pub const fn from_sha256(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Exact 256-bit digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for FullElaborationIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FullElaborationIdentity({self})")
    }
}

impl fmt::Display for FullElaborationIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// A typed projected ID that always retains its full elaboration identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedId<E: Entity> {
    id: Id<E>,
    full_identity: FullElaborationIdentity,
}

/// A projected Model ontology ID retaining its full elaboration identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedModelId {
    id: OntologyId<Model>,
    full_identity: FullElaborationIdentity,
}

impl ProjectedModelId {
    /// Model ontology identifier obtained after staging is sealed.
    #[must_use]
    pub const fn id(self) -> OntologyId<Model> {
        self.id
    }

    /// Full SHA-256 identity retained for provenance and replay.
    #[must_use]
    pub const fn full_identity(self) -> FullElaborationIdentity {
        self.full_identity
    }
}

impl<E: Entity> ProjectedId<E> {
    /// Graph identifier obtained from the collision-checked projection.
    #[must_use]
    pub const fn id(self) -> Id<E> {
        self.id
    }

    /// Full SHA-256 elaboration identity retained for provenance and replay.
    #[must_use]
    pub const fn full_identity(self) -> FullElaborationIdentity {
        self.full_identity
    }
}

/// Projection of a full SHA-256 identity into the 128 bits carried by ULID.
///
/// The trait is public only for deterministic compiler tests and future
/// compiler-internal injection. Production code should use
/// [`Sha256PrefixProjector`].
pub trait ShortIdProjector {
    /// Produce canonical big-endian ULID bytes from the full digest.
    fn project(&self, identity: FullElaborationIdentity) -> [u8; 16];
}

/// Production projection using the first 128 SHA-256 bits.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sha256PrefixProjector;

impl ShortIdProjector for Sha256PrefixProjector {
    fn project(&self, identity: FullElaborationIdentity) -> [u8; 16] {
        let mut projected = [0_u8; 16];
        projected.copy_from_slice(&identity.as_bytes()[..16]);
        projected
    }
}

/// Collision-checking staging area for all IDs in one atomic elaboration.
///
/// No typed graph ID is exposed until [`Self::finish`] consumes the allocator.
/// A hierarchy lowerer can therefore complete every projection check before
/// it constructs a graph transaction.
#[derive(Debug)]
pub struct StagingIdAllocator<P = Sha256PrefixProjector> {
    projector: P,
    limits: ElaborationIdentityLimits,
    by_identity: Vec<StagedIdentity>,
    by_projection: Vec<ProjectionIndex>,
    model_view: Option<StagedModelView>,
}

#[derive(Debug)]
struct StagedIdentity {
    identity: FullElaborationIdentity,
    kind: EntityKind,
    projected: [u8; 16],
    canonical_key: Box<[u8]>,
}

#[derive(Debug, Clone, Copy)]
struct ProjectionIndex {
    kind: EntityKind,
    projected: [u8; 16],
    identity: FullElaborationIdentity,
}

#[derive(Debug)]
struct StagedModelView {
    identity: FullElaborationIdentity,
    projected: [u8; 16],
    canonical_key: Box<[u8]>,
}

impl StagingIdAllocator<Sha256PrefixProjector> {
    /// Create an empty production allocator with default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::with_projector_and_limits(Sha256PrefixProjector, ElaborationIdentityLimits::default())
    }
}

impl Default for StagingIdAllocator<Sha256PrefixProjector> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: ShortIdProjector> StagingIdAllocator<P> {
    /// Create an allocator with an injected projector and explicit limits.
    /// This supports forced-collision tests without weakening production IDs.
    #[must_use]
    pub fn with_projector_and_limits(projector: P, limits: ElaborationIdentityLimits) -> Self {
        Self {
            projector,
            limits,
            by_identity: Vec::new(),
            by_projection: Vec::new(),
            model_view: None,
        }
    }

    /// Stage one key and return its full identity. Re-staging the same exact
    /// key is idempotent. A full-digest or projected-ID collision fails before
    /// either index is mutated.
    pub fn stage(&mut self, key: &ElaborationKey) -> Result<FullElaborationIdentity, Diagnostic> {
        let canonical_key = key.canonical_bytes()?;
        let identity = FullElaborationIdentity(Sha256::digest(&canonical_key).into());
        let kind = key.entity_kind();
        if let Ok(index) = self
            .by_identity
            .binary_search_by_key(&identity, |entry| entry.identity)
        {
            let existing = &self.by_identity[index];
            if existing.canonical_key.as_ref() != canonical_key.as_slice() || existing.kind != kind
            {
                return Err(identity_error(
                    "distinct canonical elaboration keys share one full SHA-256 identity",
                ));
            }
            return Ok(identity);
        }

        let staged_count = self
            .by_identity
            .len()
            .checked_add(usize::from(self.model_view.is_some()))
            .ok_or_else(|| identity_error("staged identity count overflows usize"))?;
        if staged_count >= self.limits.max_staged_identities {
            return Err(identity_error(format!(
                "elaboration exceeds the {} staged identity limit",
                self.limits.max_staged_identities
            )));
        }

        let projected = self.projector.project(identity);
        let projection_search = self
            .by_projection
            .binary_search_by(|entry| (entry.kind, entry.projected).cmp(&(kind, projected)));
        if let Ok(index) = projection_search {
            let existing = self.by_projection[index].identity;
            if existing != identity {
                return Err(identity_error(format!(
                    "projected {:?} identifier collision between full identities {existing} and {identity}",
                    kind
                )));
            }
        }

        self.by_identity
            .try_reserve(1)
            .map_err(|_| identity_error("cannot reserve staged elaboration identity"))?;
        self.by_projection
            .try_reserve(1)
            .map_err(|_| identity_error("cannot reserve projected identity index"))?;

        let identity_index = self
            .by_identity
            .binary_search_by_key(&identity, |entry| entry.identity)
            .unwrap_or_else(|index| index);
        self.by_identity.insert(
            identity_index,
            StagedIdentity {
                identity,
                kind,
                projected,
                canonical_key: canonical_key.into_boxed_slice(),
            },
        );
        let projection_index = projection_search.unwrap_or_else(|index| index);
        self.by_projection.insert(
            projection_index,
            ProjectionIndex {
                kind,
                projected,
                identity,
            },
        );
        Ok(identity)
    }

    /// Stage the one root Model ontology view. Re-staging the exact same key is
    /// idempotent; attempting to stage a second root fails before replacement.
    pub fn stage_model_view(
        &mut self,
        key: &ModelViewKey,
    ) -> Result<FullElaborationIdentity, Diagnostic> {
        let canonical_key = key.canonical_bytes()?;
        let identity = FullElaborationIdentity(Sha256::digest(&canonical_key).into());
        if let Some(existing) = &self.model_view {
            if existing.identity == identity
                && existing.canonical_key.as_ref() == canonical_key.as_slice()
            {
                return Ok(identity);
            }
            if existing.identity == identity {
                return Err(identity_error(
                    "distinct canonical Model view keys share one full SHA-256 identity",
                ));
            }
            return Err(identity_error(
                "one elaboration cannot stage more than one root Model view",
            ));
        }
        if self.by_identity.len() >= self.limits.max_staged_identities {
            return Err(identity_error(format!(
                "elaboration exceeds the {} staged identity limit",
                self.limits.max_staged_identities
            )));
        }

        self.model_view = Some(StagedModelView {
            identity,
            projected: self.projector.project(identity),
            canonical_key: canonical_key.into_boxed_slice(),
        });
        Ok(identity)
    }

    /// Seal all staged identities. This is the first object that can expose a
    /// typed graph ID to transaction construction.
    #[must_use]
    pub fn finish(self) -> StagedIdentities {
        StagedIdentities {
            by_identity: self.by_identity.into_boxed_slice(),
            model_view: self.model_view,
        }
    }
}

/// Immutable identities whose full and projected collision checks completed.
#[derive(Debug)]
pub struct StagedIdentities {
    by_identity: Box<[StagedIdentity]>,
    model_view: Option<StagedModelView>,
}

impl StagedIdentities {
    /// Number of distinct staged identities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_identity.len() + usize::from(self.model_view.is_some())
    }

    /// Whether no identities were staged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_identity.is_empty() && self.model_view.is_none()
    }

    /// Resolve a full identity into its checked typed graph ID.
    pub fn resolve<E: Entity>(
        &self,
        identity: FullElaborationIdentity,
    ) -> Result<ProjectedId<E>, Diagnostic> {
        let index = self
            .by_identity
            .binary_search_by_key(&identity, |entry| entry.identity)
            .map_err(|_| {
                identity_error(format!("elaboration identity {identity} was not staged"))
            })?;
        let entry = &self.by_identity[index];
        if entry.kind != E::KIND {
            return Err(identity_error(format!(
                "elaboration identity {identity} has kind {:?}, not {:?}",
                entry.kind,
                E::KIND
            )));
        }
        let value = u128::from_be_bytes(entry.projected);
        Ok(ProjectedId {
            id: Id::from_ulid(Ulid::from(value)),
            full_identity: identity,
        })
    }

    /// Resolve the staged root Model view into its checked ontology ID.
    pub fn resolve_model_view(
        &self,
        identity: FullElaborationIdentity,
    ) -> Result<ProjectedModelId, Diagnostic> {
        let staged = self
            .model_view
            .as_ref()
            .ok_or_else(|| identity_error("root Model view was not staged"))?;
        if staged.identity != identity {
            return Err(identity_error(format!(
                "Model view identity {identity} was not staged"
            )));
        }
        Ok(ProjectedModelId {
            id: OntologyId::from_ulid(Ulid::from(u128::from_be_bytes(staged.projected))),
            full_identity: identity,
        })
    }
}

fn collect_path<I, S>(
    segments: I,
    max_segments: usize,
    limits: ElaborationIdentityLimits,
    label: &'static str,
) -> Result<Path, Diagnostic>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut collected = Vec::new();
    let mut byte_len = 0_usize;
    for segment in segments {
        if collected.len() >= max_segments || collected.len() >= usize::from(u16::MAX) {
            return Err(identity_error(format!(
                "{label} exceeds the {max_segments} segment limit"
            )));
        }
        let segment = segment.into();
        if segment.is_empty() {
            return Err(identity_error(format!("{label} contains an empty segment")));
        }
        if segment.len() > limits.max_segment_bytes {
            return Err(identity_error(format!(
                "{label} segment requires {} bytes, exceeding the {} byte limit",
                segment.len(),
                limits.max_segment_bytes
            )));
        }
        byte_len = checked_add(byte_len, segment.len(), "path segment bytes")?;
        if byte_len > limits.max_path_bytes {
            return Err(identity_error(format!(
                "{label} exceeds the {} total byte limit",
                limits.max_path_bytes
            )));
        }
        collected
            .try_reserve(1)
            .map_err(|_| identity_error(format!("cannot reserve {label}")))?;
        collected.push(segment);
    }
    if collected.is_empty() {
        return Err(identity_error(format!("{label} must not be empty")));
    }
    Ok(Path {
        segments: collected,
        byte_len,
    })
}

fn encoded_path_len(path: &Path) -> Result<usize, Diagnostic> {
    let prefix_bytes = checked_mul(path.segments.len(), 4, "path length prefixes")?;
    checked_add(
        2,
        checked_add(prefix_bytes, path.byte_len, "encoded path bytes")?,
        "encoded path bytes",
    )
}

fn validate_path_against_limits(
    path: &Path,
    max_segments: usize,
    limits: ElaborationIdentityLimits,
    label: &'static str,
) -> Result<(), Diagnostic> {
    if path.segments.len() > max_segments {
        return Err(identity_error(format!(
            "{label} exceeds the {max_segments} segment limit"
        )));
    }
    if path.byte_len > limits.max_path_bytes {
        return Err(identity_error(format!(
            "{label} exceeds the {} total byte limit",
            limits.max_path_bytes
        )));
    }
    if let Some(segment) = path
        .segments
        .iter()
        .find(|segment| segment.len() > limits.max_segment_bytes)
    {
        return Err(identity_error(format!(
            "{label} segment requires {} bytes, exceeding the {} byte limit",
            segment.len(),
            limits.max_segment_bytes
        )));
    }
    Ok(())
}

fn canonical_total_len(field_lengths: [usize; 4]) -> Result<usize, Diagnostic> {
    canonical_total_len_for(MAGIC.len(), field_lengths)
}

fn canonical_total_len_for<const N: usize>(
    magic_len: usize,
    field_lengths: [usize; N],
) -> Result<usize, Diagnostic> {
    let mut total = checked_add(magic_len, 2, "canonical header bytes")?;
    for length in field_lengths {
        total = checked_add(total, 5, "canonical field header bytes")?;
        total = checked_add(total, length, "canonical field payload bytes")?;
    }
    Ok(total)
}

fn write_path_field(bytes: &mut Vec<u8>, tag: u8, path: &Path) -> Result<(), Diagnostic> {
    let payload_len = encoded_path_len(path)?;
    bytes.push(tag);
    bytes.extend_from_slice(&as_u32(payload_len, "path payload length")?.to_be_bytes());
    bytes.extend_from_slice(&as_u16(path.segments.len(), "path segment count")?.to_be_bytes());
    for segment in &path.segments {
        bytes.extend_from_slice(&as_u32(segment.len(), "path segment length")?.to_be_bytes());
        bytes.extend_from_slice(segment.as_bytes());
    }
    Ok(())
}

fn entity_code(kind: EntityKind) -> Result<u16, Diagnostic> {
    let code = match kind {
        EntityKind::Domain => 1,
        EntityKind::Representation => 2,
        EntityKind::Field => 3,
        EntityKind::Parameter => 4,
        EntityKind::Port => 5,
        EntityKind::Relation => 6,
        EntityKind::Activation => 7,
        EntityKind::Connection => 8,
        EntityKind::ClockDomain => 9,
        EntityKind::Space => 10,
        EntityKind::Discretization => 11,
        EntityKind::SolverPlan => 12,
        EntityKind::Partition => 13,
        EntityKind::Target => 14,
        EntityKind::ExecutionSchedule => 15,
        EntityKind::Experiment => 16,
        EntityKind::Observation => 17,
        EntityKind::Dataset => 18,
        EntityKind::Run => 19,
        EntityKind::Artifact => 20,
        EntityKind::ValidityDomain => 21,
        EntityKind::Evidence => 22,
        EntityKind::Transaction => 23,
        EntityKind::Actor => 24,
        EntityKind::Action => 25,
        EntityKind::Review => 26,
        EntityKind::Approval => 27,
        EntityKind::Policy => 28,
        _ => {
            return Err(identity_error(
                "entity kind has no canonical elaboration identity code",
            ));
        }
    };
    Ok(code)
}

fn as_u16(value: usize, label: &'static str) -> Result<u16, Diagnostic> {
    u16::try_from(value).map_err(|_| identity_error(format!("{label} exceeds u16")))
}

fn as_u32(value: usize, label: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| identity_error(format!("{label} exceeds u32")))
}

fn checked_add(left: usize, right: usize, label: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| identity_error(format!("{label} overflows usize")))
}

fn checked_mul(left: usize, right: usize, label: &'static str) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| identity_error(format!("{label} overflows usize")))
}

fn identity_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_LOWERING_ERROR, message)
}

#[cfg(test)]
mod tests {
    use eqiora_core::entity::kinds;

    use super::*;

    fn namespace() -> IdentityNamespace {
        IdentityNamespace::new(["org", "components", "Resistor"]).unwrap()
    }

    fn instance(name: &str) -> InstancePath {
        InstancePath::new(["plant", name]).unwrap()
    }

    fn declaration(name: &str) -> DeclarationPath {
        DeclarationPath::new(["private", name]).unwrap()
    }

    fn entity_key(instance_name: &str, declaration_name: &str, kind: EntityKind) -> ElaborationKey {
        ElaborationKey::entity(
            namespace(),
            instance(instance_name),
            declaration(declaration_name),
            kind,
        )
        .unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn canonical_encoding_and_full_digest_are_golden() {
        let key = entity_key("r1", "voltage", EntityKind::Field);

        assert_eq!(
            hex(&key.canonical_bytes().unwrap()),
            concat!(
                "4551494f5241454b0001",
                "01000000230003000000036f72670000000a636f6d706f6e656e7473000000085265736973746f72",
                "0200000011000200000005706c616e74000000027231",
                "03000000180002000000077072697661746500000007766f6c74616765",
                "0400000003000003"
            )
        );
        assert_eq!(
            key.full_identity().unwrap().to_string(),
            "990233f65afc07e15fa2fb899081e1d19385d4764a09ae61d49858ba391114cb"
        );
    }

    #[test]
    fn namespace_instance_declaration_and_kind_are_separate_domains() {
        let base = entity_key("r1", "voltage", EntityKind::Field)
            .full_identity()
            .unwrap();
        let other_namespace = ElaborationKey::entity(
            IdentityNamespace::new(["org", "other", "Resistor"]).unwrap(),
            instance("r1"),
            declaration("voltage"),
            EntityKind::Field,
        )
        .unwrap()
        .full_identity()
        .unwrap();
        let other_instance = entity_key("r2", "voltage", EntityKind::Field)
            .full_identity()
            .unwrap();
        let other_declaration = entity_key("r1", "current", EntityKind::Field)
            .full_identity()
            .unwrap();
        let other_kind = entity_key("r1", "voltage", EntityKind::Parameter)
            .full_identity()
            .unwrap();

        assert_ne!(base, other_namespace);
        assert_ne!(base, other_instance);
        assert_ne!(base, other_declaration);
        assert_ne!(base, other_kind);
    }

    #[test]
    fn generated_role_is_distinct_from_user_entity_of_same_kind() {
        let entity = ElaborationKey::entity(
            namespace(),
            instance("r1"),
            declaration("law"),
            EntityKind::Activation,
        )
        .unwrap();
        let generated = ElaborationKey::generated(
            namespace(),
            instance("r1"),
            declaration("law"),
            GeneratedRole::RelationActivation,
        )
        .unwrap();

        assert_eq!(entity.entity_kind(), generated.entity_kind());
        assert_ne!(
            entity.full_identity().unwrap(),
            generated.full_identity().unwrap()
        );
    }

    #[test]
    fn complete_exterior_family_identity_uses_the_exact_boundary_discriminator() {
        let first_boundary = FullElaborationIdentity::from_sha256([0x11; 32]);
        let second_boundary = FullElaborationIdentity::from_sha256([0x22; 32]);
        let first = ElaborationKey::boundary_family_entity(
            namespace(),
            instance("solid"),
            declaration("mechanical"),
            EntityKind::Port,
            first_boundary,
        )
        .unwrap();
        let repeated = ElaborationKey::boundary_family_entity(
            namespace(),
            instance("solid"),
            declaration("mechanical"),
            EntityKind::Port,
            first_boundary,
        )
        .unwrap();
        let second = ElaborationKey::boundary_family_entity(
            namespace(),
            instance("solid"),
            declaration("mechanical"),
            EntityKind::Port,
            second_boundary,
        )
        .unwrap();

        assert_eq!(first.entity_kind(), EntityKind::Port);
        assert_eq!(
            first.canonical_bytes().unwrap(),
            repeated.canonical_bytes().unwrap()
        );
        assert_eq!(
            first.full_identity().unwrap(),
            repeated.full_identity().unwrap()
        );
        assert_ne!(
            first.full_identity().unwrap(),
            second.full_identity().unwrap()
        );
        assert!(
            first
                .canonical_bytes()
                .unwrap()
                .ends_with(first_boundary.as_bytes())
        );
        assert!(
            second
                .canonical_bytes()
                .unwrap()
                .ends_with(second_boundary.as_bytes())
        );
    }

    #[test]
    fn complete_exterior_family_subjects_are_domain_separated() {
        let boundary = FullElaborationIdentity::from_sha256([0x33; 32]);
        let ordinary_port = entity_key("solid", "mechanical", EntityKind::Port);
        let family_port = ElaborationKey::boundary_family_entity(
            namespace(),
            instance("solid"),
            declaration("mechanical"),
            EntityKind::Port,
            boundary,
        )
        .unwrap();
        let family_relation = ElaborationKey::boundary_family_entity(
            namespace(),
            instance("solid"),
            declaration("boundary_law"),
            EntityKind::Relation,
            boundary,
        )
        .unwrap();
        let family_activation = ElaborationKey::boundary_family_generated(
            namespace(),
            instance("solid"),
            declaration("boundary_law"),
            GeneratedRole::RelationActivation,
            boundary,
        )
        .unwrap();

        assert_eq!(family_relation.entity_kind(), EntityKind::Relation);
        assert_eq!(family_activation.entity_kind(), EntityKind::Activation);
        assert_ne!(
            ordinary_port.full_identity().unwrap(),
            family_port.full_identity().unwrap()
        );
        assert_ne!(
            family_port.full_identity().unwrap(),
            family_relation.full_identity().unwrap()
        );
        assert_ne!(
            family_relation.full_identity().unwrap(),
            family_activation.full_identity().unwrap()
        );
        assert!(
            ElaborationKey::boundary_family_entity(
                namespace(),
                instance("solid"),
                declaration("not_a_family_entity"),
                EntityKind::Field,
                boundary,
            )
            .is_err()
        );
    }

    #[test]
    fn complete_exterior_family_identity_obeys_the_existing_byte_limit() {
        let limits = ElaborationIdentityLimits {
            max_canonical_key_bytes: 1,
            ..ElaborationIdentityLimits::default()
        };
        let boundary = FullElaborationIdentity::from_sha256([0x44; 32]);

        assert!(
            ElaborationKey::boundary_family_entity_with_limits(
                IdentityNamespace::new(["org"]).unwrap(),
                InstancePath::new(["root"]).unwrap(),
                DeclarationPath::new(["mechanical"]).unwrap(),
                EntityKind::Port,
                boundary,
                limits,
            )
            .is_err()
        );
        assert!(
            ElaborationKey::boundary_family_generated_with_limits(
                IdentityNamespace::new(["org"]).unwrap(),
                InstancePath::new(["root"]).unwrap(),
                DeclarationPath::new(["boundary_law"]).unwrap(),
                GeneratedRole::RelationActivation,
                boundary,
                limits,
            )
            .is_err()
        );
    }

    #[test]
    fn resource_policy_is_not_part_of_key_equality_or_identity() {
        let namespace = namespace();
        let instance = instance("r1");
        let declaration = declaration("voltage");
        let default = ElaborationKey::entity(
            namespace.clone(),
            instance.clone(),
            declaration.clone(),
            EntityKind::Field,
        )
        .unwrap();
        let relaxed = ElaborationKey::entity_with_limits(
            namespace,
            instance,
            declaration,
            EntityKind::Field,
            ElaborationIdentityLimits {
                max_canonical_key_bytes: 128 * 1_024,
                ..ElaborationIdentityLimits::default()
            },
        )
        .unwrap();

        assert_eq!(default, relaxed);
        assert_eq!(
            default.full_identity().unwrap(),
            relaxed.full_identity().unwrap()
        );
    }

    #[test]
    fn model_view_has_a_separate_domain_and_only_one_root_is_staged() {
        let root = ModelViewKey::new(namespace(), InstancePath::new(["plant"]).unwrap()).unwrap();
        let root_identity = root.full_identity().unwrap();
        let similarly_named_relation = ElaborationKey::entity(
            namespace(),
            InstancePath::new(["plant"]).unwrap(),
            DeclarationPath::new(["model"]).unwrap(),
            EntityKind::Relation,
        )
        .unwrap();
        assert_ne!(
            root_identity,
            similarly_named_relation.full_identity().unwrap()
        );

        let mut allocator = StagingIdAllocator::new();
        assert_eq!(allocator.stage_model_view(&root).unwrap(), root_identity);
        assert_eq!(allocator.stage_model_view(&root).unwrap(), root_identity);
        let other_root =
            ModelViewKey::new(namespace(), InstancePath::new(["other"]).unwrap()).unwrap();
        assert!(allocator.stage_model_view(&other_root).is_err());

        let staged = allocator.finish();
        let projected = staged.resolve_model_view(root_identity).unwrap();
        assert_eq!(projected.full_identity(), root_identity);
        assert_eq!(
            projected.id().ulid(),
            Ulid::from(u128::from_be_bytes({
                let mut bytes = [0_u8; 16];
                bytes.copy_from_slice(&root_identity.as_bytes()[..16]);
                bytes
            }))
        );
    }

    #[test]
    fn anonymous_connection_identity_is_member_permutation_invariant() {
        let a = entity_key("r1", "p", EntityKind::Port)
            .full_identity()
            .unwrap();
        let b = entity_key("r2", "p", EntityKind::Port)
            .full_identity()
            .unwrap();
        let first = ElaborationKey::anonymous_connection(
            namespace(),
            InstancePath::new(["plant"]).unwrap(),
            DeclarationPath::new(["network"]).unwrap(),
            [a, b],
        )
        .unwrap();
        let second = ElaborationKey::anonymous_connection(
            namespace(),
            InstancePath::new(["plant"]).unwrap(),
            DeclarationPath::new(["network"]).unwrap(),
            [b, a],
        )
        .unwrap();

        assert_eq!(
            first.canonical_bytes().unwrap(),
            second.canonical_bytes().unwrap()
        );
        assert_eq!(
            first.full_identity().unwrap(),
            second.full_identity().unwrap()
        );
    }

    #[derive(Debug)]
    struct ConstantProjector;

    impl ShortIdProjector for ConstantProjector {
        fn project(&self, _: FullElaborationIdentity) -> [u8; 16] {
            [7; 16]
        }
    }

    #[test]
    fn projected_collision_fails_before_ids_are_exposed() {
        let mut allocator = StagingIdAllocator::with_projector_and_limits(
            ConstantProjector,
            ElaborationIdentityLimits::default(),
        );
        let first = entity_key("r1", "voltage", EntityKind::Field);
        let second = entity_key("r2", "voltage", EntityKind::Field);
        allocator.stage(&first).unwrap();

        let diagnostic = allocator.stage(&second).unwrap_err();
        assert_eq!(diagnostic.code(), codes::LANGUAGE_LOWERING_ERROR);
        assert!(diagnostic.message().contains("collision"));
    }

    #[test]
    fn sealed_allocator_resolves_typed_ids_and_retains_full_identity() {
        let mut allocator = StagingIdAllocator::new();
        let key = entity_key("r1", "voltage", EntityKind::Field);
        let full = allocator.stage(&key).unwrap();
        assert_eq!(allocator.stage(&key).unwrap(), full);
        let staged = allocator.finish();

        let projected = staged.resolve::<kinds::Field>(full).unwrap();
        assert_eq!(projected.full_identity(), full);
        assert_eq!(
            projected.id().ulid(),
            Ulid::from(u128::from_be_bytes({
                let mut bytes = [0_u8; 16];
                bytes.copy_from_slice(&full.as_bytes()[..16]);
                bytes
            }))
        );
        assert!(staged.resolve::<kinds::Parameter>(full).is_err());
    }

    #[test]
    fn construction_limits_fail_closed() {
        let mut limits = ElaborationIdentityLimits {
            max_instance_depth: 1,
            ..ElaborationIdentityLimits::default()
        };
        assert!(InstancePath::with_limits(["root", "child"], limits).is_err());

        limits = ElaborationIdentityLimits {
            max_segment_bytes: 3,
            ..ElaborationIdentityLimits::default()
        };
        assert!(IdentityNamespace::with_limits(["four"], limits).is_err());

        limits = ElaborationIdentityLimits {
            max_anonymous_connection_members: 1,
            ..ElaborationIdentityLimits::default()
        };
        let a = FullElaborationIdentity::from_sha256([1; 32]);
        let b = FullElaborationIdentity::from_sha256([2; 32]);
        assert!(
            ElaborationKey::anonymous_connection_with_limits(
                IdentityNamespace::new(["org"]).unwrap(),
                InstancePath::new(["root"]).unwrap(),
                DeclarationPath::new(["net"]).unwrap(),
                [a, b],
                limits,
            )
            .is_err()
        );

        limits = ElaborationIdentityLimits {
            max_canonical_key_bytes: 1,
            ..ElaborationIdentityLimits::default()
        };
        assert!(
            ElaborationKey::entity_with_limits(
                IdentityNamespace::new(["org"]).unwrap(),
                InstancePath::new(["root"]).unwrap(),
                DeclarationPath::new(["field"]).unwrap(),
                EntityKind::Field,
                limits,
            )
            .is_err()
        );

        limits = ElaborationIdentityLimits {
            max_staged_identities: 1,
            ..ElaborationIdentityLimits::default()
        };
        let mut allocator =
            StagingIdAllocator::with_projector_and_limits(Sha256PrefixProjector, limits);
        allocator
            .stage(&entity_key("r1", "a", EntityKind::Field))
            .unwrap();
        assert!(
            allocator
                .stage(&entity_key("r1", "b", EntityKind::Field))
                .is_err()
        );
    }

    #[test]
    fn anonymous_connections_reject_ambiguous_membership() {
        let member = FullElaborationIdentity::from_sha256([1; 32]);
        let make = |members| {
            ElaborationKey::anonymous_connection(
                IdentityNamespace::new(["org"]).unwrap(),
                InstancePath::new(["root"]).unwrap(),
                DeclarationPath::new(["net"]).unwrap(),
                members,
            )
        };

        assert!(make(vec![member]).is_err());
        assert!(make(vec![member, member]).is_err());
    }
}
