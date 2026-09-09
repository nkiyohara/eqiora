//! Public project entry points: authored requests, frozen proposals and exact replay.

use super::*;

fn project_package(root: &Path, name: &str, version: &str, dependencies: &str) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join(PROJECT_MANIFEST), format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\nentry = \"main\"\n{dependencies}")).unwrap();
    fs::write(
        root.join("src/main.eqi"),
        "public model Main() { parameter gain: 1 = 2; relation law { gain - 2 = 0; } }",
    )
    .unwrap();
}

fn dependency(name: &str, version: &str, paths: &[&str]) -> String {
    let sources = paths
        .iter()
        .map(|path| format!("{{ path = \"{path}\" }}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("\n[dependencies.\"{name}\"]\nversion = \"{version}\"\nsources = [{sources}]\n")
}

fn selected(record: &ResolutionRecordV1, name: &str) -> String {
    record
        .nodes()
        .iter()
        .find(|node| node.identity().name.as_str() == name)
        .unwrap()
        .identity()
        .version
        .to_string()
}

#[test]
fn public_preview_backtracks_and_permutations_keep_the_complete_proposal() {
    let fixture = TestDirectory::create("version-backtracking");
    let project = fixture.child("project");
    let store = fixture.child("store");
    project_package(
        &fixture.child("a-new"),
        "A",
        "1.10.0",
        &dependency("C", "2", &["../c2"]),
    );
    project_package(
        &fixture.child("a-old"),
        "A",
        "1.9.0",
        &dependency("C", "1", &["../c1"]),
    );
    project_package(
        &fixture.child("b"),
        "B",
        "1.0.0",
        &dependency("C", "1", &["../c1"]),
    );
    project_package(&fixture.child("c1"), "C", "1.0.0", "");
    project_package(&fixture.child("c2"), "C", "2.0.0", "");
    let root_requests =
        dependency("A", "1", &["../a-new", "../a-old"]) + &dependency("B", "1", &["../b"]);
    project_package(&project, "Root", "1.0.0", &root_requests);
    let first = PackagedModelDocument::preview_local_package_project_v1(&project).unwrap();
    assert_eq!(selected(first.resolution(), "A"), "1.9.0");
    assert_eq!(selected(first.resolution(), "C"), "1.0.0");
    assert!(!project.join(PROJECT_LOCK).exists());
    assert_eq!(fs::read_dir(&store).unwrap().count(), 0);
    let reordered =
        dependency("B", "1", &["../b"]) + &dependency("A", "1", &["../a-old", "../a-new"]);
    project_package(&project, "Root", "1.0.0", &reordered);
    let second = PackagedModelDocument::preview_local_package_project_v1(&project).unwrap();
    assert_eq!(first.lock_bytes().unwrap(), second.lock_bytes().unwrap());
    assert_eq!(first.explanation(), second.explanation());
    assert!(second.explanation().contains("A@1.9.0 -> C@1 => 1.0.0"));
    assert!(
        first
            .commit(&store)
            .err()
            .unwrap()
            .to_string()
            .contains("changed since update preview")
    );
    let lock = second.lock_bytes().unwrap();
    let resolution = second.commit(&store).unwrap();
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), lock);
    assert_eq!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
        resolution
    );
}

#[test]
fn editor_keeps_locked_selection_and_source_location_without_writes() {
    let fixture = TestDirectory::create("version-editor");
    let project = fixture.child("project");
    let store = fixture.child("store");
    project_package(&fixture.child("old"), "Library", "1.0.0", "");
    project_package(&fixture.child("new"), "Library", "1.1.0", "");
    project_package(
        &project,
        "Root",
        "1.0.0",
        &dependency("Library", "1", &["../old"]),
    );
    let accepted = PackagedModelDocument::resolve_local_package_project_v1(&project, &store)
        .expect("valid initial requested project");
    assert_eq!(selected(&accepted, "Library"), "1.0.0");
    let lock = fs::read(project.join(PROJECT_LOCK)).unwrap();
    project_package(
        &project,
        "Root",
        "1.0.0",
        &dependency("Library", "1", &["../new", "../old"]),
    );
    let manifest = fs::read(project.join(PROJECT_MANIFEST)).unwrap();
    let (workspace, paths) =
        analyze_local_package_editor_project_v1(52, &project, &BTreeMap::new()).unwrap();
    assert_eq!(workspace.version(), 52);
    let old_path = PathBuf::from("../old/src/main.eqi");
    let old_file = paths
        .iter()
        .find_map(|(file, path)| (path == &old_path).then_some(file))
        .expect("locked release source provenance");
    assert!(workspace.document(old_file).is_some());
    assert!(
        !paths
            .values()
            .any(|path| path == Path::new("../new/src/main.eqi"))
    );
    assert_eq!(paths.len(), 2);
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), lock);
    assert_eq!(fs::read(project.join(PROJECT_MANIFEST)).unwrap(), manifest);
}

