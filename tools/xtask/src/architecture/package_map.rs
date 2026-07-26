//! Which package owns a source file, and what its dependency names mean.
//!
//! A glob's frozen identity has to name the package it really forwards, not the
//! text written in front of the `::*`. That text is a name in the extern
//! prelude of whichever crate the file belongs to, and Cargo lets a manifest
//! point that name at any package at all — `foo = { package = "bar" }` makes
//! `pub use foo::*;` forward `bar`. The mapping is a fact only the manifests
//! hold, and it is exactly the fact a repointed glob changes without touching a
//! line of Rust.
//!
//! The identity deliberately records the package's *name and origin*, not its
//! version. A version bump is not a repoint: including it would fail the check
//! on every ordinary `cargo update` or release bump and force a reseed of every
//! frozen glob, which is how a ratchet loses its credibility. Swapping the
//! origin — path to registry, one path to another, one git rev to another — is
//! a repoint, and does move the identity.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use super::cargo_metadata;

/// One workspace member: where its sources live, and what its dependency names
/// resolve to.
pub(super) struct Package {
    /// Repository-relative manifest directory, used to attribute a file.
    directory: String,
    identity: String,
    /// Extern-prelude name to package identity. The value is a set because a
    /// manifest may declare the same name as a normal, build and dev
    /// dependency; if those ever named different packages the identity must
    /// show all of them rather than silently pick one.
    dependencies: BTreeMap<String, BTreeSet<String>>,
}

impl Package {
    /// How this package is named when it is itself the target of a path.
    pub(super) fn identity(&self) -> &str {
        &self.identity
    }

    /// Resolves an extern-prelude name. Cargo accepts `-` in a package name but
    /// Rust cannot spell it, so the prelude is keyed by the underscored form.
    pub(super) fn dependency(&self, name: &str) -> Option<String> {
        let candidates = self.dependencies.get(name)?;
        Some(candidates.iter().cloned().collect::<Vec<_>>().join(" or "))
    }
}

/// Every workspace member, ready to attribute a repository-relative path.
pub(super) struct PackageMap {
    packages: Vec<Package>,
}

impl PackageMap {
    pub(super) fn load(root: &Path, metadata: &Value) -> Result<Self, String> {
        let mut packages = Vec::new();
        for package in cargo_metadata::members(metadata)? {
            let name = package["name"]
                .as_str()
                .ok_or_else(|| "a workspace member has no name".to_owned())?;
            let manifest = package["manifest_path"]
                .as_str()
                .ok_or_else(|| format!("{name} has no manifest_path"))?;
            let directory = relative(
                Path::new(manifest)
                    .parent()
                    .ok_or_else(|| format!("{name} has a manifest with no directory"))?
                    .to_string_lossy()
                    .as_ref(),
                root,
            );
            let identity = format!("{name} (path {directory})");
            packages.push(Package {
                directory,
                identity,
                dependencies: dependencies(package, root),
            });
        }

        if packages.is_empty() {
            return Err("cargo metadata reported no workspace members".to_owned());
        }
        Ok(Self { packages })
    }

    /// The member whose directory is the longest prefix of `relative`. Longest
    /// wins so a nested member is attributed to itself rather than to whichever
    /// ancestor happens to come first.
    pub(super) fn owner(&self, relative: &str) -> Option<&Package> {
        self.packages
            .iter()
            .filter(|package| relative.starts_with(&format!("{}/", package.directory)))
            .max_by_key(|package| package.directory.len())
    }

    #[cfg(test)]
    pub(super) fn fixture(directory: &str, name: &str, dependencies: &[(&str, &str)]) -> Self {
        let mut table: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (extern_name, identity) in dependencies {
            table
                .entry((*extern_name).to_owned())
                .or_default()
                .insert((*identity).to_owned());
        }
        Self {
            packages: vec![Package {
                directory: directory.to_owned(),
                identity: format!("{name} (path {directory})"),
                dependencies: table,
            }],
        }
    }
}

