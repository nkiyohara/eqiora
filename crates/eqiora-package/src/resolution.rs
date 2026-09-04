use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::canonical;
use crate::{
    CompilationPackageV1, ContractError, ModelPackageIdentityV1, PackageManifestV1,
    PackageReleaseV1, PackageStore, QualifiedName, ResolutionDigest, SourceBundleDigest,
    StoreError,
};

const SCHEMA: &str = "eqiora.package-resolution.v1";
const MAX_PACKAGES: usize = 65_536;
const MAX_EDGES: usize = 1_000_000;
const MAX_RESOLVED_WIRE_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Clone, Copy)]
struct ResolutionResourceLimits {
    packages: usize,
    edges: usize,
}

const RESOLUTION_LIMITS: ResolutionResourceLimits = ResolutionResourceLimits {
    packages: MAX_PACKAGES,
    edges: MAX_EDGES,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionNodeV1 {
    identity: ModelPackageIdentityV1,
    source_digest: SourceBundleDigest,
}

impl ResolutionNodeV1 {
    #[must_use]
    pub fn new(identity: ModelPackageIdentityV1, source_digest: SourceBundleDigest) -> Self {
        Self {
            identity,
            source_digest,
        }
    }

    #[must_use]
    pub fn identity(&self) -> &ModelPackageIdentityV1 {
        &self.identity
    }

    #[must_use]
    pub fn source_digest(&self) -> SourceBundleDigest {
        self.source_digest
    }
}

/// One exact direct edge in the locked dependency graph.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionEdgeV1 {
    declaring: ModelPackageIdentityV1,
    target: ModelPackageIdentityV1,
}

impl ResolutionEdgeV1 {
    pub fn new(declaring: ModelPackageIdentityV1, target: ModelPackageIdentityV1) -> Self {
        Self { declaring, target }
    }

    #[must_use]
    pub fn declaring(&self) -> &ModelPackageIdentityV1 {
        &self.declaring
    }

