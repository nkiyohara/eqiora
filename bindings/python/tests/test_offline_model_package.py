from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import tempfile
from pathlib import Path
from typing import Callable

import pytest

import eqiora


ROOT = Path(__file__).resolve().parents[3]
PRIMARY = ROOT / "verify/packages/offline-model-package/models"
PRIMARY_STORE = PRIMARY / "store"
PRIMARY_RESOLUTION_FILE = PRIMARY / "resolution.json"
EXPECTED = (
    ROOT
    / "verify/artifacts/current-model-relational-identity-transition"
    / "expected/deterministic/offline-model-package"
)
EXPECTED_TYPED = (
    ROOT
    / "verify/artifacts/current-model-relational-identity-transition"
    / "expected/deterministic/typed-execution-lineage"
)
SECONDARY = (
    ROOT
    / "verify/interfaces/python-offline-model-package/models/typed-execution-lineage"
)
HOME_SCRATCH = Path.home() / ".cache/eqiora/oracle-tests"

OFFLINE_MODEL_ID = "3JNCJVGEYX9N2QSYVEXRXWXWF4"
OFFLINE_MODEL_DIGEST = (
    "92837f0f85ff4a1310af0ca6e412d3ace81393df837d017caf5bfabeb8f6c1a1"
)
OFFLINE_RESOLUTION_DIGEST = (
    "081cca92b2a8d6ee8bba78741db2becd5d5edfa896a114a193c8e1486997b6fe"
)
OFFLINE_COMPILATION_DIGEST = (
    "a6e31415d973c5dc23a92a101ba3db7cef7b1b70b0dc51d2b73214f1fc00bf49"
)
TYPED_MODEL_ID = "7Q7ZYW89BV0RH2HSB3S5ZMTY0K"
TYPED_SOURCE_DIGEST = (
    "4f3aa811b814ac7fb959f777ff5d758804e2e68593a568ee8935b122c9565462"
)
TYPED_RESOLUTION_DIGEST = (
    "38b5bb0c7e1f8aa7baa5e690157014a974c446f8f38fcd19d6b73b981e9ca810"
)
TYPED_MODEL_DIGEST = (
    "c2c35e6b58f6ee0d40b8aa2bd0c252e519eec6f6779e39366ae2e28cdbd5300a"
)
TYPED_COMPILATION_DIGEST = (
    "6e72043a1d0569d7488717cd7ffdf54a01c7e5e65262cecc3a49fcdce645dec0"
)
LIBRARY_SOURCE = (
    "ce343238d92f202646d2dd2947d68c311eac90aa711aa9d0e3905fa170f6f3f1"
)
ROOT_SOURCE = (
    "cd7afe063d06007b97c108d3957e1bdc92e64fe47adfc7ac92975fee4f2c0d28"
)


def canonical_fixture(path: Path) -> bytes:
    stored = path.read_bytes()
    assert stored.endswith(b"\n")
    canonical = stored.removesuffix(b"\n")
    assert not canonical.endswith(b"\n")
    return canonical


PRIMARY_RESOLUTION = canonical_fixture(PRIMARY_RESOLUTION_FILE)
SECONDARY_RESOLUTION = canonical_fixture(SECONDARY / "resolution.json")
EXPECTED_MODEL = canonical_fixture(EXPECTED / "model.json")
EXPECTED_TYPED_MODEL = canonical_fixture(EXPECTED_TYPED / "model.json")


def assert_no_lineage(model: eqiora.Model) -> None:
    assert model.package_compilation_digest is None


def assert_compatibility(error: eqiora.CompatibilityError) -> None:
    assert error.category == "compatibility"
    assert error.diagnostics
    assert error.diagnostics[0].code == "EQ0901"


def tree_snapshot(root: Path) -> tuple[tuple[object, ...], ...]:
    snapshot: list[tuple[object, ...]] = []
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix()
        metadata = path.lstat()
        mode = stat.S_IMODE(metadata.st_mode)
        if path.is_symlink():
            snapshot.append((relative, "symlink", mode, os.readlink(path)))
        elif path.is_dir():
            snapshot.append((relative, "directory", mode))
        elif path.is_file():
            content = path.read_bytes()
            snapshot.append(
                (relative, "file", mode, len(content), hashlib.sha256(content).hexdigest())
            )
        else:
            snapshot.append((relative, "nonregular", mode))
    return tuple(snapshot)


