//! Glob re-export detection.
//!
//! `pub use inner::*;` forwards an unknown number of names under a second
//! canonical path. It defeats the public-surface budget, which can only see one
//! item where a glob may hand out a hundred, and it makes the question "where
//! is this type actually defined" unanswerable without a compiler. Both are
//! properties an agent needs to be able to establish mechanically, so the glob
//! is banned outright and the ones that predate the predicate are frozen.
//!
//! Any non-inherited visibility counts. `pub(crate) use inner::*;` does not
//! escape the crate, but it still manufactures a duplicate canonical path for
//! every name it forwards, which is the other half of what the predicate is
//! for. A plain `use inner::*;` is an import, not a re-export, and is ignored.
//!
//! # What is frozen
//!
//! Freezing the path *text* would not hold the glob still. The same
//! `pub use foo::*;` forwards a different crate after a Cargo rename is
//! repointed, after a private `use bar as foo;` in the same module is edited,
//! or under a different `#[cfg]`, and in all three cases the text is unchanged.
//! Because the public-surface predicate scores an unresolvable cross-crate glob
//! as a single item however much it forwards, a repoint is otherwise free in
//! both predicates. So what is frozen is a resolved identity with three parts:
//!
//! - the normalized condition the glob sits under (see [`cfg_condition`]);
//! - the path as written, which is what a reader edits;
//! - what the path's first segment resolves to — a dependency package by name
//!   and origin, a module of this crate by the file it lives in, or an
//!   explicitly recorded failure to resolve.
//!
//! # What is not frozen
//!
//! Resolution uses one crate's AST plus `cargo metadata`, which leaves gaps
//! this module names rather than papers over. They are listed in the ledger
//! header; in short, the *contents* of another crate's module cannot be seen
//! from here, a `#[cfg]` on the `mod` declaration in a parent file is not part
//! of a file-local scan, and a name introduced into scope by another glob
//! import cannot be enumerated — the last of those is at least recorded in the
//! identity when the scope contains such an import.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use syn::{Attribute, Item, ItemMod, ItemUse, UseTree, Visibility};

use super::cfg_condition;
use super::package_map::{Package, PackageMap};
use super::public_surface::path_attribute;

/// Sysroot crates, which are in every extern prelude without being declared in
/// any manifest. Naming them keeps a glob over `std` out of the unresolved
/// bucket, where it would look like a defect in the scanner.
const SYSROOT: [&str; 5] = ["std", "core", "alloc", "proc_macro", "test"];

/// One glob re-export, located precisely enough to fix without searching.
pub(super) struct GlobReexport {
    pub(super) path: String,
    pub(super) line: usize,
    /// The re-exported prefix as written, so the report says which glob.
    pub(super) target: String,
    /// The frozen semantic identity: condition, written path, and what that
    /// path resolves to.
    pub(super) identity: String,
}

impl GlobReexport {
    pub(super) fn describe(&self) -> String {
        format!("{}:{}: pub use {}::*;", self.path, self.line, self.target)
    }
}

/// Scans whole files rather than the reachable module tree: a glob is a defect
/// wherever it sits, including in a module no consumer can name.
pub(super) fn scan(
    root: &Path,
    sources: &[String],
    packages: &PackageMap,
) -> Result<Vec<GlobReexport>, String> {
    let mut found = Vec::new();
    for relative in sources {
        let absolute = root.join(relative);
        let text = fs::read_to_string(&absolute)
            .map_err(|error| format!("cannot read {}: {error}", absolute.display()))?;
        let parsed = syn::parse_file(&text)
            .map_err(|error| format!("cannot parse {}: {error}", absolute.display()))?;
        let scope = Scope {
            root,
            file: relative,
            directory: children_directory(relative),
            package: packages.owner(relative),
            conditions: Vec::new(),
        };
        collect(&parsed.items, &scope, &mut found);
    }
    Ok(found)
}