    #[must_use]
    pub fn target(&self) -> &ModelPackageIdentityV1 {
        &self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionRecordV1 {
    schema: String,
    root: ModelPackageIdentityV1,
    nodes: Vec<ResolutionNodeV1>,
    edges: Vec<ResolutionEdgeV1>,
}

impl ResolutionRecordV1 {
    pub fn new(
        root: ModelPackageIdentityV1,
        nodes: Vec<ResolutionNodeV1>,
        edges: Vec<ResolutionEdgeV1>,
    ) -> Result<Self, ContractError> {
        Self {
            schema: SCHEMA.to_owned(),
            root,
            nodes,
            edges,
        }
        .normalize()
    }

    /// Validate the allocation footprint of one package manifest and its
    /// caller-supplied exact dependency release closure.
    ///
    /// This performs no discovery and does not validate graph reachability or
    /// release semantics. It exists so composition layers can reject package
    /// and manifest-edge resource overflow before indexing or copying the
    /// supplied releases. [`Self::from_exact_releases`] repeats this preflight
    /// and owns complete graph validation.
    ///
    /// # Errors
    ///
    /// Returns a package contract error when the package or manifest-edge
    /// count exceeds the v1 resolution bounds or a checked count overflows.
    pub fn preflight_exact_release_closure(
        root_manifest: &PackageManifestV1,
        dependencies: &[PackageReleaseV1],
    ) -> Result<(), ContractError> {
        exact_release_shape_with_limits(root_manifest, dependencies, RESOLUTION_LIMITS)?;
        Ok(())
    }

    /// Derive one exact lock record from a root release and a caller-supplied
    /// complete dependency closure.
    ///
    /// This operation performs no discovery, selection, store access, or
    /// source fetching. Nodes come only from the supplied release identities
    /// and source digests; edges come only from their closed package manifests.
    /// Ordinary resolution normalization rejects duplicate, ambiguous,
    /// missing, cyclic, and unreachable inputs.
    ///
    /// # Errors
    ///
    /// Returns a package contract error when a release identity or source
    /// digest cannot be reconstructed, or the exact graph is not closed and
    /// valid.
    pub fn from_exact_releases(
        root: &PackageReleaseV1,
        dependencies: &[PackageReleaseV1],
    ) -> Result<Self, ContractError> {
        let (package_count, edge_count) =
            exact_release_shape_with_limits(root.manifest(), dependencies, RESOLUTION_LIMITS)?;
        let root_identity = root.package_identity()?;
        let mut nodes = Vec::with_capacity(package_count);
        nodes.push(ResolutionNodeV1::new(
            root_identity.clone(),
            root.source_digest()?,
        ));
        let mut edges = Vec::with_capacity(edge_count);
        append_release_edges(&mut edges, &root_identity, root)?;
        for release in dependencies {
            let identity = release.package_identity()?;
            nodes.push(ResolutionNodeV1::new(
                identity.clone(),
                release.source_digest()?,
            ));
            append_release_edges(&mut edges, &identity, release)?;
        }
        Self::new(root_identity, nodes, edges)
    }

    fn normalize(mut self) -> Result<Self, ContractError> {
        if self.schema != SCHEMA {
            return Err(ContractError::new(format!(
                "unsupported resolution schema `{}`",
                self.schema
            )));
        }
        if self.nodes.is_empty() || self.nodes.len() > MAX_PACKAGES {
            return Err(ContractError::new(
                "resolution record must contain a bounded, non-empty node set",
            ));
        }
        if self.edges.len() > MAX_EDGES {
            return Err(ContractError::new("resolution record exceeds edge limit"));
        }
        self.nodes.sort();
        for pair in self.nodes.windows(2) {
            if pair[0].identity == pair[1].identity {
                return Err(ContractError::new(format!(
                    "duplicate resolution node `{}`",
                    pair[0].identity.name
                )));
            }
            if pair[0].identity.name == pair[1].identity.name
                && pair[0].identity.version == pair[1].identity.version
            {
                return Err(ContractError::new(format!(
                    "ambiguous package identity `{}@{}` has multiple semantic digests",
                    pair[0].identity.name, pair[0].identity.version
                )));
            }
        }
        let identities: BTreeSet<_> = self
            .nodes
            .iter()
            .map(|node| node.identity.clone())
            .collect();
        if !identities.contains(&self.root) {
            return Err(ContractError::new(
                "resolution root does not have an exact node",
            ));
        }
        self.edges.sort();
        for pair in self.edges.windows(2) {
            if pair[0].declaring == pair[1].declaring && pair[0].target.name == pair[1].target.name
            {
                return Err(ContractError::new(format!(
                    "duplicate direct dependency `{}` in `{}`",
                    pair[0].target.name, pair[0].declaring.name
                )));
            }
        }
        for edge in &self.edges {
            if !identities.contains(&edge.declaring) {
                return Err(ContractError::new(format!(
                    "resolution edge declares missing package `{}`",
                    edge.declaring.name
                )));
            }
            if !identities.contains(&edge.target) {
                return Err(ContractError::new(format!(
                    "resolution edge targets missing exact identity `{}`",
                    edge.target.name
                )));
            }
        }
        validate_acyclic(&identities, &self.edges)?;
        validate_reachable(&self.root, &identities, &self.edges)?;
        Ok(self)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, ContractError> {
        canonical::from_slice::<Self>(bytes)?.normalize()
    }

    pub fn canonical_json(&self) -> Result<Vec<u8>, ContractError> {
        canonical::to_bytes(self)
    }

    pub fn digest(&self) -> Result<ResolutionDigest, ContractError> {
        Ok(ResolutionDigest::compute(&self.canonical_json()?))
    }

    #[must_use]
    pub fn root(&self) -> &ModelPackageIdentityV1 {
        &self.root
    }

    #[must_use]
    pub fn nodes(&self) -> &[ResolutionNodeV1] {
        &self.nodes
    }

    #[must_use]
    pub fn edges(&self) -> &[ResolutionEdgeV1] {
        &self.edges
    }
}

fn exact_release_shape_with_limits(
    root_manifest: &PackageManifestV1,
    dependencies: &[PackageReleaseV1],
    limits: ResolutionResourceLimits,
) -> Result<(usize, usize), ContractError> {
    let package_count = dependencies
        .len()
        .checked_add(1)
        .ok_or_else(|| ContractError::new("exact release package count overflow"))?;
    if package_count > limits.packages {
        return Err(ContractError::new(format!(
            "exact release closure exceeds the {} package limit",
            limits.packages
        )));
    }
    let mut edge_count = root_manifest.dependencies().len();
    for release in dependencies {
        edge_count = edge_count
            .checked_add(release.manifest().dependencies().len())
            .ok_or_else(|| ContractError::new("exact release edge count overflow"))?;
        if edge_count > limits.edges {
            return Err(ContractError::new(format!(
                "exact release closure exceeds the {} edge limit",
                limits.edges
            )));
        }
    }
    Ok((package_count, edge_count))
}

fn append_release_edges(
    edges: &mut Vec<ResolutionEdgeV1>,
    declaring: &ModelPackageIdentityV1,
    release: &PackageReleaseV1,
) -> Result<(), ContractError> {
    for requirement in release.manifest().dependencies() {
        edges.push(ResolutionEdgeV1::new(
            declaring.clone(),
            requirement.target().clone(),
        ));
    }
    Ok(())
}

fn validate_acyclic(
    identities: &BTreeSet<ModelPackageIdentityV1>,
    edges: &[ResolutionEdgeV1],
) -> Result<(), ContractError> {
    let mut indegree: BTreeMap<_, usize> = identities
        .iter()
        .cloned()
        .map(|identity| (identity, 0))
        .collect();
    let mut outgoing: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for edge in edges {
        *indegree.get_mut(&edge.target).expect("validated target") += 1;
        outgoing
            .entry(edge.declaring.clone())
            .or_default()
            .push(edge.target.clone());
    }
    let mut ready: VecDeque<_> = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(identity, _)| identity.clone())
        .collect();
    let mut visited = 0_usize;
    while let Some(identity) = ready.pop_front() {
        visited += 1;
        for target in outgoing.get(&identity).into_iter().flatten() {
            let count = indegree.get_mut(target).expect("validated target");
            *count -= 1;
            if *count == 0 {
                ready.push_back(target.clone());
            }
        }
    }
    if visited != identities.len() {
        return Err(ContractError::new("cyclic exact package resolution graph"));
    }
    Ok(())
}

fn validate_reachable(
    root: &ModelPackageIdentityV1,
    identities: &BTreeSet<ModelPackageIdentityV1>,
    edges: &[ResolutionEdgeV1],
) -> Result<(), ContractError> {
    let mut outgoing: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for edge in edges {
        outgoing
            .entry(edge.declaring.clone())
            .or_default()
            .push(edge.target.clone());
    }
    let mut reached = BTreeSet::new();
    let mut pending = vec![root.clone()];
    while let Some(identity) = pending.pop() {
        if reached.insert(identity.clone()) {
            pending.extend(outgoing.get(&identity).into_iter().flatten().cloned());
        }
    }
    if &reached != identities {
        return Err(ContractError::new(
            "resolution record contains a package unreachable from its root",
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub enum ResolutionError {
    Store(StoreError),
    MissingBundle(SourceBundleDigest),
    InvalidRelease {
        expected: SourceBundleDigest,
        reason: String,
    },
    IdentityMismatch {
        expected: Box<ModelPackageIdentityV1>,
        actual: Box<ModelPackageIdentityV1>,
    },
    SourceDigestMismatch {
        expected: SourceBundleDigest,
        actual: SourceBundleDigest,
    },
    DependencyMismatch(ModelPackageIdentityV1),
    Contract(ContractError),
}

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "package store failed: {error}"),
            Self::MissingBundle(digest) => {
                write!(formatter, "missing exact source bundle `{digest}`")
            }
            Self::InvalidRelease { expected, reason } => {
                write!(
                    formatter,
                    "invalid release for source bundle `{expected}`: {reason}"
                )
            }
            Self::IdentityMismatch { expected, actual } => write!(
                formatter,
                "package identity mismatch: expected `{}@{}`, got `{}@{}`",
                expected.name, expected.version, actual.name, actual.version
            ),
            Self::SourceDigestMismatch { expected, actual } => write!(
                formatter,
                "source bundle digest mismatch: expected `{expected}`, got `{actual}`"
            ),
            Self::DependencyMismatch(package) => write!(
                formatter,
                "locked dependency edges do not match manifest for `{}@{}`",
                package.name, package.version
            ),
            Self::Contract(error) => write!(formatter, "resolution contract failed: {error}"),
        }
    }
}

impl std::error::Error for ResolutionError {}

impl From<StoreError> for ResolutionError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<ContractError> for ResolutionError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

/// An exact resolver. It has no discovery, selection, network, or fallback API.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExactResolver;

impl ExactResolver {
    /// Resolve an exact record from caller-held typed releases without first
    /// cloning every canonical wire into an in-memory store.
    ///
    /// The complete release set is measured against the same aggregate wire
    /// budget used by [`Self::resolve`] before indexing it. A package-private
    /// borrowed store then serializes at most one requested release at a time
    /// and delegates all identity, digest, manifest-edge, and record checks to
    /// the ordinary resolver path.
    ///
    /// # Errors
    ///
    /// Returns a package contract, store, identity, digest, dependency, or
    /// exact-record error. No caller release or external store is mutated.
    pub fn resolve_releases(
        &self,
        record: &ResolutionRecordV1,
        root: &PackageReleaseV1,
        dependencies: &[PackageReleaseV1],
    ) -> Result<ResolvedPackageGraph, ResolutionError> {
        ResolutionRecordV1::preflight_exact_release_closure(root.manifest(), dependencies)?;
        preflight_release_wires(
            std::iter::once(root).chain(dependencies.iter()),
            MAX_RESOLVED_WIRE_BYTES,
        )?;
        let supplied = ResolutionRecordV1::from_exact_releases(root, dependencies)?;
        if &supplied != record {
            return Err(ContractError::new(
                "caller-held exact releases do not match the requested resolution record",
            )
            .into());
        }
        let store = BorrowedReleaseStore::new(root, dependencies)?;
        self.resolve(record, &store)
    }

    pub fn resolve(
        &self,
        record: &ResolutionRecordV1,
        store: &impl PackageStore,
    ) -> Result<ResolvedPackageGraph, ResolutionError> {
        let mut packages = BTreeMap::new();
        let mut resolved_wire_bytes = 0_usize;
        for node in record.nodes() {
            let remaining_wire_bytes = MAX_RESOLVED_WIRE_BYTES
                .checked_sub(resolved_wire_bytes)
                .ok_or_else(|| {
                    ContractError::new("resolved package graph exceeds total wire byte limit")
                })?;
            let bytes = store
                .load_exact(
                    node.source_digest(),
                    remaining_wire_bytes.min(canonical::MAX_CANONICAL_JSON_BYTES),
                )?
                .ok_or(ResolutionError::MissingBundle(node.source_digest()))?;
            resolved_wire_bytes = resolved_wire_bytes
                .checked_add(bytes.len())
                .ok_or_else(|| ContractError::new("resolved package byte count overflow"))?;
            if resolved_wire_bytes > MAX_RESOLVED_WIRE_BYTES {
                return Err(ContractError::new(
                    "resolved package graph exceeds total wire byte limit",
                )
                .into());
            }
            let release = PackageReleaseV1::from_json(&bytes).map_err(|error| {
                ResolutionError::InvalidRelease {
                    expected: node.source_digest(),
                    reason: error.to_string(),
                }
            })?;
            let actual_identity = release.package_identity()?;
            if actual_identity != *node.identity() {
                return Err(ResolutionError::IdentityMismatch {
                    expected: Box::new(node.identity().clone()),
                    actual: Box::new(actual_identity),
                });
            }
            let actual_source = release.source_digest()?;
            if actual_source != node.source_digest() {
                return Err(ResolutionError::SourceDigestMismatch {
                    expected: node.source_digest(),
                    actual: actual_source,
                });
            }
            packages.insert(node.identity().clone(), release);
        }

        for (identity, release) in &packages {
            let manifest_edges: BTreeSet<_> = release
                .manifest()
                .dependencies()
                .iter()
                .map(|dependency| dependency.target().clone())
                .collect();
            let locked_edges: BTreeSet<_> = record
                .edges()
                .iter()
                .filter(|edge| edge.declaring() == identity)
                .map(|edge| edge.target().clone())
                .collect();
            if manifest_edges != locked_edges {
                return Err(ResolutionError::DependencyMismatch(identity.clone()));
            }
        }

        Ok(ResolvedPackageGraph {
            root: record.root().clone(),
            resolution_digest: record.digest()?,
            packages,
            edges: record.edges().to_vec(),
            compilation_packages: record
                .nodes()
                .iter()
                .map(|node| {
                    CompilationPackageV1::new(node.identity().clone(), node.source_digest())
                })
                .collect(),
        })
    }
}

fn preflight_release_wires<'a, I>(releases: I, limit: usize) -> Result<(), ContractError>
where
    I: IntoIterator<Item = &'a PackageReleaseV1>,
{
    let mut remaining = limit;
    for release in releases {
        let bytes = canonical::encoded_len_with_limit(release, remaining)?;
        remaining = remaining
            .checked_sub(bytes)
            .ok_or_else(|| ContractError::new("resolved package byte count overflow"))?;
    }
    Ok(())
}

struct BorrowedReleaseStore<'a> {
    releases: BTreeMap<SourceBundleDigest, &'a PackageReleaseV1>,
}

impl<'a> BorrowedReleaseStore<'a> {
    fn new(
        root: &'a PackageReleaseV1,
        dependencies: &'a [PackageReleaseV1],
    ) -> Result<Self, StoreError> {
        let mut releases = BTreeMap::new();
        for release in std::iter::once(root).chain(dependencies.iter()) {
            let digest = release.source_digest()?;
            match releases.entry(digest) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(release);
                }
                std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &release => {}
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(StoreError::DigestCollision(digest));
                }
            }
        }
        Ok(Self { releases })
    }
}

