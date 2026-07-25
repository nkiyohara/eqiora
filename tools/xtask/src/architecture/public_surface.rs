//! AST-reachable public item counts, one number per crate.
//!
//! "Reachable" is the property that matters: a `pub` item inside a private
//! module is not part of the surface a downstream crate can name, and counting
//! it would tax a crate for its own internal structure. So the walk starts at
//! `src/lib.rs` and descends only where a consumer's path resolution would:
//! through `pub mod`, and into a private module that a `pub use` re-exports.
//!
//! Counted as one item each: `pub fn`, `pub struct`, `pub enum`, `pub union`,
//! `pub trait` (including trait aliases), `pub type`, `pub const`, `pub static`,
//! and every leaf of a `pub use` tree. Deliberately *not* counted: associated
//! items in `impl` and `trait` bodies, `pub mod` itself, and exported
//! `macro_rules!`. The predicate measures how many names a crate hands out, not
//! how large each name is; adding methods to an existing type is the kind of
//! growth it is supposed to permit.
//!
//! A `pub use inner::*;` over a module of the same crate is resolved and its
//! names counted: that is how `eqiora-api` publishes most of its surface, and
//! charging one item for it would leave the budget measuring almost nothing.
//! A glob over another crate cannot be resolved here and counts as one item,
//! which is a real undercount — and is precisely why the glob ban is a sibling
//! predicate rather than a nicety. No new glob may appear, so the blind spot
//! cannot widen.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use syn::{Attribute, Expr, Item, ItemMod, Lit, Meta, UseTree, Visibility};

/// One crate's measured public surface.
pub(super) struct CrateSurface {
    pub(super) name: String,
    pub(super) items: usize,
}

#[derive(Deserialize)]
struct Manifest {
    package: ManifestPackage,
}

#[derive(Deserialize)]
struct ManifestPackage {
    name: String,
}

/// Measures every library crate under `crates/`, keyed by the name in its
/// manifest rather than by directory, so a ledger entry cannot drift away from
/// the crate it claims to describe.
pub(super) fn measure(root: &Path) -> Result<Vec<CrateSurface>, String> {
    let directory = root.join("crates");
    let entries = fs::read_dir(&directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;

    let mut surfaces = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read a directory entry: {error}"))?;
        let crate_root = entry.path();
        let lib = crate_root.join("src/lib.rs");
        if !lib.is_file() {
            continue;
        }
        surfaces.push(CrateSurface {
            name: manifest_name(&crate_root)?,
            items: count_from_root(&lib)?,
        });
    }

    surfaces.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(surfaces)
}

fn manifest_name(crate_root: &Path) -> Result<String, String> {
    let path = crate_root.join("Cargo.toml");
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let manifest: Manifest = toml::from_str(&text)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    Ok(manifest.package.name)
}

fn count_from_root(lib: &Path) -> Result<usize, String> {
    let mut walk = Walk {
        visited: BTreeSet::new(),
        items: 0,
    };
    let directory = lib
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", lib.display()))?
        .to_path_buf();
    walk.file(lib, &directory)?;
    Ok(walk.items)
}

struct Walk {
    /// Loop protection. `#[path]` can point two module declarations at one
    /// file, and a self-referential pair would otherwise recurse forever.
    visited: BTreeSet<PathBuf>,
    items: usize,
}

impl Walk {
    fn file(&mut self, path: &Path, children: &Path) -> Result<(), String> {
        if !self.visited.insert(path.to_path_buf()) {
            return Ok(());
        }
        let text = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let parsed = syn::parse_file(&text)
            .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
        self.items(&parsed.items, children, path)
    }

    fn items(&mut self, items: &[Item], children: &Path, origin: &Path) -> Result<(), String> {
        // Two passes, because `pub use inner::*;` may precede `mod inner;` and
        // both orders name the same module.
        let siblings: BTreeMap<String, &ItemMod> = items
            .iter()
            .filter_map(|item| match item {
                Item::Mod(module) if !is_cfg_test(&module.attrs) => {
                    Some((module.ident.to_string(), module))
                }
                _ => None,
            })
            .collect();

        for item in items {
            match item {
                Item::Mod(module) => self.module(module, children, origin)?,
                Item::Use(use_item) if is_public(&use_item.vis) => {
                    self.re_exports(&use_item.tree, &[], &siblings, children, origin)?;
                }
                other => self.items += public_items(other),
            }
        }
        Ok(())
    }

