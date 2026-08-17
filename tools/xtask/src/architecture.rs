//! Architecture predicates with a ratcheted debt ledger.
//!
//! Structural quality is a checked property, not a matter of taste: a codebase
//! that only a human reviewer can judge as clean cannot be maintained by
//! agents. Each predicate has a hard limit and a ledger of frozen exceptions;
//! ordinary work may only move a number down.
//!
//! See `docs/development/ai-authored-platform-strategy.md`, amendment A5.

mod cargo_metadata;
mod cfg_condition;
mod dependency_graph;
mod glob_reexport;
mod package_map;
mod public_surface;
mod rfc_numbering;
mod source_shape;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use glob_reexport::GlobReexport;
use package_map::PackageMap;
use public_surface::CrateSurface;

const LEDGER: &str = "tools/ci/architecture-debt.toml";

#[derive(Debug, Deserialize)]
struct Ledger {
    limits: Limits,
    #[serde(default)]
    file_lines: Vec<FileLinesDebt>,
    #[serde(default)]
    public_surface: Vec<PublicSurfaceDebt>,
    #[serde(default)]
    glob_reexports: Vec<GlobReexportDebt>,
}

#[derive(Debug, Deserialize)]
struct Limits {
    production_file_lines: usize,
    test_file_lines: usize,
    source_file_bytes: usize,
    source_token_trees: usize,
    source_line_bytes: usize,
    public_items_per_crate: usize,
}

#[derive(Debug, Deserialize)]
struct FileLinesDebt {
    path: String,
    ceiling: usize,
    reason: String,
    removal: String,
}

