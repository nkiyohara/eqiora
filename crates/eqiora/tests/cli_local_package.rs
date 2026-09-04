use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn create() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "eqiora-cli-local-package-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test directory");
    }
}

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().expect("file parent")).expect("create file parent");
    fs::write(path, contents).expect("write fixture file");
}

#[test]
fn cli_locks_and_checks_the_same_local_package_project_offline() {
    let fixture = TestDirectory::create();
    let project = fixture.0.join("project");
    let dependency = fixture.0.join("dependency");
    let store = fixture.0.join("store");
    fs::create_dir_all(&store).expect("create package store");

    write(
        project.join("eqiora.toml"),
        "[package]\nname = \"org.example.Root\"\nversion = \"1.0.0\"\nentry = \"main\"\n\n[dependencies.\"org.example.Library\"]\nversion = \"1.0.0\"\npath = \"../dependency\"\n",
    );
    write(
        project.join("src/main.eqi"),
        "import org.example.Library.main as library; model Local {}",
    );
    write(
        dependency.join("eqiora.toml"),
        "[package]\nname = \"org.example.Library\"\nversion = \"1.0.0\"\nentry = \"main\"\n",
    );
    write(
        dependency.join("src/main.eqi"),
        "public model Shared { parameter gain: 1 = 2; relation law continuous { gain - 2 = 0; } }",
    );

    let lock = Command::new(env!("CARGO_BIN_EXE_eqiora"))
        .args(["package", "lock"])
        .arg(&project)
        .arg("--store")
        .arg(&store)
        .output()
        .expect("run package lock");
    assert!(
        lock.status.success(),
        "package lock failed: {}",
        String::from_utf8_lossy(&lock.stderr)
    );
    assert!(lock.stdout.starts_with(b"locked "));
    let accepted_lock = fs::read(project.join("eqiora.lock")).expect("read accepted lock");

    let hostile = Command::new(env!("CARGO_BIN_EXE_eqiora"))
        .args(["package", "add"])
        .arg(&project)
        .arg("org.example.\u{1b}[31m")
        .args(["--version", "1.0.0", "--path", "../dependency", "--store"])
        .arg(&store)
        .output()
        .unwrap();
    assert!(!hostile.status.success());
    assert!(!hostile.stderr.contains(&0x1b));
    assert!(String::from_utf8_lossy(&hostile.stderr).contains("\\u{1b}"));
    assert_eq!(
        fs::read(project.join("eqiora.lock")).unwrap(),
        accepted_lock
    );

    let remove = Command::new(env!("CARGO_BIN_EXE_eqiora"))
        .args(["package", "remove"])
        .arg(&project)
        .arg("org.example.Library")
        .arg("--store")
        .arg(&store)
        .output()
        .unwrap();
    assert!(
        !remove.status.success(),
        "cannot remove an imported dependency"
    );
    assert_eq!(
        fs::read(project.join("eqiora.lock")).unwrap(),
        accepted_lock
    );

    let add = Command::new(env!("CARGO_BIN_EXE_eqiora"))
        .args(["package", "add"])
        .arg(&project)
        .arg("org.example.Library")
        .args(["--version", "1.0.0", "--path", "../dependency", "--store"])
        .arg(&store)
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    assert_eq!(
        fs::read(project.join("eqiora.lock")).unwrap(),
        accepted_lock
    );

    fs::remove_dir_all(&dependency).expect("remove dependency source after locking");
    fs::remove_file(project.join("src/main.eqi")).expect("remove root source after locking");
    let check = Command::new(env!("CARGO_BIN_EXE_eqiora"))
        .args(["package", "check"])
        .arg(&project)
        .arg("--store")
        .arg(&store)
        .args(["--entry-model", "library.Shared"])
        .output()
        .expect("run package check");
    assert!(
        check.status.success(),
        "package check failed: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(check.stdout.starts_with(b"accepted "));
    assert_eq!(
        fs::read(project.join("eqiora.lock")).expect("reread accepted lock"),
        accepted_lock
    );
}
