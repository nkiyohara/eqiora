//! Fail-closed validation of the curated `eqiora` facade.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

const INVENTORY_PATH: &str = "api/eqiora-facade-v1.json";
const INVENTORY_SCHEMA: &str = "eqiora.facade-inventory/v1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema: String,
    #[serde(rename = "crate")]
    crate_name: String,
    source: PathBuf,
    root: RootInventory,
    stable_namespaces: Vec<NamespaceInventory>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RootInventory {
    stable_exports: Vec<Export>,
    stable_modules: Vec<String>,
    transitional_modules: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NamespaceInventory {
    path: String,
    source_module: String,
    stable_exports: Vec<Export>,
    transitional_exports: Vec<Export>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Export {
    name: String,
    from: String,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct PublicSurface {
    modules: BTreeSet<String>,
    uses: Vec<String>,
    unsupported: Vec<String>,
}

pub(crate) fn check() -> Result<(), String> {
    let root = workspace_root()?;
    let inventory_path = root.join(INVENTORY_PATH);
    let bytes = fs::read(&inventory_path)
        .map_err(|error| format!("cannot read {}: {error}", inventory_path.display()))?;
    let inventory: Inventory = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {}: {error}", inventory_path.display()))?;
    validate_inventory(&inventory)?;

    let source_path = root.join(&inventory.source);
    let source = fs::read_to_string(&source_path)
        .map_err(|error| format!("cannot read {}: {error}", source_path.display()))?;
    check_root_surface(&inventory, &source)?;
    for namespace in &inventory.stable_namespaces {
        check_namespace(namespace, &source)?;
    }
    Ok(())
}

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot locate workspace root".to_owned())
}

fn check_root_surface(inventory: &Inventory, source: &str) -> Result<(), String> {
    let surface = direct_public_surface(source)?;
    if !surface.unsupported.is_empty() {
        return Err(format!(
            "unclassified public root items in {}:\n{}",
            inventory.source.display(),
            surface.unsupported.join("\n")
        ));
    }
    let modules = inventory
        .root
        .stable_modules
        .iter()
        .chain(&inventory.root.transitional_modules)
        .cloned()
        .collect::<BTreeSet<_>>();
    compare_names("root facade modules", &modules, &surface.modules)?;
    compare_exports(
        "stable root facade",
        &inventory.root.stable_exports,
        &surface.uses,
    )
}

fn check_namespace(namespace: &NamespaceInventory, source: &str) -> Result<(), String> {
    let body = public_module_body(source, &namespace.source_module)?;
    let surface = direct_public_surface(body)?;
    if !surface.modules.is_empty() || !surface.unsupported.is_empty() {
        let mut items = surface
            .modules
            .iter()
            .map(|module| format!("pub mod {module}"))
            .collect::<Vec<_>>();
        items.extend(surface.unsupported);
        return Err(format!(
            "{} must contain only explicit pub use declarations; found:\n{}",
            namespace.path,
            items.join("\n")
        ));
    }
    let expected = namespace
        .stable_exports
        .iter()
        .chain(&namespace.transitional_exports)
        .cloned()
        .collect::<Vec<_>>();
    compare_exports(&namespace.path, &expected, &surface.uses)
}

fn validate_inventory(inventory: &Inventory) -> Result<(), String> {
    if inventory.schema != INVENTORY_SCHEMA {
        return Err(format!(
            "unsupported facade inventory schema {:?}; expected {INVENTORY_SCHEMA:?}",
            inventory.schema
        ));
    }
    if inventory.crate_name != "eqiora" {
        return Err(format!(
            "facade inventory names crate {:?}; expected \"eqiora\"",
            inventory.crate_name
        ));
    }
    if inventory.source.is_absolute()
        || inventory
            .source
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("facade inventory source must be a workspace-relative path".to_owned());
    }

    checked_names("stable root modules", &inventory.root.stable_modules)?;
    checked_names(
        "transitional root modules",
        &inventory.root.transitional_modules,
    )?;
    checked_exports("stable root exports", &inventory.root.stable_exports)?;
    let stable_modules = inventory
        .root
        .stable_modules
        .iter()
        .collect::<BTreeSet<_>>();
    let transitional_modules = inventory
        .root
        .transitional_modules
        .iter()
        .collect::<BTreeSet<_>>();
    if let Some(name) = stable_modules.intersection(&transitional_modules).next() {
        return Err(format!(
            "root module {name:?} cannot be both stable and transitional"
        ));
    }

    let mut paths = BTreeSet::new();
    let mut modules = BTreeSet::new();
    for namespace in &inventory.stable_namespaces {
        let expected_path = format!("{}::{}", inventory.crate_name, namespace.source_module);
        if namespace.path != expected_path {
            return Err(format!(
                "stable namespace {:?} must match its source module as {expected_path:?}",
                namespace.path
            ));
        }
        if !paths.insert(&namespace.path) {
            return Err(format!(
                "duplicate stable namespace path {:?}",
                namespace.path
            ));
        }
        if !modules.insert(&namespace.source_module) {
            return Err(format!(
                "duplicate stable namespace module {:?}",
                namespace.source_module
            ));
        }
        if !stable_modules.contains(&namespace.source_module) {
            return Err(format!(
                "stable namespace {:?} is not a stable root module",
                namespace.path
            ));
        }
        checked_exports(
            &format!("{} stable exports", namespace.path),
            &namespace.stable_exports,
        )?;
        checked_exports(
            &format!("{} transitional exports", namespace.path),
            &namespace.transitional_exports,
        )?;
        reject_overlap(namespace)?;
    }
    let missing = stable_modules
        .difference(&modules)
        .map(|name| name.as_str())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "stable root modules without namespace inventories: {}",
            missing.join(", ")
        ));
    }
    Ok(())
}