/// One module body: the file it came from, where its child modules live, the
/// package whose extern prelude its paths resolve against, and the conditions
/// every item in it inherits.
struct Scope<'a> {
    root: &'a Path,
    file: &'a str,
    /// Repository-relative directory holding this module's child module files.
    directory: PathBuf,
    package: Option<&'a Package>,
    conditions: Vec<String>,
}

impl<'a> Scope<'a> {
    fn inside(&self, module: &ItemMod) -> Scope<'a> {
        let mut conditions = self.conditions.clone();
        conditions.extend(cfg_condition::of(&module.attrs));
        Scope {
            root: self.root,
            file: self.file,
            directory: self.directory.join(module.ident.to_string()),
            package: self.package,
            conditions,
        }
    }

    /// How a `mod` declaration in this scope is named as a glob target. An
    /// out-of-line module is named by the file it resolves to, so repointing it
    /// with `#[path]` moves the identity of every glob over it.
    fn module_target(&self, module: &ItemMod) -> String {
        let name = module.ident.to_string();
        if module.content.is_some() {
            return format!("inline module `{name}` of {}", self.file);
        }
        match self.module_file(&name, &module.attrs) {
            Some(file) => format!("module file {file}"),
            None => format!(
                "unresolved (`mod {name};` in {} resolves to no file)",
                self.file
            ),
        }
    }

    /// `name.rs` or `name/mod.rs`, overridden by `#[path]`, which is the rule
    /// rustc uses.
    fn module_file(&self, name: &str, attrs: &[Attribute]) -> Option<String> {
        if let Some(overridden) = path_attribute(attrs) {
            return Some(slashed(&self.directory.join(overridden)));
        }
        let flat = self.directory.join(format!("{name}.rs"));
        if self.root.join(&flat).is_file() {
            return Some(slashed(&flat));
        }
        let nested = self.directory.join(name).join("mod.rs");
        if self.root.join(&nested).is_file() {
            return Some(slashed(&nested));
        }
        None
    }

    fn package_identity(&self) -> String {
        match self.package {
            Some(package) => package.identity().to_owned(),
            None => format!("no workspace package owns {}", self.file),
        }
    }
}

fn collect(items: &[Item], scope: &Scope<'_>, found: &mut Vec<GlobReexport>) {
    let bindings = Bindings::of(items, scope);
    for item in items {
        match item {
            Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    collect(inner, &scope.inside(module), found);
                }
            }
            Item::Use(use_item) if !matches!(use_item.vis, Visibility::Inherited) => {
                globs(use_item, scope, &bindings, found);
            }
            _ => {}
        }
    }
}

fn globs(
    use_item: &ItemUse,
    scope: &Scope<'_>,
    bindings: &Bindings,
    found: &mut Vec<GlobReexport>,
) {
    let mut conditions = scope.conditions.clone();
    conditions.extend(cfg_condition::of(&use_item.attrs));
    let context = Context {
        condition: cfg_condition::describe(&conditions),
        leading_colon: use_item.leading_colon.is_some(),
        scope,
        bindings,
    };
    walk(&use_item.tree, &[], &context, found);
}

fn walk(tree: &UseTree, prefix: &[String], context: &Context<'_>, found: &mut Vec<GlobReexport>) {
    match tree {
        UseTree::Path(path) => {
            let mut extended = prefix.to_vec();
            extended.push(path.ident.to_string());
            walk(&path.tree, &extended, context, found);
        }
        UseTree::Group(group) => {
            for branch in &group.items {
                walk(branch, prefix, context, found);
            }
        }
        // The star's own span, not the statement's, so a glob buried in a
        // multi-line group still reports the line a reader must edit.
        UseTree::Glob(glob) => {
            found.push(context.record(prefix, glob.star_token.spans[0].start().line));
        }
        UseTree::Name(_) | UseTree::Rename(_) => {}
    }
}

/// Everything one `pub use` statement needs to turn a path into an identity.
struct Context<'a> {
    condition: String,
    /// `use ::foo::*;` resolves `foo` in the extern prelude only, never against
    /// an item of this module.
    leading_colon: bool,
    scope: &'a Scope<'a>,
    bindings: &'a Bindings,
}