impl PackageStore for BorrowedReleaseStore<'_> {
    fn load_exact(
        &self,
        expected: SourceBundleDigest,
        max_bytes: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(release) = self.releases.get(&expected) else {
            return Ok(None);
        };
        canonical::encoded_len_with_limit(*release, max_bytes)?;
        Ok(Some(canonical::to_bytes_with_limit(*release, max_bytes)?))
    }
}

/// A fully verified package set, detached from store implementation details.
#[derive(Clone, Debug)]
pub struct ResolvedPackageGraph {
    root: ModelPackageIdentityV1,
    resolution_digest: ResolutionDigest,
    packages: BTreeMap<ModelPackageIdentityV1, PackageReleaseV1>,
    edges: Vec<ResolutionEdgeV1>,
    compilation_packages: Vec<CompilationPackageV1>,
}

impl ResolvedPackageGraph {
    #[must_use]
    pub fn root(&self) -> &ModelPackageIdentityV1 {
        &self.root
    }

    #[must_use]
    pub fn package(&self, identity: &ModelPackageIdentityV1) -> Option<&PackageReleaseV1> {
        self.packages.get(identity)
    }

    #[must_use]
    pub fn dependency(
        &self,
        declaring: &ModelPackageIdentityV1,
        package_name: &QualifiedName,
    ) -> Option<&ModelPackageIdentityV1> {
        self.edges
            .binary_search_by(|edge| {
                edge.declaring()
                    .cmp(declaring)
                    .then_with(|| edge.target().name.cmp(package_name))
            })
            .ok()
            .map(|index| &self.edges[index])
            .map(ResolutionEdgeV1::target)
    }

