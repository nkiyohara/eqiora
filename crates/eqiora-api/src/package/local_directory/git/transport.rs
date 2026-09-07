//! One fresh bare repository; no source configuration, checkout or executable hooks.

#[path = "process.rs"]
mod process;

use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

const MAX_STORAGE: usize = 64 * 1024 * 1024;
const MAX_FILES: usize = 4096;
const MAX_SOURCE: usize = 32 * 1024 * 1024;
const MAX_BLOB: usize = 8 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) struct Fetched {
    root: PathBuf,
    pub sources: PathBuf,
    pub commit: String,
}

impl Drop for Fetched {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub(super) fn available() -> Result<(), PackagePreparationError> {
    for path in ["/usr/bin/git", "/usr/bin/prlimit"] {
        if !fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
            return Err(error(
                "Git fetch requires Linux /usr/bin/git and /usr/bin/prlimit",
            ));
        }
    }
    Ok(())
}

pub(super) fn fetch(
    source: &GitSource,
    locked: Option<&str>,
    declaring: &Path,
) -> Result<Fetched, PackagePreparationError> {
    available()?;
    let deadline = std::time::Instant::now() + process::DEADLINE;
    let scratch = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .ok_or_else(|| error("Git fetch requires home-backed TMPDIR"))?;
    if !scratch.is_absolute() || scratch.starts_with("/tmp") || scratch.starts_with("/var/tmp") {
        return Err(error("Git fetch requires home-backed TMPDIR"));
    }
    let root = scratch.join(format!(
        "eqiora-git-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .map_err(|_| error("cannot create private Git scratch"))?;
    let mut fetched = Fetched {
        sources: root.join("source"),
        root,
        commit: String::new(),
    };
    let home = fetched.root.join("home");
    let repo = fetched.root.join("repository");
    fs::create_dir(&home)
        .and_then(|()| fs::create_dir(&repo))
        .and_then(|()| fs::create_dir(&fetched.sources))
        .map_err(|_| error("cannot prepare Git scratch"))?;
    process::run(
        &home,
        &repo,
        &["init", "--bare", "--object-format=sha1", "."],
        deadline,
    )?;
    let revision = locked.unwrap_or(&source.rev);
    let selected = if source.repository.starts_with("https://") {
        process::run(
            &home,
            &repo,
            &[
                "fetch",
                "--depth=1",
                "--no-tags",
                "--no-recurse-submodules",
                "--no-write-fetch-head",
                "--",
                &source.repository,
                &format!("{revision}:refs/eqiora/selected"),
            ],
            deadline,
        )?;
        "refs/eqiora/selected"
    } else {
        let path = PathBuf::from(&source.repository);
        let path = if path.is_absolute() {
            path
        } else {
            declaring.join(path)
        };
        let directory = open_project_root(&path)
            .map_err(|_| error("cannot open explicit local Git repository"))?;
        let source_repo = match directory.symlink_metadata(".git") {
            Ok(metadata) if metadata.is_dir() => directory
                .open_dir_nofollow(".git")
                .map_err(|_| error("invalid local Git directory"))?,
            Ok(_) => {
                return Err(error(
                    "local Git source rejects linked/external .git entries",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => directory,
            Err(_) => return Err(error("cannot inspect local Git source")),
        };
        let mut budget = (0, 0);
        for name in ["objects", "refs"] {
            let dir = source_repo
                .open_dir_nofollow(name)
                .map_err(|_| error("local Git source is missing object/ref inventory"))?;
            copy_inventory(&dir, &repo.join(name), 0, &mut budget)?;
        }
        for name in ["HEAD", "packed-refs", "shallow"] {
            match transaction::read(&source_repo, name, MAX_BLOB) {
                Ok(bytes) => {
                    charge(&mut budget, bytes.len())?;
                    fs::write(repo.join(name), bytes)
                        .map_err(|_| error("cannot retain local Git refs"))?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound && name != "HEAD" => {}
                Err(_) => return Err(error("invalid local Git ref inventory")),
            }
        }
        process::run(
            &home,
            &repo,
            &["fsck", "--full", "--strict", "--no-reflogs"],
            deadline,
        )?;
        revision
    };
    // Recheck bounded stored inventory; hard per-file limits already apply during Git execution.
    let retained =
        open_project_root(&repo).map_err(|_| error("cannot inspect fetched inventory"))?;
    check_storage(&retained, 0, &mut (0, 0))?;
    let output = process::run(
        &home,
        &repo,
        &["rev-parse", "--verify", &format!("{selected}^{{commit}}")],
        deadline,
    )?;
    let commit = std::str::from_utf8(&output)
        .map_err(|_| error("invalid resolved Git commit"))?
        .trim();
    if !is_commit(commit) || locked.is_some_and(|expected| expected != commit) {
        return Err(error(
            "Git source does not contain the accepted immutable commit",
        ));
    }
    fetched.commit = commit.to_owned();
    let tree = process::run(
        &home,
        &repo,
        &["ls-tree", "-r", "-z", "-l", "--full-tree", commit],
        deadline,
    )?;
    let mut total = 0;
    let mut count = 0;
    let mut paths = BTreeSet::new();
    for entry in tree
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        count += 1;
        if count > MAX_FILES {
            return Err(error("Git tree exceeds file count limit"));
        }
        let entry = std::str::from_utf8(entry).map_err(|_| error("Git tree path is not UTF-8"))?;
        let (facts, path) = entry
            .split_once('\t')
            .ok_or_else(|| error("invalid Git tree entry"))?;
        let facts = facts.split_whitespace().collect::<Vec<_>>();
        if facts.len() != 4
            || !matches!(facts[0], "100644" | "100755")
            || facts[1] != "blob"
            || !is_commit(facts[2])
        {
            return Err(error("Git tree rejects symlink/gitlink/non-blob entries"));
        }
        let path =
            NormalizedRelativePath::parse(path).map_err(|_| error("unsafe Git tree path"))?;
        if path.as_str().split('/').count() > 32
            || path
                .as_str()
                .split('/')
                .any(|part| part.eq_ignore_ascii_case(".git"))
            || !paths.insert(path.as_str().to_ascii_lowercase())
        {
            return Err(error("ambiguous or over-deep Git tree path"));
        }
        let size = facts[3]
            .parse::<usize>()
            .map_err(|_| error("invalid Git blob size"))?;
        total += size.min(MAX_SOURCE + 1);
        if size > MAX_BLOB || total > MAX_SOURCE {
            return Err(error("Git source exceeds expanded blob/total byte limits"));
        }
        let bytes = process::run(&home, &repo, &["cat-file", "blob", facts[2]], deadline)?;
        if bytes.len() != size {
            return Err(error("Git blob changed during admission"));
        }
        let output = fetched.sources.join(path.as_str());
        fs::create_dir_all(output.parent().expect("source parent"))
            .and_then(|()| fs::write(output, bytes))
            .map_err(|_| error("cannot materialize validated Git source"))?;
    }
    Ok(fetched)
}

fn charge(budget: &mut (usize, usize), bytes: usize) -> Result<(), PackagePreparationError> {
    budget.0 += 1;
    budget.1 = budget.1.saturating_add(bytes);
    if budget.0 > 10_000 || budget.1 > MAX_STORAGE {
        return Err(error("Git storage inventory exceeds entry/byte limits"));
    }
    Ok(())
}

fn copy_inventory(
    source: &Dir,
    target: &Path,
    depth: usize,
    budget: &mut (usize, usize),
) -> Result<(), PackagePreparationError> {
    if depth > 32 {
        return Err(error("Git storage exceeds directory depth"));
    }
    fs::create_dir_all(target).map_err(|_| error("cannot create retained Git inventory"))?;
    for entry in source
        .entries()
        .map_err(|_| error("cannot read local Git inventory"))?
    {
        let name = entry
            .map_err(|_| error("invalid local Git entry"))?
            .file_name();
        let text = name
            .to_str()
            .ok_or_else(|| error("invalid local Git path"))?;
        if matches!(text, "alternates" | "http-alternates") {
            return Err(error("Git object alternates are not admitted"));
        }
        let meta = source
            .symlink_metadata(&name)
            .map_err(|_| error("cannot inspect Git entry"))?;
        if meta.is_dir() {
            charge(budget, 0)?;
            let child = source
                .open_dir_nofollow(&name)
                .map_err(|_| error("unsafe Git directory"))?;
            copy_inventory(&child, &target.join(&name), depth + 1, budget)?;
        } else if meta.is_file() {
            let bytes = transaction::read(source, text, MAX_STORAGE)
                .map_err(|_| error("unsafe/oversized Git object file"))?;
            charge(budget, bytes.len())?;
            fs::write(target.join(name), bytes).map_err(|_| error("cannot retain Git object"))?;
        } else {
            return Err(error("Git storage rejects symlink/special entries"));
        }
    }
    Ok(())
}

fn check_storage(
    source: &Dir,
    depth: usize,
    budget: &mut (usize, usize),
) -> Result<(), PackagePreparationError> {
    if depth > 32 {
        return Err(error("Git storage exceeds directory depth"));
    }
    for entry in source
        .entries()
        .map_err(|_| error("cannot inspect fetched storage"))?
    {
        let name = entry
            .map_err(|_| error("invalid fetched storage"))?
            .file_name();
        let meta = source
            .symlink_metadata(&name)
            .map_err(|_| error("invalid fetched storage entry"))?;
        if meta.is_dir() {
            charge(budget, 0)?;
            check_storage(
                &source
                    .open_dir_nofollow(&name)
                    .map_err(|_| error("unsafe fetched directory"))?,
                depth + 1,
                budget,
            )?;
        } else if meta.is_file() {
            charge(budget, usize::try_from(meta.len()).unwrap_or(usize::MAX))?;
        } else {
            return Err(error("unsafe fetched storage entry"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires public HTTPS access to the pinned small transport fixture"]
    fn public_https_materializes_an_exact_commit_without_checkout() {
        let commit = "7fd1a60b01f91b314f59955a4e4d4e80d8edf11d";
        let source = GitSource {
            repository: "https://github.com/octocat/Hello-World.git".to_owned(),
            rev: commit.to_owned(),
        };
        source.validate().unwrap();
        let fetched = fetch(&source, None, Path::new(".")).unwrap();
        assert_eq!(fetched.commit, commit);
        assert_eq!(
            fs::read(fetched.sources.join("README")).unwrap(),
            b"Hello World!\n"
        );
        assert!(!fetched.sources.join(".git").exists());
    }
}
