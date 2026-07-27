//! RFC filename numbering and README index agreement.
//!
//! Git can merge two independently added RFC files without a conflict even
//! when both filenames claim the same number. The README can drift just as
//! silently when a file and its index entry land in different changes. This
//! predicate compares number identities from filenames; it deliberately says
//! nothing about contiguity, RFC contents, statuses, or cross-references.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const DIRECTORY: &str = "rfcs";
const INDEX: &str = "rfcs/README.md";
const EXPECTED_SHAPE: &str = "rfcs/NNNN-slug.md";
const TEMPLATE: &str = "0000-template.md";

pub(super) struct RfcNumbering {
    pub(super) files: usize,
    pub(super) indexed: usize,
    pub(super) violations: Vec<String>,
}

pub(super) fn check(root: &Path) -> Result<RfcNumbering, String> {
    check_directory(&root.join(DIRECTORY))
}

fn check_directory(directory: &Path) -> Result<RfcNumbering, String> {
    let mut by_number: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut violations = Vec::new();

    let mut files = Vec::new();
    collect_files(directory, directory, &mut files)?;
    files.sort();
    for name in files {
        // The process scaffolding is deliberately not an RFC record. Keep the
        // exclusion exact: any other 0000-prefixed file is a numbered RFC.
        if name == "README.md" || name == TEMPLATE {
            continue;
        }
        let relative = format!("{DIRECTORY}/{name}");
        match rfc_number(&name) {
            Some(number) => by_number
                .entry(number.to_owned())
                .or_default()
                .push(relative),
            None => violations.push(format!(
                "{relative}: malformed RFC filename; expected `{EXPECTED_SHAPE}`. Rename the file \
                 to use a four-digit RFC number and a non-empty slug."
            )),
        }
    }

    for paths in by_number.values_mut() {
        paths.sort();
        if paths.len() > 1 {
            violations.push(format!(
                "RFC number {} is claimed by {}. Reconcile the colliding lanes by assigning one \
                 RFC a different number and updating its README entry.",
                rfc_number_from_path(&paths[0]).expect("validated RFC path"),
                paths.join(" and ")
            ));
        }
    }

    let index_path = directory.join("README.md");
    let index_text =
        fs::read_to_string(&index_path).map_err(|error| format!("cannot read {INDEX}: {error}"))?;
    let indexed = index_numbers(&index_text);
    let on_disk: BTreeSet<&str> = by_number.keys().map(String::as_str).collect();

    for number in on_disk.difference(&indexed) {
        let paths = &by_number[*number];
        violations.push(format!(
            "{}: RFC {number} exists on disk but is absent from {INDEX}. Add its index entry, or \
             remove or renumber the file.",
            paths.join(" and ")
        ));
    }
    for number in indexed.difference(&on_disk) {
        violations.push(format!(
            "{INDEX}: RFC {number} is indexed but no file on disk claims that number. Add the \
             missing `{EXPECTED_SHAPE}` file, or remove or correct the index entry."
        ));
    }

    Ok(RfcNumbering {
        files: by_number.values().map(Vec::len).sum(),
        indexed: indexed.len(),
        violations,
    })
}

/// Recursion makes a nested file visible so its full path can fail the direct
/// `rfcs/NNNN-slug.md` convention instead of becoming a silent third category.
fn collect_files(directory: &Path, root: &Path, files: &mut Vec<String>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot read an RFC directory entry: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot stat {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_files(&path, root, files)?;
        } else if file_type.is_file() || path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| format!("{} is outside the RFC directory", path.display()))?;
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn rfc_number(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    if extension != "md" {
        return None;
    }
    let (number, slug) = stem.split_once('-')?;
    (number.len() == 4 && number.bytes().all(|byte| byte.is_ascii_digit()) && !slug.is_empty())
        .then_some(number)
}

fn rfc_number_from_path(path: &str) -> Option<&str> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(rfc_number)
}