    #[must_use]
    pub fn resolution_digest(&self) -> ResolutionDigest {
        self.resolution_digest
    }

    pub fn packages(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ModelPackageIdentityV1, &PackageReleaseV1)> {
        self.packages.iter()
    }

    #[must_use]
    pub fn compilation_packages(&self) -> &[CompilationPackageV1] {
        &self.compilation_packages
    }

    #[must_use]
    pub fn edges(&self) -> &[ResolutionEdgeV1] {
        &self.edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BundleEntryV1, BundleRoleV1, CanonicalDeclaration, DeclarationKindV1, ExactVersion,
        InMemoryPackageStore, NormalizedRelativePath, PackageDependencyV1, PackageManifestV1,
        SemanticContentV1, SemanticDeclarationV1, SourceFileV1, VisibilityV1,
    };

    fn release(name: &str, dependencies: Vec<PackageDependencyV1>, body: &str) -> PackageReleaseV1 {
        let path = NormalizedRelativePath::parse("src/package.eqi").expect("path");
        let manifest = PackageManifestV1::new(
            "package",
            QualifiedName::parse(name).expect("name"),
            ExactVersion::parse("1.0.0").expect("version"),
            dependencies,
            vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
        )
        .expect("manifest");
        let semantic = SemanticContentV1::new(vec![SemanticDeclarationV1::new(
            QualifiedName::parse("Main").expect("declaration"),
            DeclarationKindV1::Model,
            VisibilityV1::Public,
            CanonicalDeclaration::new(body).expect("canonical"),
        )])
        .expect("semantic");
        PackageReleaseV1::new(
            manifest,
            semantic,
            vec![SourceFileV1::new(
                path,
                BundleRoleV1::ModelSource,
                body.as_bytes().to_vec(),
            )],
        )
        .expect("release")
    }