def assert_store_rejection(
    store: Path,
    *,
    resolution: bytes = PRIMARY_RESOLUTION,
    entry_model: str = "Main",
    expected: type[eqiora.EqioraError] = eqiora.CompatibilityError,
) -> eqiora.EqioraError:
    before = tree_snapshot(store)
    result: object = object()
    sentinel = result
    with pytest.raises(expected) as caught:
        result = eqiora.compile_package(
            store,
            resolution,
            entry_model=entry_model,
        )
    assert result is sentinel, "a rejected call exposed a partial Model"
    assert tree_snapshot(store) == before, "compile_package mutated its selected store"
    return caught.value


def copied_store(parent: Path, name: str = "保管庫") -> Path:
    store = parent / name
    shutil.copytree(PRIMARY_STORE, store)
    return store


def with_scratch(callback: Callable[[Path], None]) -> None:
    HOME_SCRATCH.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="python-offline-model-package-",
        dir=HOME_SCRATCH,
    ) as directory:
        callback(Path(directory))


def test_exact_package_projection_is_the_frozen_ordinary_model() -> None:
    model = eqiora.compile_package(
        PRIMARY_STORE,
        PRIMARY_RESOLUTION,
        entry_model="Main",
    )
    assert type(model) is eqiora.Model
    assert model.to_json() == EXPECTED_MODEL
    assert model.model_id == OFFLINE_MODEL_ID
    assert model.revision.model_id == OFFLINE_MODEL_ID
    assert model.revision.number == 1
    assert model.digest == OFFLINE_MODEL_DIGEST
    assert model.revision.digest == OFFLINE_MODEL_DIGEST
    assert model.package_compilation_digest == OFFLINE_COMPILATION_DIGEST
    with pytest.raises(AttributeError):
        model.package_compilation_digest = "0" * 64
    with pytest.raises(AttributeError):
        del model.package_compilation_digest

    identities = json.loads(
        (ROOT / "verify/packages/offline-model-package/expected/identities.json").read_text(
            encoding="utf-8"
        )
    )
    assert identities["resolution_digest"] == OFFLINE_RESOLUTION_DIGEST
    assert identities["model_digest"] == model.digest
    assert identities["compilation_digest"] == model.package_compilation_digest

    replayed = eqiora.replay(model.to_json())
    assert replayed.to_json() == model.to_json()
    assert replayed.revision == model.revision
    assert replayed == model
    assert hash(replayed) == hash(model)
    assert repr(replayed) == repr(model)
    assert_no_lineage(replayed)

    fingerprint = model.structural_fingerprint
    assert fingerprint == replayed.structural_fingerprint
    assert model.structurally_equivalent(replayed)
    assert model.field_ids == []
    assert model.parameter_ids == []
    assert model.package_compilation_digest == OFFLINE_COMPILATION_DIGEST

    reacquired = eqiora.compile_package(
        str(PRIMARY_STORE),
        PRIMARY_RESOLUTION,
        entry_model="Main",
    )
    assert reacquired == model
    assert reacquired.package_compilation_digest == OFFLINE_COMPILATION_DIGEST

    source = eqiora.compile(
        """
model source_model {
  field x: 1 = 1;
  relation hold continuous { derivative(x) = 0; }
}
"""
    )
    assert_no_lineage(source)
    field = eqiora.Field("x", initial=1.0)
    relation = eqiora.Relation("hold", residual=eqiora.derivative(field))
    defined = eqiora.Model.define("defined_model", field, relation)
    assert_no_lineage(defined)


