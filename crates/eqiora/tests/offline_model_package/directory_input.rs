use std::path::PathBuf;
use std::{fs, time};

use eqiora::package::{
    AuthorManifestV1, AuthorPackageDirectory, AuthorPackageDirectoryError,
    AuthorPackageDirectoryResource, BundleEntryV1, BundleRoleV1, ExactVersion,
    NormalizedRelativePath, QualifiedName,
};

const AUTHOR_MANIFEST_LIMIT: u64 = 16 * 1024 * 1024;
const AUTHOR_SOURCE_LIMIT: u64 = 256 * 1024 * 1024;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn create(label: &str) -> Self {
        let nonce = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "eqiora-package-evidence-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create evidence directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_manifest(root: &TestDirectory, paths: &[&str]) {
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.example.DirectoryEvidence").expect("evidence package name"),
        ExactVersion::parse("1.0.0").expect("evidence package version"),
        vec![],
        paths
            .iter()
            .map(|path| {
                BundleEntryV1::new(
                    NormalizedRelativePath::parse(*path).expect("evidence bundle path"),
                    BundleRoleV1::ModelSource,
                )
            })
            .collect(),
    )
    .expect("evidence manifest");
    fs::write(
        root.0.join("package.json"),
        manifest.canonical_json().expect("evidence manifest bytes"),
    )
    .expect("write evidence manifest");
}

fn write_source(root: &TestDirectory, path: &str, bytes: &[u8]) {
    let path = root.0.join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create evidence source parent");
    }
    fs::write(path, bytes).expect("write evidence source");
}

#[test]
fn reads_only_inventory_and_rejects_invalid_entries() {
    let directory = TestDirectory::create("inventory");
    write_source(&directory, "src/main.eqi", b"model Main {}");
    write_manifest(&directory, &["src/main.eqi"]);
    let package = AuthorPackageDirectory::open_ambient(&directory.0).expect("open evidence root");
    let expected = package.read_sources().expect("read evidence sources");

    write_source(&directory, "unlisted.eqi", &[0xff]);
    assert_eq!(
        package.read_sources().expect("ignore unlisted source"),
        expected
    );

    fs::remove_file(directory.0.join("src/main.eqi")).expect("remove inventoried source");
    assert!(matches!(
        package.read_sources(),
        Err(AuthorPackageDirectoryError::EntryIo { path, .. })
            if path.as_str() == "src/main.eqi"
    ));
    fs::create_dir(directory.0.join("src/main.eqi")).expect("create non-regular entry");
    assert!(matches!(
        package.read_sources(),
        Err(AuthorPackageDirectoryError::NonRegularFile { path })
            if path.as_str() == "src/main.eqi"
    ));
}

#[test]
fn enforces_manifest_and_source_read_budgets() {
    let manifest_oversize = TestDirectory::create("manifest-limit");
    fs::File::create(manifest_oversize.0.join("package.json"))
        .expect("create sparse manifest")
        .set_len(AUTHOR_MANIFEST_LIMIT + 1)
        .expect("size sparse manifest");
    assert!(matches!(
        AuthorPackageDirectory::open_ambient(&manifest_oversize.0)
            .expect("open manifest-limit root")
            .read_sources(),
        Err(AuthorPackageDirectoryError::LimitExceeded {
            resource: AuthorPackageDirectoryResource::ManifestBytes,
            observed,
            limit,
            ..
        }) if observed == AUTHOR_MANIFEST_LIMIT + 1 && limit == AUTHOR_MANIFEST_LIMIT
    ));

    let source_oversize = TestDirectory::create("source-limit");
    write_manifest(&source_oversize, &["main.eqi"]);
    fs::File::create(source_oversize.0.join("main.eqi"))
        .expect("create sparse source")
        .set_len(AUTHOR_SOURCE_LIMIT + 1)
        .expect("size sparse source");
    assert!(matches!(
        AuthorPackageDirectory::open_ambient(&source_oversize.0)
            .expect("open source-limit root")
            .read_sources(),
        Err(AuthorPackageDirectoryError::LimitExceeded {
            resource: AuthorPackageDirectoryResource::SourceFileBytes,
            observed,
            limit,
            ..
        }) if observed == AUTHOR_SOURCE_LIMIT + 1 && limit == AUTHOR_SOURCE_LIMIT
    ));
}

#[cfg(unix)]
#[test]
fn rejects_symlink_redirection_and_retains_its_root() {
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    let special = TestDirectory::create("special-file");
    let _listener =
        UnixListener::bind(special.0.join("main.eqi")).expect("create special-file entry");
    write_manifest(&special, &["main.eqi"]);
    assert!(
        AuthorPackageDirectory::open_ambient(&special.0)
            .expect("open special-file root")
            .read_sources()
            .is_err()
    );

    let final_link = TestDirectory::create("final-symlink");
    write_source(&final_link, "target.eqi", b"model Main {}");
    symlink("target.eqi", final_link.0.join("main.eqi")).expect("create final symlink");
    write_manifest(&final_link, &["main.eqi"]);
    assert!(
        AuthorPackageDirectory::open_ambient(&final_link.0)
            .expect("open final-link root")
            .read_sources()
            .is_err()
    );

    let intermediate_link = TestDirectory::create("intermediate-symlink");
    write_source(&intermediate_link, "target/main.eqi", b"model Main {}");
    symlink("target", intermediate_link.0.join("src")).expect("create intermediate symlink");
    write_manifest(&intermediate_link, &["src/main.eqi"]);
    assert!(
        AuthorPackageDirectory::open_ambient(&intermediate_link.0)
            .expect("open intermediate-link root")
            .read_sources()
            .is_err()
    );

    let retained = TestDirectory::create("retained-root");
    write_source(&retained, "main.eqi", b"model Original {}");
    write_manifest(&retained, &["main.eqi"]);
    let package = AuthorPackageDirectory::open_ambient(&retained.0).expect("retain root");
    let expected = package.read_sources().expect("read original root");
    let moved = retained.0.with_extension("moved");
    fs::rename(&retained.0, &moved).expect("move retained root");
    fs::create_dir(&retained.0).expect("create replacement root");
    write_source(&retained, "main.eqi", b"model Replacement {}");
    write_manifest(&retained, &["main.eqi"]);
    assert_eq!(
        package.read_sources().expect("read retained root"),
        expected
    );
    fs::remove_dir_all(&retained.0).expect("remove replacement root");
    fs::rename(moved, &retained.0).expect("restore retained root for cleanup");
}
