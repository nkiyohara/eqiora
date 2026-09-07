use super::*;
use std::process::Command;

fn command(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("/usr/bin/git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(repo)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn fixture(name: &str) -> (TestDirectory, PathBuf, PathBuf, PathBuf, String) {
    let fixture = TestDirectory::create(name);
    let repo = fixture.child("repository");
    let project = fixture.child("project");
    let store = fixture.child("store");
    command(&repo, &["init", "--initial-branch=main"]);
    let sources = author_sources(
        "org.example.Git",
        "public model Shared { parameter gain: 1 = 2; relation law { gain - 2 = 0; } }",
        vec![],
    );
    write_package(&repo, "src", &sources, &[]);
    command(&repo, &["add", "."]);
    command(&repo, &["commit", "-m", "first"]);
    let commit = command(&repo, &["rev-parse", "HEAD"]);
    write_package(
        &project,
        "src",
        &author_sources(
            "org.example.Root",
            "import org.example.Git.main as library; model Main {}",
            vec![],
        ),
        &[],
    );
    (fixture, repo, project, store, commit)
}

#[test]
fn git_relative_repository_is_resolved_from_its_declaring_manifest() {
    let (_fixture, repo, project, store, _) = fixture("git-relative-declaring");
    let outer = project.join("outer");
    write_package(
        &outer,
        "src",
        &author_sources("org.example.Outer", "public model Outer {}", vec![]),
        &[],
    );
    fs::rename(repo, outer.join("repository")).unwrap();
    let manifest = fs::read_to_string(outer.join(PROJECT_MANIFEST)).unwrap();
    fs::write(outer.join(PROJECT_MANIFEST), format!("{manifest}\n[dependencies.\"org.example.Git\"]\nversion=\"1.0.0\"\ngit={{repository=\"./repository\",rev=\"refs/heads/main\"}}\n")).unwrap();
    fs::write(project.join(PROJECT_MANIFEST), "[package]\nname=\"org.example.Root\"\nversion=\"1.0.0\"\nentry=\"main\"\n[dependencies.\"org.example.Outer\"]\nversion=\"1.0.0\"\npath=\"outer\"\n").unwrap();
    fs::write(
        project.join("src/main.eqi"),
        "import org.example.Outer.main as outer; model Main {}",
    )
    .unwrap();
    let resolution =
        PackagedModelDocument::resolve_local_package_project_v1(&project, &store).unwrap();
    assert_eq!(resolution.edges().len(), 2);
    fs::remove_dir_all(outer).unwrap();
    assert_eq!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
        resolution
    );
}

#[test]
fn malformed_git_manifest_diagnostics_never_include_transport_secrets() {
    let (_fixture, repo, project, store, commit) = fixture("git-parse-secret");
    PackagedModelDocument::add_git_package_dependency_v1(
        &project,
        &store,
        "org.example.Git",
        "1.0.0",
        repo.to_str().unwrap(),
        &commit,
    )
    .unwrap();
    let lock = fs::read(project.join(PROJECT_LOCK)).unwrap();
    for suffix in [", typo=true}", ", broken=}"] {
        let manifest = format!(
            "[package]\nname=\"org.example.Root\"\nversion=\"1.0.0\"\nentry=\"main\"\n[dependencies.\"org.example.Git\"]\nversion=\"1.0.0\"\ngit={{repository=\"https://SECRET@example.invalid/repo\",rev=\"refs/heads/main\"{suffix}\n"
        );
        fs::write(project.join(PROJECT_MANIFEST), &manifest).unwrap();
        let error = PackagedModelDocument::resolve_local_package_project_v1(&project, &store)
            .unwrap_err()
            .to_string();
        assert!(error.contains("cannot decode eqiora.toml"));
        assert!(!error.contains("SECRET"));
        assert!(!error.contains("https://"));
        assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), lock);
        assert_eq!(
            fs::read_to_string(project.join(PROJECT_MANIFEST)).unwrap(),
            manifest
        );
    }
}

