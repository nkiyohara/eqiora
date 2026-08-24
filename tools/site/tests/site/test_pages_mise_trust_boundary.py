from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[4]
SCRATCH_ROOT = Path.home() / ".cache/eqiora/site-oracle-tests"

CANDIDATE = "bbe44db3719ce950be5eca4c0d4b6922f2ff0390"
CANDIDATE_TREE = "6134f3601a65b2b145e869e11405637b3191b148"
WORKFLOW_BLOB = "af67433475661735d6e89ece4a7edada58fa070a"
WORKFLOW_SHA256 = "65a1239023ffcf9a02a0dd8d8c5334af41c9727ded4f0b8dfc435b694348969f"
MISE_TOML_BLOB = "ef2d22fc5b05e0764fb0ea7e2b564c5db02bf976"
MISE_TOML_SHA256 = "4b6cd65d27ffa70546d38d7b5f94346e58afb5ddbde7ca30141e7d87b7eeea15"
MISE_LOCK_BLOB = "a880c5639ce8c8947f3fc8ddef24ec721338074a"
MISE_LOCK_SHA256 = "ce87f3f1c58906439309a44e13e3b0d6f0fbec875f4c1dc61b8ed253e384e4a0"
RUNNER_BLOB = "f8c66e061e275522e9d9bc330442de053debfbbd"
RUNNER_SHA256 = "e7081ebcb6044a78834341367ecfcfb995924a25d2d528903da40fcde7a298ed"

STEP_NAME = "      - name: Build and verify with only loopback networking\n"
ENV_LINE = "                -- env \\\n"
CHDIR_LINE = '                  --chdir="$EQIORA_SITE_SOURCE_ROOT" \\\n'
TRUST_LINE = (
    "                  MISE_TRUSTED_CONFIG_PATHS="
    '"$EQIORA_SITE_SOURCE_ROOT/mise.toml" \\\n'
)
HOME_LINE = '                  HOME="$EQIORA_RUNNER_HOME" \\\n'
RUNNER_LINE = (
    '                  "$EQIORA_SITE_SOURCE_ROOT/tools/site/'
    'run_offline_site_checks.sh"\n'
)
HISTORY_FETCH_LINES = (
    "          git fetch --no-tags --no-recurse-submodules --no-write-fetch-head \\\n"
    "            origin refs/pull/501/head\n"
)
NODE_MODULES_MOVE_LINE = (
    '          mv docs/site/node_modules '
    '"$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules"\n'
)
FROZEN_SUCCESSOR_ERROR = (
    "workflow is not the frozen mise trust and history fetch successor"
)
UNTRUSTED = "not trusted"
MISE_VERSION = "2026.5.10 linux-x64 (2026-05-16)"


def _git_environment() -> dict[str, str]:
    return {
        "LC_ALL": "C",
        "LANG": "C",
        "TZ": "UTC",
        "PATH": os.defpath,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_NO_REPLACE_OBJECTS": "1",
    }


def _git_object_repository() -> Path:
    value = os.environ.get("EQIORA_SITE_GIT_OBJECT_REPOSITORY")
    root = Path(value) if value else REPOSITORY
    root = root.resolve()
    if not root.is_dir():
        raise AssertionError("the Git object repository is unavailable")
    return root