    #[test]
    fn exact_graph_resolves_offline_and_is_order_independent() {
        let leaf = release("org.example.Leaf", vec![], "model Main {}\n");
        let leaf_id = leaf.package_identity().expect("identity");
        let dependency = PackageDependencyV1::new(leaf_id.clone());
        let root = release("org.example.Root", vec![dependency], "model Main {}\n");
        let root_id = root.package_identity().expect("identity");
        assert!(
            exact_release_shape_with_limits(
                root.manifest(),
                std::slice::from_ref(&leaf),
                ResolutionResourceLimits {
                    packages: 1,
                    edges: 1,
                },
            )
            .expect_err("package limit before allocation")
            .to_string()
            .contains("package limit")
        );
        assert!(
            exact_release_shape_with_limits(
                root.manifest(),
                std::slice::from_ref(&leaf),
                ResolutionResourceLimits {
                    packages: 2,
                    edges: 0,
                },
            )
            .expect_err("edge limit before allocation")
            .to_string()
            .contains("edge limit")
        );
        let mut store = InMemoryPackageStore::default();
        let leaf_source = store.insert(&leaf).expect("insert");
        let root_source = store.insert(&root).expect("insert");
        let edge = ResolutionEdgeV1::new(root_id.clone(), leaf_id.clone());
        let first = ResolutionRecordV1::new(
            root_id.clone(),
            vec![
                ResolutionNodeV1::new(leaf_id.clone(), leaf_source),
                ResolutionNodeV1::new(root_id.clone(), root_source),
            ],
            vec![edge.clone()],
        )
        .expect("record");
        let second = ResolutionRecordV1::new(
            root_id.clone(),
            vec![
                ResolutionNodeV1::new(root_id.clone(), root_source),
                ResolutionNodeV1::new(leaf_id.clone(), leaf_source),
            ],
            vec![edge],
        )
        .expect("record");
        assert_eq!(first.canonical_json(), second.canonical_json());
        assert_eq!(first.digest(), second.digest());
        let derived = ResolutionRecordV1::from_exact_releases(&root, std::slice::from_ref(&leaf))
            .expect("derived exact record");
        assert_eq!(derived, first);
        assert!(
            ResolutionRecordV1::from_exact_releases(&root, &[leaf.clone(), leaf.clone()]).is_err()
        );
        let extra = release("org.example.Extra", vec![], "model Main {}\n");
        assert!(ResolutionRecordV1::from_exact_releases(&root, &[leaf.clone(), extra]).is_err());
        let graph = ExactResolver.resolve(&first, &store).expect("resolve");
        let borrowed = ExactResolver
            .resolve_releases(&first, &root, std::slice::from_ref(&leaf))
            .expect("resolve caller-held releases");
        assert!(
            ExactResolver
                .resolve_releases(&first, &leaf, std::slice::from_ref(&root))
                .is_err(),
            "the explicit root cannot be replaced by a dependency"
        );
        let unrecorded = release("org.example.Unrecorded", vec![], "model Main {}\n");
        assert!(
            ExactResolver
                .resolve_releases(&first, &root, &[leaf.clone(), unrecorded])
                .is_err(),
            "unrecorded releases cannot be hidden in the borrowed store"
        );
        let leaf_variant = PackageReleaseV1::new(
            leaf.manifest().clone(),
            leaf.semantic().clone(),
            vec![SourceFileV1::new(
                NormalizedRelativePath::parse("src/package.eqi").expect("path"),
                BundleRoleV1::ModelSource,
                b"model  Main {}\n".to_vec(),
            )],
        )
        .expect("same-semantic source variant");
        assert_eq!(
            leaf_variant.package_identity().expect("variant identity"),
            leaf_id
        );
        assert_ne!(
            leaf_variant.source_digest().expect("variant source"),
            leaf_source
        );
        assert!(
            ExactResolver
                .resolve_releases(&first, &root, &[leaf_variant])
                .is_err(),
            "recorded source digests cannot be substituted"
        );
        assert_eq!(graph.root(), &root_id);
        assert_eq!(graph.package(&leaf_id), Some(&leaf));
        assert_eq!(graph.dependency(&root_id, &leaf_id.name), Some(&leaf_id));
        assert_eq!(graph.packages().count(), 2);
        assert_eq!(borrowed.root(), graph.root());
        assert_eq!(borrowed.packages().count(), graph.packages().count());

        let root_bytes = canonical::encoded_len_with_limit(&root, usize::MAX).expect("root bytes");
        let leaf_bytes = canonical::encoded_len_with_limit(&leaf, usize::MAX).expect("leaf bytes");
        let exact_bytes = root_bytes.checked_add(leaf_bytes).expect("wire total");
        preflight_release_wires([&root, &leaf], exact_bytes).expect("exact aggregate limit");
        assert!(preflight_release_wires([&root, &leaf], exact_bytes - 1).is_err());
    }

