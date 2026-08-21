use std::collections::BTreeSet;
use std::fmt;

use eqiora_core::Diagnostic;
use sha2::{Digest, Sha256};

use crate::{ResolvedArrayScalarV1, ResolvedArrayV1, invalid_artifact, validate_text};

/// Raw SHA-256 of one complete logical external source byte stream.
///
/// Unlike [`crate::ArtifactDigest`], this identity has no domain prefix. The
/// distinct type prevents a source-byte digest from being substituted for a
/// canonical artifact identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawSourceSha256(String);

impl RawSourceSha256 {
    /// Hash the complete source bytes without a domain separator.
    #[must_use]
    pub fn from_source_bytes(bytes: &[u8]) -> Self {
        Self::from_sha256(Sha256::digest(bytes).into())
    }

    /// Encode complete SHA-256 bytes as canonical lowercase hexadecimal.
    #[must_use]
    pub fn from_sha256(bytes: [u8; 32]) -> Self {
        let mut encoded = String::with_capacity(64);
        for byte in bytes {
            use fmt::Write as _;
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self(encoded)
    }

    /// Parse a canonical raw-source digest.
    ///
    /// # Errors
    /// Returns `EQ0901` for any form other than 64 lowercase hexadecimal
    /// characters.
    pub fn from_hex(value: impl Into<String>) -> Result<Self, Diagnostic> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid_artifact(
                "raw source digest must be 64 lowercase hexadecimal SHA-256 characters",
            ));
        }
        Ok(Self(value))
    }

    /// Canonical lowercase hexadecimal form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Complete SHA-256 bytes.
    #[must_use]
    pub fn sha256_bytes(&self) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        for (index, pair) in self.0.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            bytes[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
        }
        bytes
    }
}

impl fmt::Display for RawSourceSha256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Exact stable identity of one admitted external-format adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAdapterIdentityV1 {
    id: String,
    version: String,
}

impl ExternalAdapterIdentityV1 {
    /// Construct a stable lowercase dotted/kebab adapter ID and exact version.
    ///
    /// # Errors
    /// Returns `EQ0901` for invalid or control-bearing text.
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Result<Self, Diagnostic> {
        let id = id.into();
        let version = version.into();
        validate_adapter_id(&id)?;
        validate_text("external adapter version", &version)?;
        Ok(Self { id, version })
    }

    /// Stable adapter identity.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Exact adapter implementation version, never a compatibility range.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Closed role of one native component beneath a format adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalRuntimeRoleV1 {
    /// Rust binding called by the format adapter.
    RustBinding,
    /// Native storage library called by its binding.
    NativeStorageLibrary,
}

/// One exact runtime component in outer-to-inner call order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRuntimeComponentV1 {
    role: ExternalRuntimeRoleV1,
    implementation: String,
    version: String,
}

impl ExternalRuntimeComponentV1 {
    /// Construct one exact runtime-stack entry.
    ///
    /// # Errors
    /// Returns `EQ0901` for empty or control-bearing text.
    pub fn new(
        role: ExternalRuntimeRoleV1,
        implementation: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, Diagnostic> {
        let implementation = implementation.into();
        let version = version.into();
        validate_text("external runtime implementation", &implementation)?;
        validate_text("external runtime version", &version)?;
        Ok(Self {
            role,
            implementation,
            version,
        })
    }

    /// Runtime role.
    #[must_use]
    pub const fn role(&self) -> ExternalRuntimeRoleV1 {
        self.role
    }

    /// Exact implementation identity.
    #[must_use]
    pub fn implementation(&self) -> &str {
        &self.implementation
    }

    /// Exact resolved version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Adapter-relative path through element children in source order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralSelectorV1 {
    element_path: Vec<u32>,
}

impl StructuralSelectorV1 {
    /// Construct one structural selector. The empty path is contextually valid
    /// only for metadata source ordinal zero.
    #[must_use]
    pub fn new(element_path: Vec<u32>) -> Self {
        Self { element_path }
    }

    /// Element-child indices from the metadata root.
    #[must_use]
    pub fn element_path(&self) -> &[u32] {
        &self.element_path
    }

    /// Whether this selector denotes the complete metadata document.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.element_path.is_empty()
    }
}

/// One selected source entity with an optional non-selecting display name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSourceEntityV1 {
    selector: StructuralSelectorV1,
    display_name: Option<String>,
}