#[test]
fn open_fetch_and_compilation_keep_the_lock_until_an_explicit_update() {
    let fixture = TestDirectory::create("version-lock");
    let project = fixture.child("project");
    let store = fixture.child("store");
    let old = fixture.child("old");
    let new = fixture.child("new");
    project_package(&old, "Library", "1.0.0", "");
    project_package(&new, "Library", "1.1.0", "");
    project_package(
        &project,
        "Root",
        "1.0.0",
        &dependency("Library", "1", &["../old"]),
    );
    let accepted =
        PackagedModelDocument::resolve_local_package_project_v1(&project, &store).unwrap();
    let lock = fs::read(project.join(PROJECT_LOCK)).unwrap();
    let baseline = PackagedModelDocument::compile_locked(
        &DirectoryPackageStore::open_ambient(&store).unwrap(),
        &accepted,
        "Main",
    )
    .unwrap()
    .model()
    .structural_fingerprint();
    // Source inventory is transport configuration, not an implicit update.
    project_package(
        &project,
        "Root",
        "1.0.0",
        &dependency("Library", "1", &["../new", "../old"]),
    );
    assert_eq!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
        accepted
    );
    let fetched = fixture.child("fetched");
    assert_eq!(
        PackagedModelDocument::fetch_local_package_project_v1(&project, &fetched).unwrap(),
        accepted
    );
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), lock);
    assert_eq!(
        PackagedModelDocument::compile_locked(
            &DirectoryPackageStore::open_ambient(&fetched).unwrap(),
            &accepted,
            "Main"
        )
        .unwrap()
        .model()
        .structural_fingerprint(),
        baseline
    );
    let proposal = PackagedModelDocument::preview_local_package_project_v1(&project).unwrap();
    assert_eq!(selected(proposal.resolution(), "Library"), "1.1.0");
    let bytes = proposal.lock_bytes().unwrap();
    // The proposal retains actual source bytes rather than rereading a mutable candidate.
    fs::write(
        new.join("src/main.eqi"),
        "public model ChangedAfterPreview() {}",
    )
    .unwrap();
    let updated = proposal.commit(&store).unwrap();
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), bytes);
    assert_eq!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
        updated
    );
    assert!(PackagedModelDocument::fetch_local_package_project_v1(&project, &fetched).is_err());
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), bytes);
    // A widened request is a declaration change even when the selected version still matches.
    project_package(
        &project,
        "Root",
        "1.0.0",
        &dependency("Library", ">=1.0.0,<3.0.0", &["../new", "../old"]),
    );
    assert!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store)
            .err()
            .unwrap()
            .to_string()
            .contains("authored requests differ")
    );
    let changed_request =
        PackagedModelDocument::resolve_local_package_project_v1(&project, &store).unwrap();
    assert_eq!(selected(&changed_request, "Library"), "1.1.0");
    assert_eq!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
        changed_request
    );
}