impl Context<'_> {
    fn record(&self, prefix: &[String], line: usize) -> GlobReexport {
        let target = if prefix.is_empty() {
            "<crate root>".to_owned()
        } else {
            prefix.join("::")
        };
        GlobReexport {
            path: self.scope.file.to_owned(),
            line,
            identity: format!(
                "{} | {target}::* | {}",
                self.condition,
                self.resolve(prefix)
            ),
            target,
        }
    }

    fn resolve(&self, segments: &[String]) -> String {
        self.root_of(segments, self.leading_colon, &mut BTreeSet::new())
    }

    /// Resolves the first segment, which is the only one this scan can resolve:
    /// everything after it names an item of another module, whose contents are
    /// not visible from one crate's AST.
    fn root_of(
        &self,
        segments: &[String],
        leading_colon: bool,
        expanded: &mut BTreeSet<String>,
    ) -> String {
        let Some(first) = segments.first() else {
            return "unresolved (a glob with no path)".to_owned();
        };
        if leading_colon {
            return self.external(first);
        }
        match first.as_str() {
            "crate" => format!("this crate, {}", self.scope.package_identity()),
            // The enclosing module is identified by the file, which is the
            // ledger key, so a lexical path cannot be repointed from elsewhere.
            "super" => format!("the module enclosing {}", self.scope.file),
            "self" => match segments.get(1) {
                Some(next) => self.in_scope(next, expanded).unwrap_or_else(|| {
                    format!("unresolved (`self::{next}` names no item of this module)")
                }),
                None => "unresolved (`self::*`)".to_owned(),
            },
            name => self
                .in_scope(name, expanded)
                .unwrap_or_else(|| self.external(name)),
        }
    }

    /// A name declared by this module: a `mod`, or a `use` that renames some
    /// other path onto it. The alias is followed, because a one-word edit to a
    /// private `use` is the cheapest way to repoint a glob.
    fn in_scope(&self, name: &str, expanded: &mut BTreeSet<String>) -> Option<String> {
        // Already being followed. `use foo;` imports a name onto itself, and a
        // genuine alias cycle does not compile, so fall through to the extern
        // prelude rather than recurse.
        if !expanded.insert(name.to_owned()) {
            return None;
        }
        match self.bindings.names.get(name)? {
            Binding::Module(target) => Some(target.clone()),
            Binding::Alias {
                segments,
                leading_colon,
            } => Some(format!(
                "alias `{name}` = `{}{}` -> {}",
                if *leading_colon { "::" } else { "" },
                segments.join("::"),
                self.root_of(segments, *leading_colon, expanded)
            )),
        }
    }

    /// The extern prelude of the package that owns the file, which is where a
    /// Cargo rename does its work.
    fn external(&self, name: &str) -> String {
        let Some(package) = self.scope.package else {
            return format!(
                "unresolved (`{name}`; no workspace package owns {}){}",
                self.scope.file,
                self.shadowed()
            );
        };
        if let Some(identity) = package.dependency(name) {
            return format!("package {identity}{}", self.shadowed());
        }
        if SYSROOT.contains(&name) {
            return format!("sysroot crate {name}{}", self.shadowed());
        }
        format!(
            "unresolved (`{name}` is neither an item of this module nor a declared dependency of \
             {}){}",
            package.identity(),
            self.shadowed()
        )
    }

    /// A glob import in the same module can introduce a name this scan cannot
    /// enumerate, and such a name shadows the extern prelude. Only *untracked*
    /// ones are flagged: a `pub use x::*;` in the same module is itself a frozen
    /// entry, so its own identity already moves when it is repointed, whereas a
    /// plain `use x::*;` is invisible to the ledger. Recording the flag in the
    /// identity keeps that blind spot from opening or closing silently.
    fn shadowed(&self) -> &'static str {
        if self.bindings.untracked_glob_import {
            " (an untracked glob import in this module may shadow this name)"
        } else {
            ""
        }
    }
}

