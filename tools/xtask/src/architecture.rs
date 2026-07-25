//! Architecture predicates with a ratcheted debt ledger.
//!
//! Structural quality is a checked property, not a matter of taste: a codebase
//! that only a human reviewer can judge as clean cannot be maintained by
//! agents. Each predicate has a hard limit and a ledger of frozen exceptions;
//! ordinary work may only move a number down.
//!
//! See `docs/development/ai-authored-platform-strategy.md`, amendment A5.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const LEDGER: &str = "tools/ci/architecture-debt.toml";

#[derive(Debug, Deserialize)]
struct Ledger {
    limits: Limits,
    #[serde(default)]
    file_lines: Vec<FileLinesDebt>,
}

#[derive(Debug, Deserialize)]
struct Limits {
    production_file_lines: usize,
    test_file_lines: usize,
}

#[derive(Debug, Deserialize)]
struct FileLinesDebt {
    path: String,
    ceiling: usize,
    reason: String,
    removal: String,
}

/// A file's role, which selects the limit it must satisfy.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Production,
    Test,
}

impl Role {
    /// Tests carry fixture data and assertions that do not compress the way
    /// production logic does, so they get a higher ceiling.
    fn of(path: &Path) -> Self {
        let text = path.to_string_lossy();
        let is_test = text.contains("/tests/")
            || text.contains("/benches/")
            || text.contains("/examples/")
            || path.file_name().is_some_and(|name| name == "tests.rs");
        if is_test {
            Self::Test
        } else {
            Self::Production
        }
    }

    fn limit(self, limits: &Limits) -> usize {
        match self {
            Self::Production => limits.production_file_lines,
            Self::Test => limits.test_file_lines,
        }
    }
}

pub fn check() -> Result<(), String> {
    let root = repository_root()?;
    let ledger = load_ledger(&root)?;
    let debts: BTreeMap<&str, &FileLinesDebt> = ledger
        .file_lines
        .iter()
        .map(|debt| (debt.path.as_str(), debt))
        .collect();

    let mut sources = Vec::new();
    collect_rust_sources(&root.join("crates"), &root, &mut sources)?;
    sources.sort();

    let mut violations = Vec::new();
    let mut measured = BTreeMap::new();

    for relative in &sources {
        let lines = count_lines(&root.join(relative))?;
        measured.insert(relative.clone(), lines);
        let limit = Role::of(Path::new(relative)).limit(&ledger.limits);

        match debts.get(relative.as_str()) {
            Some(debt) if lines > debt.ceiling => violations.push(format!(
                "{relative}: {lines} lines exceeds its frozen ceiling of {}. \
                 The ledger only ratchets down; reduce the file or justify a new ceiling as an \
                 architecture change.\n  reason: {}\n  removal: {}",
                debt.ceiling, debt.reason, debt.removal
            )),
            Some(_) => {}
            None if lines > limit => violations.push(format!(
                "{relative}: {lines} lines exceeds the {limit}-line limit and has no ledger entry. \
                 Split it by responsibility, or record the exception with a reason and a removal \
                 condition."
            )),
            None => {}
        }
    }

    violations.extend(stale_entries(&ledger, &measured));

    if violations.is_empty() {
        println!(
            "architecture predicates hold across {} files ({} ledger exceptions)",
            sources.len(),
            ledger.file_lines.len()
        );
        return Ok(());
    }

    Err(format!(
        "architecture predicate violations:\n{}",
        violations
            .iter()
            .map(|entry| format!("- {entry}"))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

/// Entries that no longer describe reality must be deleted, otherwise the
/// ledger silently re-authorizes growth that has already been paid back.
fn stale_entries(ledger: &Ledger, measured: &BTreeMap<String, usize>) -> Vec<String> {
    ledger
        .file_lines
        .iter()
        .filter_map(|debt| match measured.get(&debt.path) {
            None => Some(format!(
                "{}: ledger entry refers to a file that no longer exists; delete the entry.",
                debt.path
            )),
            Some(&lines) => {
                let limit = Role::of(Path::new(&debt.path)).limit(&ledger.limits);
                (lines <= limit).then(|| {
                    format!(
                        "{}: now {lines} lines, within the {limit}-line limit; delete its ledger \
                         entry so the exception cannot be reclaimed.",
                        debt.path
                    )
                })
            }
        })
        .collect()
}

fn load_ledger(root: &Path) -> Result<Ledger, String> {
    let path = root.join(LEDGER);
    let text =
        fs::read_to_string(&path).map_err(|error| format!("cannot read {LEDGER}: {error}"))?;
    toml::from_str(&text).map_err(|error| format!("cannot parse {LEDGER}: {error}"))
}

fn collect_rust_sources(
    directory: &Path,
    root: &Path,
    sources: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read a directory entry: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot stat {}: {error}", path.display()))?;

        if file_type.is_dir() {
            if entry.file_name() != "target" {
                collect_rust_sources(&path, root, sources)?;
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| format!("{} is outside the repository", path.display()))?;
            sources.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    Ok(())
}

/// Physical lines, so the measurement matches what a reviewer scrolls through
/// and cannot be gamed by reflowing statements.
fn count_lines(path: &Path) -> Result<usize, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.is_empty() {
        return Ok(0);
    }
    let newlines = bytes.iter().filter(|&&byte| byte == b'\n').count();
    Ok(if bytes.last() == Some(&b'\n') {
        newlines
    } else {
        newlines + 1
    })
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot locate the repository root from the xtask manifest".to_owned())
}
