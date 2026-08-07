from __future__ import annotations

import base64
import copy
import hashlib
import io
import json
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from dataclasses import replace
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/release"))

import candidate_manifest as candidate_manifest_module  # noqa: E402
from candidate_manifest import (  # noqa: E402
    ManifestError,
    PROFILE_CHECKS,
    file_sha256,
    load_candidate,
    require_candidate_profile,
    verify_artifacts,
)
from testpypi_replay import release_files  # noqa: E402


V3_FORMAT = "eqiora.python-distribution-candidate/v3"
CONTRACT_SHA256 = "3f3a9f1a5b54bf5b874d996c8807bbb7e88439737fd245d69e7a8aeb7a1a87c1"
PROTECTED_BASE_SHA = "3dfb1086168afc6f9fb61f9ca43d21ca9953048b"
NODE_EXECUTABLE_SHA256 = (
    "f3432a45b03b2da0d270095fdd8813dc34cbea73f5fc8b18c7a384b7cf9b333a"
)
NPM_PACKAGE_INTEGRITY = (
    "sha512-A74XL8OxmcegZDMWPkWb5bEQppg8HdYwW3rBD2sPoS4UQHVajfaxBkqyzLeJ3wR0kZ+"
    "5xoTjItxXaF7eIXUsyw=="
)
ANYWIDGET_WHEEL_SHA256 = (
    "c574d9acc6503ad27b37a9acea48f957a8ba7c9c9876cfcb37898931c098ce9d"
)
BROWSERS_JSON_SHA256 = (
    "f306eed529599b1eaf2f8a85db9de2b23e1a3fe36c2b66434b7c9434fb627a99"
)
THREE_LICENSE_SHA256 = (
    "8b378ebe60e2fe500158cb0ac71cb5e8b7d92953c2abcc63a0eb90499653b5bc"
)
ANYWIDGET_LICENSE_SHA256 = (
    "22c698b6e5f3878c292471980ffd352ee0fad053f9428c2281f34b5e28a6151f"
)
NOTEBOOK_CHECKS = frozenset(
    {
        "frontend:lock-integrity",
        "frontend:license-notices",
        "frontend:bundle-byte-rebuild",
        "wheel-family:notebook-metadata",
        "cp313:notebook-anywidget-0.11.0",
        "cp313:jupyterlab-4.6.2-bare-mesh",
        "cp313:marimo-0.23.16-bare-mesh",
        "cp313:notebook-managed-chromium-r1234",
        "cp313:notebook-no-external-network",
        "cp313:notebook-cleanup-and-mutation",
    }
)
ASSET_BYTES = {
    "eqiora/_presentation/static/mesh-view.mjs": b"// synthetic oracle asset\n",
    "eqiora/_presentation/static/mesh-view.css": b"/* synthetic oracle asset */\n",
    "eqiora/_presentation/static/THIRD_PARTY_NOTICES.txt": (
        b"Synthetic Three.js notice used only by the release-schema oracle.\n"
    ),
}
BASE_METADATA = b"""\
Metadata-Version: 2.4
Name: eqiora
Version: 0.1.0a1
Requires-Python: <3.15,>=3.11
Provides-Extra: jax
Provides-Extra: matplotlib
Provides-Extra: torch
Requires-Dist: numpy<3,>=2.1
Requires-Dist: torch>=2.13,<2.14; extra == "torch"
Requires-Dist: jax==0.11.0; python_version >= "3.12" and extra == "jax"
Requires-Dist: jaxlib==0.11.0; python_version >= "3.12" and extra == "jax"
Requires-Dist: matplotlib==3.11.1; extra == "matplotlib"

signal-free v2 candidate
"""
BASE_PYPROJECT = b"""\
[project]
name = "eqiora"
version = "0.1.0a1"
dependencies = ["numpy>=2.1,<3"]
"""


def canonical_json_bytes(value: object) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def structured_sha256(value: object) -> str:
    return hashlib.sha256(canonical_json_bytes(value)).hexdigest()


def _write_wheel(
    path: Path,
    *,
    metadata: bytes = BASE_METADATA,
    extra_members: dict[str, bytes] | None = None,
) -> None:
    members = {
        "eqiora/__init__.py": b"__version__ = '0.1.0a1'\n",
        "eqiora-0.1.0a1.dist-info/METADATA": metadata,
        "eqiora-0.1.0a1.dist-info/WHEEL": b"Wheel-Version: 1.0\n",
        "eqiora-0.1.0a1.dist-info/RECORD": b"",
    }
    members.update(extra_members or {})
    with zipfile.ZipFile(path, mode="w") as archive:
        for name, payload in sorted(members.items()):
            archive.writestr(name, payload)


def _write_sdist(
    path: Path,
    *,
    pyproject: bytes = BASE_PYPROJECT,
    pkg_info: bytes = BASE_METADATA,
    extra_members: dict[str, bytes] | None = None,
) -> None:
    prefix = "eqiora-0.1.0a1/"
    members = {
        f"{prefix}PKG-INFO": pkg_info,
        f"{prefix}pyproject.toml": pyproject,
        f"{prefix}bindings/python/python/eqiora/__init__.py": (
            b"__version__ = '0.1.0a1'\n"
        ),
    }
    members.update(
        {f"{prefix}{name}": payload for name, payload in (extra_members or {}).items()}
    )
    with tarfile.open(path, mode="w:gz") as archive:
        for name, payload in sorted(members.items()):
            member = tarfile.TarInfo(name)
            member.mode = 0o644
            member.mtime = 0
            member.size = len(payload)
            archive.addfile(member, io.BytesIO(payload))


def _rewrite_wheel(
    path: Path,
    *,
    metadata: bytes | None = None,
    extra_members: dict[str, bytes] | None = None,
) -> None:
    with zipfile.ZipFile(path) as archive:
        members = {name: archive.read(name) for name in archive.namelist()}
    if metadata is not None:
        members["eqiora-0.1.0a1.dist-info/METADATA"] = metadata
    members.update(extra_members or {})
    with zipfile.ZipFile(path, mode="w") as archive:
        for name, payload in sorted(members.items()):
            archive.writestr(name, payload)


def _rewrite_sdist(
    path: Path,
    *,
    remove_members: tuple[str, ...] = (),
    replace_members: dict[str, bytes] | None = None,
) -> None:
    with tarfile.open(path, mode="r:*") as archive:
        members = {
            member.name: archive.extractfile(member).read()
            for member in archive.getmembers()
            if member.isfile()
        }
    for name in remove_members:
        members.pop(name)
    members.update(replace_members or {})
    with tarfile.open(path, mode="w:gz") as archive:
        for name, payload in sorted(members.items()):
            member = tarfile.TarInfo(name)
            member.mode = 0o644
            member.mtime = 0
            member.size = len(payload)
            archive.addfile(member, io.BytesIO(payload))


def _refresh_artifact_records(artifacts: Path, document: dict) -> None:
    by_name = {record["filename"]: record for record in document["artifacts"]}
    for path in artifacts.iterdir():
        if path.name not in by_name:
            continue
        payload = path.read_bytes()
        by_name[path.name]["size"] = len(payload)
        by_name[path.name]["sha256"] = hashlib.sha256(payload).hexdigest()