impl SelectedSourceEntityV1 {
    /// Construct a non-root selected entity.
    ///
    /// # Errors
    /// Returns `EQ0901` for a root selector or invalid display text.
    pub fn new(
        selector: StructuralSelectorV1,
        display_name: Option<String>,
    ) -> Result<Self, Diagnostic> {
        require_non_root("selected source entity", &selector)?;
        validate_optional_text("selected source display name", display_name.as_deref())?;
        Ok(Self {
            selector,
            display_name,
        })
    }

    /// Structural selector that alone identifies the source entity.
    #[must_use]
    pub const fn selector(&self) -> &StructuralSelectorV1 {
        &self.selector
    }

    /// Optional inspectable name that never selects content.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
}

/// Explicit grid and ordered attribute selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportSelectionV1 {
    grid: SelectedSourceEntityV1,
    attributes: Vec<SelectedSourceEntityV1>,
}

impl ExternalImportSelectionV1 {
    /// Construct one grid and caller-ordered, structurally unique attributes.
    ///
    /// # Errors
    /// Returns `EQ0901` when any selector is repeated.
    pub fn new(
        grid: SelectedSourceEntityV1,
        attributes: Vec<SelectedSourceEntityV1>,
    ) -> Result<Self, Diagnostic> {
        let mut selectors = BTreeSet::new();
        selectors.insert(grid.selector.clone());
        for attribute in &attributes {
            if !selectors.insert(attribute.selector.clone()) {
                return Err(invalid_artifact(
                    "external import attribute selectors must be unique",
                ));
            }
        }
        Ok(Self { grid, attributes })
    }

    /// Selected grid.
    #[must_use]
    pub const fn grid(&self) -> &SelectedSourceEntityV1 {
        &self.grid
    }

    /// Selected attributes in explicit caller order.
    #[must_use]
    pub fn attributes(&self) -> &[SelectedSourceEntityV1] {
        &self.attributes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExternalSourceKind {
    MetadataDocument,
    ExternalArraySource,
}

/// Complete bytes and provenance labels for one observed source occurrence.
///
/// The bytes are intentionally not serializable through this type. A manifest
/// constructor hashes them and persists only the raw digest and typed labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportSourceV1 {
    kind: ExternalSourceKind,
    origin_selector: StructuralSelectorV1,
    display_locator: Option<String>,
    bytes: Vec<u8>,
}

impl ExternalImportSourceV1 {
    /// Observe the complete metadata document at source ordinal zero.
    ///
    /// # Errors
    /// Returns `EQ0901` for invalid display text.
    pub fn metadata_document(
        bytes: Vec<u8>,
        display_locator: Option<String>,
    ) -> Result<Self, Diagnostic> {
        validate_optional_text("metadata display locator", display_locator.as_deref())?;
        Ok(Self {
            kind: ExternalSourceKind::MetadataDocument,
            origin_selector: StructuralSelectorV1::new(Vec::new()),
            display_locator,
            bytes,
        })
    }

    /// Observe one external array-source occurrence declared by metadata.
    ///
    /// Repeated bytes or locators remain distinct calls and distinct source
    /// ordinals. The structural declaring selector must be non-root.
    ///
    /// # Errors
    /// Returns `EQ0901` for a root selector or invalid display text.
    pub fn external_array_source(
        origin_selector: StructuralSelectorV1,
        bytes: Vec<u8>,
        display_locator: Option<String>,
    ) -> Result<Self, Diagnostic> {
        require_non_root("external array source", &origin_selector)?;
        validate_optional_text(
            "external array source display locator",
            display_locator.as_deref(),
        )?;
        Ok(Self {
            kind: ExternalSourceKind::ExternalArraySource,
            origin_selector,
            display_locator,
            bytes,
        })
    }

    pub(crate) const fn kind(&self) -> ExternalSourceKind {
        self.kind
    }

    /// Metadata structural selector that declared this occurrence.
    #[must_use]
    pub const fn origin_selector(&self) -> &StructuralSelectorV1 {
        &self.origin_selector
    }

    /// Optional inspectable locator that is never dereferenced by artifacts.
    #[must_use]
    pub fn display_locator(&self) -> Option<&str> {
        self.display_locator.as_deref()
    }

