//! Absolute Rust source-shape predicates.
//!
//! Every discovered `.rs` below `crates/` and `tools/` is measured without a
//! path exception. The limits are absolute rather than debt-ratcheted: raw
//! bytes bound the input, recursive proc_macro2 token trees bound syntax
//! density, physical-line bytes bound embedded representations, and a complete
//! syn visitor rejects formatting suppression on any module.

use std::fs;
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{Attribute, ItemMod, Meta, Token};

use super::Limits;

#[derive(Default)]
pub(super) struct Summary {
    pub(super) files: usize,
    pub(super) max_bytes: FileMaximum,
    pub(super) max_token_trees: FileMaximum,
    pub(super) max_line_bytes: LineMaximum,
    pub(super) whole_module_skips: usize,
}

#[derive(Default)]
pub(super) struct FileMaximum {
    pub(super) value: usize,
    pub(super) path: String,
}

impl FileMaximum {
    fn observe(&mut self, path: &str, value: usize) {
        if self.path.is_empty() || value > self.value {
            self.value = value;
            self.path = path.to_owned();
        }
    }
}

#[derive(Default)]
pub(super) struct LineMaximum {
    pub(super) value: usize,
    pub(super) path: String,
    pub(super) line: usize,
}

impl LineMaximum {
    fn observe(&mut self, path: &str, line: usize, value: usize) {
        if self.path.is_empty() || value > self.value {
            self.value = value;
            self.path = path.to_owned();
            self.line = line;
        }
    }
}

struct FileShape {
    bytes: usize,
    token_trees: usize,
    max_line_bytes: usize,
    max_line: usize,
    module_skips: Vec<ModuleSkip>,
}

struct ModuleSkip {
    line: usize,
    module: String,
    conditional: bool,
}

pub(super) fn violations(
    limits: &Limits,
    root: &Path,
    sources: &[String],
) -> Result<(Vec<String>, Summary), String> {
    let mut violations = Vec::new();
    let mut summary = Summary {
        files: sources.len(),
        ..Summary::default()
    };

    for relative in sources {
        let absolute = root.join(relative);
        let bytes = fs::read(&absolute)
            .map_err(|error| format!("cannot read {}: {error}", absolute.display()))?;
        let shape = measure_source(relative, &bytes)?;
        summary.max_bytes.observe(relative, shape.bytes);
        summary.max_token_trees.observe(relative, shape.token_trees);
        summary
            .max_line_bytes
            .observe(relative, shape.max_line, shape.max_line_bytes);
        summary.whole_module_skips += shape.module_skips.len();
        violations.extend(file_violations(relative, &shape, limits));
    }

    Ok((violations, summary))
}

fn measure_source(relative: &str, bytes: &[u8]) -> Result<FileShape, String> {
    let source = std::str::from_utf8(bytes)
        .map_err(|error| format!("cannot decode {relative} as UTF-8 Rust source: {error}"))?;
    let tokens = source
        .parse::<TokenStream>()
        .map_err(|error| format!("cannot tokenize {relative}: {error}"))?;
    let parsed =
        syn::parse_file(source).map_err(|error| format!("cannot parse {relative}: {error}"))?;
    let (max_line_bytes, max_line) = maximum_physical_line(bytes);
    let module_skips = module_skips(&parsed)
        .map_err(|error| format!("cannot inspect module attributes in {relative}: {error}"))?;

    Ok(FileShape {
        bytes: bytes.len(),
        token_trees: recursive_token_tree_count(tokens),
        max_line_bytes,
        max_line,
        module_skips,
    })
}