    #[test]
    fn graph_rejects_duplicate_missing_cyclic_and_unreachable_nodes() {
        let a = release("org.example.A", vec![], "model Main {}\n");
        let b = release("org.example.B", vec![], "model Main {}\n");
        let a_id = a.package_identity().expect("identity");
        let b_id = b.package_identity().expect("identity");
        let a_node = ResolutionNodeV1::new(a_id.clone(), a.source_digest().expect("source"));
        let b_node = ResolutionNodeV1::new(b_id.clone(), b.source_digest().expect("source"));
        assert!(
            ResolutionRecordV1::new(a_id.clone(), vec![a_node.clone(), a_node.clone()], vec![])
                .is_err()
        );
        assert!(
            ResolutionRecordV1::new(
                a_id.clone(),
                vec![a_node.clone()],
                vec![ResolutionEdgeV1::new(a_id.clone(), b_id.clone())]
            )
            .is_err()
        );
        let cycle = vec![
            ResolutionEdgeV1::new(a_id.clone(), b_id.clone()),
            ResolutionEdgeV1::new(b_id.clone(), a_id.clone()),
        ];
        assert!(
            ResolutionRecordV1::new(a_id.clone(), vec![a_node.clone(), b_node.clone()], cycle)
                .is_err()
        );
        assert!(ResolutionRecordV1::new(a_id, vec![a_node, b_node], vec![]).is_err());
    }