#[test]
fn git_retains_complete_repository_local_closure_for_offline_compile() {
    let (_fixture, repo, project, store, _) = fixture("git-closure");
    write_package(
        &repo.join("dependency"),
        "src",
        &author_sources("org.example.Inner", "public model Inner {}", vec![]),
        &[],
    );
    let manifest = fs::read_to_string(repo.join(PROJECT_MANIFEST)).unwrap();
    fs::write(repo.join(PROJECT_MANIFEST), format!("{manifest}\n[dependencies.\"org.example.Inner\"]\nversion=\"1.0.0\"\npath=\"dependency\"\n")).unwrap();
    fs::write(
        repo.join("src/main.eqi"),
        "import org.example.Inner.main as inner; public model Shared { parameter gain: 1 = 2; relation law { gain - 2 = 0; } }",
    )
    .unwrap();
    command(&repo, &["add", "."]);
    command(&repo, &["commit", "-m", "repository-local closure"]);
    let resolution = PackagedModelDocument::add_git_package_dependency_v1(
        &project,
        &store,
        "org.example.Git",
        "1.0.0",
        repo.to_str().unwrap(),
        "refs/heads/main",
    )
    .unwrap();
    assert_eq!(resolution.edges().len(), 2);
    fs::remove_dir_all(repo).unwrap();
    let reopened = PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap();
    assert_eq!(reopened, resolution);
    PackagedModelDocument::compile_locked(
        &DirectoryPackageStore::open_ambient(&store).unwrap(),
        &reopened,
        "library.Shared",
    )
    .unwrap();
}

#[test]
fn git_branch_fetch_is_pinned_update_is_explicit_and_vendor_is_offline() {
    let (fixture, repo, project, store, commit) = fixture("git-pin");
    let first = PackagedModelDocument::add_git_package_dependency_v1(
        &project,
        &store,
        "org.example.Git",
        "1.0.0",
        repo.to_str().unwrap(),
        "refs/heads/main",
    )
    .unwrap();
    let lock = read_project_lock(&open_project_root(&project).unwrap()).unwrap();
    assert_eq!(lock.git[0].commit, commit);
    assert_eq!(lock.git[0].request, "refs/heads/main");
    command(&repo, &["tag", "-a", "release", "-m", "tagged"]);
    assert_eq!(
        PackagedModelDocument::add_git_package_dependency_v1(
            &project,
            &store,
            "org.example.Git",
            "1.0.0",
            repo.to_str().unwrap(),
            "refs/tags/release"
        )
        .unwrap(),
        first
    );
    PackagedModelDocument::add_git_package_dependency_v1(
        &project,
        &store,
        "org.example.Git",
        "1.0.0",
        repo.to_str().unwrap(),
        "refs/heads/main",
    )
    .unwrap();
    assert!(
        !String::from_utf8(lock.bytes().unwrap())
            .unwrap()
            .contains(repo.to_str().unwrap())
    );
    let original = PackagedModelDocument::compile_locked(
        &DirectoryPackageStore::open_ambient(&store).unwrap(),
        &first,
        "library.Shared",
    )
    .unwrap();
    fs::write(
        repo.join("src/main.eqi"),
        "public model Shared { parameter gain: 1 = 3; relation law { gain - 3 = 0; } }",
    )
    .unwrap();
    command(&repo, &["add", "."]);
    command(&repo, &["commit", "-m", "second"]);
    assert_eq!(
        PackagedModelDocument::fetch_local_package_project_v1(&project, &store).unwrap(),
        first
    );
    let second = PackagedModelDocument::resolve_local_package_project_v1(&project, &store).unwrap();
    assert_ne!(second, first);
    let vendor = fixture.child("vendor");
    PackagedModelDocument::vendor_local_package_project_v1(&project, &store, &vendor).unwrap();
    fs::remove_dir_all(repo).unwrap();
    fs::remove_dir_all(store).unwrap();
    let reopened = PackagedModelDocument::open_local_package_project_v1(&project, &vendor).unwrap();
    assert_eq!(reopened, second);
    let replay = PackagedModelDocument::compile_locked(
        &DirectoryPackageStore::open_ambient(&vendor).unwrap(),
        &reopened,
        "library.Shared",
    )
    .unwrap();
    assert_ne!(
        original.model().structural_fingerprint(),
        replay.model().structural_fingerprint()
    );
}

