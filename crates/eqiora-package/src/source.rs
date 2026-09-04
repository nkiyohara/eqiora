use std::collections::BTreeMap;
use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::canonical;
use crate::{
    BundleRoleV1, ContractError, ModelPackageIdentityV1, NormalizedRelativePath, PackageManifestV1,
    SourceBundleDigest,
};

const SCHEMA: &str = "eqiora.source-bundle.v1";
const MAX_FILES: usize = 65_536;
pub(crate) const MAX_TOTAL_BYTES: usize = 256 * 1024 * 1024;

fn max_encoded_bytes(decoded: usize) -> usize {
    decoded.div_ceil(3) * 4
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalBytes(Vec<u8>);

impl Serialize for CanonicalBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&Base64Display(&self.0))
    }
}

struct Base64Display<'a>(&'a [u8]);

impl fmt::Display for Base64Display<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        const INPUT_CHUNK_BYTES: usize = 3 * 1_024;
        const OUTPUT_CHUNK_BYTES: usize = 4 * 1_024;

        let mut output = [0_u8; OUTPUT_CHUNK_BYTES];
        for input in self.0.chunks(INPUT_CHUNK_BYTES) {
            let encoded = BASE64_STANDARD
                .encode_slice(input, &mut output)
                .map_err(|_| fmt::Error)?;
            let text = std::str::from_utf8(&output[..encoded]).map_err(|_| fmt::Error)?;
            formatter.write_str(text)?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for CanonicalBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CanonicalBytesVisitor)
    }
}

struct CanonicalBytesVisitor;

impl<'de> Visitor<'de> for CanonicalBytesVisitor {
    type Value = CanonicalBytes;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("canonical padded base64 source bytes")
    }

    fn visit_borrowed_str<E>(self, encoded: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        decode_canonical_bytes(encoded).map_err(E::custom)
    }

    fn visit_str<E>(self, encoded: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        decode_canonical_bytes(encoded).map_err(E::custom)
    }

    fn visit_string<E>(self, encoded: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        decode_canonical_bytes(&encoded).map_err(E::custom)
    }
}

fn decode_canonical_bytes(encoded: &str) -> Result<CanonicalBytes, String> {
    if encoded.len() > max_encoded_bytes(MAX_TOTAL_BYTES) {
        return Err("source payload exceeds the encoded byte limit".to_owned());
    }
    BASE64_STANDARD
        .decode(encoded)
        .map(CanonicalBytes)
        .map_err(|error| format!("source bytes must use canonical padded base64: {error}"))
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFileV1 {
    path: NormalizedRelativePath,
    role: BundleRoleV1,
    bytes: CanonicalBytes,
}

impl SourceFileV1 {
    #[must_use]
    pub fn new(path: NormalizedRelativePath, role: BundleRoleV1, bytes: Vec<u8>) -> Self {
        Self {
            path,
            role,
            bytes: CanonicalBytes(bytes),
        }
    }

    #[must_use]
    pub fn path(&self) -> &NormalizedRelativePath {
        &self.path
    }

    #[must_use]
    pub fn role(&self) -> BundleRoleV1 {
        self.role
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes.0
    }
}

/// Bounded author inputs admitted before compiler-derived package semantics
/// exist.
///
/// This is an in-memory construction boundary, not a durable artifact or an
/// alternative source-bundle identity. It guarantees exact manifest/file
/// inventory, canonical ordering, UTF-8 model sources, and aggregate resource
/// limits so compiler composition never starts from an unchecked byte bag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageSourcesV1 {
    manifest: PackageManifestV1,
    files: Vec<SourceFileV1>,
}

impl PackageSourcesV1 {
    /// Admit one exact package manifest and its complete file inventory.
    ///
    /// # Errors
    ///
    /// Returns a package contract error when the manifest is invalid, file
    /// paths or roles differ from its inventory, a model source is not UTF-8,
    /// or a v1 resource bound is exceeded.
    pub fn new(
        manifest: PackageManifestV1,
        files: Vec<SourceFileV1>,
    ) -> Result<Self, ContractError> {
        let manifest = manifest.normalize()?;
        let files = normalize_author_files(&manifest, files)?;
        Ok(Self { manifest, files })
    }

    /// Canonical package manifest retained by this admitted input.
    #[must_use]
    pub const fn manifest(&self) -> &PackageManifestV1 {
        &self.manifest
    }

    /// Complete source and documentation files in canonical path order.
    #[must_use]
    pub fn files(&self) -> &[SourceFileV1] {
        &self.files
    }

