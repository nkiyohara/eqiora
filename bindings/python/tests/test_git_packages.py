from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Iterator
from pathlib import Path

import pytest

import eqiora

pytestmark = pytest.mark.skipif(sys.platform != "linux", reason="Git acquisition uses Linux containment")


@pytest.fixture
def git_scratch(monkeypatch: pytest.MonkeyPatch) -> Iterator[None]:
    with tempfile.TemporaryDirectory(prefix="eqiora-git-test-", dir=Path.home()) as directory:
        monkeypatch.setenv("TMPDIR", directory)
        yield


def git(repo: Path, *args: str) -> str:
    return subprocess.check_output(
        ["/usr/bin/git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
         "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", *args],
        cwd=repo, env={"PATH": "/usr/bin:/bin", "HOME": str(repo), "GIT_CONFIG_NOSYSTEM": "1"},
        text=True,
    ).strip()


def test_git_project_pins_branch_and_reopens_without_repository(tmp_path: Path, git_scratch: None) -> None:
    repo = tmp_path / "repository"
    (repo / "src").mkdir(parents=True)
    (repo / "eqiora.toml").write_text('[package]\nname="org.example.Git"\nversion="1.0.0"\nentry="main"\n')
    source = "public model Shared { parameter gain: 1 = 2; relation law continuous { gain - 2 = 0; } }"
    (repo / "src/main.eqi").write_text(source)
    git(repo, "init", "--initial-branch=main")
    git(repo, "add", ".")
    git(repo, "commit", "-m", "first")
    commit = git(repo, "rev-parse", "HEAD")
    project = tmp_path / "project"
    (project / "src").mkdir(parents=True)
    (project / "eqiora.toml").write_text('[package]\nname="org.example.Root"\nversion="1.0.0"\nentry="main"\n')
    (project / "src/main.eqi").write_text("import org.example.Git.main as library; model Main {}")
    store = tmp_path / "store"
    store.mkdir()
    resolution = eqiora.add_git_dependency(project, store, "org.example.Git", version="1.0.0", repository=str(repo), revision="refs/heads/main")
    accepted = (project / "eqiora.lock").read_bytes()
    assert json.loads(accepted)["git"][0]["commit"] == commit
    model = eqiora.compile_package(store, resolution, entry_model="library.Shared")
    (repo / "src/main.eqi").write_text(source.replace("2", "3"))
    git(repo, "add", ".")
    git(repo, "commit", "-m", "second")
    assert eqiora.fetch_project(project, store) == resolution
    assert (project / "eqiora.lock").read_bytes() == accepted
    with pytest.raises(eqiora.CompatibilityError) as error:
        eqiora.add_git_dependency(project, store, "org.example.Git", version="1.0.0", repository="https://user:SECRET@example.invalid/repo", revision=commit)
    assert "SECRET" not in str(error.value)
    assert (project / "eqiora.lock").read_bytes() == accepted
    vendor = project / "vendor"
    vendor.mkdir()
    assert eqiora.vendor_project(project, store, vendor) == resolution
    shutil.rmtree(repo)
    shutil.rmtree(store)
    moved = tmp_path / "moved"
    project.rename(moved)
    reopened = eqiora.open_project(moved, moved / "vendor")
    assert reopened == resolution
    replay = eqiora.compile_package(moved / "vendor", reopened, entry_model="library.Shared")
    assert replay.digest == model.digest