    /// Complete logical source bytes hashed by manifest construction/replay.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Raw, unprefixed SHA-256 over the complete bytes.
    #[must_use]
    pub fn digest(&self) -> RawSourceSha256 {
        RawSourceSha256::from_source_bytes(&self.bytes)
    }
}

/// One normalized resolved array plus its source occurrence and declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedImportArrayV1 {
    source_ordinal: u32,
    origin_selector: StructuralSelectorV1,
    storage_display_selector: Option<String>,
    array: ResolvedArrayV1,
}

impl ResolvedImportArrayV1 {
    /// Bind a normalized array to one source occurrence and non-root metadata
    /// declaration selector.
    ///
    /// # Errors
    /// Returns `EQ0901` for a root selector or invalid display text.
    pub fn new(
        source_ordinal: u32,
        origin_selector: StructuralSelectorV1,
        storage_display_selector: Option<String>,
        array: ResolvedArrayV1,
    ) -> Result<Self, Diagnostic> {
        require_non_root("resolved import array", &origin_selector)?;
        validate_optional_text(
            "resolved array storage display selector",
            storage_display_selector.as_deref(),
        )?;
        Ok(Self {
            source_ordinal,
            origin_selector,
            storage_display_selector,
            array,
        })
    }

    /// Source occurrence ordinal, where zero is the metadata document.
    #[must_use]
    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    /// Structural metadata element that declares this array.
    #[must_use]
    pub const fn origin_selector(&self) -> &StructuralSelectorV1 {
        &self.origin_selector
    }

    /// Optional display-only storage selector.
    #[must_use]
    pub fn storage_display_selector(&self) -> Option<&str> {
        self.storage_display_selector.as_deref()
    }

    /// Exact normalized resolved-array DTO.
    #[must_use]
    pub const fn array(&self) -> &ResolvedArrayV1 {
        &self.array
    }
}

/// Complete nonserializable source/array observation used to build or replay a
/// manifest assertion.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalImportObservationV1 {
    metadata: ExternalImportSourceV1,
    external_sources: Vec<ExternalImportSourceV1>,
    mesh_geometry: ResolvedImportArrayV1,
    mesh_topology: ResolvedImportArrayV1,
    fields: Vec<ResolvedImportArrayV1>,
}

impl ExternalImportObservationV1 {
    /// Validate deterministic source-occurrence and normalized array order.
    ///
    /// Geometry is `f64`, topology is `u64`, and fields retain their resolved
    /// scalar grammar. External source ordinals must appear in first-use order
    /// and every declared occurrence must be used.
    ///
    /// # Errors
    /// Returns `EQ0901` for wrong source roles, duplicate structural selectors,
    /// dangling/out-of-order source references, or geometry/topology scalar
    /// mismatch.
    pub fn new(
        metadata: ExternalImportSourceV1,
        external_sources: Vec<ExternalImportSourceV1>,
        mesh_geometry: ResolvedImportArrayV1,
        mesh_topology: ResolvedImportArrayV1,
        fields: Vec<ResolvedImportArrayV1>,
    ) -> Result<Self, Diagnostic> {
        if metadata.kind != ExternalSourceKind::MetadataDocument
            || !metadata.origin_selector.is_root()
        {
            return Err(invalid_artifact(
                "external import source ordinal zero must be the root metadata document",
            ));
        }
        let mut source_selectors = BTreeSet::new();
        for source in &external_sources {
            if source.kind != ExternalSourceKind::ExternalArraySource {
                return Err(invalid_artifact(
                    "external import nonzero sources must be external-array occurrences",
                ));
            }
            if !source_selectors.insert(source.origin_selector.clone()) {
                return Err(invalid_artifact(
                    "external array source occurrence selectors must be unique",
                ));
            }
        }
        let source_count = external_sources
            .len()
            .checked_add(1)
            .ok_or_else(|| invalid_artifact("external import source count overflows usize"))?;
        u32::try_from(source_count)
            .map_err(|_| invalid_artifact("external import source count exceeds portable u32"))?;
        if mesh_geometry.array.scalar() != ResolvedArrayScalarV1::F64 {
            return Err(invalid_artifact(
                "resolved mesh geometry requires f64 scalar values",
            ));
        }
        if mesh_topology.array.scalar() != ResolvedArrayScalarV1::U64 {
            return Err(invalid_artifact(
                "resolved mesh topology requires u64 scalar values",
            ));
        }

        let ordered_arrays = std::iter::once(&mesh_geometry)
            .chain(std::iter::once(&mesh_topology))
            .chain(fields.iter());
        let mut array_selectors = BTreeSet::new();
        let mut seen_sources = vec![false; source_count];
        let mut next_external = 1_usize;
        for array in ordered_arrays {
            if !array_selectors.insert(array.origin_selector.clone()) {
                return Err(invalid_artifact(
                    "resolved import array origin selectors must be unique",
                ));
            }
            let source = usize::try_from(array.source_ordinal)
                .map_err(|_| invalid_artifact("resolved source ordinal exceeds local usize"))?;
            if source >= source_count {
                return Err(invalid_artifact(
                    "resolved import array references a missing source occurrence",
                ));
            }
            if source > 0 && !seen_sources[source] {
                if source != next_external {
                    return Err(invalid_artifact(
                        "external source ordinals must follow resolved-array first-use order",
                    ));
                }
                if external_sources[source - 1].origin_selector != array.origin_selector {
                    return Err(invalid_artifact(
                        "external source occurrence must be declared by its first resolved array origin",
                    ));
                }
                seen_sources[source] = true;
                next_external += 1;
            }
        }
        if next_external != source_count {
            return Err(invalid_artifact(
                "every external source occurrence must be used by a resolved array",
            ));
        }
        Ok(Self {
            metadata,
            external_sources,
            mesh_geometry,
            mesh_topology,
            fields,
        })
    }