#[test]
fn git_rejects_unsafe_refs_secrets_and_missing_revision_without_publication() {
    let (_fixture, repo, project, store, commit) = fixture("git-invalid");
    PackagedModelDocument::add_git_package_dependency_v1(
        &project,
        &store,
        "org.example.Git",
        "1.0.0",
        repo.to_str().unwrap(),
        &commit,
    )
    .unwrap();
    let manifest = fs::read(project.join(PROJECT_MANIFEST)).unwrap();
    let lock = fs::read(project.join(PROJECT_LOCK)).unwrap();
    for revision in [
        "HEAD",
        "main",
        "refs/heads/main~1",
        "refs/heads/../secret",
        "refs/heads/missing",
        "0000000000000000000000000000000000000000",
    ] {
        assert!(
            PackagedModelDocument::add_git_package_dependency_v1(
                &project,
                &store,
                "org.example.Git",
                "1.0.0",
                repo.to_str().unwrap(),
                revision
            )
            .is_err()
        );
    }
    for url in [
        "https://user:SECRET@example.invalid/repo",
        "https://example.invalid/repo?SECRET",
        "ssh://SECRET/repo",
        "ext::SECRET",
    ] {
        let error = PackagedModelDocument::add_git_package_dependency_v1(
            &project,
            &store,
            "org.example.Git",
            "1.0.0",
            url,
            &commit,
        )
        .unwrap_err()
        .to_string();
        assert!(!error.contains("SECRET"));
    }
    assert_eq!(fs::read(project.join(PROJECT_MANIFEST)).unwrap(), manifest);
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), lock);
}

#[test]
fn git_ignores_source_hooks_filters_and_configuration_but_rejects_links() {
    use std::os::unix::fs::symlink;
    let (_fixture, repo, project, store, commit) = fixture("git-hooks");
    let sentinel = repo.join("EXECUTED");
    command(
        &repo,
        &[
            "config",
            "uploadpack.packObjectsHook",
            &format!("touch {}", sentinel.display()),
        ],
    );
    command(
        &repo,
        &[
            "config",
            "filter.evil.smudge",
            &format!("touch {}", sentinel.display()),
        ],
    );
    fs::write(repo.join(".gitattributes"), "* filter=evil\n").unwrap();
    PackagedModelDocument::add_git_package_dependency_v1(
        &project,
        &store,
        "org.example.Git",
        "1.0.0",
        repo.to_str().unwrap(),
        &commit,
    )
    .unwrap();
    assert!(!sentinel.exists());
    let lock = fs::read(project.join(PROJECT_LOCK)).unwrap();
    symlink("/etc/passwd", repo.join("attack")).unwrap();
    command(&repo, &["-c", "filter.evil.clean=cat", "add", "."]);
    command(&repo, &["commit", "-m", "link"]);
    let error = PackagedModelDocument::resolve_local_package_project_v1(&project, &store);
    // Exact pinned request still selects the old safe tree.
    assert!(error.is_ok());
    assert!(
        PackagedModelDocument::add_git_package_dependency_v1(
            &project,
            &store,
            "org.example.Git",
            "1.0.0",
            repo.to_str().unwrap(),
            "refs/heads/main"
        )
        .unwrap_err()
        .to_string()
        .contains("symlink/gitlink")
    );
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), lock);
}