fn reject_overlap(namespace: &NamespaceInventory) -> Result<(), String> {
    let stable = namespace
        .stable_exports
        .iter()
        .map(|export| &export.name)
        .collect::<BTreeSet<_>>();
    let transitional = namespace
        .transitional_exports
        .iter()
        .map(|export| &export.name)
        .collect::<BTreeSet<_>>();
    if let Some(name) = stable.intersection(&transitional).next() {
        return Err(format!(
            "{} export {name:?} cannot be both stable and transitional",
            namespace.path
        ));
    }
    Ok(())
}

fn checked_names(label: &str, values: &[String]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.is_empty()
            || !value
                .chars()
                .all(|character| character == '_' || character.is_ascii_alphanumeric())
        {
            return Err(format!("{label} contains invalid identifier {value:?}"));
        }
        if !seen.insert(value) {
            return Err(format!("{label} contains duplicate {value:?}"));
        }
    }
    Ok(())
}

fn checked_exports(label: &str, exports: &[Export]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for export in exports {
        if export.name.is_empty() || export.from.is_empty() {
            return Err(format!("{label} contains an empty name or source"));
        }
        if !names.insert(&export.name) {
            return Err(format!("{label} contains duplicate name {:?}", export.name));
        }
        if !sources.insert(&export.from) {
            return Err(format!(
                "{label} contains duplicate source {:?}",
                export.from
            ));
        }
    }
    Ok(())
}

fn direct_public_surface(source: &str) -> Result<PublicSurface, String> {
    let mut surface = PublicSurface::default();
    let mut depth = 0_i32;
    let mut pending_use = None::<String>;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(statement) = &mut pending_use {
            statement.push(' ');
            statement.push_str(trimmed);
            if trimmed.contains(';') {
                surface.uses.push(std::mem::take(statement));
                pending_use = None;
            }
            continue;
        }
        if depth == 0 && trimmed.starts_with("pub use ") {
            if trimmed.contains(';') {
                surface.uses.push(trimmed.to_owned());
            } else {
                pending_use = Some(trimmed.to_owned());
            }
            continue;
        }
        if depth == 0 && trimmed.starts_with("pub mod ") {
            let name = trimmed
                .trim_start_matches("pub mod ")
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .next()
                .unwrap_or_default();
            if name.is_empty() {
                return Err(format!("cannot parse public module {trimmed:?}"));
            }
            surface.modules.insert(name.to_owned());
        } else if depth == 0 && trimmed.starts_with("pub ") {
            surface.unsupported.push(trimmed.to_owned());
        }
        depth += brace_delta(trimmed);
        if depth < 0 {
            return Err("unbalanced closing brace in facade source".to_owned());
        }
    }
    if let Some(statement) = pending_use {
        return Err(format!("unterminated public use {statement:?}"));
    }
    if depth != 0 {
        return Err("unbalanced braces in facade source".to_owned());
    }
    Ok(surface)
}

fn brace_delta(line: &str) -> i32 {
    if line.starts_with("//") {
        return 0;
    }
    line.bytes().filter(|byte| *byte == b'{').count() as i32
        - line.bytes().filter(|byte| *byte == b'}').count() as i32
}

fn public_module_body<'a>(source: &'a str, module: &str) -> Result<&'a str, String> {
    let declaration = format!("pub mod {module}");
    let start = source
        .find(&declaration)
        .ok_or_else(|| format!("stable facade module {module:?} is missing"))?;
    let opening = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| format!("stable facade module {module:?} has no inline body"))?;
    let mut depth = 0_i32;
    for (offset, byte) in source[opening..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(&source[opening + 1..opening + offset]);
                }
            }
            _ => {}
        }
    }
    Err(format!(
        "stable facade module {module:?} has an unclosed body"
    ))
}

fn compare_names(
    label: &str,
    expected: &BTreeSet<String>,
    actual: &BTreeSet<String>,
) -> Result<(), String> {
    if expected == actual {
        return Ok(());
    }
    let missing = expected.difference(actual).cloned().collect::<Vec<_>>();
    let unexpected = actual.difference(expected).cloned().collect::<Vec<_>>();
    Err(format!(
        "{label} drifted; missing [{}], unexpected [{}]",
        missing.join(", "),
        unexpected.join(", ")
    ))
}