    fn module(&mut self, module: &ItemMod, children: &Path, origin: &Path) -> Result<(), String> {
        if !matches!(module.vis, Visibility::Public(_)) || is_cfg_test(&module.attrs) {
            return Ok(());
        }
        self.descend(module, children, origin)
    }

    /// Counts a module's public items regardless of the module's own
    /// visibility, for the case where a re-export is what makes it reachable.
    fn descend(&mut self, module: &ItemMod, children: &Path, origin: &Path) -> Result<(), String> {
        let name = module.ident.to_string();
        match &module.content {
            Some((_, inner)) => {
                let path = children.join(&name);
                if !self.visited.insert(path.clone()) {
                    return Ok(());
                }
                self.items(inner, &path, origin)
            }
            None => {
                let (file, grandchildren) = resolve(children, &name, &module.attrs, origin)?;
                self.file(&file, &grandchildren)
            }
        }
    }

    /// Walks one `pub use` tree, accumulating the path segments so a glob can
    /// be resolved rather than guessed at.
    fn re_exports(
        &mut self,
        tree: &UseTree,
        prefix: &[String],
        siblings: &BTreeMap<String, &ItemMod>,
        children: &Path,
        origin: &Path,
    ) -> Result<(), String> {
        match tree {
            UseTree::Path(path) => {
                let mut extended = prefix.to_vec();
                extended.push(path.ident.to_string());
                self.re_exports(&path.tree, &extended, siblings, children, origin)
            }
            UseTree::Group(group) => group
                .items
                .iter()
                .try_for_each(|branch| self.re_exports(branch, prefix, siblings, children, origin)),
            UseTree::Name(_) | UseTree::Rename(_) => {
                self.items += 1;
                Ok(())
            }
            UseTree::Glob(_) => self.glob(prefix, siblings, children, origin),
        }
    }

    /// `pub use inner::*;` over a module of this crate is counted by walking
    /// `inner`, because those names really are handed out — `eqiora-api`
    /// publishes most of its surface this way, and charging one item for it
    /// would make the budget measure nothing.
    ///
    /// A glob over anything else — another crate, `crate::`, `super::`, a
    /// deeper path — is counted as one item, which is a known undercount. That
    /// is why `[[glob_reexports]]` freezes every glob at an exact count: this
    /// predicate cannot see through the remaining ones, so no new one is
    /// allowed to appear.
    fn glob(
        &mut self,
        prefix: &[String],
        siblings: &BTreeMap<String, &ItemMod>,
        children: &Path,
        origin: &Path,
    ) -> Result<(), String> {
        let local = match prefix {
            [name] => Some(name),
            [first, name] if first == "self" => Some(name),
            _ => None,
        };
        match local.and_then(|name| siblings.get(name.as_str())) {
            Some(module) => self.descend(module, children, origin),
            None => {
                self.items += 1;
                Ok(())
            }
        }
    }
}

/// Resolves `mod name;` to its file and to the directory its own children live
/// in, following the rules rustc uses: `name.rs` or `name/mod.rs`, overridden
/// by `#[path]`. A declaration whose file is missing is an error rather than a
/// zero, because silently counting nothing is how a surface budget stops
/// measuring anything.
fn resolve(
    children: &Path,
    name: &str,
    attrs: &[Attribute],
    origin: &Path,
) -> Result<(PathBuf, PathBuf), String> {
    if let Some(overridden) = path_attribute(attrs) {
        let file = children.join(overridden);
        let directory = descendant_directory(&file);
        return Ok((file, directory));
    }

    let flat = children.join(format!("{name}.rs"));
    if flat.is_file() {
        return Ok((flat, children.join(name)));
    }
    let nested = children.join(name).join("mod.rs");
    if nested.is_file() {
        return Ok((nested, children.join(name)));
    }
    Err(format!(
        "{}: `mod {name};` resolves to neither {} nor {}",
        origin.display(),
        flat.display(),
        nested.display()
    ))
}

/// Children of a `#[path]` module live beside the file, in a directory named
/// for it — except for `mod.rs`, whose children are its own siblings.
fn descendant_directory(file: &Path) -> PathBuf {
    let parent = file.parent().unwrap_or(Path::new("")).to_path_buf();
    match file.file_stem().and_then(|stem| stem.to_str()) {
        Some("mod") | None => parent,
        Some(stem) => parent.join(stem),
    }
}