def test_commit_clears_only_the_accepted_parent_lineage() -> None:
    model = eqiora.compile_package(
        SECONDARY / "store",
        SECONDARY_RESOLUTION,
        entry_model="Main",
    )
    assert model.model_id == TYPED_MODEL_ID
    assert model.revision.number == 1
    assert model.digest == TYPED_MODEL_DIGEST
    assert model.to_json() == EXPECTED_TYPED_MODEL
    assert model.package_compilation_digest == TYPED_COMPILATION_DIGEST

    identities = json.loads(
        (ROOT / "verify/packages/typed-execution-lineage/expected/identities.json").read_text(
            encoding="utf-8"
        )
    )
    assert identities["source_bundle_sha256"] == TYPED_SOURCE_DIGEST
    assert identities["resolution_sha256"] == TYPED_RESOLUTION_DIGEST
    assert identities["model_sha256"] == model.digest
    assert identities["package_compilation_sha256"] == model.package_compilation_digest

    edit = model.preview_value_edit("wave_number", 4.0)
    child = model.commit(edit)
    assert child.model_id == model.model_id
    assert child.revision.number == 2
    assert child.digest != model.digest
    assert_no_lineage(child)
    assert model.package_compilation_digest == TYPED_COMPILATION_DIGEST
    assert_no_lineage(eqiora.replay(child.to_json()))


def test_argument_shapes_require_explicit_path_bytes_and_keyword_selector() -> None:
    before = tree_snapshot(PRIMARY_STORE)
    with pytest.raises(TypeError):
        eqiora.compile_package(
            os.fsencode(PRIMARY_STORE),
            PRIMARY_RESOLUTION,
            entry_model="Main",
        )
    for invalid in [
        bytearray(PRIMARY_RESOLUTION),
        memoryview(PRIMARY_RESOLUTION),
        PRIMARY_RESOLUTION.decode("utf-8"),
        {"resolution": PRIMARY_RESOLUTION},
    ]:
        with pytest.raises(TypeError):
            eqiora.compile_package(PRIMARY_STORE, invalid, entry_model="Main")

    class ResolutionBytes(bytes):
        pass

    bytes_subclass = eqiora.compile_package(
        PRIMARY_STORE,
        ResolutionBytes(PRIMARY_RESOLUTION),
        entry_model="Main",
    )
    assert bytes_subclass.digest == OFFLINE_MODEL_DIGEST
    assert bytes_subclass.package_compilation_digest == OFFLINE_COMPILATION_DIGEST
    with pytest.raises(TypeError):
        eqiora.compile_package(PRIMARY_STORE, PRIMARY_RESOLUTION)
    with pytest.raises(TypeError):
        eqiora.compile_package(PRIMARY_STORE, PRIMARY_RESOLUTION, "Main")
    with pytest.raises(TypeError):
        eqiora.compile_package(PRIMARY_STORE, PRIMARY_RESOLUTION, entry_model=1)
    assert tree_snapshot(PRIMARY_STORE) == before


def test_resolution_bytes_are_exact_and_store_relative() -> None:
    before = tree_snapshot(PRIMARY_STORE)
    decoded = json.loads(PRIMARY_RESOLUTION)
    reordered = json.dumps(
        {
            "nodes": decoded["nodes"],
            "schema": decoded["schema"],
            "root": decoded["root"],
            "edges": decoded["edges"],
        },
        separators=(",", ":"),
    ).encode("utf-8")
    duplicate_key = (
        b'{"schema":"eqiora.package-resolution.v1",' + PRIMARY_RESOLUTION[1:]
    )
    for invalid in [
        b"{}",
        b"{not-json",
        b" " + PRIMARY_RESOLUTION,
        reordered,
        duplicate_key,
        PRIMARY_RESOLUTION_FILE.read_bytes(),
    ]:
        with pytest.raises(eqiora.CompatibilityError) as caught:
            eqiora.compile_package(PRIMARY_STORE, invalid, entry_model="Main")
        assert_compatibility(caught.value)
        assert tree_snapshot(PRIMARY_STORE) == before

    mismatch = assert_store_rejection(
        SECONDARY / "store",
        resolution=PRIMARY_RESOLUTION,
    )
    assert isinstance(mismatch, eqiora.CompatibilityError)
    assert_compatibility(mismatch)
    second = eqiora.compile_package(
        SECONDARY / "store",
        SECONDARY_RESOLUTION,
        entry_model="Main",
    )
    assert second.model_id == TYPED_MODEL_ID