#[derive(Debug, Deserialize)]
struct PublicSurfaceDebt {
    #[serde(rename = "crate")]
    package: String,
    ceiling: usize,
    /// Absent for a crate merely frozen at its measured width; required once
    /// the frozen number sits above the budget, where it is a real exception.
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    removal: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GlobReexportDebt {
    path: String,
    /// The exact resolved identity of each glob, sorted. Neither a count nor
    /// the path text holds a glob still: the public-surface budget scores an
    /// unresolvable cross-crate glob as a single item however much it forwards,
    /// so repointing one has to be what fails here. An identity therefore
    /// carries the condition it sits under, the path as written, and what that
    /// path's first segment resolves to — see `glob_reexport` for what the
    /// resolution can and cannot see.
    identities: Vec<String>,
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

    let mut sources = Vec::new();
    collect_rust_sources(&root.join("crates"), &root, &mut sources)?;
    sources.sort();

    let mut scanned = sources.clone();
    collect_rust_sources(&root.join("tools"), &root, &mut scanned)?;
    scanned.sort();

    let (mut violations, source_shape) = source_shape::violations(&ledger.limits, &root, &scanned)?;
    violations.extend(file_line_violations(&ledger, &root, &scanned)?);

    let surfaces = public_surface::measure(&root)?;
    violations.extend(public_surface_violations(&ledger, &surfaces));

    // Read once and shared: a glob's identity names the package its first
    // segment resolves to, which only the manifests know, and the cycle check
    // reads the same member graph.
    let metadata = cargo_metadata::load(&root)?;
    let packages = PackageMap::load(&root, &metadata)?;

    // A glob re-export is a defect in the repository's own tooling as much as
    // in a published crate, so this scan is not limited to `crates/`.
    let globs = glob_reexport::scan(&root, &scanned, &packages)?;
    violations.extend(glob_violations(&ledger, &globs));

    // `append` rather than `extend`, so the counts in `cycles` survive for the
    // summary instead of being partially moved out.
    let mut cycles = dependency_graph::check(&metadata)?;
    violations.append(&mut cycles.violations);

    let rfcs = rfc_numbering::check(&root)?;
    violations.extend(rfcs.violations.iter().cloned());

    if violations.is_empty() {
        report(
            &ledger,
            &source_shape,
            &surfaces,
            &globs,
            &scanned,
            &cycles,
            &rfcs,
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

/// One line per predicate, so a passing run still says what was measured. A
/// check that prints only "ok" cannot be distinguished from one that silently
/// stopped looking at anything.
fn report(
    ledger: &Ledger,
    source_shape: &source_shape::Summary,
    surfaces: &[CrateSurface],
    globs: &[GlobReexport],
    scanned: &[String],
    cycles: &dependency_graph::Cycles,
    rfcs: &rfc_numbering::RfcNumbering,
) {
    let budget = ledger.limits.public_items_per_crate;
    let above_budget = ledger
        .public_surface
        .iter()
        .filter(|debt| debt.ceiling > budget)
        .count();

    println!(
        "source shape: {} Rust files within {}/{}/{} byte/token-tree/line-byte limits; maxima {} \
         bytes at {}, {} recursive token trees at {}, {} line bytes at {}:{}; {} whole-module \
         rustfmt skips",
        source_shape.files,
        ledger.limits.source_file_bytes,
        ledger.limits.source_token_trees,
        ledger.limits.source_line_bytes,
        source_shape.max_bytes.value,
        source_shape.max_bytes.path,
        source_shape.max_token_trees.value,
        source_shape.max_token_trees.path,
        source_shape.max_line_bytes.value,
        source_shape.max_line_bytes.path,
        source_shape.max_line_bytes.line,
        source_shape.whole_module_skips,
    );
    println!(
        "file lines: {} Rust files within their role ceilings ({} frozen exceptions)",
        scanned.len(),
        ledger.file_lines.len()
    );
    println!(
        "public surface: {} crates measured, {} frozen at an exact item count ({} of those above \
         the {budget}-item budget)",
        surfaces.len(),
        ledger.public_surface.len(),
        above_budget
    );
    println!(
        "glob re-exports: {} frozen by resolved identity across {} files, none new or repointed in \
         {} scanned files",
        globs.len(),
        ledger.glob_reexports.len(),
        scanned.len()
    );
    println!(
        "dependency graph: {} workspace crates, {} normal/build/dev edges, no cycle",
        cycles.packages, cycles.edges
    );
    println!(
        "RFC numbering: {} RFC files and {} indexed numbers, no duplicate number or index drift",
        rfcs.files, rfcs.indexed
    );
}

fn file_line_violations(
    ledger: &Ledger,
    root: &Path,
    sources: &[String],
) -> Result<Vec<String>, String> {
    let debts: BTreeMap<&str, &FileLinesDebt> = ledger
        .file_lines
        .iter()
        .map(|debt| (debt.path.as_str(), debt))
        .collect();

    let mut violations = Vec::new();
    let mut measured = BTreeMap::new();

    for relative in sources {
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
            // An exact freeze, not a headroom allowance. A ceiling left above
            // the current size lets a partial repayment be spent again: a file
            // cut from 2,000 to 1,500 lines would otherwise keep 500 lines of
            // silent room to regrow into.
            Some(debt) if lines < debt.ceiling && lines > limit => violations.push(format!(
                "{relative}: now {lines} lines against a frozen ceiling of {}; lower the ceiling \
                 to {lines} so the repaid lines cannot be silently reclaimed.",
                debt.ceiling
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

    violations.extend(stale_entries(ledger, &measured));
    Ok(violations)
}

/// "Existing crates may not grow" is stronger than "existing crates stay under
/// a limit", so every crate carries a frozen count rather than only the ones
/// over budget: without that, a crate measured at 106 could add 22 names and
/// nothing would say so. An entry is therefore an exact freeze, not a headroom
/// allowance — widening *and* narrowing a surface both have to move the number,
/// which is what makes a public API change visible in review.
///
/// A crate with no entry at all fails too, including a brand new one. Letting
/// an unlisted crate fall back to the budget would make deleting a ledger line
/// a way to buy headroom, and would let a new crate reach the budget without
/// anyone deciding that was the intended surface. The budget survives as the
/// threshold above which a freeze has to justify itself.
fn public_surface_violations(ledger: &Ledger, surfaces: &[CrateSurface]) -> Vec<String> {
    let limit = ledger.limits.public_items_per_crate;
    let debts: BTreeMap<&str, &PublicSurfaceDebt> = ledger
        .public_surface
        .iter()
        .map(|debt| (debt.package.as_str(), debt))
        .collect();

    let mut violations = Vec::new();
    for surface in surfaces {
        match debts.get(surface.name.as_str()) {
            Some(debt) => violations.extend(frozen_surface(limit, debt, surface)),
            None => violations.push(unfrozen_surface(limit, surface)),
        }
    }

    let measured: BTreeMap<&str, usize> = surfaces
        .iter()
        .map(|surface| (surface.name.as_str(), surface.items))
        .collect();
    violations.extend(
        ledger
            .public_surface
            .iter()
            .filter(|debt| !measured.contains_key(debt.package.as_str()))
            .map(|debt| {
                format!(
                    "{}: ledger entry refers to a crate that no longer exists; delete the entry.",
                    debt.package
                )
            }),
    );

    violations
}

/// Checks one frozen crate: the count must match exactly, and a freeze above
/// the budget is a genuine exception, so it must say why it exists and what
/// would retire it. A freeze at or below the budget is only a measurement and
/// needs no prose.
fn frozen_surface(limit: usize, debt: &PublicSurfaceDebt, surface: &CrateSurface) -> Vec<String> {
    let mut violations = Vec::new();
    let name = &surface.name;

    if surface.items > debt.ceiling {
        violations.push(format!(
            "{name}: {} AST-reachable public items exceeds its frozen ceiling of {}. The ledger \
             only ratchets down; withdraw a name, or justify the wider surface as an architecture \
             change.{}",
            surface.items,
            debt.ceiling,
            justification(debt)
        ));
    } else if surface.items < debt.ceiling {
        violations.push(format!(
            "{name}: now {} AST-reachable public items against a frozen ceiling of {}; lower the \
             ceiling to {} so the withdrawn names cannot be silently reclaimed.",
            surface.items, debt.ceiling, surface.items
        ));
    }

    if debt.ceiling > limit && (debt.reason.is_none() || debt.removal.is_none()) {
        violations.push(format!(
            "{name}: frozen at {} items, above the {limit}-item budget, so the entry needs both a \
             reason and a removal condition.",
            debt.ceiling
        ));
    }

    violations
}

fn unfrozen_surface(limit: usize, surface: &CrateSurface) -> String {
    let justify = if surface.items > limit {
        format!(", and a reason and removal condition, since that is above the {limit}-item budget")
    } else {
        String::new()
    };
    format!(
        "{}: {} AST-reachable public items with no ledger entry. Freeze the surface with \
         `[[public_surface]] crate = \"{}\", ceiling = {}`{justify}.",
        surface.name, surface.items, surface.name, surface.items
    )
}

fn justification(debt: &PublicSurfaceDebt) -> String {
    match (&debt.reason, &debt.removal) {
        (Some(reason), Some(removal)) => format!("\n  reason: {reason}\n  removal: {removal}"),
        _ => String::new(),
    }
}

/// Unlike file length, a glob count only moves when someone deliberately adds
/// or deletes one. So this ledger ratchets exactly: removing a glob must lower
/// the number in the same change, or the budget stays available for the next
/// one to claim.
fn glob_violations(ledger: &Ledger, globs: &[GlobReexport]) -> Vec<String> {
    let debts: BTreeMap<&str, &GlobReexportDebt> = ledger
        .glob_reexports
        .iter()
        .map(|debt| (debt.path.as_str(), debt))
        .collect();

    let mut found: BTreeMap<&str, Vec<&GlobReexport>> = BTreeMap::new();
    for glob in globs {
        found.entry(glob.path.as_str()).or_default().push(glob);
    }

    let mut violations: Vec<String> = found
        .iter()
        .filter_map(|(path, entries)| match debts.get(path) {
            Some(debt) if !identities_match(entries, &debt.identities) => {
                Some(mismatch(path, entries, debt))
            }
            Some(_) => None,
            None => Some(format!(
                "{path}: {} glob re-export(s) with no ledger entry. A glob forwards an unknown \
                 number of names under a second canonical path and is not counted by the public \
                 surface budget; name the items instead:\n{}",
                entries.len(),
                describe(entries)
            )),
        })
        .collect();

    violations.extend(
        ledger
            .glob_reexports
            .iter()
            .filter(|debt| !found.contains_key(debt.path.as_str()))
            .map(|debt| {
                format!(
                    "{}: no glob re-export remains (or the file is gone); delete the ledger entry.",
                    debt.path
                )
            }),
    );

    violations
}

fn sorted_identities(entries: &[&GlobReexport]) -> Vec<String> {
    let mut identities = entries
        .iter()
        .map(|entry| entry.identity.clone())
        .collect::<Vec<_>>();
    identities.sort();
    identities
}

/// Compared as a sorted multiset, so a repointed or re-conditioned glob fails
/// even though the count is unchanged.
fn identities_match(entries: &[&GlobReexport], frozen: &[String]) -> bool {
    sorted_identities(entries) == frozen
}

/// Reports the difference rather than both full lists: a file with twenty-six
/// frozen globs would otherwise answer a one-line edit with fifty-two lines,
/// and the one that moved would be the reader's problem to find.
fn mismatch(path: &str, entries: &[&GlobReexport], debt: &GlobReexportDebt) -> String {
    let found = sorted_identities(entries);
    let gained = difference(&found, &debt.identities);
    let lost = difference(&debt.identities, &found);
    let located: BTreeMap<&str, &GlobReexport> = entries
        .iter()
        .map(|entry| (entry.identity.as_str(), *entry))
        .collect();

    // Nothing gained and nothing lost, yet the lists differ: the entry holds the
    // right identities in the wrong order. Saying so beats printing two empty
    // sections, which is what a reader would otherwise have to interpret.
    if gained.is_empty() && lost.is_empty() {
        return format!(
            "{path}: the ledger holds the right glob identities in the wrong order. They are \
             compared as a sorted list so that a review diff shows one line per change; sort the \
             `identities` entry."
        );
    }

    format!(
        "{path}: glob re-export identities do not match the ledger. Adding, removing, repointing \
         or re-conditioning a glob must move the entry in the same change.\n{}{}  reason: {}\n  \
         removal: {}",
        listing("no longer frozen (present in the tree)", &gained, &located),
        listing("frozen but gone (not found in the tree)", &lost, &located),
        debt.reason,
        debt.removal
    )
}

/// Multiset difference: a duplicated identity that appears twice where the
/// ledger froze it once is a change, and has to survive the comparison.
fn difference(left: &[String], right: &[String]) -> Vec<String> {
    let mut remaining: Vec<&String> = right.iter().collect();
    let mut only = Vec::new();
    for identity in left {
        match remaining.iter().position(|other| *other == identity) {
            Some(index) => {
                remaining.swap_remove(index);
            }
            None => only.push(identity.clone()),
        }
    }
    only
}

fn listing(
    heading: &str,
    identities: &[String],
    located: &BTreeMap<&str, &GlobReexport>,
) -> String {
    if identities.is_empty() {
        return String::new();
    }
    let lines = identities
        .iter()
        .map(|identity| match located.get(identity.as_str()) {
            Some(entry) => format!("    {identity}\n      at {}", entry.describe()),
            None => format!("    {identity}"),
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("  {heading}:\n{lines}\n")
}

fn describe(entries: &[&GlobReexport]) -> String {
    entries
        .iter()
        .map(|entry| format!("    {}\n      {}", entry.describe(), entry.identity))
        .collect::<Vec<_>>()
        .join("\n")
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
            collect_rust_sources(&path, root, sources)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    impl Scratch {
        fn create() -> Self {
            let base = std::env::var_os("TMPDIR")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .expect("TMPDIR or HOME is set for test scratch");
            let unique = format!(
                "eqiora-xtask-source-discovery-{}-{}",
                std::process::id(),
                NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
            );
            let path = base.join(unique);
            fs::create_dir(&path).expect("unique test scratch is created");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("test scratch is removed");
        }
    }

    #[test]
    fn source_discovery_does_not_exclude_a_directory_named_target() {
        let scratch = Scratch::create();
        let hidden = scratch.0.join("crates/example/target/x.rs");
        fs::create_dir_all(hidden.parent().expect("fixture has a parent"))
            .expect("fixture directory is created");
        fs::write(&hidden, "fn discovered() {}\n").expect("fixture source is written");

        let mut sources = Vec::new();
        collect_rust_sources(&scratch.0.join("crates"), &scratch.0, &mut sources)
            .expect("fixture sources are discovered");
        sources.sort();

        assert_eq!(sources, ["crates/example/target/x.rs"]);
    }

    fn surface(name: &str, items: usize) -> CrateSurface {
        CrateSurface {
            name: name.to_owned(),
            items,
        }
    }

    fn debt(package: &str, ceiling: usize, justified: bool) -> PublicSurfaceDebt {
        PublicSurfaceDebt {
            package: package.to_owned(),
            ceiling,
            reason: justified.then(|| "frozen for a test".to_owned()),
            removal: justified.then(|| "delete the test".to_owned()),
        }
    }

    #[test]
    fn a_surface_at_its_frozen_count_is_silent() {
        assert!(frozen_surface(128, &debt("c", 12, false), &surface("c", 12)).is_empty());
    }

    #[test]
    fn a_grown_surface_names_its_frozen_ceiling() {
        let violations = frozen_surface(
            128,
            &debt("eqiora-graph", 11, false),
            &surface("eqiora-graph", 12),
        );
        assert_eq!(
            violations,
            [
                "eqiora-graph: 12 AST-reachable public items exceeds its frozen ceiling of 11. \
                 The ledger only ratchets down; withdraw a name, or justify the wider surface as \
                 an architecture change."
            ]
        );
    }

    #[test]
    fn a_shrunk_surface_must_lower_its_ceiling() {
        let violations = frozen_surface(128, &debt("c", 20, false), &surface("c", 18));
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].contains("lower the ceiling to 18"),
            "{violations:?}"
        );
    }

    #[test]
    fn an_above_budget_freeze_without_prose_is_a_violation() {
        let violations = frozen_surface(128, &debt("c", 300, false), &surface("c", 300));
        assert_eq!(
            violations,
            [
                "c: frozen at 300 items, above the 128-item budget, so the entry needs both a \
                 reason and a removal condition."
            ]
        );
    }

    #[test]
    fn an_above_budget_breach_repeats_its_justification() {
        let violations = frozen_surface(128, &debt("c", 300, true), &surface("c", 301));
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].contains("reason: frozen for a test"),
            "{violations:?}"
        );
        assert!(
            violations[0].contains("removal: delete the test"),
            "{violations:?}"
        );
    }

    fn glob(identity: &str) -> GlobReexport {
        GlobReexport {
            path: "crates/c/src/lib.rs".to_owned(),
            line: 7,
            target: "inner".to_owned(),
            identity: identity.to_owned(),
        }
    }

    fn glob_debt(identities: &[&str]) -> GlobReexportDebt {
        GlobReexportDebt {
            path: "crates/c/src/lib.rs".to_owned(),
            identities: identities.iter().map(|entry| (*entry).to_owned()).collect(),
            reason: "frozen for a test".to_owned(),
            removal: "delete the test".to_owned(),
        }
    }

    #[test]
    fn a_repointed_glob_reports_only_the_identity_that_moved() {
        let entries = [glob("frozen one"), glob("repointed two")];
        let borrowed: Vec<&GlobReexport> = entries.iter().collect();
        let debt = glob_debt(&["frozen one", "original two"]);
        assert!(!identities_match(&borrowed, &debt.identities));

        let message = mismatch("crates/c/src/lib.rs", &borrowed, &debt);
        assert!(message.contains("no longer frozen"), "{message}");
        assert!(message.contains("repointed two"), "{message}");
        assert!(message.contains("frozen but gone"), "{message}");
        assert!(message.contains("original two"), "{message}");
        // The identity that did not move stays out of the report.
        assert!(!message.contains("frozen one"), "{message}");
    }

    #[test]
    fn a_ledger_entry_in_the_wrong_order_is_named_as_such() {
        let entries = [glob("a"), glob("b")];
        let borrowed: Vec<&GlobReexport> = entries.iter().collect();
        let debt = glob_debt(&["b", "a"]);
        assert!(!identities_match(&borrowed, &debt.identities));
        assert!(
            mismatch("crates/c/src/lib.rs", &borrowed, &debt)
                .contains("right glob identities in the wrong order"),
        );
    }

    #[test]
    fn a_duplicated_identity_does_not_cancel_against_a_single_frozen_one() {
        let entries = [glob("same"), glob("same")];
        let borrowed: Vec<&GlobReexport> = entries.iter().collect();
        assert!(!identities_match(&borrowed, &["same".to_owned()]));
        assert_eq!(
            difference(&sorted_identities(&borrowed), &["same".to_owned()]),
            ["same"]
        );
    }

    #[test]
    fn an_unlisted_crate_is_told_what_entry_to_add() {
        assert_eq!(
            unfrozen_surface(128, &surface("eqiora-new", 40)),
            "eqiora-new: 40 AST-reachable public items with no ledger entry. Freeze the surface \
             with `[[public_surface]] crate = \"eqiora-new\", ceiling = 40`."
        );
    }

    #[test]
    fn an_unlisted_crate_over_budget_is_also_told_to_justify_it() {
        let message = unfrozen_surface(128, &surface("eqiora-wide", 300));
        assert!(
            message.contains("ceiling = 300`, and a reason and removal condition"),
            "{message}"
        );
    }
}