/// The names one module body declares, in the namespace a `use` path's first
/// segment is resolved against.
struct Bindings {
    names: BTreeMap<String, Binding>,
    /// Whether the module imports names by a glob the ledger does not freeze.
    untracked_glob_import: bool,
}

enum Binding {
    /// Already rendered, because a module's identity is its file rather than
    /// anything that needs resolving further.
    Module(String),
    Alias {
        segments: Vec<String>,
        leading_colon: bool,
    },
}

impl Bindings {
    fn of(items: &[Item], scope: &Scope<'_>) -> Self {
        let mut names = BTreeMap::new();
        // Modules first: a `use` may not shadow a `mod` of the same name in the
        // same module, so the declaration is the one that binds.
        for item in items {
            if let Item::Mod(module) = item {
                names.insert(
                    module.ident.to_string(),
                    Binding::Module(scope.module_target(module)),
                );
            }
        }

        let mut untracked_glob_import = false;
        for item in items {
            if let Item::Use(use_item) = item {
                bind(
                    &use_item.tree,
                    &[],
                    Origin {
                        leading_colon: use_item.leading_colon.is_some(),
                        tracked: !matches!(use_item.vis, Visibility::Inherited),
                    },
                    &mut names,
                    &mut untracked_glob_import,
                );
            }
        }
        Self {
            names,
            untracked_glob_import,
        }
    }
}

/// The two facts about a `use` statement that its individual leaves need.
#[derive(Clone, Copy)]
struct Origin {
    leading_colon: bool,
    /// Whether a glob in this statement is itself frozen by the ledger.
    tracked: bool,
}

fn bind(
    tree: &UseTree,
    prefix: &[String],
    origin: Origin,
    names: &mut BTreeMap<String, Binding>,
    untracked_glob_import: &mut bool,
) {
    match tree {
        UseTree::Path(path) => {
            let mut extended = prefix.to_vec();
            extended.push(path.ident.to_string());
            bind(&path.tree, &extended, origin, names, untracked_glob_import);
        }
        UseTree::Group(group) => {
            for branch in &group.items {
                bind(branch, prefix, origin, names, untracked_glob_import);
            }
        }
        UseTree::Glob(_) => *untracked_glob_import |= !origin.tracked,
        UseTree::Name(name) => {
            let ident = name.ident.to_string();
            // `use a::b::{self, c};` binds `b`, not `self`.
            let (bound, segments) = match (ident.as_str(), prefix.last()) {
                ("self", Some(last)) => (last.clone(), prefix.to_vec()),
                ("self", None) => return,
                _ => {
                    let mut segments = prefix.to_vec();
                    segments.push(ident.clone());
                    (ident, segments)
                }
            };
            insert_alias(names, bound, segments, origin.leading_colon);
        }
        UseTree::Rename(rename) => {
            let ident = rename.ident.to_string();
            let mut segments = prefix.to_vec();
            if ident != "self" {
                segments.push(ident);
            }
            insert_alias(
                names,
                rename.rename.to_string(),
                segments,
                origin.leading_colon,
            );
        }
    }
}

fn insert_alias(
    names: &mut BTreeMap<String, Binding>,
    bound: String,
    segments: Vec<String>,
    leading_colon: bool,
) {
    names.entry(bound).or_insert(Binding::Alias {
        segments,
        leading_colon,
    });
}