@pytest.mark.parametrize(
    "selector",
    [
        "",
        "Unknown",
        "org.example.parallel.Main",
        "electrical.Resistor",
        "Resistor",
        "Ground",
    ],
)
def test_selector_stays_bare_root_local_and_model_typed(selector: str) -> None:
    error = assert_store_rejection(
        PRIMARY_STORE,
        entry_model=selector,
        expected=eqiora.ValidationError,
    )
    assert error.category == "validation"
    assert error.diagnostics


def test_missing_mutated_and_unrelated_store_entries() -> None:
    def run(parent: Path) -> None:
        unicode_store = copied_store(parent)
        unicode_model = eqiora.compile_package(
            str(unicode_store),
            PRIMARY_RESOLUTION,
            entry_model="Main",
        )
        assert unicode_model.digest == OFFLINE_MODEL_DIGEST

        unrelated = copied_store(parent, "unrelated")
        (unrelated / "caller-note.json").write_text("not part of the closure\n", encoding="utf-8")
        accepted = eqiora.compile_package(
            unrelated,
            PRIMARY_RESOLUTION,
            entry_model="Main",
        )
        assert accepted.to_json() == EXPECTED_MODEL
        assert accepted.package_compilation_digest == OFFLINE_COMPILATION_DIGEST

        missing = copied_store(parent, "missing")
        (missing / f"{LIBRARY_SOURCE}.json").unlink()
        assert_compatibility(assert_store_rejection(missing))

        mutated = copied_store(parent, "mutated")
        release_path = mutated / f"{ROOT_SOURCE}.json"
        release = json.loads(release_path.read_bytes())
        documentation = release["source"]["files"][0]
        encoded = documentation["bytes"]
        documentation["bytes"] = ("J" if encoded[0] != "J" else "I") + encoded[1:]
        release_path.write_bytes(json.dumps(release, separators=(",", ":")).encode("utf-8"))
        assert_compatibility(assert_store_rejection(mutated))

        nonregular = copied_store(parent, "nonregular")
        entry = nonregular / f"{ROOT_SOURCE}.json"
        entry.unlink()
        entry.mkdir()
        assert_compatibility(assert_store_rejection(nonregular))

        missing_root = parent / "absent-store"
        with pytest.raises(eqiora.CompatibilityError) as caught:
            eqiora.compile_package(
                missing_root,
                PRIMARY_RESOLUTION,
                entry_model="Main",
            )
        assert_compatibility(caught.value)

        regular_root = parent / "not-a-directory"
        regular_root.write_text("not a store\n", encoding="utf-8")
        with pytest.raises(eqiora.CompatibilityError) as caught:
            eqiora.compile_package(
                regular_root,
                PRIMARY_RESOLUTION,
                entry_model="Main",
            )
        assert_compatibility(caught.value)

    with_scratch(run)


@pytest.mark.skipif(os.name != "posix", reason="registered symlink oracle is POSIX-only")
def test_store_and_exact_entry_symlinks_fail_closed() -> None:
    def run(parent: Path) -> None:
        entry_store = copied_store(parent, "entry-symlink")
        entry = entry_store / f"{ROOT_SOURCE}.json"
        target = parent / "outside-release.json"
        entry.replace(target)
        entry.symlink_to(target)
        assert_compatibility(assert_store_rejection(entry_store))

        real_store = copied_store(parent, "real-store")
        linked_store = parent / "linked-store"
        linked_store.symlink_to(real_store, target_is_directory=True)
        before = tree_snapshot(real_store)
        with pytest.raises(eqiora.CompatibilityError) as caught:
            eqiora.compile_package(
                linked_store,
                PRIMARY_RESOLUTION,
                entry_model="Main",
            )
        assert_compatibility(caught.value)
        assert tree_snapshot(real_store) == before

    with_scratch(run)