    /// Metadata source at ordinal zero.
    #[must_use]
    pub const fn metadata(&self) -> &ExternalImportSourceV1 {
        &self.metadata
    }

    /// External source occurrences in deterministic first-use order.
    #[must_use]
    pub fn external_sources(&self) -> &[ExternalImportSourceV1] {
        &self.external_sources
    }

    /// Resolved mesh geometry array.
    #[must_use]
    pub const fn mesh_geometry(&self) -> &ResolvedImportArrayV1 {
        &self.mesh_geometry
    }

    /// Resolved mesh topology array.
    #[must_use]
    pub const fn mesh_topology(&self) -> &ResolvedImportArrayV1 {
        &self.mesh_topology
    }

    /// Resolved fields in explicit selected-attribute order.
    #[must_use]
    pub fn fields(&self) -> &[ResolvedImportArrayV1] {
        &self.fields
    }
}

fn validate_adapter_id(value: &str) -> Result<(), Diagnostic> {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(invalid_artifact("external adapter ID must not be empty"));
    };
    let last = value.as_bytes()[value.len() - 1];
    if !is_adapter_alphanumeric(first)
        || !is_adapter_alphanumeric(last)
        || !value
            .bytes()
            .all(|byte| is_adapter_alphanumeric(byte) || matches!(byte, b'.' | b'-'))
        || value
            .as_bytes()
            .windows(2)
            .any(|pair| matches!(pair, [b'.' | b'-', b'.' | b'-']))
    {
        return Err(invalid_artifact(
            "external adapter ID must be lowercase alphanumeric dotted/kebab ASCII",
        ));
    }
    Ok(())
}

fn is_adapter_alphanumeric(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit()
}

fn require_non_root(label: &str, selector: &StructuralSelectorV1) -> Result<(), Diagnostic> {
    if selector.is_root() {
        Err(invalid_artifact(format!(
            "{label} requires a non-root structural selector",
        )))
    } else {
        Ok(())
    }
}