fn compare_exports(label: &str, expected: &[Export], uses: &[String]) -> Result<(), String> {
    let expected = export_map(label, expected.iter().cloned())?;
    let actual = uses
        .iter()
        .map(|statement| expand_public_use(statement))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten();
    let actual = export_map(label, actual)?;
    if expected == actual {
        return Ok(());
    }
    let missing = differing_exports(&expected, &actual);
    let unexpected = differing_exports(&actual, &expected);
    Err(format!(
        "{label} export drift; missing [{}], unexpected [{}]",
        missing.join(", "),
        unexpected.join(", ")
    ))
}

fn differing_exports(
    source: &BTreeMap<String, String>,
    other: &BTreeMap<String, String>,
) -> Vec<String> {
    source
        .iter()
        .filter(|(name, path)| other.get(*name) != Some(*path))
        .map(|(name, path)| format!("{name} <- {path}"))
        .collect()
}

fn export_map(
    label: &str,
    exports: impl IntoIterator<Item = Export>,
) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    for export in exports {
        if let Some(previous) = map.insert(export.name.clone(), export.from.clone()) {
            return Err(format!(
                "{label} exports {:?} from both {previous:?} and {:?}",
                export.name, export.from
            ));
        }
    }
    Ok(map)
}

fn expand_public_use(statement: &str) -> Result<Vec<Export>, String> {
    if statement
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|token| token == "as")
    {
        return Err(format!(
            "stable facade import aliases are forbidden: {statement}"
        ));
    }
    let compact = statement
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let tree = compact
        .strip_prefix("pubuse")
        .and_then(|tree| tree.strip_suffix(';'))
        .ok_or_else(|| format!("unsupported public use {statement:?}"))?;
    if tree.contains('*') {
        return Err(format!(
            "stable facade glob re-export is forbidden: {statement}"
        ));
    }
    let Some(opening) = tree.find('{') else {
        return Ok(vec![single_export(tree, None)?]);
    };
    let closing = tree
        .rfind('}')
        .ok_or_else(|| format!("unclosed public use group {statement:?}"))?;
    if closing + 1 != tree.len()
        || tree[opening + 1..closing].contains('{')
        || tree[opening + 1..closing].contains('}')
    {
        return Err(format!(
            "nested or trailing stable use syntax is forbidden: {statement}"
        ));
    }
    let prefix = tree[..opening].trim_end_matches("::");
    if prefix.is_empty() {
        return Err(format!("public use group has an empty source: {statement}"));
    }
    tree[opening + 1..closing]
        .split(',')
        .filter(|item| !item.is_empty())
        .map(|item| {
            if item == "self" {
                Ok(Export {
                    name: terminal(prefix)?.to_owned(),
                    from: prefix.to_owned(),
                })
            } else {
                single_export(item, Some(prefix))
            }
        })
        .collect()
}

fn single_export(item: &str, prefix: Option<&str>) -> Result<Export, String> {
    let from = prefix.map_or_else(|| item.to_owned(), |prefix| format!("{prefix}::{item}"));
    Ok(Export {
        name: terminal(item)?.to_owned(),
        from,
    })
}

fn terminal(path: &str) -> Result<&str, String> {
    path.rsplit("::")
        .next()
        .filter(|segment| !segment.is_empty())
        .ok_or_else(|| format!("invalid Rust path {path:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_exports_preserve_exact_provider_paths() {
        let exports = expand_public_use("pub use crate::source::{self, Alpha, aliases};").unwrap();
        assert_eq!(
            exports,
            vec![
                Export {
                    name: "source".to_owned(),
                    from: "crate::source".to_owned(),
                },
                Export {
                    name: "Alpha".to_owned(),
                    from: "crate::source::Alpha".to_owned(),
                },
                Export {
                    name: "aliases".to_owned(),
                    from: "crate::source::aliases".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn stable_glob_is_rejected() {
        let error = expand_public_use("pub use internal::*;").unwrap_err();
        assert!(error.contains("glob re-export is forbidden"));
    }

    #[test]
    fn stable_import_alias_is_rejected() {
        let error = expand_public_use("pub use internal::Thing as Other;").unwrap_err();
        assert!(error.contains("import aliases are forbidden"));
    }

    #[test]
    fn public_surface_does_not_hide_new_root_items() {
        let source =
            "pub use core::Thing;\npub mod api {\n pub use app::Model;\n}\npub struct Surprise;\n";
        let surface = direct_public_surface(source).unwrap();
        assert_eq!(surface.modules, BTreeSet::from(["api".to_owned()]));
        assert_eq!(surface.uses, vec!["pub use core::Thing;"]);
        assert_eq!(surface.unsupported, vec!["pub struct Surprise;"]);
    }

    #[test]
    fn changed_provider_is_export_drift() {
        let expected = [Export {
            name: "Model".to_owned(),
            from: "app::Model".to_owned(),
        }];
        let error =
            compare_exports("api", &expected, &["pub use other::Model;".to_owned()]).unwrap_err();
        assert!(error.contains("Model <- app::Model"));
        assert!(error.contains("Model <- other::Model"));
    }
}
