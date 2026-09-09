//! Exact Git commit materialization across explicit multi-candidate sources.

use super::*;

fn write_manifest(root: &Path, name: &str, version: &str, dependencies: &str) {
    fs::write(
        root.join(PROJECT_MANIFEST),
        format!("[package]\nname={name:?}\nversion={version:?}\nentry=\"main\"\n{dependencies}"),
    )
    .unwrap();
}

fn git_requests(repositories: &[&Path]) -> String {
    let sources = repositories
        .iter()
        .map(|repo| {
            format!(
                "{{ git = {{ repository = {:?}, rev = \"refs/heads/main\" }} }}",
                repo.to_str().unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[dependencies.\"org.example.Git\"]\nversion=\"1\"\nsources=[{sources}]\n")
}

#[test]
fn identical_git_mirrors_with_one_ref_keep_all_commits_without_location_identity() {
    let (fixture, repo, project, store, first_commit) = fixture("git-mirror-version");
    let mirror = fixture.child("mirror");
    let sources = author_sources(
        "org.example.Git",
        "public model Shared() { parameter gain: 1 = 2; relation law { gain - 2 = 0; } }",
        vec![],
    );
    write_package(&mirror, "src", &sources, &[]);
    command(&mirror, &["init", "--initial-branch=main"]);
    command(&mirror, &["add", "."]);
    command(
        &mirror,
        &[
            "commit",
            "-m",
            "different commit with identical author bytes",
        ],
    );
    let second_commit = command(&mirror, &["rev-parse", "HEAD"]);
    assert_ne!(first_commit, second_commit);
    write_manifest(
        &project,
        "org.example.Root",
        "1.0.0",
        &git_requests(&[&repo, &mirror]),
    );
    let first = PackagedModelDocument::preview_local_package_project_v1(&project).unwrap();
    write_manifest(
        &project,
        "org.example.Root",
        "1.0.0",
        &git_requests(&[&mirror, &repo]),
    );
    let second = PackagedModelDocument::preview_local_package_project_v1(&project).unwrap();
    assert_eq!(first.lock_bytes().unwrap(), second.lock_bytes().unwrap());
    assert_eq!(first.explanation(), second.explanation());
    let bytes = second.lock_bytes().unwrap();
    let lock = lock::ProjectLock::decode(&bytes).unwrap();
    assert_eq!(lock.git.len(), 2);
    assert!(lock.git.iter().all(|pin| pin.request == "refs/heads/main"));
    let mut remote_pin = lock.git[0].clone();
    remote_pin.repository = Some("https://example.invalid/first.git".to_owned());
    let first_remote = super::super::super::git::GitSource {
        repository: "https://example.invalid/first.git".to_owned(),
        rev: "refs/heads/main".to_owned(),
    };
    let second_remote = super::super::super::git::GitSource {
        repository: "https://example.invalid/second.git".to_owned(),
        rev: "refs/heads/main".to_owned(),
    };
    assert!(first_remote.matches_pin_transport(&remote_pin));
    assert!(!second_remote.matches_pin_transport(&remote_pin));
    let remote = lock::ProjectLock::new(
        lock.resolution.clone(),
        vec![remote_pin],
        lock.requests.clone(),
    )
    .unwrap();
    assert_eq!(
        lock::ProjectLock::decode(&remote.bytes().unwrap())
            .unwrap()
            .resolution,
        lock.resolution
    );
    assert!(
        !String::from_utf8(bytes.clone())
            .unwrap()
            .contains(repo.to_str().unwrap())
    );
    let accepted = second.commit(&store).unwrap();
    let fetched = fixture.child("fetched");
    assert_eq!(
        PackagedModelDocument::fetch_local_package_project_v1(&project, &fetched).unwrap(),
        accepted
    );
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), bytes);
    // A rejected object inventory is fatal; another valid mirror must not hide it.
    let damaged = if lock.git[0].commit == first_commit {
        &repo
    } else {
        &mirror
    };
    let commit = &lock.git[0].commit;
    let object = damaged
        .join(".git/objects")
        .join(&commit[..2])
        .join(&commit[2..]);
    fs::remove_file(&object).unwrap();
    fs::write(object, b"corrupt object").unwrap();
    assert!(PackagedModelDocument::fetch_local_package_project_v1(&project, &fetched).is_err());
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), bytes);
    assert_eq!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
        accepted
    );
}

#[test]
fn git_candidates_participate_in_global_backtracking_then_fetch_only_the_lock() {
    let (fixture, repo, project, store, _) = fixture("git-global-version");
    let newer = fixture.child("a-new-repository");
    let b = fixture.child("b");
    for (location, version, c_version) in [(&repo, "1.0.0", "1.0.0"), (&newer, "1.1.0", "2.0.0")] {
        fs::create_dir_all(location.join("src")).unwrap();
        fs::create_dir_all(location.join("c/src")).unwrap();
        write_manifest(
            location,
            "org.example.Git",
            version,
            &format!("[dependencies.C]\nversion={c_version:?}\nsources=[{{path=\"c\"}}]\n"),
        );
        fs::write(
            location.join("src/main.eqi"),
            "public model Shared() { parameter gain: 1 = 2; relation law { gain - 2 = 0; } }",
        )
        .unwrap();
        write_manifest(&location.join("c"), "C", c_version, "");
        fs::write(location.join("c/src/main.eqi"), "public model C() {}").unwrap();
        if location == &newer {
            command(location, &["init", "--initial-branch=main"]);
        }
        command(location, &["add", "."]);
        command(location, &["commit", "-m", "candidate closure"]);
    }
    fs::create_dir_all(b.join("src")).unwrap();
    write_manifest(
        &b,
        "B",
        "1.0.0",
        "[dependencies.C]\nversion=\"1\"\nsources=[{path=\"../repository/c\"}]\n",
    );
    fs::write(b.join("src/main.eqi"), "public model B() {}").unwrap();
    let requests = git_requests(&[&newer, &repo])
        + "[dependencies.B]\nversion=\"1\"\nsources=[{path=\"../b\"}]\n";
    write_manifest(&project, "org.example.Root", "1.0.0", &requests);
    let proposal = PackagedModelDocument::preview_local_package_project_v1(&project).unwrap();
    let lock = lock::ProjectLock::decode(&proposal.lock_bytes().unwrap()).unwrap();
    assert_eq!(lock.git.len(), 1);
    assert_eq!(lock.git[0].version.as_str(), "1.0.0");
    let accepted = proposal.commit(&store).unwrap();
    let bytes = fs::read(project.join(PROJECT_LOCK)).unwrap();
    let fetched = fixture.child("fetched");
    assert_eq!(
        PackagedModelDocument::fetch_local_package_project_v1(&project, &fetched).unwrap(),
        accepted
    );
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), bytes);
}