fn file_violations(relative: &str, shape: &FileShape, limits: &Limits) -> Vec<String> {
    let mut violations = Vec::new();
    if shape.bytes > limits.source_file_bytes {
        violations.push(format!(
            "{relative}: {} raw source bytes exceeds the no-debt limit of {} bytes; split the \
             file by responsibility.",
            shape.bytes, limits.source_file_bytes
        ));
    }
    if shape.token_trees > limits.source_token_trees {
        violations.push(format!(
            "{relative}: {} recursive proc_macro2 token trees exceeds the no-debt limit of {}; \
             split the file by responsibility.",
            shape.token_trees, limits.source_token_trees
        ));
    }
    if shape.max_line_bytes > limits.source_line_bytes {
        violations.push(format!(
            "{relative}:{}: physical line content is {} bytes, exceeding the no-debt limit of {} \
             bytes. LF is excluded and a preceding CR remains content; reflow or extract the \
             representation.",
            shape.max_line, shape.max_line_bytes, limits.source_line_bytes
        ));
    }
    violations.extend(shape.module_skips.iter().map(|skip| {
        let form = if skip.conditional {
            "a cfg_attr containing rustfmt::skip"
        } else {
            "#[rustfmt::skip]"
        };
        format!(
            "{relative}:{}: module `{}` carries {form}, which disables formatting for the whole \
             module; split or format the module. Individual non-module rustfmt skips remain \
             permitted.",
            skip.line, skip.module
        )
    }));
    violations
}

/// Counts every leaf `TokenTree` once and every `Group` once in addition to
/// recursively counting the trees inside it. proc_macro2 discards whitespace
/// and comments before this walk.
fn recursive_token_tree_count(tokens: TokenStream) -> usize {
    tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Group(group) => 1 + recursive_token_tree_count(group.stream()),
            TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => 1,
        })
        .sum()
}

/// The content of a physical line excludes its terminating LF. A CR before
/// that LF is ordinary source content and therefore counts toward the limit.
/// Equal maxima keep the first physical line for deterministic reporting.
fn maximum_physical_line(bytes: &[u8]) -> (usize, usize) {
    if bytes.is_empty() {
        return (0, 0);
    }

    let mut maximum = 0;
    let mut maximum_line = 1;
    let mut start = 0;
    let mut line = 1;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            let length = index - start;
            if length > maximum {
                maximum = length;
                maximum_line = line;
            }
            start = index + 1;
            line += 1;
        }
    }
    if start < bytes.len() {
        let length = bytes.len() - start;
        if length > maximum {
            maximum = length;
            maximum_line = line;
        }
    }
    (maximum, maximum_line)
}

fn module_skips(file: &syn::File) -> syn::Result<Vec<ModuleSkip>> {
    let mut visitor = ModuleSkipVisitor::default();
    visitor.visit_file(file);
    match visitor.error {
        Some(error) => Err(error),
        None => Ok(visitor.found),
    }
}

#[derive(Default)]
struct ModuleSkipVisitor {
    parents: Vec<String>,
    found: Vec<ModuleSkip>,
    error: Option<syn::Error>,
}

impl<'ast> Visit<'ast> for ModuleSkipVisitor {
    fn visit_item_mod(&mut self, module: &'ast ItemMod) {
        let name = module.ident.to_string();
        let qualified = if self.parents.is_empty() {
            name.clone()
        } else {
            format!("{}::{name}", self.parents.join("::"))
        };
        if self.error.is_none() {
            match module_skip_kind(&module.attrs) {
                Ok(Some(conditional)) => self.found.push(ModuleSkip {
                    line: module.ident.span().start().line,
                    module: qualified,
                    conditional,
                }),
                Ok(None) => {}
                Err(error) => self.error = Some(error),
            }
        }

        self.parents.push(name);
        syn::visit::visit_item_mod(self, module);
        self.parents.pop();
    }
}

/// `false` is a direct `#[rustfmt::skip]`; `true` is a conditional one.
fn module_skip_kind(attrs: &[Attribute]) -> syn::Result<Option<bool>> {
    for attr in attrs {
        if is_rustfmt_skip(attr.path()) {
            return Ok(Some(false));
        }
        let Meta::List(list) = &attr.meta else {
            continue;
        };
        if list.path.is_ident("cfg_attr") && cfg_attr_contains_rustfmt_skip(list)? {
            return Ok(Some(true));
        }
    }
    Ok(None)
}