#[test]
fn acquisition_validation_and_installation_failures_preserve_a_usable_pair() {
    let fixture = TestDirectory::create("version-failures");
    let project = fixture.child("project");
    let store = fixture.child("store");
    let old = fixture.child("old");
    let new = fixture.child("new");
    project_package(&old, "Library", "1.0.0", "");
    project_package(&new, "Library", "1.1.0", "");
    project_package(
        &project,
        "Root",
        "1.0.0",
        &dependency("Library", "1", &["../old"]),
    );
    let accepted =
        PackagedModelDocument::resolve_local_package_project_v1(&project, &store).unwrap();
    let locked = fs::read(project.join(PROJECT_LOCK)).unwrap();
    for paths in [vec!["../old", "../missing"], vec!["../missing", "../old"]] {
        project_package(
            &project,
            "Root",
            "1.0.0",
            &dependency("Library", "1", &paths),
        );
        let manifest = fs::read(project.join(PROJECT_MANIFEST)).unwrap();
        assert!(PackagedModelDocument::resolve_local_package_project_v1(&project, &store).is_err());
        assert_eq!(fs::read(project.join(PROJECT_MANIFEST)).unwrap(), manifest);
        assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), locked);
        assert_eq!(
            PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
            accepted
        );
    }
    project_package(
        &project,
        "Root",
        "1.0.0",
        &dependency("Library", "1", &["../old", "../new"]),
    );
    let manifest = fs::read(project.join(PROJECT_MANIFEST)).unwrap();
    // The version matches, but the compiler's existing unit owner rejects the selected release.
    fs::write(
        new.join("src/main.eqi"),
        "public component Invalid() { parameter width: MissingUnit = 1; }",
    )
    .unwrap();
    let error = PackagedModelDocument::resolve_local_package_project_v1(&project, &store)
        .err()
        .unwrap();
    assert!(
        matches!(error, PackagePreparationError::DirectoryPreparation { source, .. } if matches!(*source, PackagePreparationError::Diagnostics(_)))
    );
    assert_eq!(fs::read(project.join(PROJECT_MANIFEST)).unwrap(), manifest);
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), locked);
    assert_eq!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
        accepted
    );
    fs::write(new.join("src/main.eqi"), "public model Main() {}").unwrap();
    let proposal = PackagedModelDocument::preview_local_package_project_v1(&project).unwrap();
    assert_eq!(selected(proposal.resolution(), "Library"), "1.1.0");
    let bad_store = fixture.0.join("not-a-store");
    fs::write(&bad_store, "ordinary file").unwrap();
    assert!(proposal.commit(&bad_store).is_err());
    assert_eq!(fs::read(project.join(PROJECT_MANIFEST)).unwrap(), manifest);
    assert_eq!(fs::read(project.join(PROJECT_LOCK)).unwrap(), locked);
    assert_eq!(
        PackagedModelDocument::open_local_package_project_v1(&project, &store).unwrap(),
        accepted
    );
}

#[test]
fn identical_mirrors_coalesce_but_conflicting_content_and_precedence_do_not() {
    let fixture = TestDirectory::create("version-mirrors");
    let project = fixture.child("project");
    let first = fixture.child("first");
    let second = fixture.child("second");
    project_package(&first, "Library", "1.0.0", "");
    project_package(&second, "Library", "1.0.0", "");
    project_package(
        &project,
        "Root",
        "1.0.0",
        &dependency("Library", "1", &["../first", "../second"]),
    );
    let mirror = PackagedModelDocument::preview_local_package_project_v1(&project).unwrap();
    assert_eq!(mirror.resolution().nodes().len(), 2);
    fs::write(second.join("README.md"), "different author content").unwrap();
    assert!(
        PackagedModelDocument::preview_local_package_project_v1(&project)
            .err()
            .unwrap()
            .to_string()
            .contains("conflicting content")
    );
    fs::remove_file(second.join("README.md")).unwrap();
    project_package(&first, "Library", "1.0.0+one", "");
    project_package(&second, "Library", "1.0.0+two", "");
    assert!(
        PackagedModelDocument::preview_local_package_project_v1(&project)
            .err()
            .unwrap()
            .to_string()
            .contains("equal-precedence")
    );
    project_package(
        &project,
        "Root",
        "1.0.0",
        &dependency("Library", "1.0.0+two", &["../first", "../second"]),
    );
    assert_eq!(
        selected(
            PackagedModelDocument::preview_local_package_project_v1(&project)
                .unwrap()
                .resolution(),
            "Library"
        ),
        "1.0.0+two"
    );
}