def candidate_document(root: Path) -> tuple[Path, Path, dict]:
    artifacts = root / "artifacts"
    artifacts.mkdir()
    records = []
    for filename, kind, python in (
        ("eqiora-0.1.0a1.tar.gz", "sdist", None),
        (
            "eqiora-0.1.0a1-cp311-cp311-manylinux_2_17_x86_64.whl",
            "wheel",
            "3.11",
        ),
        (
            "eqiora-0.1.0a1-cp312-cp312-manylinux_2_17_x86_64.whl",
            "wheel",
            "3.12",
        ),
        (
            "eqiora-0.1.0a1-cp313-cp313-manylinux_2_17_x86_64.whl",
            "wheel",
            "3.13",
        ),
        (
            "eqiora-0.1.0a1-cp314-cp314-manylinux_2_17_x86_64.whl",
            "wheel",
            "3.14",
        ),
    ):
        artifact = artifacts / filename
        if kind == "sdist":
            _write_sdist(artifact)
        else:
            _write_wheel(artifact)
        data = artifact.read_bytes()
        record = {
            "filename": filename,
            "kind": kind,
            "size": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        }
        if python is not None:
            record.update(
                {
                    "python": python,
                    "abi": f"cp{python.replace('.', '')}",
                    "platform": "manylinux_2_17_x86_64",
                }
            )
        records.append(record)
    document = {
        "format": "eqiora.python-distribution-candidate/v2",
        "project": "eqiora",
        "version": "0.1.0a1",
        "acceptance": "complete",
        "source": {
            "commit": "1" * 40,
            "expected_tag": "v0.1.0a1",
            "tags": [],
            "tree": "clean",
        },
        "build": {
            "sdist_rebuilt": True,
            "wheel_family": {
                "implementation": "CPython",
                "ordinary_gil": True,
                "versions": ["3.11", "3.12", "3.13", "3.14"],
                "platform": "manylinux_2_17_x86_64",
                "abi3": False,
            },
            "dependency_profiles": {
                "numpy_floor": {
                    "python": "3.12",
                    "requirement": "numpy==2.1.0",
                    "observed": "2.1.0",
                    "profile": "cp312:numpy-2.1.0-floor",
                },
            },
        },
        "artifacts": records,
        "checks": [
            "generated-public-api",
            "sdist-to-wheel-rebuild",
            "twine-strict",
            "cp311:installed-wheel",
            "cp311:base-and-numpy",
            "cp311:packaged-mixed-boundary-elasticity-demo",
            "cp311:packaged-fixed-reference-fsi-demo",
            "cp311:async-and-cancellation",
            "cp311:strict-base-typing",
            "cp311:public-smoke-base",
            "cp311:matplotlib-free-base",
            "cp312:installed-wheel",
            "cp312:base-and-numpy",
            "cp312:packaged-mixed-boundary-elasticity-demo",
            "cp312:packaged-fixed-reference-fsi-demo",
            "cp312:async-and-cancellation",
            "cp312:strict-base-typing",
            "cp312:public-smoke-base",
            "cp312:numpy-2.1.0-floor",
            "cp312:matplotlib-free-base",
            "cp313:installed-wheel",
            "cp313:base-and-numpy",
            "cp313:packaged-mixed-boundary-elasticity-demo",
            "cp313:packaged-fixed-reference-fsi-demo",
            "cp313:async-and-cancellation",
            "cp313:strict-base-typing",
            "cp313:public-smoke-base",
            "cp313:matplotlib-free-base",
            "cp314:installed-wheel",
            "cp314:base-and-numpy",
            "cp314:packaged-mixed-boundary-elasticity-demo",
            "cp314:packaged-fixed-reference-fsi-demo",
            "cp314:async-and-cancellation",
            "cp314:strict-base-typing",
            "cp314:public-smoke-base",
            "cp314:matplotlib-free-base",
            "cp313:torch",
            "cp313:jax",
            "cp313:matplotlib",
            "cp313:public-smoke-torch",
            "cp313:public-smoke-jax",
            "cp313:packaged-exact-cylinder-pressure-demo",
            "cp313:packaged-mixed-boundary-displacement-demo",
            "cp313:packaged-fixed-reference-fsi-still",
            "cp313:complete-public-typing",
        ],
    }
    manifest = root / "candidate.json"
    manifest.write_text(json.dumps(document), encoding="utf-8")
    return manifest, artifacts, document


def notebook_metadata(
    *requirements: str,
    provides: tuple[str, ...] = ("notebook",),
) -> bytes:
    lines = BASE_METADATA.decode("utf-8").splitlines()
    body = lines.index("")
    additions = [*(f"Provides-Extra: {name}" for name in provides)]
    additions.extend(f"Requires-Dist: {requirement}" for requirement in requirements)
    return "\n".join([*lines[:body], *additions, "", "v3 notebook candidate", ""]).encode(
        "utf-8"
    )


def _manifest_artifacts(document: dict) -> list[dict]:
    artifacts = [
        {
            "filename": record["filename"],
            "kind": record["kind"],
            "size": record["size"],
            "sha256": record["sha256"],
        }
        for record in document["artifacts"]
    ]
    return sorted(artifacts, key=lambda item: item["filename"].encode("utf-8"))