    /// Transfer the validated parts to the compiler/package composition layer.
    #[must_use]
    pub fn into_parts(self) -> (PackageManifestV1, Vec<SourceFileV1>) {
        (self.manifest, self.files)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBundleIdentityV1 {
    pub package: ModelPackageIdentityV1,
    pub source_digest: SourceBundleDigest,
}

/// Exact source, package-manifest, and diagnostic payload for one semantic
/// package identity.
///
/// The manifest is inside this digest domain because exact dependency targets
/// and inventory roles affect exact resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBundleV1 {
    schema: String,
    package: ModelPackageIdentityV1,
    manifest: PackageManifestV1,
    files: Vec<SourceFileV1>,
}

impl SourceBundleV1 {
    pub fn new(
        package: ModelPackageIdentityV1,
        manifest: PackageManifestV1,
        files: Vec<SourceFileV1>,
    ) -> Result<Self, ContractError> {
        Self {
            schema: SCHEMA.to_owned(),
            package,
            manifest,
            files,
        }
        .normalize()
    }

    pub(crate) fn normalize(mut self) -> Result<Self, ContractError> {
        if self.schema != SCHEMA {
            return Err(ContractError::new(format!(
                "unsupported source bundle schema `{}`",
                self.schema
            )));
        }
        let sources = PackageSourcesV1::new(self.manifest, self.files)?;
        (self.manifest, self.files) = sources.into_parts();
        if self.package.name != *self.manifest.name()
            || self.package.version != *self.manifest.version()
        {
            return Err(ContractError::new(
                "source bundle package name/version does not match its package manifest",
            ));
        }
        Ok(self)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, ContractError> {
        canonical::from_slice::<Self>(bytes)?.normalize()
    }

    pub fn canonical_json(&self) -> Result<Vec<u8>, ContractError> {
        canonical::to_bytes(self)
    }

    pub fn identity(&self) -> Result<SourceBundleIdentityV1, ContractError> {
        let bytes = self.canonical_json()?;
        Ok(SourceBundleIdentityV1 {
            package: self.package.clone(),
            source_digest: SourceBundleDigest::compute(&bytes),
        })
    }

    #[must_use]
    pub fn package(&self) -> &ModelPackageIdentityV1 {
        &self.package
    }

    #[must_use]
    pub fn manifest(&self) -> &PackageManifestV1 {
        &self.manifest
    }

    #[must_use]
    pub fn files(&self) -> &[SourceFileV1] {
        &self.files
    }
}

fn normalize_author_files(
    manifest: &PackageManifestV1,
    mut files: Vec<SourceFileV1>,
) -> Result<Vec<SourceFileV1>, ContractError> {
    if files.len() > MAX_FILES {
        return Err(ContractError::new("source bundle exceeds file-count limit"));
    }
    let mut total = 0_usize;
    let mut has_model_source = false;
    for file in &files {
        total = total
            .checked_add(file.bytes.0.len())
            .ok_or_else(|| ContractError::new("source bundle byte count overflow"))?;
        if file.role == BundleRoleV1::ModelSource {
            has_model_source = true;
            std::str::from_utf8(&file.bytes.0).map_err(|error| {
                ContractError::new(format!(
                    "model source `{}` is not valid UTF-8: {error}",
                    file.path
                ))
            })?;
        }
    }
    if !has_model_source {
        return Err(ContractError::new(
            "source bundle must contain at least one model source",
        ));
    }
    if total > MAX_TOTAL_BYTES {
        return Err(ContractError::new("source bundle exceeds total byte limit"));
    }
    files.sort();
    for pair in files.windows(2) {
        if pair[0].path == pair[1].path {
            return Err(ContractError::new(format!(
                "duplicate source file path `{}`",
                pair[0].path
            )));
        }
    }
    let expected: BTreeMap<_, _> = manifest
        .bundle()
        .iter()
        .map(|entry| (entry.path(), entry.role()))
        .collect();
    let actual: BTreeMap<_, _> = files
        .iter()
        .map(|file| (file.path(), file.role()))
        .collect();
    if expected != actual {
        return Err(ContractError::new(
            "source bundle files do not exactly match the author inventory",
        ));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BundleEntryV1, ExactVersion, PackageSemanticDigest, QualifiedName};

    fn setup() -> (PackageManifestV1, ModelPackageIdentityV1) {
        let manifest = PackageManifestV1::new(
            "basic",
            QualifiedName::parse("org.example.Basic").expect("name"),
            ExactVersion::parse("1.0.0").expect("version"),
            vec![],
            vec![
                BundleEntryV1::new(
                    NormalizedRelativePath::parse("README.md").expect("path"),
                    BundleRoleV1::Documentation,
                ),
                BundleEntryV1::new(
                    NormalizedRelativePath::parse("src/basic.eqi").expect("path"),
                    BundleRoleV1::ModelSource,
                ),
            ],
        )
        .expect("manifest");
        let identity = ModelPackageIdentityV1::new(
            manifest.name().clone(),
            manifest.version().clone(),
            PackageSemanticDigest::parse(&"12".repeat(32)).expect("digest"),
        );
        (manifest, identity)
    }

    #[test]
    fn source_digest_is_order_independent_and_byte_exact() {
        let (manifest, identity) = setup();
        let source = SourceFileV1::new(
            NormalizedRelativePath::parse("src/basic.eqi").expect("path"),
            BundleRoleV1::ModelSource,
            b"model Basic {}\n".to_vec(),
        );
        let docs = SourceFileV1::new(
            NormalizedRelativePath::parse("README.md").expect("path"),
            BundleRoleV1::Documentation,
            b"docs\n".to_vec(),
        );
        let first = SourceBundleV1::new(
            identity.clone(),
            manifest.clone(),
            vec![source.clone(), docs.clone()],
        )
        .expect("bundle");
        let second = SourceBundleV1::new(
            identity.clone(),
            manifest.clone(),
            vec![docs.clone(), source],
        )
        .expect("bundle");
        let changed = SourceBundleV1::new(
            identity,
            manifest,
            vec![
                docs,
                SourceFileV1::new(
                    NormalizedRelativePath::parse("src/basic.eqi").expect("path"),
                    BundleRoleV1::ModelSource,
                    b"model  Basic {}\n".to_vec(),
                ),
            ],
        )
        .expect("bundle");
        assert_eq!(first.identity(), second.identity());
        assert_ne!(first.identity(), changed.identity());
        assert_eq!(
            SourceBundleV1::from_json(&first.canonical_json().expect("JSON")),
            Ok(first)
        );
    }

    #[test]
    fn source_bytes_stream_canonical_base64_without_changing_the_wire() {
        let bytes: Vec<_> = (0_u8..=255).cycle().take(3 * 1_024 + 2).collect();
        let file = SourceFileV1::new(
            NormalizedRelativePath::parse("src/stream.eqi").expect("path"),
            BundleRoleV1::ModelSource,
            bytes.clone(),
        );
        let wire = canonical::to_bytes(&file).expect("streamed source wire");
        let value: serde_json::Value = serde_json::from_slice(&wire).expect("source JSON");
        assert_eq!(
            value["bytes"].as_str().expect("base64 string"),
            BASE64_STANDARD.encode(&bytes)
        );
        assert_eq!(
            canonical::from_slice::<SourceFileV1>(&wire).expect("borrowed decode"),
            file
        );

        let large = SourceFileV1::new(
            NormalizedRelativePath::parse("src/large.eqi").expect("path"),
            BundleRoleV1::ModelSource,
            vec![0_u8; 64 * 1_024],
        );
        assert!(canonical::encoded_len_with_limit(&large, 32).is_err());
        assert!(canonical::to_bytes_with_limit(&large, 32).is_err());

        for noncanonical in ["YQ", "YR=="] {
            let wire =
                format!(r#"{{"path":"src/a.eqi","role":"model_source","bytes":"{noncanonical}"}}"#);
            assert!(canonical::from_slice::<SourceFileV1>(wire.as_bytes()).is_err());
        }
    }

    #[test]
    fn author_sources_canonicalize_order_and_reject_inventory_drift() {
        let (manifest, _) = setup();
        let source = SourceFileV1::new(
            NormalizedRelativePath::parse("src/basic.eqi").expect("path"),
            BundleRoleV1::ModelSource,
            b"model Basic {}\n".to_vec(),
        );
        let docs = SourceFileV1::new(
            NormalizedRelativePath::parse("README.md").expect("path"),
            BundleRoleV1::Documentation,
            b"docs\n".to_vec(),
        );

        let admitted = PackageSourcesV1::new(manifest.clone(), vec![source.clone(), docs.clone()])
            .expect("admitted author sources");
        assert_eq!(admitted.files(), &[docs.clone(), source.clone()]);

        let missing = PackageSourcesV1::new(manifest.clone(), vec![source.clone()])
            .expect_err("missing inventory entry must fail");
        assert!(missing.to_string().contains("exactly match"));

        let wrong_role = PackageSourcesV1::new(
            manifest.clone(),
            vec![
                SourceFileV1::new(
                    docs.path().clone(),
                    BundleRoleV1::ModelSource,
                    docs.bytes().to_vec(),
                ),
                source,
            ],
        )
        .expect_err("role drift must fail");
        assert!(wrong_role.to_string().contains("exactly match"));

        let invalid_utf8 = PackageSourcesV1::new(
            manifest,
            vec![
                docs,
                SourceFileV1::new(
                    NormalizedRelativePath::parse("src/basic.eqi").expect("path"),
                    BundleRoleV1::ModelSource,
                    vec![0xff],
                ),
            ],
        )
        .expect_err("invalid UTF-8 must fail before semantic derivation");
        assert!(invalid_utf8.to_string().contains("not valid UTF-8"));
    }

    #[test]
    fn model_source_must_be_utf8() {
        let (manifest, identity) = setup();
        let result = SourceBundleV1::new(
            identity,
            manifest,
            vec![
                SourceFileV1::new(
                    NormalizedRelativePath::parse("README.md").expect("path"),
                    BundleRoleV1::Documentation,
                    vec![],
                ),
                SourceFileV1::new(
                    NormalizedRelativePath::parse("src/basic.eqi").expect("path"),
                    BundleRoleV1::ModelSource,
                    vec![0xff],
                ),
            ],
        );
        assert!(result.is_err());
    }
}