fn validate_optional_text(label: &str, value: Option<&str>) -> Result<(), Diagnostic> {
    if let Some(value) = value {
        validate_text(label, value)?;
    }
    Ok(())
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("RawSourceSha256 contains validated lowercase hexadecimal"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selector(path: &[u32]) -> StructuralSelectorV1 {
        StructuralSelectorV1::new(path.to_vec())
    }

    fn array(source: u32, path: &[u32], value: ResolvedArrayV1) -> ResolvedImportArrayV1 {
        ResolvedImportArrayV1::new(source, selector(path), None, value).unwrap()
    }

    #[test]
    fn raw_source_digest_matches_an_unprefixed_oracle() {
        let source = b"complete source\0bytes";
        let digest = RawSourceSha256::from_source_bytes(source);
        assert_eq!(
            digest.sha256_bytes(),
            <[u8; 32]>::from(Sha256::digest(source))
        );
        assert_eq!(
            RawSourceSha256::from_hex(digest.to_string()).unwrap(),
            digest
        );
        assert!(RawSourceSha256::from_hex("AB").is_err());
    }

    #[test]
    fn observation_preserves_repeated_sources_by_first_use_not_content() {
        let metadata = ExternalImportSourceV1::metadata_document(b"xml".to_vec(), None).unwrap();
        let first = ExternalImportSourceV1::external_array_source(
            selector(&[1, 0]),
            b"same".to_vec(),
            Some("data.h5".to_owned()),
        )
        .unwrap();
        let second = ExternalImportSourceV1::external_array_source(
            selector(&[1, 1]),
            b"same".to_vec(),
            Some("data.h5".to_owned()),
        )
        .unwrap();
        let observation = ExternalImportObservationV1::new(
            metadata,
            vec![first, second],
            array(
                1,
                &[1, 0],
                ResolvedArrayV1::from_f64(vec![1, 2], vec![0.0, 1.0]).unwrap(),
            ),
            array(
                2,
                &[1, 1],
                ResolvedArrayV1::from_u64(vec![1, 2], vec![0, 1]).unwrap(),
            ),
            vec![array(
                1,
                &[1, 2],
                ResolvedArrayV1::from_f64(vec![2], vec![2.0, 3.0]).unwrap(),
            )],
        )
        .unwrap();
        assert_eq!(observation.external_sources().len(), 2);
        assert_eq!(
            observation.external_sources()[0].digest(),
            observation.external_sources()[1].digest()
        );
    }

    #[test]
    fn observation_rejects_dangling_unused_and_out_of_order_sources() {
        let build = |topology_source| {
            ExternalImportObservationV1::new(
                ExternalImportSourceV1::metadata_document(Vec::new(), None).unwrap(),
                vec![
                    ExternalImportSourceV1::external_array_source(selector(&[2]), vec![1], None)
                        .unwrap(),
                    ExternalImportSourceV1::external_array_source(selector(&[3]), vec![2], None)
                        .unwrap(),
                ],
                array(
                    1,
                    &[2],
                    ResolvedArrayV1::from_f64(vec![1], vec![0.0]).unwrap(),
                ),
                array(
                    topology_source,
                    &[3],
                    ResolvedArrayV1::from_u64(vec![1], vec![0]).unwrap(),
                ),
                Vec::new(),
            )
        };
        assert!(build(0).unwrap_err().message().contains("every external"));
        assert!(build(3).unwrap_err().message().contains("missing source"));

        let out_of_order = ExternalImportObservationV1::new(
            ExternalImportSourceV1::metadata_document(Vec::new(), None).unwrap(),
            vec![
                ExternalImportSourceV1::external_array_source(selector(&[3]), vec![1], None)
                    .unwrap(),
                ExternalImportSourceV1::external_array_source(selector(&[2]), vec![2], None)
                    .unwrap(),
            ],
            array(
                2,
                &[2],
                ResolvedArrayV1::from_f64(vec![1], vec![0.0]).unwrap(),
            ),
            array(
                1,
                &[3],
                ResolvedArrayV1::from_u64(vec![1], vec![0]).unwrap(),
            ),
            Vec::new(),
        )
        .unwrap_err();
        assert!(out_of_order.message().contains("first-use order"));
    }

    #[test]
    fn selection_is_structural_and_display_names_may_repeat() {
        let grid = SelectedSourceEntityV1::new(selector(&[0]), Some("same".to_owned())).unwrap();
        let first = SelectedSourceEntityV1::new(selector(&[1]), Some("same".to_owned())).unwrap();
        let second = SelectedSourceEntityV1::new(selector(&[2]), Some("same".to_owned())).unwrap();
        assert!(ExternalImportSelectionV1::new(grid.clone(), vec![first, second]).is_ok());
        assert!(ExternalImportSelectionV1::new(grid.clone(), vec![grid]).is_err());
    }

    #[test]
    fn adapter_and_text_inputs_fail_closed() {
        for id in ["", ".xdmf", "xdmf-", "Xdmf", "xdmf__reader", "xdmf..reader"] {
            assert!(ExternalAdapterIdentityV1::new(id, "1").is_err());
        }
        assert!(ExternalAdapterIdentityV1::new("eqiora.xdmf-reader", "1").is_ok());
        assert!(
            ExternalRuntimeComponentV1::new(
                ExternalRuntimeRoleV1::RustBinding,
                "binding\nname",
                "1",
            )
            .is_err()
        );
    }
}