def _file_record(relative_path: str, payload: bytes) -> dict:
    return {
        "relative_path": relative_path,
        "mode": 0o644,
        "size": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


def _bind_receipt(
    manifest: Path,
    artifacts: Path,
    document: dict,
    receipt_path: Path,
    receipt: dict,
) -> None:
    _refresh_artifact_records(artifacts, document)
    receipt["candidate"]["artifacts"] = _manifest_artifacts(document)
    receipt_bytes = canonical_json_bytes(receipt)
    receipt_path.write_bytes(receipt_bytes)
    document["build"]["frontend"]["h2_receipt_sha256"] = hashlib.sha256(
        receipt_bytes
    ).hexdigest()
    manifest.write_text(json.dumps(document), encoding="utf-8")


def complete_v3_candidate_document(
    root: Path,
) -> tuple[Path, Path, dict, Path, dict]:
    """Create synthetic schema bytes, never an Eqiora product build or H2 result."""
    manifest, artifacts, document = candidate_document(root)
    writer_revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"],
        cwd=REPOSITORY_ROOT,
        text=True,
    ).strip()
    source_date_epoch = int(
        subprocess.check_output(
            ["git", "show", "-s", "--format=%ct", writer_revision],
            cwd=REPOSITORY_ROOT,
            text=True,
        ).strip()
    )
    document["source"]["commit"] = writer_revision
    exact_requirement = 'anywidget == 0.11.0 ; extra == "notebook"'
    wheel_metadata = notebook_metadata(exact_requirement)
    for record in document["artifacts"]:
        artifact = artifacts / record["filename"]
        if record["kind"] == "wheel":
            _rewrite_wheel(
                artifact,
                metadata=wheel_metadata,
                extra_members=ASSET_BYTES,
            )

    package_json = b'{"private":true,"packageManager":"npm@11.16.0"}\n'
    package_lock = b'{"lockfileVersion":3,"packages":{}}\n'
    source = b"export const renderer = 'synthetic-oracle-only';\n"
    config = b"export default {build: {sourcemap: false}};\n"
    pyproject = BASE_PYPROJECT + b"""\

[project.optional-dependencies]
notebook = ["anywidget==0.11.0"]
"""
    sdist_members = {
        "bindings/python/frontend/package.json": package_json,
        "bindings/python/frontend/package-lock.json": package_lock,
        "bindings/python/frontend/src/mesh-view.ts": source,
        "bindings/python/frontend/vite.config.ts": config,
        "bindings/python/src/notebook_hook.rs": b"fn _repr_mimebundle_() {}\n",
        **{
            f"bindings/python/python/{name}": payload
            for name, payload in ASSET_BYTES.items()
        },
    }
    _write_sdist(
        artifacts / "eqiora-0.1.0a1.tar.gz",
        pyproject=pyproject,
        pkg_info=wheel_metadata,
        extra_members=sdist_members,
    )
    _refresh_artifact_records(artifacts, document)

    source_inventory = sorted(
        (
            _file_record("package-lock.json", package_lock),
            _file_record("package.json", package_json),
            _file_record("src/mesh-view.ts", source),
        ),
        key=lambda item: item["relative_path"].encode("utf-8"),
    )
    config_inventory = [
        {
            "relative_path": "vite.config.ts",
            "sha256": hashlib.sha256(config).hexdigest(),
        }
    ]
    direct_pins = sorted(
        (
            {"name": "three", "version": "0.185.1"},
            {"name": "@types/three", "version": "0.185.4"},
            {"name": "@anywidget/types", "version": "0.4.0"},
            {"name": "typescript", "version": "7.0.2"},
            {"name": "vite", "version": "8.2.0"},
            {"name": "vitest", "version": "4.1.10"},
            {"name": "@biomejs/biome", "version": "2.5.6"},
            {"name": "@playwright/test", "version": "1.62.1"},
        ),
        key=lambda item: (item["name"].encode("utf-8"), item["version"].encode("utf-8")),
    )
    integrity = "sha512-" + base64.b64encode(bytes(64)).decode("ascii")
    locked_packages = sorted(
        (
            {
                "lock_path": f"node_modules/{pin['name']}",
                "name": pin["name"],
                "version": pin["version"],
                "resolved": (
                    f"https://registry.npmjs.org/{pin['name']}/-/"
                    f"package-{pin['version']}.tgz"
                ),
                "integrity": integrity,
                "selected_optional": False,
                "lifecycle_scripts": [],
            }
            for pin in direct_pins
        ),
        key=lambda item: item["lock_path"].encode("utf-8"),
    )
    module_graph = [
        {
            "output": "mesh-view.mjs",
            "input": "node_modules/three/build/three.module.js",
            "package": "three",
            "version": "0.185.1",
        },
        {
            "output": "mesh-view.mjs",
            "input": "src/mesh-view.ts",
            "package": "eqiora",
            "version": "0.1.0a1",
        },
    ]
    output_inventory = [
        _file_record(relative_path, payload)
        for relative_path, payload in sorted(
            ASSET_BYTES.items(), key=lambda item: item[0].encode("utf-8")
        )
    ]
    python_wheels = [
        {
            "name": "anywidget",
            "version": "0.11.0",
            "filename": "anywidget-0.11.0-py3-none-any.whl",
            "sha256": ANYWIDGET_WHEEL_SHA256,
        }
    ]
    install_script_inventory = [
        {
            "lock_path": item["lock_path"],
            "name": item["name"],
            "version": item["version"],
            "lifecycle_scripts": item["lifecycle_scripts"],
        }
        for item in locked_packages
    ]
    frontend = {
        "node": "v24.18.1",
        "npm": "11.16.0",
        "h2_receipt_sha256": "0" * 64,
        "package_json_sha256": hashlib.sha256(package_json).hexdigest(),
        "package_lock_sha256": hashlib.sha256(package_lock).hexdigest(),
        "source_inventory_sha256": structured_sha256(source_inventory),
        "config_inventory_sha256": structured_sha256(config_inventory),
        "locked_packages_sha256": structured_sha256(locked_packages),
        "install_script_inventory_sha256": structured_sha256(
            install_script_inventory
        ),
        "bundler_module_graph_sha256": structured_sha256(module_graph),
        "node_executable_sha256": NODE_EXECUTABLE_SHA256,
        "npm_package_integrity": NPM_PACKAGE_INTEGRITY,
        "assets": {
            path: {"size": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}
            for path, payload in ASSET_BYTES.items()
        },
        "licenses": {
            "three@0.185.1": {
                "expression": "MIT",
                "source_license_sha256": THREE_LICENSE_SHA256,
            },
            "anywidget@0.11.0": {
                "expression": "MIT",
                "source_license_sha256": ANYWIDGET_LICENSE_SHA256,
            },
        },
        "runtime": {
            "python": "3.13",
            "anywidget": "0.11.0",
            "jupyterlab": "4.6.2",
            "marimo": "0.23.16",
            "anywidget_wheel_sha256": ANYWIDGET_WHEEL_SHA256,
            "resolved_environment_sha256": structured_sha256(python_wheels),
        },
        "browser": {
            "playwright": "1.62.1",
            "chromium_revision": "1234",
            "browser_version": "151.0.7922.34",
            "browsers_json_sha256": BROWSERS_JSON_SHA256,
            "platform": "linux-x86_64",
            "downloaded_archive_sha256": "a" * 64,
            "executable_sha256": "b" * 64,
        },
    }
    document["format"] = V3_FORMAT
    document["build"]["frontend"] = frontend
    document["checks"].extend(sorted(NOTEBOOK_CHECKS))

    run = {
        "isolated_directory_id": "clean-run-1",
        "npm_ci_exit": 0,
        "build_exit": 0,
        "output_inventory": output_inventory,
        "emitted_imports": [],
        "source_maps": [],
        "external_request_count_after_npm_ci": 0,
    }
    receipt = {
        "probe": {
            "contract_sha256": CONTRACT_SHA256,
            "protected_base_sha": PROTECTED_BASE_SHA,
            "writer_revision": document["source"]["commit"],
            "verdict": "PASS",
        },
        "candidate": {
            "project": "eqiora",
            "version": document["version"],
            "source_commit": document["source"]["commit"],
            "artifacts": _manifest_artifacts(document),
        },
        "environment": {
            "os": "Linux",
            "architecture": "x86_64",
            "libc": "glibc-2.17",
            "node_version": "v24.18.1",
            "node_executable_sha256": NODE_EXECUTABLE_SHA256,
            "npm_version": "11.16.0",
            "npm_package_integrity": NPM_PACKAGE_INTEGRITY,
            "locale": "C.UTF-8",
            "timezone": "UTC",
            "source_date_epoch": source_date_epoch,
            "environment_allowlist": [
                "HOME",
                "LANG",
                "LC_ALL",
                "PATH",
                "SOURCE_DATE_EPOCH",
                "TZ",
            ],
        },
        "inputs": {
            "source_root_inventory": source_inventory,
            "package_json_sha256": frontend["package_json_sha256"],
            "package_lock_sha256": frontend["package_lock_sha256"],
            "lockfile_version": 3,
            "config_inventory": config_inventory,
            "direct_pins": direct_pins,
            "locked_packages": locked_packages,
            "anywidget_wheel_sha256": ANYWIDGET_WHEEL_SHA256,
        },
        "build": {
            "npm_ci_command_argv": ["npm", "ci", "--ignore-scripts"],
            "exact_command_argv": ["npm", "run", "build"],
            "network_policy": "registry-only-during-npm-ci;offline-after",
            "bundler_version": "8.2.0",
            "bundler_module_graph": module_graph,
            "externals": [],
        },
        "clean_run_1": run,
        "clean_run_2": {**copy.deepcopy(run), "isolated_directory_id": "clean-run-2"},
        "comparison": {
            "complete_relative_path_set_equal": True,
            "modes_equal": True,
            "sizes_equal": True,
            "sha256_bytes_equal": True,
            "diff": [],
        },
        "licenses": {
            "components": [
                {
                    "package": "three",
                    "version": "0.185.1",
                    "license_expression": "MIT",
                    "source_license_path": "node_modules/three/LICENSE",
                    "source_license_sha256": THREE_LICENSE_SHA256,
                    "emitted_outputs": ["mesh-view.mjs"],
                }
            ],
            "notice_path": (
                "eqiora/_presentation/static/THIRD_PARTY_NOTICES.txt"
            ),
            "notice_sha256": frontend["assets"][
                "eqiora/_presentation/static/THIRD_PARTY_NOTICES.txt"
            ]["sha256"],
            "unmapped_emitted_modules": [],
        },
        "browser": {
            "playwright_test_integrity": integrity,
            "playwright_core_integrity": integrity,
            "browsers_json_sha256": BROWSERS_JSON_SHA256,
            "browser_name": "chromium",
            "revision": "1234",
            "browser_version": "151.0.7922.34",
            "platform": "linux-x86_64",
            "downloaded_archive_sha256": "a" * 64,
            "executable_sha256": "b" * 64,
        },
        "python_host": {
            "python": "3.13",
            "resolved_environment_sha256": structured_sha256(python_wheels),
            "wheels": python_wheels,
        },
    }
    receipt_path = root / "eqiora-0.1.0a1-python-candidate-h2.json"
    _bind_receipt(manifest, artifacts, document, receipt_path, receipt)
    return manifest, artifacts, document, receipt_path, receipt


