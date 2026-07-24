use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::{ContractError, PackageReleaseV1, SourceBundleDigest};

/// A source-bundle-addressed store with no discovery or fallback operation.
pub trait PackageStore {
    /// Loads only `expected`, refusing an entry larger than `max_bytes` before
    /// copying or reading its payload into memory.
    fn load_exact(
        &self,
        expected: SourceBundleDigest,
        max_bytes: usize,
    ) -> Result<Option<Vec<u8>>, StoreError>;
}

#[derive(Debug)]
#[non_exhaustive]
pub enum StoreError {
    /// Opening or inspecting the supplied store root capability failed.
    RootIo {
        /// Ambient root path, when the adapter opened it for the caller.
        path: Option<PathBuf>,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// A handle-relative read of one exact content-addressed entry failed.
    EntryIo {
        /// Expected source-bundle digest naming the entry.
        digest: SourceBundleDigest,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// A decoded package release violated its closed contract.
    Contract(ContractError),
    /// Distinct release bytes claimed one source-bundle digest.
    DigestCollision(SourceBundleDigest),
    /// The caller-supplied root handle does not identify a directory.
    RootNotDirectory {
        /// Ambient root path, when the adapter opened it for the caller.
        path: Option<PathBuf>,
    },
    /// An exact content-addressed entry is not a regular file.
    NonRegularEntry(SourceBundleDigest),
    /// An exact entry exceeded the active read budget.
    ReleaseTooLarge {
        /// Expected source-bundle digest naming the entry.
        digest: SourceBundleDigest,
        /// Observed metadata length or first byte beyond the limit.
        observed: u64,
        /// Maximum accepted entry length.
        limit: u64,
    },
    /// A bounded owned buffer could not be reserved.
    Allocation {
        /// Expected source-bundle digest naming the entry.
        digest: SourceBundleDigest,
        /// Allocation failure reported by the standard collection contract.
        source: std::collections::TryReserveError,
    },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootIo { path, source } => match path {
                Some(path) => write!(
                    formatter,
                    "cannot open package store root {}: {source}",
                    path.display()
                ),
                None => write!(formatter, "cannot inspect package store root: {source}"),
            },
            Self::EntryIo { digest, source } => {
                write!(
                    formatter,
                    "cannot read package store entry `{digest}`: {source}"
                )
            }
            Self::Contract(error) => write!(formatter, "invalid package release: {error}"),
            Self::DigestCollision(digest) => write!(
                formatter,
                "distinct package release bytes share source-bundle digest `{digest}`"
            ),
            Self::RootNotDirectory { path } => match path {
                Some(path) => write!(
                    formatter,
                    "package store root {} must be a directory",
                    path.display()
                ),
                None => formatter.write_str("package store root must be a directory"),
            },
            Self::NonRegularEntry(digest) => write!(
                formatter,
                "package store entry `{digest}` must be a regular file"
            ),
            Self::ReleaseTooLarge {
                digest,
                observed,
                limit,
            } => write!(
                formatter,
                "package store entry `{digest}` has {observed} bytes, exceeding the limit {limit}"
            ),
            Self::Allocation { digest, source } => write!(
                formatter,
                "cannot allocate package store entry `{digest}`: {source}"
            ),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootIo { source, .. } | Self::EntryIo { source, .. } => Some(source),
            Self::Contract(error) => Some(error),
            Self::Allocation { source, .. } => Some(source),
            Self::DigestCollision(_)
            | Self::RootNotDirectory { .. }
            | Self::NonRegularEntry(_)
            | Self::ReleaseTooLarge { .. } => None,
        }
    }
}

impl From<ContractError> for StoreError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryPackageStore {
    releases: BTreeMap<SourceBundleDigest, Vec<u8>>,
}

impl InMemoryPackageStore {
    pub fn insert(&mut self, release: &PackageReleaseV1) -> Result<SourceBundleDigest, StoreError> {
        let digest = release.source_digest()?;
        let bytes = release.canonical_json()?;
        match self.releases.entry(digest) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(bytes);
            }
            std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &bytes => {}
            std::collections::btree_map::Entry::Occupied(_) => {
                return Err(StoreError::DigestCollision(digest));
            }
        }
        Ok(digest)
    }

    /// Inserts caller-supplied bytes under an expected key for adversarial tests
    /// and external stores. The resolver still verifies the bytes before use.
    pub fn insert_unchecked(&mut self, key: SourceBundleDigest, bytes: Vec<u8>) {
        self.releases.insert(key, bytes);
    }
}

impl PackageStore for InMemoryPackageStore {
    fn load_exact(
        &self,
        expected: SourceBundleDigest,
        max_bytes: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(bytes) = self.releases.get(&expected) else {
            return Ok(None);
        };
        if bytes.len() > max_bytes {
            return Err(StoreError::ReleaseTooLarge {
                digest: expected,
                observed: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                limit: u64::try_from(max_bytes).unwrap_or(u64::MAX),
            });
        }
        let mut copy = Vec::new();
        copy.try_reserve_exact(bytes.len())
            .map_err(|source| StoreError::Allocation {
                digest: expected,
                source,
            })?;
        copy.extend_from_slice(bytes);
        Ok(Some(copy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_enforces_the_callers_load_budget_before_clone() {
        let digest = SourceBundleDigest::parse(&"12".repeat(32)).expect("digest");
        let mut store = InMemoryPackageStore::default();
        store.insert_unchecked(digest, vec![0_u8; 5]);
        assert!(matches!(
            store.load_exact(digest, 4),
            Err(StoreError::ReleaseTooLarge {
                digest: actual,
                observed: 5,
                limit: 4,
            }) if actual == digest
        ));
        assert_eq!(store.load_exact(digest, 5).expect("load"), Some(vec![0; 5]));
    }
}