/// Every RFC-shaped Markdown link target in the README is an index claim.
fn index_numbers(text: &str) -> BTreeSet<&str> {
    let mut numbers = BTreeSet::new();
    let Some((_, records)) = text.split_once("## RFC records") else {
        return numbers;
    };
    let records = records
        .split_once("\n## ")
        .map_or(records, |(section, _)| section);
    let mut remaining = records;
    while let Some((_, after_open)) = remaining.split_once("](") {
        let Some((target, after_close)) = after_open.split_once(')') else {
            break;
        };
        if let Some(number) = rfc_number_from_path(target) {
            numbers.insert(number);
        }
        remaining = after_close;
    }
    numbers
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        root: std::path::PathBuf,
    }

    impl Fixture {
        fn new(index: &str, files: &[&str]) -> Self {
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "eqiora-rfc-numbering-{}-{sequence}",
                std::process::id()
            ));
            let rfcs = root.join(DIRECTORY);
            fs::create_dir_all(&rfcs).expect("create RFC fixture");
            fs::write(rfcs.join("README.md"), index).expect("write RFC fixture index");
            for file in files {
                let path = rfcs.join(file);
                fs::create_dir_all(path.parent().expect("RFC fixture file parent"))
                    .expect("create RFC fixture file parent");
                fs::write(path, "").expect("write RFC fixture file");
            }
            Self { root }
        }

        fn check(&self) -> RfcNumbering {
            super::check(&self.root).expect("check RFC fixture")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove RFC fixture");
        }
    }

    fn index(files: &[&str]) -> String {
        let entries = files
            .iter()
            .map(|file| format!("- [RFC]({file})"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("## RFC records\n\n{entries}\n")
    }

    #[test]
    fn current_repository_has_consistent_rfc_numbers() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("repository root");
        let violations = check(root).expect("check repository RFCs").violations;
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn duplicate_numbers_name_both_paths() {
        let files = ["0079-first-lane.md", "0079-second-lane.md"];
        let fixture = Fixture::new(&index(&["0079-first-lane.md"]), &files);
        let violations = fixture.check().violations;

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("rfcs/0079-first-lane.md"));
        assert!(violations[0].contains("rfcs/0079-second-lane.md"));
    }

    #[test]
    fn exact_0075_collision_is_rejected() {
        let files = [
            "0075-fem-form-compiler-poisson-q1.md",
            "0075-exact-cartesian-domain-edit.md",
        ];
        let fixture = Fixture::new(&index(&["0075-fem-form-compiler-poisson-q1.md"]), &files);
        let violations = fixture.check().violations;

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains(files[0]), "{violations:?}");
        assert!(violations[0].contains(files[1]), "{violations:?}");
    }

    #[test]
    fn file_missing_from_index_is_distinguished() {
        let fixture = Fixture::new("## RFC records\n", &["0079-unlisted.md"]);
        let violations = fixture.check().violations;

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("exists on disk but is absent"),
            "{violations:?}"
        );
    }

    #[test]
    fn index_entry_missing_its_file_is_distinguished() {
        let fixture = Fixture::new(&index(&["0079-absent.md"]), &[]);
        let violations = fixture.check().violations;

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("is indexed but no file on disk"),
            "{violations:?}"
        );
    }

    #[test]
    fn gaps_are_allowed() {
        let files = ["0001-one.md", "0002-two.md", "0004-four.md"];
        let fixture = Fixture::new(&index(&files), &files);

        assert!(fixture.check().violations.is_empty());
    }

    #[test]
    fn malformed_filename_names_the_expected_shape() {
        let fixture = Fixture::new("## RFC records\n", &["79-no-four-digit-number.md"]);
        let violations = fixture.check().violations;

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("rfcs/79-no-four-digit-number.md"));
        assert!(violations[0].contains(EXPECTED_SHAPE));
    }

    #[test]
    fn a_nested_file_cannot_become_a_silent_third_category() {
        let fixture = Fixture::new("## RFC records\n", &["drafts/0079-hidden.md"]);
        let violations = fixture.check().violations;

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("rfcs/drafts/0079-hidden.md"));
        assert!(violations[0].contains(EXPECTED_SHAPE));
    }
}