/// Both dependency kinds and both rename mechanisms end up here: `rename` is
/// what Cargo puts in the extern prelude when `package = "..."` is used, and
/// `name` is the package that name actually resolves to.
fn dependencies(package: &Value, root: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut table: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let Some(declared) = package["dependencies"].as_array() else {
        return table;
    };

    for dependency in declared {
        let Some(name) = dependency["name"].as_str() else {
            continue;
        };
        let extern_name = dependency["rename"]
            .as_str()
            .unwrap_or(name)
            .replace('-', "_");
        table
            .entry(extern_name)
            .or_default()
            .insert(identity(name, dependency, root));
    }
    table
}

/// Name plus origin. A path dependency is recorded by its repository-relative
/// directory so moving a crate is visible; a registry or git dependency is
/// recorded by the source string Cargo resolved, which carries the registry URL
/// and, for git, the locked revision.
fn identity(name: &str, dependency: &Value, root: &Path) -> String {
    let origin = match (dependency["path"].as_str(), dependency["source"].as_str()) {
        (Some(path), _) => format!("path {}", relative(path, root)),
        (None, Some(source)) => source.to_owned(),
        (None, None) => "unknown origin".to_owned(),
    };
    format!("{name} ({origin})")
}

/// Cargo reports absolute paths; the ledger stores repository-relative ones so
/// a frozen identity does not depend on where the checkout lives.
fn relative(path: &str, root: &Path) -> String {
    Path::new(path)
        .strip_prefix(root)
        .map(|stripped| stripped.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> Value {
        serde_json::json!({
            "workspace_members": ["a 0.1.0 (path+file:///w/crates/a)"],
            "packages": [{
                "id": "a 0.1.0 (path+file:///w/crates/a)",
                "name": "a",
                "manifest_path": "/w/crates/a/Cargo.toml",
                "dependencies": [
                    {"name": "b", "rename": null, "source": null, "path": "/w/crates/b"},
                    {"name": "real-package", "rename": "alias", "source": null,
                     "path": "/w/crates/real"},
                    {"name": "serde", "rename": null,
                     "source": "registry+https://github.com/rust-lang/crates.io-index"}
                ]
            }]
        })
    }

    #[test]
    fn a_path_dependency_is_identified_by_its_repository_relative_directory() {
        let map = PackageMap::load(Path::new("/w"), &metadata()).expect("fixture loads");
        let package = map
            .owner("crates/a/src/lib.rs")
            .expect("file is attributed");
        assert_eq!(
            package.dependency("b").as_deref(),
            Some("b (path crates/b)")
        );
    }

    #[test]
    fn a_cargo_rename_resolves_to_the_package_it_actually_names() {
        let map = PackageMap::load(Path::new("/w"), &metadata()).expect("fixture loads");
        let package = map
            .owner("crates/a/src/lib.rs")
            .expect("file is attributed");
        assert_eq!(
            package.dependency("alias").as_deref(),
            Some("real-package (path crates/real)")
        );
        assert!(package.dependency("real_package").is_none());
    }

    #[test]
    fn a_registry_dependency_keeps_its_source_url() {
        let map = PackageMap::load(Path::new("/w"), &metadata()).expect("fixture loads");
        let package = map
            .owner("crates/a/src/lib.rs")
            .expect("file is attributed");
        assert_eq!(
            package.dependency("serde").as_deref(),
            Some("serde (registry+https://github.com/rust-lang/crates.io-index)")
        );
    }

    #[test]
    fn a_file_outside_every_member_has_no_owner() {
        let map = PackageMap::load(Path::new("/w"), &metadata()).expect("fixture loads");
        assert!(map.owner("tools/other/src/lib.rs").is_none());
    }

    #[test]
    fn the_owning_package_names_itself() {
        let map = PackageMap::load(Path::new("/w"), &metadata()).expect("fixture loads");
        let package = map
            .owner("crates/a/src/lib.rs")
            .expect("file is attributed");
        assert_eq!(package.identity(), "a (path crates/a)");
    }
}