    #[test]
    fn resolver_rejects_missing_and_digest_mismatched_store_entries() {
        let root_release = release("org.example.Root", vec![], "model Main {}\n");
        let identity = root_release.package_identity().expect("identity");
        let source = root_release.source_digest().expect("source");
        let record = ResolutionRecordV1::new(
            identity,
            vec![ResolutionNodeV1::new(
                root_release.package_identity().expect("identity"),
                source,
            )],
            vec![],
        )
        .expect("record");
        assert!(matches!(
            ExactResolver.resolve(&record, &InMemoryPackageStore::default()),
            Err(ResolutionError::MissingBundle(_))
        ));
        let different = release("org.example.Other", vec![], "model Main {}\n");
        let mut store = InMemoryPackageStore::default();
        store.insert_unchecked(source, different.canonical_json().expect("JSON"));
        assert!(matches!(
            ExactResolver.resolve(&record, &store),
            Err(ResolutionError::IdentityMismatch { .. })
        ));

        let wrong_version = ModelPackageIdentityV1::new(
            root_release.package_identity().expect("identity").name,
            crate::ExactVersion::parse("2.0.0").expect("version"),
            root_release
                .package_identity()
                .expect("identity")
                .semantic_digest,
        );
        let wrong_version_record = ResolutionRecordV1::new(
            wrong_version.clone(),
            vec![ResolutionNodeV1::new(wrong_version, source)],
            vec![],
        )
        .expect("record");
        let mut actual_store = InMemoryPackageStore::default();
        actual_store.insert(&root_release).expect("insert");
        assert!(matches!(
            ExactResolver.resolve(&wrong_version_record, &actual_store),
            Err(ResolutionError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn graph_rejects_ambiguous_name_and_exact_version() {
        let release = release("org.example.Root", vec![], "model Main {}\n");
        let first = release.package_identity().expect("identity");
        let second = ModelPackageIdentityV1::new(
            first.name.clone(),
            first.version.clone(),
            crate::PackageSemanticDigest::parse(&"ab".repeat(32)).expect("semantic digest"),
        );
        let source = release.source_digest().expect("source");
        assert!(
            ResolutionRecordV1::new(
                first.clone(),
                vec![
                    ResolutionNodeV1::new(first.clone(), source),
                    ResolutionNodeV1::new(second.clone(), source),
                ],
                vec![ResolutionEdgeV1::new(first, second),],
            )
            .is_err()
        );
    }
}
