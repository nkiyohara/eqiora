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

use std::fs;
use std::path::Path;

use syn::{Item, ItemUse, UseTree, Visibility};

/// One glob re-export, located precisely enough to fix without searching.
pub(super) struct GlobReexport {
    pub(super) path: String,
    pub(super) line: usize,
    /// The re-exported prefix, so the report says which glob, not just where.
    pub(super) target: String,
}

impl GlobReexport {
    pub(super) fn describe(&self) -> String {
        format!("{}:{}: pub use {}::*;", self.path, self.line, self.target)
    }
}

/// Scans whole files rather than the reachable module tree: a glob is a defect
/// wherever it sits, including in a module no consumer can name.
pub(super) fn scan(root: &Path, sources: &[String]) -> Result<Vec<GlobReexport>, String> {
    let mut found = Vec::new();
    for relative in sources {
        let absolute = root.join(relative);
        let text = fs::read_to_string(&absolute)
            .map_err(|error| format!("cannot read {}: {error}", absolute.display()))?;
        let parsed = syn::parse_file(&text)
            .map_err(|error| format!("cannot parse {}: {error}", absolute.display()))?;
        collect(&parsed.items, relative, &mut found);
    }
    Ok(found)
}

fn collect(items: &[Item], relative: &str, found: &mut Vec<GlobReexport>) {
    for item in items {
        match item {
            Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    collect(inner, relative, found);
                }
            }
            Item::Use(use_item) if !matches!(use_item.vis, Visibility::Inherited) => {
                globs(use_item, relative, found);
            }
            _ => {}
        }
    }
}

fn globs(use_item: &ItemUse, relative: &str, found: &mut Vec<GlobReexport>) {
    walk(&use_item.tree, String::new(), relative, found);
}

fn walk(tree: &UseTree, prefix: String, relative: &str, found: &mut Vec<GlobReexport>) {
    match tree {
        UseTree::Path(path) => {
            let extended = extend(&prefix, &path.ident.to_string());
            walk(&path.tree, extended, relative, found);
        }
        UseTree::Group(group) => {
            for branch in &group.items {
                walk(branch, prefix.clone(), relative, found);
            }
        }
        UseTree::Glob(glob) => found.push(GlobReexport {
            path: relative.to_owned(),
            // The star's own span, not the statement's, so a glob buried in a
            // multi-line group still reports the line a reader must edit.
            line: glob.star_token.spans[0].start().line,
            target: if prefix.is_empty() {
                "<crate root>".to_owned()
            } else {
                prefix
            },
        }),
        UseTree::Name(_) | UseTree::Rename(_) => {}
    }
}

fn extend(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() {
        segment.to_owned()
    } else {
        format!("{prefix}::{segment}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(source: &str) -> Vec<String> {
        let parsed = syn::parse_file(source).expect("test source parses");
        let mut found = Vec::new();
        collect(&parsed.items, "scratch.rs", &mut found);
        found.iter().map(GlobReexport::describe).collect()
    }

    #[test]
    fn a_private_glob_import_is_not_a_re_export() {
        assert!(find("use std::fmt::*;").is_empty());
    }

    #[test]
    fn restricted_visibility_still_duplicates_canonical_paths() {
        assert_eq!(
            find("pub(crate) use inner::deep::*;"),
            ["scratch.rs:1: pub use inner::deep::*;"]
        );
    }

    #[test]
    fn a_glob_inside_a_group_reports_its_own_line() {
        let found = find("pub use a::{\n    b,\n    c::*,\n};");
        assert_eq!(found, ["scratch.rs:3: pub use a::c::*;"]);
    }

    #[test]
    fn nested_inline_modules_are_searched() {
        assert_eq!(
            find("mod outer { pub mod inner { pub use far::*; } }"),
            ["scratch.rs:1: pub use far::*;"]
        );
    }

    #[test]
    fn named_re_exports_are_left_alone() {
        assert!(find("pub use a::b::{c, d as e};").is_empty());
    }
}