/// A crate root's and a `mod.rs`'s child modules are its own siblings; any
/// other file's children live in a directory named after it.
fn children_directory(relative: &str) -> PathBuf {
    let path = Path::new(relative);
    let parent = path.parent().unwrap_or(Path::new("")).to_path_buf();
    match path.file_stem().and_then(|stem| stem.to_str()) {
        Some("lib" | "main" | "mod") | None => parent,
        Some(stem) => parent.join(stem),
    }
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRATE_FILE: &str = "crates/host/src/lib.rs";

    fn packages() -> PackageMap {
        PackageMap::fixture(
            "crates/host",
            "host",
            &[
                ("far", "far (path crates/far)"),
                ("other", "other (path crates/other)"),
                ("alias", "renamed-package (path crates/renamed)"),
            ],
        )
    }

    /// Parses an inline crate root and reports each glob it contains.
    fn scan_source(source: &str, packages: &PackageMap) -> Vec<GlobReexport> {
        let parsed = syn::parse_file(source).expect("test source parses");
        let scope = Scope {
            root: Path::new("/nonexistent"),
            file: CRATE_FILE,
            directory: children_directory(CRATE_FILE),
            package: packages.owner(CRATE_FILE),
            conditions: Vec::new(),
        };
        let mut found = Vec::new();
        collect(&parsed.items, &scope, &mut found);
        found
    }

    fn find(source: &str) -> Vec<String> {
        scan_source(source, &packages())
            .iter()
            .map(GlobReexport::describe)
            .collect()
    }

    fn identity(source: &str) -> String {
        let found = scan_source(source, &packages());
        assert_eq!(found.len(), 1, "expected exactly one glob in {source:?}");
        found[0].identity.clone()
    }

    #[test]
    fn a_private_glob_import_is_not_a_re_export() {
        assert!(find("use std::fmt::*;").is_empty());
    }

    #[test]
    fn restricted_visibility_still_duplicates_canonical_paths() {
        assert_eq!(
            find("pub(crate) use inner::deep::*;"),
            ["crates/host/src/lib.rs:1: pub use inner::deep::*;"]
        );
    }

    #[test]
    fn a_glob_inside_a_group_reports_its_own_line() {
        let found = find("pub use far::{\n    b,\n    c::*,\n};");
        assert_eq!(found, ["crates/host/src/lib.rs:3: pub use far::c::*;"]);
    }

    #[test]
    fn nested_inline_modules_are_searched() {
        assert_eq!(
            find("mod outer { pub mod inner { pub use far::*; } }"),
            ["crates/host/src/lib.rs:1: pub use far::*;"]
        );
    }

    #[test]
    fn named_re_exports_are_left_alone() {
        assert!(find("pub use far::b::{c, d as e};").is_empty());
    }

    #[test]
    fn a_dependency_glob_names_the_package_it_resolves_to() {
        assert_eq!(
            identity("pub use far::*;"),
            "unconditional | far::* | package far (path crates/far)"
        );
    }

    /// The first bypass: the Rust text is untouched and the manifest decides
    /// which package `alias` names.
    #[test]
    fn a_cargo_rename_is_part_of_the_identity() {
        assert_eq!(
            identity("pub use alias::*;"),
            "unconditional | alias::* | package renamed-package (path crates/renamed)"
        );
        let repointed = PackageMap::fixture(
            "crates/host",
            "host",
            &[("alias", "different-package (path crates/different)")],
        );
        let found = scan_source("pub use alias::*;", &repointed);
        assert_ne!(found[0].identity, identity("pub use alias::*;"));
    }

    /// The second bypass: a private `use` in the same module decides what the
    /// public glob forwards.
    #[test]
    fn a_private_alias_indirection_is_followed() {
        assert_eq!(
            identity("use far as shim;\npub use shim::*;"),
            "unconditional | shim::* | alias `shim` = `far` -> package far (path crates/far)"
        );
        assert_ne!(
            identity("use far as shim;\npub use shim::*;"),
            identity("use other as shim;\npub use shim::*;")
        );
    }

    /// The third bypass: same text, different public surface per configuration.
    #[test]
    fn the_enclosing_condition_is_part_of_the_identity() {
        assert_eq!(
            identity("#[cfg(feature = \"a\")]\npub use far::*;"),
            "cfg(feature = \"a\") | far::* | package far (path crates/far)"
        );
        assert_ne!(
            identity("#[cfg(feature = \"a\")]\npub use far::*;"),
            identity("#[cfg(feature = \"b\")]\npub use far::*;")
        );
    }

    #[test]
    fn a_condition_on_an_enclosing_module_is_inherited() {
        assert_eq!(
            identity("#[cfg(feature = \"a\")]\npub mod gate { pub use far::*; }"),
            "cfg(feature = \"a\") | far::* | package far (path crates/far)"
        );
    }

    #[test]
    fn an_inline_module_target_names_the_file_it_sits_in() {
        assert_eq!(
            identity("mod inner { pub fn a() {} }\npub use inner::*;"),
            "unconditional | inner::* | inline module `inner` of crates/host/src/lib.rs"
        );
    }

    #[test]
    fn an_out_of_line_module_that_resolves_to_no_file_says_so() {
        assert_eq!(
            identity("mod inner;\npub use inner::*;"),
            "unconditional | inner::* | unresolved (`mod inner;` in crates/host/src/lib.rs \
             resolves to no file)"
        );
    }

    #[test]
    fn a_path_attribute_repoints_a_local_module_target() {
        assert_eq!(
            identity("#[path = \"elsewhere.rs\"]\nmod inner;\npub use inner::*;"),
            "unconditional | inner::* | module file crates/host/src/elsewhere.rs"
        );
    }

    #[test]
    fn a_leading_colon_skips_the_module_and_uses_the_prelude() {
        assert_eq!(
            identity("mod far { pub fn a() {} }\npub use ::far::*;"),
            "unconditional | far::* | package far (path crates/far)"
        );
    }

    #[test]
    fn an_undeclared_first_segment_is_recorded_as_unresolved() {
        assert!(
            identity("pub use mystery::*;").contains("unresolved (`mystery`"),
            "{}",
            identity("pub use mystery::*;")
        );
    }

    #[test]
    fn a_sysroot_glob_is_not_mistaken_for_a_scanner_failure() {
        assert_eq!(
            identity("pub use core::fmt::*;"),
            "unconditional | core::fmt::* | sysroot crate core"
        );
    }

    #[test]
    fn a_crate_relative_glob_names_the_owning_package() {
        assert_eq!(
            identity("pub use crate::inner::*;"),
            "unconditional | crate::inner::* | this crate, host (path crates/host)"
        );
    }

    #[test]
    fn an_untracked_glob_import_records_that_the_name_may_be_shadowed() {
        assert!(
            identity("use other::*;\npub use far::*;")
                .contains("(an untracked glob import in this module may shadow this name)")
        );
    }

    /// A second `pub use` glob in the same module is itself frozen, so it is not
    /// the silent hole the marker exists to record.
    #[test]
    fn a_second_frozen_glob_in_the_same_module_is_not_flagged() {
        let found = scan_source("pub use far::*;\npub use other::*;", &packages());
        assert_eq!(
            found
                .iter()
                .map(|entry| entry.identity.clone())
                .collect::<Vec<_>>(),
            [
                "unconditional | far::* | package far (path crates/far)",
                "unconditional | other::* | package other (path crates/other)",
            ]
        );
    }

    #[test]
    fn a_self_import_of_a_dependency_does_not_loop() {
        assert_eq!(
            identity("use far;\npub use far::*;"),
            "unconditional | far::* | alias `far` = `far` -> package far (path crates/far)"
        );
    }

    #[test]
    fn the_remaining_segments_stay_visible_in_the_written_path() {
        assert_eq!(
            identity("pub use far::deep::inner::*;"),
            "unconditional | far::deep::inner::* | package far (path crates/far)"
        );
    }

    #[test]
    fn a_crate_root_child_module_directory_is_its_own_directory() {
        assert_eq!(
            children_directory("crates/a/src/lib.rs"),
            PathBuf::from("crates/a/src")
        );
        assert_eq!(
            children_directory("crates/a/src/x/mod.rs"),
            PathBuf::from("crates/a/src/x")
        );
        assert_eq!(
            children_directory("crates/a/src/x.rs"),
            PathBuf::from("crates/a/src/x")
        );
    }
}