#[test]
fn git_rejects_missing_closure_path_escape_gitlinks_and_large_expansion() {
    let (_fixture, repo, project, store, commit) = fixture("git-inventory");
    PackagedModelDocument::add_git_package_dependency_v1(
        &project,
        &store,
        "org.example.Git",
        "1.0.0",
        repo.to_str().unwrap(),
        &commit,
    )
    .unwrap();
    let accepted = fs::read(project.join(PROJECT_LOCK)).unwrap();
    let manifest = fs::read_to_string(repo.join(PROJECT_MANIFEST)).unwrap();
    for path in ["missing", "../outside"] {
        fs::write(repo.join(PROJECT_MANIFEST), format!("{manifest}\n[dependencies.\"org.example.Missing\"]\nversion=\"1.0.0\"\npath=\"{path}\"\n")).unwrap();
        command(&repo, &["add", "."]);
        command(&repo, &["commit", "-m", "invalid closure"]);
        assert!(
            PackagedModelDocument::add_git_package_dependency_v1(
                &project,
                &store,
                "org.example.Git",
                "1.0.0",
                repo.to_str().unwrap(),
                "refs/heads/main"
            )
            .is_err()
        );
        assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), accepted);
    }
    fs::write(repo.join(PROJECT_MANIFEST), manifest).unwrap();
    fs::write(repo.join("large"), vec![b'x'; 8 * 1024 * 1024 + 1]).unwrap();
    command(&repo, &["add", "."]);
    command(&repo, &["commit", "-m", "large expansion"]);
    assert!(
        PackagedModelDocument::add_git_package_dependency_v1(
            &project,
            &store,
            "org.example.Git",
            "1.0.0",
            repo.to_str().unwrap(),
            "refs/heads/main"
        )
        .unwrap_err()
        .to_string()
        .contains("expanded")
    );
    command(&repo, &["rm", "large"]);
    command(
        &repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},submodule"),
        ],
    );
    command(&repo, &["commit", "-m", "gitlink"]);
    assert!(
        PackagedModelDocument::add_git_package_dependency_v1(
            &project,
            &store,
            "org.example.Git",
            "1.0.0",
            repo.to_str().unwrap(),
            "refs/heads/main"
        )
        .unwrap_err()
        .to_string()
        .contains("symlink/gitlink")
    );
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), accepted);
}

#[test]
fn git_refuses_corrupt_object_identity_and_old_or_tampered_project_locks() {
    let (_fixture, repo, project, store, commit) = fixture("git-corrupt");
    let resolution = PackagedModelDocument::add_git_package_dependency_v1(
        &project,
        &store,
        "org.example.Git",
        "1.0.0",
        repo.to_str().unwrap(),
        &commit,
    )
    .unwrap();
    let accepted = fs::read(project.join(PROJECT_LOCK)).unwrap();
    fs::write(
        project.join(PROJECT_LOCK),
        resolution.canonical_json().unwrap(),
    )
    .unwrap();
    assert!(PackagedModelDocument::open_local_package_project_v1(&project, &store).is_err());
    let mut lock: serde_json::Value = serde_json::from_slice(&accepted).unwrap();
    lock["git"][0]["commit"] = "main".into();
    fs::write(
        project.join(PROJECT_LOCK),
        serde_json::to_vec(&lock).unwrap(),
    )
    .unwrap();
    assert!(PackagedModelDocument::open_local_package_project_v1(&project, &store).is_err());
    fs::write(project.join(PROJECT_LOCK), &accepted).unwrap();
    let object = repo
        .join(".git/objects")
        .join(&commit[..2])
        .join(&commit[2..]);
    fs::remove_file(&object).unwrap();
    fs::write(object, b"corrupt claimed object").unwrap();
    assert!(PackagedModelDocument::fetch_local_package_project_v1(&project, &store).is_err());
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), accepted);
    assert_eq!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
        resolution
    );
}