def load_candidate_family(
    manifest: Path,
    artifacts: Path,
    *,
    requested_profiles: tuple[str, ...] = (),
    h2_receipt: Path | None = None,
) -> object:
    loader = getattr(candidate_manifest_module, "load_candidate_family")
    return loader(
        manifest,
        artifacts,
        requested_profiles=requested_profiles,
        h2_receipt=h2_receipt,
    )


class CandidateManifestTests(unittest.TestCase):
    def test_exact_artifact_set_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, artifacts, document = candidate_document(Path(temporary))
            candidate = load_candidate(manifest)
            verify_artifacts(candidate, artifacts)
            self.assertEqual(candidate.version, "0.1.0a1")
            self.assertEqual(candidate.commit, "1" * 40)
            self.assertEqual(len(candidate.artifacts), 5)
            self.assertEqual(file_sha256(manifest), hashlib.sha256(manifest.read_bytes()).hexdigest())

            payload = {
                "info": {"name": "eqiora", "version": "0.1.0a1"},
                "urls": [
                    {
                        "filename": record["filename"],
                        "url": f"https://test-files.pythonhosted.org/{record['filename']}",
                        "size": record["size"],
                        "digests": {"sha256": record["sha256"]},
                    }
                    for record in document["artifacts"]
                ],
            }
            self.assertEqual(
                set(release_files(payload, candidate)),
                {record["filename"] for record in document["artifacts"]},
            )

    def test_substitution_and_extra_file_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, artifacts, _ = candidate_document(Path(temporary))
            candidate = load_candidate(manifest)
            first = artifacts / candidate.artifacts[0].filename
            first.write_bytes(b"substituted")
            with self.assertRaisesRegex(ManifestError, "size differs"):
                verify_artifacts(candidate, artifacts)

        with tempfile.TemporaryDirectory() as temporary:
            manifest, artifacts, _ = candidate_document(Path(temporary))
            candidate = load_candidate(manifest)
            (artifacts / "unreviewed.whl").write_bytes(b"x")
            with self.assertRaisesRegex(ManifestError, "unexpected"):
                verify_artifacts(candidate, artifacts)

    def test_numpy_floor_profile_drift_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, _, document = candidate_document(Path(temporary))
            document["build"]["dependency_profiles"]["numpy_floor"][
                "observed"
            ] = "2.2.0"
            manifest.write_text(json.dumps(document), encoding="utf-8")

            with self.assertRaisesRegex(ManifestError, "NumPy floor profile drifted"):
                load_candidate(manifest)

    def test_every_required_profile_fails_when_its_manifest_check_is_absent(
        self,
    ) -> None:
        for profile, required in PROFILE_CHECKS.items():
            if profile == "notebook":
                # The shared compatibility fixture must remain a genuinely
                # signal-free v2 candidate. Notebook is exercised only by the
                # separate complete-v3 fixture below.
                continue
            with (
                self.subTest(profile=profile),
                tempfile.TemporaryDirectory() as temporary,
            ):
                manifest, _, document = candidate_document(Path(temporary))
                omitted = min(required)
                document["checks"].remove(omitted)
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaisesRegex(
                    ManifestError,
                    rf"candidate {profile} profile omits required check {omitted!r}",
                ):
                    load_candidate(manifest)

    def test_profile_projection_rejects_an_unsuccessful_accepted_entry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, _, _ = candidate_document(Path(temporary))
            candidate = load_candidate(manifest)
            failed = min(PROFILE_CHECKS["torch"])
            unsuccessful = replace(
                candidate,
                checks=candidate.checks - {failed},
            )

            with self.assertRaisesRegex(
                ManifestError,
                rf"candidate torch profile omits required check {failed!r}",
            ):
                require_candidate_profile(unsuccessful, "torch")

    def test_testpypi_metadata_hash_and_host_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, _, document = candidate_document(Path(temporary))
            candidate = load_candidate(manifest)
            payload = {
                "info": {"name": "eqiora", "version": "0.1.0a1"},
                "urls": [
                    {
                        "filename": record["filename"],
                        "url": f"https://test-files.pythonhosted.org/{record['filename']}",
                        "size": record["size"],
                        "digests": {"sha256": record["sha256"]},
                    }
                    for record in document["artifacts"]
                ],
            }
            payload["urls"][0]["digests"]["sha256"] = "0" * 64
            with self.assertRaisesRegex(ManifestError, "metadata hash"):
                release_files(payload, candidate)
            payload["urls"][0]["digests"]["sha256"] = document["artifacts"][0][
                "sha256"
            ]
            payload["urls"][0]["url"] = "https://example.invalid/file"
            with self.assertRaisesRegex(ManifestError, "unexpected host"):
                release_files(payload, candidate)

    def test_signal_free_v2_family_remains_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, artifacts, _ = candidate_document(Path(temporary))
            candidate = load_candidate_family(
                manifest,
                artifacts,
                requested_profiles=("base",),
            )

        self.assertEqual(candidate.version, "0.1.0a1")

    def test_notebook_profile_has_ten_exact_checks(self) -> None:
        self.assertEqual(PROFILE_CHECKS["notebook"], NOTEBOOK_CHECKS)

    def test_every_n1_signal_forces_v3_before_reader_selection(self) -> None:
        cases = (
            "hook-only-sdist",
            "asset-only-wheel",
            "frontend-path-only-sdist",
            "unmarked-anywidget-wheel",
            "anywidget-only-sdist-pkg-info",
            "anywidget-only-sdist-pyproject",
            "notebook-extra-only-sdist",
            "provides-extra-only-wheel",
            "notebook-check-only",
            "frontend-schema-only",
            "v3-format-only",
            "requested-notebook-only",
        )
        for case in cases:
            with (
                self.subTest(case=case),
                tempfile.TemporaryDirectory() as temporary,
            ):
                manifest, artifacts, document = candidate_document(Path(temporary))
                requested_profiles: tuple[str, ...] = ()
                sdist = artifacts / "eqiora-0.1.0a1.tar.gz"
                first_wheel = artifacts / document["artifacts"][1]["filename"]
                if case == "hook-only-sdist":
                    _write_sdist(
                        sdist,
                        extra_members={"bindings/python/src/lib.rs": b"_repr_mimebundle_"},
                    )
                elif case == "asset-only-wheel":
                    _rewrite_wheel(
                        first_wheel,
                        extra_members={
                            "eqiora/_presentation/static/mesh-view.css": b"x"
                        },
                    )
                elif case == "frontend-path-only-sdist":
                    _write_sdist(
                        sdist,
                        extra_members={"bindings/python/frontend/README": b"x"},
                    )
                elif case == "unmarked-anywidget-wheel":
                    _rewrite_wheel(
                        first_wheel,
                        metadata=notebook_metadata(
                            "AnyWidget==0.11.0", provides=()
                        ),
                    )
                elif case == "anywidget-only-sdist-pkg-info":
                    _write_sdist(
                        sdist,
                        pkg_info=notebook_metadata(
                            'anywidget==0.11.0; extra == "other"', provides=()
                        ),
                    )
                elif case == "anywidget-only-sdist-pyproject":
                    _write_sdist(
                        sdist,
                        pyproject=(
                            BASE_PYPROJECT
                            + b'\n[project.optional-dependencies]\nother=["anywidget"]\n'
                        ),
                    )
                elif case == "notebook-extra-only-sdist":
                    _write_sdist(
                        sdist,
                        pyproject=(
                            BASE_PYPROJECT
                            + b'\n[project.optional-dependencies]\nnotebook=["numpy"]\n'
                        ),
                    )
                elif case == "provides-extra-only-wheel":
                    _rewrite_wheel(
                        first_wheel,
                        metadata=notebook_metadata(provides=("notebook",)),
                    )
                elif case == "notebook-check-only":
                    document["checks"].append(min(NOTEBOOK_CHECKS))
                elif case == "frontend-schema-only":
                    document["build"]["frontend"] = {}
                elif case == "v3-format-only":
                    document["format"] = V3_FORMAT
                elif case == "requested-notebook-only":
                    requested_profiles = ("notebook",)
                _refresh_artifact_records(artifacts, document)
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaisesRegex(ManifestError, "v3"):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=requested_profiles,
                    )

    def test_complete_v3_family_and_candidate_bound_receipt_are_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, artifacts, _, receipt_path, _ = complete_v3_candidate_document(
                root
            )
            candidate = load_candidate_family(
                manifest,
                artifacts,
                requested_profiles=("notebook",),
                h2_receipt=receipt_path,
            )
            verify_artifacts(candidate, artifacts)

        self.assertEqual(candidate.version, "0.1.0a1")

    def test_every_wheel_requires_one_exact_notebook_requirement(self) -> None:
        metadata_mutants = {
            "missing": notebook_metadata(),
            "duplicate": notebook_metadata(
                'anywidget==0.11.0; extra == "notebook"',
                'anywidget==0.11.0; extra == "notebook"',
            ),
            "duplicate-extra": notebook_metadata(
                'anywidget==0.11.0; extra == "notebook"',
                provides=("notebook", "notebook"),
            ),
            "ranged": notebook_metadata(
                'anywidget>=0.11.0; extra == "notebook"'
            ),
            "wrong-marker": notebook_metadata(
                'anywidget==0.11.0; extra == "widget"'
            ),
            "interpreter-marker": notebook_metadata(
                'anywidget==0.11.0; extra == "notebook" and python_version >= "3.13"'
            ),
            "unmarked": notebook_metadata("anywidget==0.11.0"),
            "self-disabled": notebook_metadata(
                'anywidget==0.11.0; extra == "notebook" and python_version < "0"'
            ),
        }
        for name, metadata in metadata_mutants.items():
            with (
                self.subTest(name=name),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                wheel = artifacts / document["artifacts"][1]["filename"]
                _rewrite_wheel(wheel, metadata=metadata)
                _bind_receipt(
                    manifest, artifacts, document, receipt_path, receipt
                )

                with self.assertRaisesRegex(ManifestError, "anywidget|notebook"):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_family_scan_rejects_unsafe_archive_members_before_schema_choice(
        self,
    ) -> None:
        for kind in ("sdist", "wheel"):
            with (
                self.subTest(kind=kind),
                tempfile.TemporaryDirectory() as temporary,
            ):
                manifest, artifacts, document = candidate_document(Path(temporary))
                if kind == "sdist":
                    sdist = artifacts / document["artifacts"][0]["filename"]
                    with tarfile.open(sdist, mode="w:gz") as archive:
                        payload = b"unsafe"
                        member = tarfile.TarInfo("../escape")
                        member.size = len(payload)
                        archive.addfile(member, io.BytesIO(payload))
                else:
                    wheel = artifacts / document["artifacts"][1]["filename"]
                    _rewrite_wheel(wheel, extra_members={"../escape": b"unsafe"})
                _refresh_artifact_records(artifacts, document)
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaisesRegex(
                    ManifestError, "unsafe|travers|escape|relative"
                ):
                    load_candidate_family(manifest, artifacts)

    def test_v3_sdist_and_every_wheel_share_one_closed_asset_inventory(self) -> None:
        cases = (
            ("sdist-missing", "sdist", "missing"),
            ("sdist-empty", "sdist", "empty"),
            ("sdist-extra", "sdist", "extra"),
            ("sdist-modified-notice", "sdist", "notice"),
            ("wheel-missing", "wheel", "missing"),
            ("wheel-empty", "wheel", "empty"),
            ("wheel-extra", "wheel", "extra"),
            ("wheel-modified-notice", "wheel", "notice"),
        )
        for name, artifact_kind, mutation in cases:
            with (
                self.subTest(name=name),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                asset = "eqiora/_presentation/static/mesh-view.css"
                notice = (
                    "eqiora/_presentation/static/THIRD_PARTY_NOTICES.txt"
                )
                if artifact_kind == "wheel":
                    wheel = artifacts / document["artifacts"][1]["filename"]
                    if mutation == "missing":
                        with zipfile.ZipFile(wheel) as archive:
                            members = {
                                member: archive.read(member)
                                for member in archive.namelist()
                                if member != asset
                            }
                        with zipfile.ZipFile(wheel, mode="w") as archive:
                            for member, payload in sorted(members.items()):
                                archive.writestr(member, payload)
                    elif mutation == "empty":
                        _rewrite_wheel(wheel, extra_members={asset: b""})
                    elif mutation == "extra":
                        _rewrite_wheel(
                            wheel,
                            extra_members={
                                "eqiora/_presentation/static/unreviewed.js": b"x"
                            },
                        )
                    else:
                        _rewrite_wheel(
                            wheel, extra_members={notice: b"changed notice"}
                        )
                else:
                    sdist = artifacts / document["artifacts"][0]["filename"]
                    prefix = "eqiora-0.1.0a1/bindings/python/python/"
                    if mutation == "missing":
                        _rewrite_sdist(
                            sdist,
                            remove_members=(f"{prefix}{asset}",),
                        )
                    elif mutation == "empty":
                        _rewrite_sdist(
                            sdist,
                            replace_members={f"{prefix}{asset}": b""},
                        )
                    elif mutation == "extra":
                        _rewrite_sdist(
                            sdist,
                            replace_members={
                                f"{prefix}eqiora/_presentation/static/unreviewed.js": b"x"
                            },
                        )
                    else:
                        _rewrite_sdist(
                            sdist,
                            replace_members={
                                f"{prefix}{notice}": b"changed notice"
                            },
                        )
                _bind_receipt(
                    manifest, artifacts, document, receipt_path, receipt
                )

                with self.assertRaisesRegex(ManifestError, "asset|notice|inventory"):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_v3_source_package_lock_and_config_bytes_are_candidate_bound(self) -> None:
        members = (
            "bindings/python/frontend/package.json",
            "bindings/python/frontend/package-lock.json",
            "bindings/python/frontend/src/mesh-view.ts",
            "bindings/python/frontend/vite.config.ts",
        )
        for relative_path in members:
            with (
                self.subTest(relative_path=relative_path),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                sdist = artifacts / document["artifacts"][0]["filename"]
                _rewrite_sdist(
                    sdist,
                    replace_members={
                        f"eqiora-0.1.0a1/{relative_path}": b"changed retained input"
                    },
                )
                _bind_receipt(
                    manifest, artifacts, document, receipt_path, receipt
                )

                with self.assertRaisesRegex(
                    ManifestError, "source|package|config|lock|hash"
                ):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_v3_frontend_schema_is_closed_and_exactly_typed(self) -> None:
        mutations = {
            "extra-top-level-key": lambda frontend: frontend.__setitem__(
                "unreviewed", True
            ),
            "missing-key": lambda frontend: frontend.pop("package_json_sha256"),
            "boolean-size": lambda frontend: frontend["assets"][
                "eqiora/_presentation/static/mesh-view.mjs"
            ].__setitem__("size", True),
            "zero-size": lambda frontend: frontend["assets"][
                "eqiora/_presentation/static/mesh-view.mjs"
            ].__setitem__("size", 0),
            "uppercase-hash": lambda frontend: frontend.__setitem__(
                "source_inventory_sha256", "A" * 64
            ),
            "wrong-node": lambda frontend: frontend.__setitem__(
                "node", "v24.18.0"
            ),
            "wrong-npm-integrity": lambda frontend: frontend.__setitem__(
                "npm_package_integrity", "sha512-unreviewed"
            ),
            "extra-asset": lambda frontend: frontend["assets"].__setitem__(
                "eqiora/_presentation/static/extra.js",
                {"size": 1, "sha256": "0" * 64},
            ),
            "asset-extra-key": lambda frontend: frontend["assets"][
                "eqiora/_presentation/static/mesh-view.css"
            ].__setitem__("mode", 0o644),
            "license-expression": lambda frontend: frontend["licenses"][
                "three@0.185.1"
            ].__setitem__("expression", "Apache-2.0"),
            "license-hash": lambda frontend: frontend["licenses"][
                "anywidget@0.11.0"
            ].__setitem__("source_license_sha256", "0" * 64),
            "runtime-version": lambda frontend: frontend["runtime"].__setitem__(
                "marimo", "0.23.15"
            ),
            "browser-revision": lambda frontend: frontend["browser"].__setitem__(
                "chromium_revision", "1235"
            ),
            "empty-platform": lambda frontend: frontend["browser"].__setitem__(
                "platform", ""
            ),
            "browser-extra-key": lambda frontend: frontend["browser"].__setitem__(
                "channel", "stable"
            ),
        }
        for name, mutate in mutations.items():
            with (
                self.subTest(name=name),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, _ = (
                    complete_v3_candidate_document(root)
                )
                mutate(document["build"]["frontend"])
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaises(ManifestError):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_v3_notebook_checks_are_closed_without_weakening_old_profiles(self) -> None:
        for mutation in ("missing", "misspelled", "extra"):
            with (
                self.subTest(mutation=mutation),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, _ = (
                    complete_v3_candidate_document(root)
                )
                check = min(NOTEBOOK_CHECKS)
                document["checks"].remove(check)
                if mutation == "misspelled":
                    document["checks"].append(f"{check}-typo")
                elif mutation == "extra":
                    document["checks"].extend(
                        (check, "cp313:notebook-unreviewed")
                    )
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaisesRegex(ManifestError, "notebook|Notebook"):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_receipt_bytes_are_canonical_and_detached(self) -> None:
        encodings = {
            "pretty": lambda receipt: json.dumps(
                receipt, indent=2, sort_keys=True
            ).encode("utf-8"),
            "trailing-newline": lambda receipt: canonical_json_bytes(receipt) + b"\n",
            "bom": lambda receipt: b"\xef\xbb\xbf" + canonical_json_bytes(receipt),
            "non-finite": lambda receipt: canonical_json_bytes(receipt).replace(
                str(receipt["environment"]["source_date_epoch"]).encode("ascii"),
                b"NaN",
                1,
            ),
        }
        for name, encode in encodings.items():
            with (
                self.subTest(name=name),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                noncanonical = encode(receipt)
                receipt_path.write_bytes(noncanonical)
                document["build"]["frontend"]["h2_receipt_sha256"] = (
                    hashlib.sha256(noncanonical).hexdigest()
                )
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaisesRegex(ManifestError, "canonical"):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, artifacts, document, receipt_path, _ = (
                complete_v3_candidate_document(root)
            )
            document["build"]["frontend"]["h2_receipt_sha256"] = "0" * 64
            manifest.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(ManifestError, "receipt|SHA-256"):
                load_candidate_family(
                    manifest,
                    artifacts,
                    requested_profiles=("notebook",),
                    h2_receipt=receipt_path,
                )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, artifacts, _, receipt_path, _ = complete_v3_candidate_document(
                root
            )
            renamed = receipt_path.with_name("copied-h2.json")
            receipt_path.rename(renamed)
            with self.assertRaisesRegex(ManifestError, "filename|receipt"):
                load_candidate_family(
                    manifest,
                    artifacts,
                    requested_profiles=("notebook",),
                    h2_receipt=renamed,
                )

    def test_every_structured_inventory_uses_the_frozen_canonical_preimage(self) -> None:
        names = (
            "source_inventory_sha256",
            "config_inventory_sha256",
            "locked_packages_sha256",
            "install_script_inventory_sha256",
            "bundler_module_graph_sha256",
        )
        for name in names:
            with (
                self.subTest(name=name),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, _ = (
                    complete_v3_candidate_document(root)
                )
                document["build"]["frontend"][name] = "0" * 64
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaisesRegex(ManifestError, "inventory|preimage|hash"):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, artifacts, document, receipt_path, _ = (
                complete_v3_candidate_document(root)
            )
            document["build"]["frontend"]["runtime"][
                "resolved_environment_sha256"
            ] = "0" * 64
            manifest.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(ManifestError, "environment|preimage|hash"):
                load_candidate_family(
                    manifest,
                    artifacts,
                    requested_profiles=("notebook",),
                    h2_receipt=receipt_path,
                )

    def test_receipt_schema_records_paths_order_and_duplicates_are_closed(self) -> None:
        def extra_key(receipt: dict) -> None:
            receipt["probe"]["extra"] = True

        def alias_extra_key(receipt: dict) -> None:
            receipt["inputs"]["locked_packages"][0]["extra"] = True

        def traversal(receipt: dict) -> None:
            receipt["inputs"]["source_root_inventory"][0][
                "relative_path"
            ] = "../escape"

        def unsorted(receipt: dict) -> None:
            receipt["inputs"]["direct_pins"].reverse()

        def unsorted_environment(receipt: dict) -> None:
            receipt["environment"]["environment_allowlist"].reverse()

        def unsorted_output(receipt: dict) -> None:
            receipt["clean_run_1"]["output_inventory"].reverse()

        def duplicate(receipt: dict) -> None:
            receipt["inputs"]["direct_pins"].append(
                copy.deepcopy(receipt["inputs"]["direct_pins"][0])
            )

        def unsafe_basename(receipt: dict) -> None:
            receipt["candidate"]["artifacts"][0]["filename"] = "dir/file.whl"

        mutations = {
            "object-extra": extra_key,
            "alias-extra": alias_extra_key,
            "traversal": traversal,
            "unsorted": unsorted,
            "unsorted-environment": unsorted_environment,
            "unsorted-output": unsorted_output,
            "duplicate": duplicate,
            "unsafe-basename": unsafe_basename,
        }
        for name, mutate in mutations.items():
            with (
                self.subTest(name=name),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                mutate(receipt)
                receipt_bytes = canonical_json_bytes(receipt)
                receipt_path.write_bytes(receipt_bytes)
                document["build"]["frontend"]["h2_receipt_sha256"] = (
                    hashlib.sha256(receipt_bytes).hexdigest()
                )
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaises(ManifestError):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_every_receipt_object_and_record_alias_rejects_extra_keys(self) -> None:
        object_paths = (
            (),
            ("probe",),
            ("candidate",),
            ("candidate", "artifacts", 0),
            ("environment",),
            ("inputs",),
            ("inputs", "source_root_inventory", 0),
            ("inputs", "config_inventory", 0),
            ("inputs", "direct_pins", 0),
            ("inputs", "locked_packages", 0),
            ("build",),
            ("build", "bundler_module_graph", 0),
            ("clean_run_1",),
            ("clean_run_1", "output_inventory", 0),
            ("clean_run_2",),
            ("comparison",),
            ("licenses",),
            ("licenses", "components", 0),
            ("browser",),
            ("python_host",),
            ("python_host", "wheels", 0),
        )

        def locate(value: object, path: tuple[object, ...]) -> object:
            for key in path:
                value = value[key]  # type: ignore[index]
            return value

        for path in object_paths:
            with (
                self.subTest(path=path),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                target = locate(receipt, path)
                target["unreviewed"] = True  # type: ignore[index]
                receipt_bytes = canonical_json_bytes(receipt)
                receipt_path.write_bytes(receipt_bytes)
                document["build"]["frontend"]["h2_receipt_sha256"] = (
                    hashlib.sha256(receipt_bytes).hexdigest()
                )
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaisesRegex(ManifestError, "key|schema|member"):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_receipt_required_keys_and_json_types_are_exact(self) -> None:
        missing_keys = (
            (("probe",), "contract_sha256"),
            (("candidate",), "artifacts"),
            (("environment",), "source_date_epoch"),
            (("inputs",), "locked_packages"),
            (("build",), "bundler_module_graph"),
            (("clean_run_1",), "output_inventory"),
            (("comparison",), "diff"),
            (("licenses",), "components"),
            (("browser",), "revision"),
            (("python_host",), "wheels"),
        )
        wrong_types = (
            (("candidate", "artifacts", 0), "size", True),
            (("environment",), "source_date_epoch", True),
            (("inputs",), "lockfile_version", True),
            (("inputs", "source_root_inventory", 0), "mode", True),
            (("inputs", "locked_packages", 0), "selected_optional", 0),
            (("clean_run_1",), "npm_ci_exit", False),
            (
                ("clean_run_1",),
                "external_request_count_after_npm_ci",
                False,
            ),
            (("comparison",), "modes_equal", 1),
            (("python_host",), "python", 3.13),
        )

        def locate(value: object, path: tuple[object, ...]) -> object:
            for key in path:
                value = value[key]  # type: ignore[index]
            return value

        for path, key in missing_keys:
            with (
                self.subTest(missing=(path, key)),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                locate(receipt, path).pop(key)  # type: ignore[union-attr]
                receipt_bytes = canonical_json_bytes(receipt)
                receipt_path.write_bytes(receipt_bytes)
                document["build"]["frontend"]["h2_receipt_sha256"] = (
                    hashlib.sha256(receipt_bytes).hexdigest()
                )
                manifest.write_text(json.dumps(document), encoding="utf-8")
                with self.assertRaises(ManifestError):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

        for path, key, value in wrong_types:
            with (
                self.subTest(wrong_type=(path, key)),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                locate(receipt, path)[key] = value  # type: ignore[index]
                receipt_bytes = canonical_json_bytes(receipt)
                receipt_path.write_bytes(receipt_bytes)
                document["build"]["frontend"]["h2_receipt_sha256"] = (
                    hashlib.sha256(receipt_bytes).hexdigest()
                )
                manifest.write_text(json.dumps(document), encoding="utf-8")
                with self.assertRaises(ManifestError):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_receipt_exact_identity_and_candidate_binding_reject_drift(self) -> None:
        mutations = (
            (("probe",), "contract_sha256", "0" * 64),
            (("probe",), "protected_base_sha", "0" * 40),
            (("probe",), "verdict", "FAIL"),
            (("candidate",), "project", "other"),
            (("candidate",), "source_commit", "2" * 40),
            (("candidate", "artifacts", 0), "sha256", "0" * 64),
            (("environment",), "node_version", "v24.18.0"),
            (("environment",), "node_executable_sha256", "0" * 64),
            (("environment",), "npm_version", "11.15.0"),
            (("environment",), "npm_package_integrity", "sha512-wrong"),
            (("environment",), "locale", "en_GB.UTF-8"),
            (("environment",), "timezone", "Europe/London"),
            (("build",), "npm_ci_command_argv", ["npm", "ci"]),
            (("build",), "exact_command_argv", ["npm", "run", "bundle"]),
            (("build",), "network_policy", "online"),
            (("build",), "bundler_version", "8.1.0"),
            (("browser",), "browsers_json_sha256", "0" * 64),
            (("browser",), "browser_name", "chrome"),
            (("browser",), "revision", "1235"),
            (("browser",), "browser_version", "151.0.7922.35"),
            (("browser",), "platform", ""),
            (("python_host",), "python", "3.12"),
        )

        def locate(value: object, path: tuple[object, ...]) -> object:
            for key in path:
                value = value[key]  # type: ignore[index]
            return value

        for path, key, value in mutations:
            with (
                self.subTest(path=path, key=key),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                locate(receipt, path)[key] = value  # type: ignore[index]
                receipt_bytes = canonical_json_bytes(receipt)
                receipt_path.write_bytes(receipt_bytes)
                document["build"]["frontend"]["h2_receipt_sha256"] = (
                    hashlib.sha256(receipt_bytes).hexdigest()
                )
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaises(ManifestError):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_lock_integrity_scripts_and_pass_predicates_fail_closed(self) -> None:
        def nonregistry(receipt: dict) -> None:
            receipt["inputs"]["locked_packages"][0]["resolved"] = "file:../local"

        def invalid_integrity(receipt: dict) -> None:
            receipt["inputs"]["locked_packages"][0]["integrity"] = "sha256-bad"

        def changed_script_inventory(receipt: dict) -> None:
            receipt["inputs"]["locked_packages"][0]["lifecycle_scripts"] = [
                {"name": "postinstall", "command": "unreviewed"}
            ]

        def unequal_bytes(receipt: dict) -> None:
            receipt["comparison"]["sha256_bytes_equal"] = False

        def diff(receipt: dict) -> None:
            receipt["comparison"]["diff"] = ["mesh-view.mjs"]

        def source_map(receipt: dict) -> None:
            receipt["clean_run_2"]["source_maps"] = ["mesh-view.mjs.map"]

        def emitted_import(receipt: dict) -> None:
            receipt["clean_run_1"]["emitted_imports"] = ["https://cdn.invalid/x.js"]

        def post_install_network(receipt: dict) -> None:
            receipt["clean_run_1"]["external_request_count_after_npm_ci"] = 1

        def external(receipt: dict) -> None:
            receipt["build"]["externals"] = ["three"]

        def unmapped_license(receipt: dict) -> None:
            receipt["licenses"]["unmapped_emitted_modules"] = ["three"]

        mutations = {
            "nonregistry": nonregistry,
            "invalid-integrity": invalid_integrity,
            "script-preimage": changed_script_inventory,
            "unequal-bytes": unequal_bytes,
            "nonempty-diff": diff,
            "source-map": source_map,
            "emitted-import": emitted_import,
            "post-install-network": post_install_network,
            "external": external,
            "unmapped-license": unmapped_license,
        }
        for name, mutate in mutations.items():
            with (
                self.subTest(name=name),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                mutate(receipt)
                receipt_bytes = canonical_json_bytes(receipt)
                receipt_path.write_bytes(receipt_bytes)
                document["build"]["frontend"]["h2_receipt_sha256"] = (
                    hashlib.sha256(receipt_bytes).hexdigest()
                )
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaises(ManifestError):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

    def test_assets_notices_licenses_browser_and_python_host_are_candidate_bound(
        self,
    ) -> None:
        mutations = (
            ("notice-hash", ("licenses", "notice_sha256"), "0" * 64),
            (
                "three-license",
                ("licenses", "components", 0, "source_license_sha256"),
                "0" * 64,
            ),
            ("browser-revision", ("browser", "revision"), "1235"),
            ("browser-version", ("browser", "browser_version"), "0"),
            ("python", ("python_host", "python"), "3.12"),
            (
                "anywidget-wheel",
                ("python_host", "wheels", 0, "sha256"),
                "0" * 64,
            ),
        )

        def assign(root: object, path: tuple[object, ...], value: object) -> None:
            target = root
            for key in path[:-1]:
                target = target[key]  # type: ignore[index]
            target[path[-1]] = value  # type: ignore[index]

        for name, path, value in mutations:
            with (
                self.subTest(name=name),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                manifest, artifacts, document, receipt_path, receipt = (
                    complete_v3_candidate_document(root)
                )
                assign(receipt, path, value)
                receipt_bytes = canonical_json_bytes(receipt)
                receipt_path.write_bytes(receipt_bytes)
                document["build"]["frontend"]["h2_receipt_sha256"] = (
                    hashlib.sha256(receipt_bytes).hexdigest()
                )
                manifest.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaises(ManifestError):
                    load_candidate_family(
                        manifest,
                        artifacts,
                        requested_profiles=("notebook",),
                        h2_receipt=receipt_path,
                    )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, artifacts, document, receipt_path, receipt = (
                complete_v3_candidate_document(root)
            )
            wheel = artifacts / document["artifacts"][1]["filename"]
            _rewrite_wheel(
                wheel,
                extra_members={
                    "eqiora/_presentation/static/mesh-view.mjs": b"changed"
                },
            )
            _bind_receipt(manifest, artifacts, document, receipt_path, receipt)
            with self.assertRaisesRegex(ManifestError, "asset|byte|hash"):
                load_candidate_family(
                    manifest,
                    artifacts,
                    requested_profiles=("notebook",),
                    h2_receipt=receipt_path,
                )

    def test_cross_candidate_canonical_receipt_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, artifacts, document, receipt_path, receipt = (
                complete_v3_candidate_document(root)
            )
            receipt["candidate"]["version"] = "0.1.0a2"
            receipt_bytes = canonical_json_bytes(receipt)
            receipt_path.write_bytes(receipt_bytes)
            document["build"]["frontend"]["h2_receipt_sha256"] = hashlib.sha256(
                receipt_bytes
            ).hexdigest()
            manifest.write_text(json.dumps(document), encoding="utf-8")

            with self.assertRaisesRegex(ManifestError, "candidate|version"):
                load_candidate_family(
                    manifest,
                    artifacts,
                    requested_profiles=("notebook",),
                    h2_receipt=receipt_path,
                )

    def test_cross_family_and_cross_source_canonical_receipts_are_rejected(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "a").mkdir()
            (root / "b").mkdir()
            manifest_a, _, _, receipt_a, _ = complete_v3_candidate_document(root / "a")
            receipt_bytes = receipt_a.read_bytes()
            self.assertTrue(manifest_a.is_file())

            manifest_b, artifacts_b, document_b, receipt_b, _ = (
                complete_v3_candidate_document(root / "b")
            )
            wheel_b = artifacts_b / document_b["artifacts"][1]["filename"]
            _rewrite_wheel(
                wheel_b,
                extra_members={"eqiora/cross-family-mutant.txt": b"different family"},
            )
            _refresh_artifact_records(artifacts_b, document_b)
            receipt_b.write_bytes(receipt_bytes)
            document_b["build"]["frontend"]["h2_receipt_sha256"] = hashlib.sha256(
                receipt_bytes
            ).hexdigest()
            manifest_b.write_text(json.dumps(document_b), encoding="utf-8")

            with self.assertRaisesRegex(ManifestError, "artifact|candidate|receipt"):
                load_candidate_family(
                    manifest_b,
                    artifacts_b,
                    requested_profiles=("notebook",),
                    h2_receipt=receipt_b,
                )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, artifacts, document, receipt_path, receipt = (
                complete_v3_candidate_document(root)
            )
            other_revision = "2" * 40
            receipt["probe"]["writer_revision"] = other_revision
            receipt["candidate"]["source_commit"] = other_revision
            receipt_bytes = canonical_json_bytes(receipt)
            receipt_path.write_bytes(receipt_bytes)
            document["build"]["frontend"]["h2_receipt_sha256"] = hashlib.sha256(
                receipt_bytes
            ).hexdigest()
            manifest.write_text(json.dumps(document), encoding="utf-8")

            with self.assertRaisesRegex(ManifestError, "commit|revision|source"):
                load_candidate_family(
                    manifest,
                    artifacts,
                    requested_profiles=("notebook",),
                    h2_receipt=receipt_path,
                )


if __name__ == "__main__":
    unittest.main()