def _git(*arguments: str, output_limit: int = 65_536) -> bytes:
    executable = shutil.which("git", path=os.defpath)
    if executable is None:
        raise AssertionError("git is unavailable from the system path")
    result = subprocess.run(
        [executable, "-C", str(_git_object_repository()), *arguments],
        check=False,
        env=_git_environment(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=15,
    )
    if result.returncode != 0 or result.stderr or len(result.stdout) > output_limit:
        raise AssertionError(
            f"bounded Git identity query failed: {arguments!r}; "
            f"status={result.returncode}; stderr={result.stderr!r}"
        )
    return result.stdout


def _blob_id(payload: bytes) -> str:
    header = f"blob {len(payload)}\0".encode("ascii")
    return hashlib.sha1(header + payload, usedforsecurity=False).hexdigest()


def _replace_once(text: str, old: str, new: str) -> str:
    if text.count(old) != 1:
        raise AssertionError(f"mutation target is not unique: {old!r}")
    return text.replace(old, new, 1)


def _step(text: str) -> str:
    if text.count(STEP_NAME) != 1:
        raise AssertionError("the isolated Pages build step is not unique")
    start = text.index(STEP_NAME)
    end = text.find("      - name:", start + len(STEP_NAME))
    if end < 0:
        raise AssertionError("the isolated Pages build step has no boundary")
    return text[start:end]


def _workflow_errors(text: str, known_good: str) -> list[str]:
    if text == known_good:
        return []
    return [FROZEN_SUCCESSOR_ERROR]


def _write_regular(path: Path, payload: bytes) -> None:
    path.write_bytes(payload)
    path.chmod(0o644)
    details = path.stat()
    if not stat.S_ISREG(details.st_mode) or stat.S_IMODE(details.st_mode) != 0o644:
        raise AssertionError(f"oracle input is not a regular mode-0644 file: {path}")


class PagesMiseTrustBoundaryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not sys.platform.startswith("linux"):
            raise AssertionError("the Pages mise trust oracle is Linux-only")
        SCRATCH_ROOT.mkdir(parents=True, exist_ok=True)
        if SCRATCH_ROOT.is_symlink() or not SCRATCH_ROOT.is_dir():
            raise AssertionError("the home-backed oracle scratch root is unsafe")

        parent_home = os.environ.get("HOME")
        if not parent_home:
            raise AssertionError("the captured parent HOME is required")
        cls.parent_home = parent_home

        mise = shutil.which("mise", path=os.environ.get("PATH", os.defpath))
        if mise is None:
            raise AssertionError("mise is unavailable")
        cls.mise = Path(mise).resolve()
        version = subprocess.run(
            [str(cls.mise), "--version"],
            check=False,
            env={**os.environ, "LC_ALL": "C", "TZ": "UTC"},
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=10,
        )
        if version.returncode != 0 or version.stdout.strip() != MISE_VERSION:
            raise AssertionError(
                f"expected exact mise {MISE_VERSION!r}, got "
                f"status={version.returncode}, stdout={version.stdout!r}, "
                f"stderr={version.stderr!r}"
            )

        env_executable = shutil.which("env", path=os.defpath)
        if env_executable is None:
            raise AssertionError("GNU env is unavailable")
        cls.env_executable = Path(env_executable).resolve()
        env_version = subprocess.run(
            [str(cls.env_executable), "--version"],
            check=False,
            env={"LC_ALL": "C", "PATH": os.defpath},
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=10,
        )
        if (
            env_version.returncode != 0
            or "env (GNU coreutils)" not in env_version.stdout.splitlines()[0]
        ):
            raise AssertionError("the oracle requires GNU coreutils env")

        identities = {
            f"{CANDIDATE}^{{tree}}": CANDIDATE_TREE,
            f"{CANDIDATE}:.github/workflows/pages.yml": WORKFLOW_BLOB,
            f"{CANDIDATE}:mise.toml": MISE_TOML_BLOB,
            f"{CANDIDATE}:mise.lock": MISE_LOCK_BLOB,
            f"{CANDIDATE}:tools/site/run_offline_site_checks.sh": RUNNER_BLOB,
        }
        _git("cat-file", "-e", f"{CANDIDATE}^{{commit}}")
        for expression, expected in identities.items():
            actual = _git("rev-parse", expression).decode("ascii").strip()
            if actual != expected:
                raise AssertionError(
                    f"frozen candidate identity changed for {expression}: {actual}"
                )

        cls.candidate_workflow_bytes = _git("cat-file", "blob", WORKFLOW_BLOB)
        cls.mise_toml_bytes = _git("cat-file", "blob", MISE_TOML_BLOB)
        cls.mise_lock_bytes = _git("cat-file", "blob", MISE_LOCK_BLOB)
        cls.runner_bytes = _git("cat-file", "blob", RUNNER_BLOB)
        payloads = (
            (cls.candidate_workflow_bytes, WORKFLOW_BLOB, WORKFLOW_SHA256),
            (cls.mise_toml_bytes, MISE_TOML_BLOB, MISE_TOML_SHA256),
            (cls.mise_lock_bytes, MISE_LOCK_BLOB, MISE_LOCK_SHA256),
            (cls.runner_bytes, RUNNER_BLOB, RUNNER_SHA256),
        )
        for payload, expected_blob, expected_sha256 in payloads:
            if (
                _blob_id(payload) != expected_blob
                or hashlib.sha256(payload).hexdigest() != expected_sha256
            ):
                raise AssertionError("a frozen Pages input payload changed")

        cls.candidate_workflow = cls.candidate_workflow_bytes.decode("utf-8")
        target = ENV_LINE + HOME_LINE
        cls.mise_trust_successor_workflow = _replace_once(
            cls.candidate_workflow,
            target,
            ENV_LINE + CHDIR_LINE + TRUST_LINE + HOME_LINE,
        )
        cls.known_good_workflow = _replace_once(
            cls.mise_trust_successor_workflow,
            NODE_MODULES_MOVE_LINE,
            HISTORY_FETCH_LINES + NODE_MODULES_MOVE_LINE,
        )
        if cls.candidate_workflow.count(CHDIR_LINE) != 0:
            raise AssertionError("the frozen candidate unexpectedly contains env chdir")
        if cls.candidate_workflow.count(TRUST_LINE) != 0:
            raise AssertionError(
                "the frozen candidate unexpectedly contains exact-file trust"
            )
        if cls.mise_trust_successor_workflow.count(HISTORY_FETCH_LINES) != 0:
            raise AssertionError(
                "the frozen mise trust successor unexpectedly contains history fetch"
            )

        cls.temporary = tempfile.TemporaryDirectory(dir=SCRATCH_ROOT)
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.root = Path(cls.temporary.name).resolve()
        cls.source = (cls.root / "source").resolve()
        cls.checkout = (cls.root / "checkout").resolve()
        cls.source.mkdir()
        cls.checkout.mkdir()
        if cls.source == cls.checkout:
            raise AssertionError("source and checkout identities must be distinct")
        for root in (cls.source, cls.checkout):
            _write_regular(root / "mise.toml", cls.mise_toml_bytes)
            _write_regular(root / "mise.lock", cls.mise_lock_bytes)
        for name in ("config", "state", "cache", "data"):
            (cls.root / name).mkdir()

        cls.probe = cls.root / "probe.py"
        cls.probe.write_text(
            textwrap.dedent(
                """\
                import json
                import os
                import subprocess
                import sys
                from pathlib import Path

                def run_mise(*arguments):
                    result = subprocess.run(
                        [os.environ["ORACLE_MISE"], *arguments],
                        check=False,
                        env=os.environ,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                        text=True,
                        timeout=10,
                    )
                    if result.returncode != 0:
                        sys.stdout.write(result.stdout)
                        sys.stderr.write(result.stderr)
                        raise SystemExit(result.returncode or 1)
                    return result.stdout

                node = run_mise("current", "node").strip()
                configs_text = run_mise("config", "ls", "--json")
                configs = json.loads(configs_text)
                report = {
                    "configs": configs,
                    "cwd": str(Path.cwd().resolve()),
                    "home": os.environ.get("HOME"),
                    "node": node,
                }
                payload = json.dumps(report, sort_keys=True)
                Path(os.environ["ORACLE_SENTINEL"]).write_text(
                    payload + "\\n", encoding="utf-8"
                )
                print(payload)
                """
            ),
            encoding="utf-8",
        )
        cls.probe.chmod(0o600)

    @classmethod
    def _clean_environment(cls) -> dict[str, str]:
        environment = os.environ.copy()
        for name in tuple(environment):
            if name.startswith("MISE_") or name in {"CI", "GITHUB_ACTIONS"}:
                environment.pop(name, None)
        environment.update({"LC_ALL": "C", "LANG": "C", "TZ": "UTC"})
        return environment

    def _invoke(
        self,
        *,
        chdir: Path | None,
        trust: str | None,
        pwd: Path | None = None,
        late_chdir: Path | None = None,
        changed_home: str | None = None,
    ) -> tuple[subprocess.CompletedProcess[str], Path]:
        sentinel = self.root / "sentinel.json"
        sentinel.unlink(missing_ok=True)
        command = [str(self.env_executable)]
        if chdir is not None:
            command.append(f"--chdir={chdir}")
        assignments = {
            "HOME": self.parent_home if changed_home is None else changed_home,
            "PATH": os.defpath,
            "LC_ALL": "C",
            "LANG": "C",
            "TZ": "UTC",
            "MISE_CONFIG_DIR": str(self.root / "config"),
            "MISE_STATE_DIR": str(self.root / "state"),
            "MISE_CACHE_DIR": str(self.root / "cache"),
            "MISE_DATA_DIR": str(self.root / "data"),
            "MISE_OFFLINE": "1",
            "ORACLE_MISE": str(self.mise),
            "ORACLE_SENTINEL": str(sentinel),
        }
        if trust is not None:
            assignments["MISE_TRUSTED_CONFIG_PATHS"] = trust
        if pwd is not None:
            assignments["PWD"] = str(pwd)
        command.extend(f"{name}={value}" for name, value in assignments.items())
        if late_chdir is not None:
            command.append(f"--chdir={late_chdir}")
        command.extend([str(Path(sys.executable).resolve()), str(self.probe)])
        result = subprocess.run(
            command,
            check=False,
            cwd=self.checkout,
            env=self._clean_environment(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=15,
        )
        return result, sentinel

    def _assert_success(
        self, result: subprocess.CompletedProcess[str], sentinel: Path
    ) -> dict[str, object]:
        self.assertEqual(
            result.returncode,
            0,
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )
        self.assertTrue(sentinel.is_file())
        report = json.loads(sentinel.read_text(encoding="utf-8"))
        self.assertEqual(report["cwd"], str(self.source))
        self.assertEqual(report["home"], self.parent_home)
        self.assertEqual(report["node"], "24.18.1")
        config_identity = json.dumps(report["configs"], sort_keys=True)
        self.assertIn(str(self.source / "mise.toml"), config_identity)
        self.assertIn("node", config_identity)
        return report

    def _assert_untrusted(
        self,
        result: subprocess.CompletedProcess[str],
        sentinel: Path,
        expected_config: Path,
    ) -> None:
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(sentinel.exists())
        diagnostic = result.stdout + result.stderr
        self.assertIn(UNTRUSTED, diagnostic)
        self.assertIn(str(expected_config), diagnostic)

    def _assert_known_good_structure(self, text: str) -> None:
        self.assertEqual(_workflow_errors(text, self.known_good_workflow), [])
        step = _step(text)
        self.assertEqual(text.count(CHDIR_LINE), 1)
        self.assertEqual(text.count(TRUST_LINE), 1)
        self.assertEqual(step.count(ENV_LINE), 1)
        self.assertEqual(step.count(HOME_LINE), 1)
        self.assertEqual(step.count(RUNNER_LINE), 1)
        self.assertEqual(text.count(HISTORY_FETCH_LINES), 1)
        self.assertIn(HISTORY_FETCH_LINES + NODE_MODULES_MOVE_LINE, text)
        self.assertIn(ENV_LINE + CHDIR_LINE + TRUST_LINE + HOME_LINE, step)
        self.assertIn(
            '                  EQIORA_SITE_SOURCE_ROOT="$EQIORA_SITE_SOURCE_ROOT" \\\n',
            step,
        )
        self.assertIn(
            "                  EQIORA_SITE_GIT_OBJECT_REPOSITORY="
            '"$EQIORA_SITE_GIT_OBJECT_REPOSITORY" \\\n',
            step,
        )
        self.assertNotIn("GITHUB_WORKSPACE", step)
        self.assertNotIn("MISE_TRUSTED_CONFIG_PATHS", step.split(ENV_LINE, 1)[0])
        self.assertNotIn("MISE_TRUSTED_CONFIG_PATHS", text.replace(TRUST_LINE, ""))
        self.assertNotIn("mise trust", text)
        self.assertNotIn("MISE_YES", text)
        self.assertIn("              ip link set lo up\n", step)
        self.assertIn("              exec setpriv \\\n", step)
        self.assertIn('                --reuid "$EQIORA_RUNNER_UID" \\\n', step)
        self.assertIn('                --regid "$EQIORA_RUNNER_GID" \\\n', step)
        self.assertIn("                --clear-groups \\\n", step)
        self.assertEqual(
            text.replace(HISTORY_FETCH_LINES, "", 1),
            self.mise_trust_successor_workflow,
        )
        self.assertEqual(
            text.replace(HISTORY_FETCH_LINES, "", 1).replace(
                CHDIR_LINE + TRUST_LINE, "", 1
            ),
            self.candidate_workflow,
        )

    def test_00_synthetic_exact_file_positive_precedes_rejections(self) -> None:
        result, sentinel = self._invoke(
            chdir=self.source,
            trust=str(self.source / "mise.toml"),
        )
        self._assert_success(result, sentinel)

    def test_01_frozen_successor_is_structural_and_executable(self) -> None:
        self._assert_known_good_structure(self.known_good_workflow)
        result, sentinel = self._invoke(
            chdir=self.source,
            trust=str(self.source / "mise.toml"),
        )
        self._assert_success(result, sentinel)

    def test_02_causal_negatives_reach_the_intended_trust_boundary(self) -> None:
        source_config = self.source / "mise.toml"
        checkout_config = self.checkout / "mise.toml"

        result, sentinel = self._invoke(chdir=self.source, trust=None)
        self._assert_untrusted(result, sentinel, source_config)

        result, sentinel = self._invoke(chdir=None, trust=str(source_config))
        self._assert_untrusted(result, sentinel, checkout_config)

        result, sentinel = self._invoke(
            chdir=None,
            trust=str(source_config),
            late_chdir=self.source,
        )
        self._assert_untrusted(result, sentinel, checkout_config)

        result, sentinel = self._invoke(
            chdir=None,
            trust=str(source_config),
            pwd=self.source,
        )
        self._assert_untrusted(result, sentinel, checkout_config)

        for name, trusted in (
            ("lock-only", self.source / "mise.lock"),
            ("checkout-only", checkout_config),
            ("glob", Path(f"{self.source}/mise*.toml")),
        ):
            with self.subTest(mutant=name):
                result, sentinel = self._invoke(
                    chdir=self.source,
                    trust=str(trusted),
                )
                self._assert_untrusted(result, sentinel, source_config)

        for name, trusted in (
            ("broad-source-directory", str(self.source)),
            (
                "inert-second-lock-entry",
                os.pathsep.join((str(source_config), str(self.source / "mise.lock"))),
            ),
        ):
            with self.subTest(overbroad=name):
                result, sentinel = self._invoke(
                    chdir=self.source,
                    trust=trusted,
                )
                self._assert_success(result, sentinel)

    def test_03_named_authority_order_and_identity_mutants_are_rejected(self) -> None:
        good = self.known_good_workflow
        preserve = "--preserve-env=ASTRO_TELEMETRY_DISABLED"
        mutants = {
            "delete-history-fetch": _replace_once(good, HISTORY_FETCH_LINES, ""),
            "changed-history-fetch": _replace_once(
                good,
                HISTORY_FETCH_LINES,
                HISTORY_FETCH_LINES.replace("refs/pull/501/head", "refs/pull/500/head"),
            ),
            "moved-history-fetch": _replace_once(
                _replace_once(good, HISTORY_FETCH_LINES, ""),
                NODE_MODULES_MOVE_LINE,
                NODE_MODULES_MOVE_LINE + HISTORY_FETCH_LINES,
            ),
            "delete-trust": _replace_once(good, TRUST_LINE, ""),
            "delete-chdir": _replace_once(good, CHDIR_LINE, ""),
            "chdir-after-assignment": _replace_once(
                _replace_once(good, CHDIR_LINE, ""),
                HOME_LINE,
                HOME_LINE + CHDIR_LINE,
            ),
            "chdir-checkout": _replace_once(
                good,
                CHDIR_LINE,
                '                  --chdir="$GITHUB_WORKSPACE" \\\n',
            ),
            "chdir-root": _replace_once(
                good, CHDIR_LINE, "                  --chdir=/ \\\n"
            ),
            "pwd-only": _replace_once(
                good,
                CHDIR_LINE,
                '                  PWD="$EQIORA_SITE_SOURCE_ROOT" \\\n',
            ),
            "trust-checkout": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                '"$GITHUB_WORKSPACE/mise.toml" \\\n',
            ),
            "trust-lock-only": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                '"$EQIORA_SITE_SOURCE_ROOT/mise.lock" \\\n',
            ),
            "trust-source-directory": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                '"$EQIORA_SITE_SOURCE_ROOT" \\\n',
            ),
            "trust-scratch-directory": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                '"$EQIORA_API_SCRATCH" \\\n',
            ),
            "trust-runner-temp": _replace_once(
                good,
                TRUST_LINE,
                '                  MISE_TRUSTED_CONFIG_PATHS="$RUNNER_TEMP" \\\n',
            ),
            "trust-root": _replace_once(
                good, TRUST_LINE, "                  MISE_TRUSTED_CONFIG_PATHS=/ \\\n"
            ),
            "trust-relative": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS=mise.toml \\\n",
            ),
            "trust-glob": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                '"$EQIORA_SITE_SOURCE_ROOT/mise*.toml" \\\n',
            ),
            "second-checkout-path": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                '"$EQIORA_SITE_SOURCE_ROOT/mise.toml:$GITHUB_WORKSPACE/mise.toml" \\\n',
            ),
            "second-lock-path": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                '"$EQIORA_SITE_SOURCE_ROOT/mise.toml:'
                '$EQIORA_SITE_SOURCE_ROOT/mise.lock" \\\n',
            ),
            "trust-outside-child-vector": _replace_once(
                _replace_once(good, TRUST_LINE, ""),
                "              exec setpriv \\\n",
                "              export MISE_TRUSTED_CONFIG_PATHS="
                '"$EQIORA_SITE_SOURCE_ROOT/mise.toml"\n'
                "              exec setpriv \\\n",
            ),
            "sudo-preserved-trust": _replace_once(
                _replace_once(good, TRUST_LINE, ""),
                preserve,
                "--preserve-env=MISE_TRUSTED_CONFIG_PATHS,ASTRO_TELEMETRY_DISABLED",
            ),
            "persisted-mise-trust": _replace_once(
                good,
                "              exec setpriv \\\n",
                '              mise trust "$EQIORA_SITE_SOURCE_ROOT/mise.toml"\n'
                "              exec setpriv \\\n",
            ),
            "prompt-yes": _replace_once(
                good, TRUST_LINE, "                  MISE_YES=1 \\\n"
            ),
            "inherited-ci": _replace_once(
                good, TRUST_LINE, "                  CI=1 \\\n"
            ),
            "inherited-github-actions": _replace_once(
                good, TRUST_LINE, "                  GITHUB_ACTIONS=1 \\\n"
            ),
            "changed-home": _replace_once(
                good,
                HOME_LINE,
                '                  HOME="$EQIORA_SITE_SOURCE_ROOT" \\\n',
            ),
            "unquoted-trust": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                "$EQIORA_SITE_SOURCE_ROOT/mise.toml \\\n",
            ),
            "unquoted-chdir": _replace_once(
                good,
                CHDIR_LINE,
                "                  --chdir=$EQIORA_SITE_SOURCE_ROOT \\\n",
            ),
            "source-alias": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                '"$SOURCE_ROOT/mise.toml" \\\n',
            ),
            "symlink-source-alias": _replace_once(
                good,
                TRUST_LINE,
                "                  MISE_TRUSTED_CONFIG_PATHS="
                '"$EQIORA_SITE_SOURCE_ALIAS/mise.toml" \\\n',
            ),
            "checkout-runner": _replace_once(
                good,
                RUNNER_LINE,
                '                  "$GITHUB_WORKSPACE/tools/site/'
                'run_offline_site_checks.sh"\n',
            ),
            "conflated-git-object-authority": _replace_once(
                good,
                "                  EQIORA_SITE_GIT_OBJECT_REPOSITORY="
                '"$EQIORA_SITE_GIT_OBJECT_REPOSITORY" \\\n',
                "                  EQIORA_SITE_GIT_OBJECT_REPOSITORY="
                '"$EQIORA_SITE_SOURCE_ROOT" \\\n',
            ),
            "removed-network-namespace": _replace_once(
                good,
                "            unshare --net -- bash -ceu '\n",
                "            bash -ceu '\n",
            ),
            "changed-mise-action": _replace_once(
                good,
                "jdx/mise-action@5228313ee0372e111a38da051671ca30fc5a96db",
                "jdx/mise-action@0000000000000000000000000000000000000000",
            ),
            "changed-mise-version": _replace_once(
                good, "          version: 2026.5.10\n", "          version: 2026.5.11\n"
            ),
            "relaxed-cleanup-manifest": _replace_once(
                good,
                "                  manifest_sha256 = artifact_manifest_digest(runner_fd)\n",
                "                  manifest_sha256 = '0' * 64\n",
            ),
        }
        self.assertGreaterEqual(len(mutants), 30)
        for name, mutant in mutants.items():
            with self.subTest(mutant=name):
                self.assertNotEqual(mutant, good)
                self.assertEqual(
                    _workflow_errors(mutant, good),
                    [FROZEN_SUCCESSOR_ERROR],
                )

    def test_99_current_workflow_binds_the_frozen_successor(self) -> None:
        current = (REPOSITORY / ".github/workflows/pages.yml").read_text(
            encoding="utf-8"
        )
        errors = _workflow_errors(current, self.known_good_workflow)
        if errors:
            self.assertEqual(
                current,
                self.candidate_workflow,
                "the RED workflow is not the authenticated frozen candidate",
            )
        self.assertEqual(
            errors,
            [],
            "candidate RED only because GNU env --chdir and exact archived "
            "mise.toml trust are absent or misplaced",
        )
        self._assert_known_good_structure(current)


if __name__ == "__main__":
    unittest.main()