fn cfg_attr_contains_rustfmt_skip(list: &syn::MetaList) -> syn::Result<bool> {
    let arguments = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    for argument in arguments {
        if is_rustfmt_skip(argument.path()) {
            return Ok(true);
        }
        if let Meta::List(nested) = argument
            && nested.path.is_ident("cfg_attr")
            && cfg_attr_contains_rustfmt_skip(&nested)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_rustfmt_skip(path: &syn::Path) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 2
        && path.segments[0].ident == "rustfmt"
        && path.segments[1].ident == "skip"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permissive_limits() -> Limits {
        Limits {
            production_file_lines: usize::MAX,
            test_file_lines: usize::MAX,
            source_file_bytes: usize::MAX,
            source_token_trees: usize::MAX,
            source_line_bytes: usize::MAX,
            public_items_per_crate: usize::MAX,
        }
    }

    fn fixture_shape(source: &str) -> FileShape {
        measure_source("fixture.rs", source.as_bytes()).expect("test source measures")
    }

    #[test]
    fn raw_source_bytes_at_the_limit_pass_and_one_over_fails() {
        let shape = fixture_shape("fn accepted() {}\n");
        let mut limits = permissive_limits();
        limits.source_file_bytes = shape.bytes;
        assert!(file_violations("fixture.rs", &shape, &limits).is_empty());

        limits.source_file_bytes -= 1;
        let violations = file_violations("fixture.rs", &shape, &limits);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("raw source bytes"), "{violations:?}");
    }

    #[test]
    fn recursive_token_trees_at_the_limit_pass_and_one_over_fails() {
        let shape = fixture_shape("fn accepted() {}\n");
        let mut limits = permissive_limits();
        limits.source_token_trees = shape.token_trees;
        assert!(file_violations("fixture.rs", &shape, &limits).is_empty());

        limits.source_token_trees -= 1;
        let violations = file_violations("fixture.rs", &shape, &limits);
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].contains("recursive proc_macro2 token trees"),
            "{violations:?}"
        );
    }

    #[test]
    fn recursive_token_count_charges_groups_and_their_contents() {
        let tokens = "f(a)"
            .parse::<TokenStream>()
            .expect("test source tokenizes");
        assert_eq!(recursive_token_tree_count(tokens), 3);
    }

    #[test]
    fn physical_line_bytes_at_the_limit_pass_and_one_over_fails() {
        let shape = fixture_shape("fn accepted() {}\n");
        let mut limits = permissive_limits();
        limits.source_line_bytes = shape.max_line_bytes;
        assert!(file_violations("fixture.rs", &shape, &limits).is_empty());

        limits.source_line_bytes -= 1;
        let violations = file_violations("fixture.rs", &shape, &limits);
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].contains("physical line content"),
            "{violations:?}"
        );
    }

    #[test]
    fn physical_lines_exclude_lf_but_retain_cr() {
        let shape = fixture_shape("// x\r\n// y\n");
        assert_eq!((shape.max_line_bytes, shape.max_line), (5, 1));
    }

    #[test]
    fn a_direct_skip_on_an_external_module_is_rejected() {
        let shape = fixture_shape("#[rustfmt::skip]\nmod generated;\n");
        let violations = file_violations("fixture.rs", &shape, &permissive_limits());
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].contains("module `generated`"),
            "{violations:?}"
        );
        assert!(violations[0].contains("#[rustfmt::skip]"), "{violations:?}");
    }

    #[test]
    fn a_cfg_attr_skip_on_a_nested_inline_module_is_rejected() {
        let shape = fixture_shape(
            "mod outer {\n    #[cfg_attr(feature = \"fixture\", rustfmt::skip)]\n    mod generated {}\n}\n",
        );
        let violations = file_violations("fixture.rs", &shape, &permissive_limits());
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].contains("module `outer::generated`"),
            "{violations:?}"
        );
        assert!(violations[0].contains("cfg_attr"), "{violations:?}");
    }

    #[test]
    fn a_skip_on_a_local_module_inside_a_function_is_rejected() {
        let shape = fixture_shape("fn container() {\n    #[rustfmt::skip]\n    mod hidden {}\n}\n");
        let violations = file_violations("fixture.rs", &shape, &permissive_limits());
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("module `hidden`"), "{violations:?}");
    }

    #[test]
    fn a_skip_on_a_non_module_item_remains_permitted() {
        let shape = fixture_shape("#[rustfmt::skip]\nfn generated() { let x=1; }\n");
        assert!(file_violations("fixture.rs", &shape, &permissive_limits()).is_empty());
    }
}
