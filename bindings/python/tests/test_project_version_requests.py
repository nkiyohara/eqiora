"""Installed Python uses the same frozen native project proposal and exact lock."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

import eqiora


def package(path: Path, name: str, version: str, dependencies: str = "") -> None:
    (path / "src").mkdir(parents=True, exist_ok=True)
    (path / "eqiora.toml").write_text(
        f"[package]\nname='{name}'\nversion='{version}'\nentry='main'\n{dependencies}",
        encoding="utf-8",
    )
    (path / "src/main.eqi").write_text(
        "public model Main() { parameter gain: 1 = 2; relation law { gain - 2 = 0; } }",
        encoding="utf-8",
    )


def request(paths: list[str], version: str = "1") -> str:
    sources = ", ".join(f"{{path='{path}'}}" for path in paths)
    return f"[dependencies.Library]\nversion='{version}'\nsources=[{sources}]\n"


def test_preview_preserves_requests_and_commits_the_exact_inspected_lock(tmp_path: Path) -> None:
    project, store = tmp_path / "project", tmp_path / "store"
    store.mkdir()
    package(tmp_path / "old", "Library", "1.9.0")
    package(tmp_path / "new", "Library", "1.10.0")
    package(project, "Root", "1.0.0", request(["../old", "../new"]))
    proposal = eqiora.preview_local_project(project)
    assert isinstance(proposal, eqiora.ProjectUpdate)
    assert "Root@1.0.0" in repr(proposal)
    assert "pending=true" in repr(proposal)
    assert not (project / "eqiora.lock").exists()
    assert not list(store.iterdir())
    lock = json.loads(proposal.lock)
    assert lock["schema"] == "eqiora.project-lock.v2"
    assert lock["requests"] == [{
        "declaring": "Root", "declaring_version": "1.0.0",
        "dependency": "Library", "request": "1", "selected": "1.10.0",
    }]
    assert "Root@1.0.0 -> Library@1 => 1.10.0" in proposal.explanation
    with pytest.raises(AttributeError):
        proposal.lock = b"replacement"
    package(project, "Root", "1.0.0", request(["../new", "../old"]))
    reordered = eqiora.preview_local_project(project)
    assert reordered.lock == proposal.lock
    assert reordered.explanation == proposal.explanation
    with pytest.raises(eqiora.CompatibilityError, match="changed since update preview"):
        proposal.commit(store)
    resolution = reordered.commit(store)
    assert resolution == reordered.resolution
    assert (project / "eqiora.lock").read_bytes() == reordered.lock
    assert "pending=false" in repr(reordered)
    with pytest.raises(ValueError, match="already been consumed"):
        reordered.commit(store)
    assert eqiora.open_project(project, store) == resolution
    assert eqiora.compile_package(store, resolution, entry="Main").digest


def test_open_fetch_and_failed_update_keep_the_exact_selection(tmp_path: Path) -> None:
    project, store = tmp_path / "project", tmp_path / "store"
    store.mkdir()
    package(tmp_path / "old", "Library", "1.0.0")
    package(tmp_path / "new", "Library", "1.1.0")
    package(project, "Root", "1.0.0", request(["../old"]))
    accepted = eqiora.resolve_local_project(project, store)
    lock = (project / "eqiora.lock").read_bytes()
    original = eqiora.compile_package(store, accepted, entry="Main")
    package(project, "Root", "1.0.0", request(["../new", "../old"]))
    assert eqiora.open_project(project, store) == accepted
    fetched = tmp_path / "fetched"
    fetched.mkdir()
    assert eqiora.fetch_project(project, fetched) == accepted
    assert eqiora.compile_package(fetched, accepted, entry="Main").digest == original.digest
    assert (project / "eqiora.lock").read_bytes() == lock
    (tmp_path / "new/src/main.eqi").write_text(
        "public component Invalid() { parameter value: MissingUnit = 1; }", encoding="utf-8"
    )
    manifest = (project / "eqiora.toml").read_bytes()
    with pytest.raises(eqiora.CompatibilityError):
        eqiora.update_project(project, store)
    assert (project / "eqiora.toml").read_bytes() == manifest
    assert (project / "eqiora.lock").read_bytes() == lock
    assert eqiora.open_project(project, store) == accepted
    package(tmp_path / "new", "Library", "1.1.0")
    updated = eqiora.update_project(project, store)
    assert updated != accepted
    assert json.loads((project / "eqiora.lock").read_bytes())["requests"][0]["selected"] == "1.1.0"
    package(project, "Root", "1.0.0", request(["../new", "../old"], ">=1.0.0,<2.0.0"))
    with pytest.raises(eqiora.CompatibilityError, match="authored requests differ"):
        eqiora.open_project(project, store)
    assert eqiora.update_project(project, store) == updated


def test_literal_prerelease_and_candidate_identity_are_not_transport_order(tmp_path: Path) -> None:
    project = tmp_path / "project"
    package(tmp_path / "one", "Library", "1.2.3-rc.1")
    package(tmp_path / "two", "Library", "1.2.3-rc.2")
    package(project, "Root", "1.0.0", request(["../two", "../one"], "1.2.3-rc.1"))
    assert json.loads(eqiora.preview_local_project(project).lock)["requests"][0]["selected"] == "1.2.3-rc.1"
    package(project, "Root", "1.0.0", request(["../two", "../one"], "1.2"))
    with pytest.raises(eqiora.CompatibilityError, match="no single release"):
        eqiora.preview_local_project(project)
    for value in ("0", "latest", "^1.2.3", ">=1.2.3"):
        package(project, "Root", "1.0.0", request(["../one"], value))
        with pytest.raises(eqiora.CompatibilityError, match="invalid version request"):
            eqiora.preview_local_project(project)
