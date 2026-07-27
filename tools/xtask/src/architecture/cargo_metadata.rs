//! One `cargo metadata` invocation, shared by every predicate that needs it.
//!
//! Two predicates now read the same workspace facts: the cycle check needs the
//! member graph, and the glob-re-export identity needs each member's declared
//! dependencies together with their Cargo renames. Shelling out twice would let
//! the two disagree if a manifest changed between the calls, and would double
//! the cost of the slowest step in the check — so the JSON is read once and
//! handed to both.

use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// `--no-deps` keeps registry packages out of `packages`. Every fact these
/// predicates need is declared by a workspace member — its own dependency
/// table, renames included — and resolving the full graph would additionally
/// require the registry to be reachable, which turns an architecture check into
/// a network test.
pub(super) fn load(root: &Path) -> Result<Value, String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid cargo metadata: {error}"))
}

/// The `packages` entries that are workspace members, in metadata order.
///
/// `--no-deps` already restricts the array to members, but the filter is kept
/// explicit: a future flag change that widens the array must not silently start
/// attributing registry packages to repository files.
pub(super) fn members(metadata: &Value) -> Result<Vec<&Value>, String> {
    let members: BTreeSet<&str> = metadata["workspace_members"]
        .as_array()
        .ok_or_else(|| "metadata has no workspace_members".to_owned())?
        .iter()
        .filter_map(Value::as_str)
        .collect();

    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| "metadata has no packages".to_owned())?;

    Ok(packages
        .iter()
        .filter(|package| {
            package["id"]
                .as_str()
                .is_some_and(|id| members.contains(id))
        })
        .collect())
}