fn path_attribute(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| match &attr.meta {
        Meta::NameValue(pair) if pair.path.is_ident("path") => match &pair.value {
            Expr::Lit(literal) => match &literal.lit {
                Lit::Str(text) => Some(text.value()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    })
}

/// Only the exact `#[cfg(test)]` form. A module gated behind a more elaborate
/// predicate is counted, which over-reports rather than under-reports: the
/// failure direction of a budget should be a build that complains, not a budget
/// that quietly stops seeing things.
fn is_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| match &attr.meta {
        Meta::List(list) => list.path.is_ident("cfg") && list.tokens.to_string().trim() == "test",
        _ => false,
    })
}

fn public_items(item: &Item) -> usize {
    match item {
        Item::Const(inner) => usize::from(is_public(&inner.vis)),
        Item::Enum(inner) => usize::from(is_public(&inner.vis)),
        Item::Fn(inner) => usize::from(is_public(&inner.vis)),
        Item::Static(inner) => usize::from(is_public(&inner.vis)),
        Item::Struct(inner) => usize::from(is_public(&inner.vis)),
        Item::Trait(inner) => usize::from(is_public(&inner.vis)),
        Item::TraitAlias(inner) => usize::from(is_public(&inner.vis)),
        Item::Type(inner) => usize::from(is_public(&inner.vis)),
        Item::Union(inner) => usize::from(is_public(&inner.vis)),
        // `Item::Use` and `Item::Mod` are not here: both can widen the surface
        // by more than one name, so `Walk` handles them with the module scope
        // in hand.
        _ => 0,
    }
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Counts an inline crate root, which exercises the whole walk except for
    /// module-file resolution.
    fn count(source: &str) -> usize {
        let mut walk = Walk {
            visited: BTreeSet::new(),
            items: 0,
        };
        let parsed = syn::parse_file(source).expect("test source parses");
        walk.items(&parsed.items, Path::new("src"), Path::new("src/lib.rs"))
            .expect("inline modules need no files");
        walk.items
    }

    #[test]
    fn private_and_restricted_items_are_not_surface() {
        assert_eq!(
            count("fn a() {} pub(crate) fn b() {} pub(super) struct C; pub fn d() {}"),
            1
        );
    }

    #[test]
    fn every_use_leaf_is_a_handed_out_name() {
        assert_eq!(count("pub use a::b::{c, d::e, f as g};"), 3);
    }

    #[test]
    fn associated_items_do_not_inflate_the_count() {
        assert_eq!(
            count("pub struct S; impl S { pub fn a(&self) {} pub fn b(&self) {} }"),
            1
        );
    }

    #[test]
    fn a_pub_item_in_a_private_module_is_unreachable() {
        assert_eq!(
            count("mod hidden { pub fn a() {} } pub mod shown { pub fn b() {} pub fn c() {} }"),
            2
        );
    }

    #[test]
    fn a_glob_over_a_local_module_counts_what_it_forwards() {
        assert_eq!(
            count("mod inner { pub fn a() {} pub struct B; fn c() {} } pub use inner::*;"),
            2
        );
    }

    #[test]
    fn a_local_glob_resolves_before_its_module_is_declared() {
        assert_eq!(
            count("pub use self::inner::*; mod inner { pub fn a() {} pub fn b() {} }"),
            2
        );
    }

    #[test]
    fn a_module_that_is_both_public_and_globbed_is_counted_once() {
        assert_eq!(
            count("pub mod inner { pub fn a() {} pub fn b() {} } pub use inner::*;"),
            2
        );
    }

    #[test]
    fn a_glob_over_another_crate_stays_at_one_and_is_left_to_the_glob_ban() {
        assert_eq!(count("pub use other_crate::*;"), 1);
    }

    #[test]
    fn a_missing_module_file_fails_rather_than_counting_zero() {
        let error = resolve(Path::new("src"), "absent", &[], Path::new("src/lib.rs")).unwrap_err();
        assert!(error.contains("`mod absent;`"), "{error}");
    }

    #[test]
    fn path_modules_nest_beside_their_file() {
        assert_eq!(
            descendant_directory(Path::new("src/a/b.rs")),
            PathBuf::from("src/a/b")
        );
        assert_eq!(
            descendant_directory(Path::new("src/a/mod.rs")),
            PathBuf::from("src/a")
        );
    }
}
