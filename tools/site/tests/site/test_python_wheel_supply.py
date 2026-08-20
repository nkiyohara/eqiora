from __future__ import annotations

import os
import shutil
import subprocess
import sys
import sysconfig
import tempfile
import textwrap
import unittest
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[4]
RUNNER = REPOSITORY / "tools/site/run_offline_site_checks.sh"
SOURCE_SHA = "a" * 40
POST_IDENTITY_SENTINEL = 86


def _write(path: Path, value: str | bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(value, bytes):
        path.write_bytes(value)
    else:
        path.write_text(value, encoding="utf-8")


def _executable(path: Path, value: str) -> None:
    _write(path, value)
    path.chmod(0o755)


def _compile_native(output: Path, version: str) -> None:
    source = output.with_suffix(".c")
    _write(
        source,
        textwrap.dedent(
            f"""
            #include <Python.h>
            #include <stdio.h>
            #include <stdlib.h>

            static struct PyModuleDef module = {{
                PyModuleDef_HEAD_INIT, "_eqiora", NULL, -1, NULL
            }};

            PyMODINIT_FUNC PyInit__eqiora(void) {{
                const char *trace = getenv("TRACE_FILE");
                if (trace != NULL) {{
                    FILE *stream = fopen(trace, "a");
                    if (stream != NULL) {{
                        fputs("native-import\\n", stream);
                        fclose(stream);
                    }}
                }}
                PyObject *result = PyModule_Create(&module);
                if (result != NULL &&
                    PyModule_AddStringConstant(result, "__version__", "{version}") < 0) {{
                    Py_DECREF(result);
                    return NULL;
                }}
                return result;
            }}
            """
        ).lstrip(),
    )
    subprocess.run(
        [
            "cc",
            "-shared",
            "-fPIC",
            f"-I{sysconfig.get_path('include')}",
            str(source),
            "-o",
            str(output),
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


class PythonWheelSupplyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls._native_root = tempfile.TemporaryDirectory()
        root = Path(cls._native_root.name)
        suffix = sysconfig.get_config_var("EXT_SUFFIX")
        if not isinstance(suffix, str) or not suffix.startswith("."):
            raise AssertionError(f"unusable native extension suffix: {suffix!r}")
        cls._extension_suffix = suffix
        cls._native = root / f"_eqiora{suffix}"
        cls._wrong_native = root / f"wrong/_eqiora{suffix}"
        cls._wrong_native.parent.mkdir()
        _compile_native(cls._native, "0.1.0a1")
        _compile_native(cls._wrong_native, "0.1.0a2")

    @classmethod
    def tearDownClass(cls) -> None:
        cls._native_root.cleanup()

    def _fixture_site(self, root: Path, mutation: str) -> Path:
        site = root / f"fixture-{mutation or 'ordinary'}"
        package = site / "eqiora"
        package.mkdir(parents=True)
        package_version = (
            "0.1.0a2" if mutation == "wrong-package-version" else "0.1.0a1"
        )
        package_source = f"""
import os

with open(os.environ["TRACE_FILE"], "a", encoding="utf-8") as stream:
    stream.write("public-import\\n")
__version__ = {package_version!r}
"""
        if mutation == "wrong-module-name":
            package_source += """
import importlib
_native = importlib.import_module("eqiora._eqiora")
_native.__name__ = "eqiora.not_the_native_module"
"""
        if mutation in {
            "top-level-wrong-exception-name-after-positives",
            "top-level-closure-before-positives",
        }:
            package_source += r"""
import importlib as _probe_importlib
import importlib.machinery as _probe_importlib_machinery
import importlib.metadata as _probe_metadata
import os as _probe_os
import sys as _probe_sys
import types as _probe_types

_PROBE_POSITIVES = {
    "positive-public-version",
    "positive-public-origin",
    "positive-native-version",
    "positive-native-origin",
    "positive-distribution-version",
    "positive-distribution-origin",
}


def _probe_record(value):
    with open(_probe_os.environ["TRACE_FILE"], "a", encoding="utf-8") as stream:
        stream.write(value + "\n")


def _probe_positives_complete():
    with open(_probe_os.environ["TRACE_FILE"], encoding="utf-8") as stream:
        observed = set(stream.read().splitlines())
    return _PROBE_POSITIVES <= observed


class _TrackedModule(_probe_types.ModuleType):
    def __getattribute__(self, name):
        value = super().__getattribute__(name)
        role = super().__getattribute__("_probe_role")
        if name == "__version__":
            _probe_record(f"positive-{role}-version")
        elif name == "__file__":
            _probe_record(f"positive-{role}-origin")
        return value


class _TrackedDistribution:
    def __init__(self, distribution):
        self._distribution = distribution

    @property
    def version(self):
        value = self._distribution.version
        _probe_record("positive-distribution-version")
        return value

    def locate_file(self, path):
        value = self._distribution.locate_file(path)
        _probe_record("positive-distribution-origin")
        return value

    def __getattr__(self, name):
        value = getattr(self._distribution, name)
        if name == "_path":
            _probe_record("positive-distribution-origin")
        return value


_probe_original_from_name = _probe_metadata.Distribution.from_name


def _probe_from_name(_class, name):
    return _TrackedDistribution(_probe_original_from_name(name))


_probe_metadata.Distribution.from_name = classmethod(_probe_from_name)


class _TopLevelLoader:
    def __init__(self, wrong_name=False):
        self._wrong_name = wrong_name

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        if not self._wrong_name:
            module.__version__ = "0.1.0a1"
            return
        _probe_record("namespace-import-wrong-name-after-positives")
        error = ModuleNotFoundError("ambient dependency is absent")
        error.name = "ambient_dependency"
        raise error


class _TopLevelProbeFinder:
    def __init__(self):
        self._calls = 0

    def find_spec(self, fullname, path=None, target=None):
        if fullname != "_eqiora":
            return None
        complete = _probe_positives_complete()
        mutation = _probe_os.environ["P_MUTANT"]
        if mutation == "top-level-closure-before-positives":
            _probe_record(
                "namespace-order-after-positives"
                if complete
                else "namespace-order-before-positives"
            )
            if complete:
                return _probe_importlib_machinery.ModuleSpec(
                    fullname, _TopLevelLoader()
                )
            return None

        self._calls += 1
        if self._calls == 1:
            _probe_record(
                "namespace-spec-absence-after-positives"
                if complete
                else "namespace-spec-absence-before-positives"
            )
            return None
        return _probe_importlib_machinery.ModuleSpec(
            fullname, _TopLevelLoader(wrong_name=True)
        )


_probe_native = _probe_importlib.import_module("eqiora._eqiora")
_probe_native._probe_role = "native"
_probe_native.__class__ = _TrackedModule
_probe_sys.modules[__name__]._probe_role = "public"
_probe_sys.modules[__name__].__class__ = _TrackedModule
_probe_sys.meta_path.insert(0, _TopLevelProbeFinder())
"""
        _write(package / "__init__.py", textwrap.dedent(package_source).lstrip())

        if mutation == "non-native-stand-in":
            _write(
                package / "_eqiora.py",
                "import os\n"
                "with open(os.environ['TRACE_FILE'], 'a', encoding='utf-8') as stream:\n"
                "    stream.write('native-import\\n')\n"
                "__version__ = '0.1.0a1'\n",
            )
        elif mutation not in {"top-level-only", "missing-native"}:
            native = (
                self._wrong_native
                if mutation == "wrong-native-version"
                else self._native
            )
            shutil.copy2(native, package / f"_eqiora{self._extension_suffix}")

        if mutation in {"top-level-only", "both-top-level-and-package-local"}:
            shutil.copy2(self._native, site / f"_eqiora{self._extension_suffix}")
        elif mutation == "top-level-unrelated-import-failure":
            _write(
                site / "_eqiora.py",
                'error = ModuleNotFoundError("ambient dependency is absent")\n'
                "error.name = 'ambient_dependency'\n"
                "raise error\n",
            )

        distribution_version = (
            "0.1.0a2" if mutation == "wrong-distribution-version" else "0.1.0a1"
        )
        metadata = site / "eqiora-0.1.0a1.dist-info/METADATA"
        _write(
            metadata,
            f"Metadata-Version: 2.1\nName: eqiora\nVersion: {distribution_version}\n",
        )
        _write(metadata.parent / "top_level.txt", "eqiora\n")
        return site

    def _mock_uv(self, path: Path) -> None:
        _executable(
            path,
            textwrap.dedent(
                f"""
                #!{sys.executable}
                from __future__ import annotations

                import os
                import shutil
                import subprocess
                import sys
                from pathlib import Path

                args = sys.argv[1:]
                trace = Path(os.environ["TRACE_FILE"])

                def record(value: str) -> None:
                    with trace.open("a", encoding="utf-8") as stream:
                        stream.write(value + "\\n")

                def option(name: str) -> Path:
                    return Path(args[args.index(name) + 1])

                if args == ["--version"]:
                    print("uv 0.12.1 (x86_64-unknown-linux-musl)")
                    raise SystemExit(0)

                mutation = os.environ.get("W_MUTANT", "")
                if args and args[0] == "build":
                    output = option("--out-dir")
                    wheel = output / "eqiora-0.1.0a1-cp313-cp313-linux_x86_64.whl"
                    control = output / ".gitignore"
                    record("build-ok")

                    if mutation != "zero-wheels":
                        if mutation == "wheel-symlink":
                            backing = output.parent / "wheel-backing.whl"
                            backing.write_bytes(b"wheel")
                            wheel.symlink_to(backing)
                        elif mutation == "wheel-fifo":
                            os.mkfifo(wheel)
                        elif mutation == "wheel-hard-link":
                            backing = output.parent / "wheel-backing.whl"
                            backing.write_bytes(b"wheel")
                            os.link(backing, wheel)
                        else:
                            wheel.write_bytes(b"wheel")

                    if mutation == "control-absent":
                        pass
                    elif mutation == "control-directory":
                        control.mkdir()
                    elif mutation == "control-symlink":
                        backing = output.parent / "control-backing"
                        backing.write_bytes(b"*")
                        control.symlink_to(backing)
                    elif mutation == "control-fifo":
                        os.mkfifo(control)
                    elif mutation == "control-hard-link":
                        backing = output.parent / "control-backing"
                        backing.write_bytes(b"*")
                        os.link(backing, control)
                    elif mutation == "control-renamed":
                        (output / ".uv-control").write_bytes(b"*")
                    else:
                        contents = {{
                            "control-empty": b"",
                            "control-newline": b"*\\n",
                            "control-other-byte": b"?",
                        }}.get(mutation, b"*")
                        control.write_bytes(contents)

                    if mutation == "extra-hidden-entry":
                        (output / ".unexpected").write_bytes(b"x")
                    elif mutation == "extra-regular-entry":
                        (output / "unexpected.txt").write_bytes(b"x")
                    elif mutation == "nested-entry":
                        nested = output / "nested"
                        nested.mkdir()
                        (nested / "unexpected").write_bytes(b"x")
                    elif mutation == "second-wheel":
                        (output / "eqiora_second-0.1.0a1-cp313.whl").write_bytes(b"wheel")
                    raise SystemExit(0)

                if args and args[0] == "venv":
                    destination = Path(args[-1])
                    subprocess.run(
                        [{sys.executable!r}, "-m", "venv", "--without-pip", str(destination)],
                        check=True,
                    )
                    record("venv")
                    wheels = Path(os.environ["WHEELS_ROOT"])
                    if mutation == "control-transition-to-symlink":
                        control = wheels / ".gitignore"
                        control.unlink(missing_ok=True)
                        backing = wheels.parent / "transitioned-control"
                        backing.write_bytes(b"*")
                        control.symlink_to(backing)
                        record("transition-applied")
                    elif mutation == "control-transition-to-regular":
                        control = wheels / ".gitignore"
                        replacement = wheels / ".gitignore.replacement"
                        replacement.write_bytes(b"*")
                        os.replace(replacement, control)
                        record("transition-applied")
                    elif mutation == "wheel-transition-to-symlink":
                        wheel = wheels / "eqiora-0.1.0a1-cp313-cp313-linux_x86_64.whl"
                        wheel.unlink(missing_ok=True)
                        backing = wheels.parent / "transitioned-wheel.whl"
                        backing.write_bytes(b"wheel")
                        wheel.symlink_to(backing)
                        record("transition-applied")
                    elif mutation == "wheel-transition-to-regular":
                        wheel = wheels / "eqiora-0.1.0a1-cp313-cp313-linux_x86_64.whl"
                        replacement = wheels / "wheel.replacement"
                        replacement.write_bytes(b"wheel")
                        os.replace(replacement, wheel)
                        record("transition-applied")
                    raise SystemExit(0)

                if args[:2] == ["pip", "install"]:
                    if args.count("--python") != 1:
                        raise SystemExit("install did not name one interpreter")
                    python_index = args.index("--python")
                    if python_index + 1 >= len(args):
                        raise SystemExit("install omitted the interpreter value")
                    python = Path(args[python_index + 1])
                    expected_venv = Path(os.environ["EQIORA_API_SCRATCH"]) / "venv"
                    if python != expected_venv / "bin/python":
                        raise SystemExit("install escaped the fresh venv interpreter")
                    if args.count("--no-index") != 1 or args.count("--no-deps") != 1:
                        raise SystemExit("install relaxed no-index/no-deps")
                    consumed = {{0, 1, python_index, python_index + 1}}
                    consumed.add(args.index("--no-index"))
                    consumed.add(args.index("--no-deps"))
                    inputs = [value for index, value in enumerate(args) if index not in consumed]
                    expected_wheel = (
                        Path(os.environ["WHEELS_ROOT"])
                        / "eqiora-0.1.0a1-cp313-cp313-linux_x86_64.whl"
                    )
                    if inputs != [str(expected_wheel)]:
                        raise SystemExit("install did not consume only the admitted wheel")
                    venv = python.parent.parent
                    major_minor = f"python{{sys.version_info.major}}.{{sys.version_info.minor}}"
                    site = venv / "lib" / major_minor / "site-packages"
                    site.mkdir(parents=True, exist_ok=True)
                    fixture = Path(os.environ["FIXTURE_SITE"])
                    identity_mutation = os.environ.get("P_MUTANT", "")
                    external_origins = {{
                        "origin-source",
                        "origin-user-site",
                        "origin-system-site",
                        "origin-other-venv",
                    }}
                    if identity_mutation in external_origins:
                        (site / "eqiora-external.pth").write_text(
                            str(fixture) + "\\n", encoding="utf-8"
                        )
                    elif identity_mutation == "package-symlink-escape":
                        (site / "eqiora").symlink_to(fixture / "eqiora", target_is_directory=True)
                        shutil.copytree(
                            fixture / "eqiora-0.1.0a1.dist-info",
                            site / "eqiora-0.1.0a1.dist-info",
                        )
                    elif identity_mutation == "native-symlink-escape":
                        for child in fixture.iterdir():
                            target = site / child.name
                            if child.is_dir():
                                shutil.copytree(child, target)
                            else:
                                shutil.copy2(child, target)
                        native = next((site / "eqiora").glob("_eqiora*.so"))
                        external = Path(os.environ["FIXTURE_EXTERNAL"]) / native.name
                        external.parent.mkdir(parents=True, exist_ok=True)
                        native.replace(external)
                        native.symlink_to(external)
                    elif identity_mutation == "distribution-root-escape":
                        shutil.copytree(fixture / "eqiora", site / "eqiora")
                        external = Path(os.environ["FIXTURE_EXTERNAL"])
                        shutil.copytree(
                            fixture / "eqiora-0.1.0a1.dist-info",
                            external / "eqiora-0.1.0a1.dist-info",
                        )
                        (site / "eqiora-distribution-external.pth").write_text(
                            str(external) + "\\n", encoding="utf-8"
                        )
                    else:
                        for child in fixture.iterdir():
                            target = site / child.name
                            if child.is_dir():
                                shutil.copytree(child, target)
                            else:
                                shutil.copy2(child, target)
                    record("install")
                    raise SystemExit(0)

                raise SystemExit(f"unexpected uv invocation: {{args!r}}")
                """
            ).lstrip(),
        )

    def _layout(
        self, root: Path, w_mutant: str, p_mutant: str
    ) -> tuple[Path, dict[str, str], Path]:
        scratch = root / "scratch"
        source = scratch / "source"
        mocks = root / "mocks"
        browser_root = root / "browser-supply/eqiora-pw-1.62.1-r1234"
        browser = browser_root / "chromium-1234/chrome-linux64/chrome"
        trace = root / "trace.log"
        fixture = self._fixture_site(root, p_mutant)

        (scratch / "build").mkdir(parents=True)
        (scratch / "uv-cache").mkdir()
        (source / "docs/site/node_modules").mkdir(parents=True)
        _write(
            source / "Cargo.toml",
            '[workspace]\nmembers = []\n[workspace.package]\nversion = "0.1.0-alpha.1"\n',
        )
        _write(source / "docs/site/package.json", "{}\n")
        _write(
            source / "tools/release/python_candidate_common.py",
            "def python_distribution_version(version):\n"
            "    return version.replace('-alpha.', 'a')\n",
        )
        runner = source / "tools/site/run_offline_site_checks.sh"
        runner.parent.mkdir(parents=True)
        shutil.copy2(RUNNER, runner)
        runner.chmod(0o755)

        runner_text = RUNNER.read_text(encoding="utf-8")
        browser_version = (
            "Google Chrome for Testing 151.0.7922.34 "
            if "Google Chrome for Testing 151.0.7922.34 " in runner_text
            else "HeadlessChrome 151.0.7922.34"
        )
        _executable(browser, f"#!/bin/sh\nprintf '%s\\n' '{browser_version}'\n")

        mocks.mkdir()
        _executable(
            mocks / "node",
            f"""#!/bin/sh
if test "${{1:-}}" = --version; then
  echo v24.18.1
  exit 0
fi
for argument in "$@"; do
  if test "$argument" = -e; then
    printf '%s\\n' '{browser}'
    exit 0
  fi
done
exit 0
""",
        )
        _executable(
            mocks / "npm",
            '#!/bin/sh\nif test "${1:-}" = --version; then echo 11.16.0; fi\nexit 0\n',
        )
        _executable(
            mocks / "rustc",
            "#!/bin/sh\necho 'rustc 1.97.1 (fixture 2026-08-01)'\n",
        )
        _executable(mocks / "dpkg-query", "#!/bin/sh\nexit 0\n")
        _executable(
            mocks / "python3",
            textwrap.dedent(
                f"""
                #!{sys.executable}
                import os
                import sys

                args = sys.argv[1:]
                if args == ["--version"]:
                    print("Python 3.13.14")
                    raise SystemExit(0)
                if args[:2] == ["-m", "unittest"]:
                    raise SystemExit(0)
                if args and args[0] == "-":
                    source = sys.stdin.read()
                    if "python_distribution_version" in source:
                        print("0.1.0-alpha.1 0.1.0a1")
                    elif (
                        "external DNS sentinel" in source
                        or "from tools.site.check_site import check_source" in source
                    ):
                        pass
                    else:
                        completed = __import__("subprocess").run(
                            [{sys.executable!r}, *args],
                            input=source,
                            text=True,
                            check=False,
                        )
                        raise SystemExit(completed.returncode)
                    raise SystemExit(0)
                completed = __import__("subprocess").run(
                    [{sys.executable!r}, *args], check=False
                )
                raise SystemExit(completed.returncode)
                """
            ).lstrip(),
        )
        _executable(
            mocks / "cargo",
            textwrap.dedent(
                f"""
                #!{sys.executable}
                import os
                import sys
                from pathlib import Path

                args = sys.argv[1:]
                trace = Path(os.environ["TRACE_FILE"])
                if "build" in args:
                    target = Path(args[args.index("--target-dir") + 1])
                    release = target / "release"
                    release.mkdir(parents=True)
                    eqiora = release / "eqiora"
                    eqiora.write_text(
                        "#!/bin/sh\\necho 'eqiora 0.1.0-alpha.1'\\n", encoding="utf-8"
                    )
                    eqiora.chmod(0o755)
                    mcp = release / "eqiora-mcp"
                    mcp.write_text("#!/bin/sh\\nexit 0\\n", encoding="utf-8")
                    mcp.chmod(0o755)
                    with trace.open("a", encoding="utf-8") as stream:
                        stream.write("cargo-build\\n")
                    raise SystemExit(0)
                with trace.open("a", encoding="utf-8") as stream:
                    stream.write("post-python-identity\\n")
                raise SystemExit({POST_IDENTITY_SENTINEL})
                """
            ).lstrip(),
        )
        self._mock_uv(mocks / "uv")

        environment = os.environ.copy()
        environment.update(
            {
                "PATH": f"{mocks}{os.pathsep}{environment['PATH']}",
                "LC_ALL": "C",
                "TZ": "UTC",
                "npm_config_offline": "true",
                "CARGO_NET_OFFLINE": "true",
                "UV_OFFLINE": "1",
                "EQIORA_API_SCRATCH": str(scratch.resolve()),
                "EQIORA_SITE_SOURCE_ROOT": str(source.resolve()),
                "EQIORA_SITE_ASTRO_OUT_DIR": str((scratch / "astro").resolve()),
                "EQIORA_SITE_RUSTDOC_TARGET": str(
                    (scratch / "rustdoc-target").resolve()
                ),
                "EQIORA_SITE_RUSTDOC_STAGE": str((scratch / "rustdoc-stage").resolve()),
                "EQIORA_SITE_ARTIFACT": str((scratch / "build/site").resolve()),
                "EQIORA_SITE_SOURCE_SHA": SOURCE_SHA,
                "PLAYWRIGHT_BROWSERS_PATH": str(browser_root.resolve()),
                "TRACE_FILE": str(trace),
                "W_MUTANT": w_mutant,
                "P_MUTANT": p_mutant,
                "WHEELS_ROOT": str((scratch / "wheels").resolve()),
                "FIXTURE_SITE": str(fixture.resolve()),
                "FIXTURE_EXTERNAL": str((root / f"external-{p_mutant}").resolve()),
            }
        )
        return runner, environment, trace

    def _run(
        self, w_mutant: str = "", p_mutant: str = ""
    ) -> tuple[subprocess.CompletedProcess[str], list[str]]:
        with tempfile.TemporaryDirectory() as temporary:
            runner, environment, trace = self._layout(
                Path(temporary), w_mutant, p_mutant
            )
            result = subprocess.run(
                [str(runner)],
                check=False,
                cwd=environment["EQIORA_SITE_SOURCE_ROOT"],
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                timeout=15,
            )
            observations = (
                trace.read_text(encoding="utf-8").splitlines() if trace.exists() else []
            )
            return result, observations

    def test_00_ordinary_supply_reaches_post_identity_boundary_before_mutants(
        self,
    ) -> None:
        result, observations = self._run()
        self.assertEqual(
            result.returncode,
            POST_IDENTITY_SENTINEL,
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}\ntrace:{observations}",
        )
        expected = [
            "cargo-build",
            "build-ok",
            "venv",
            "install",
            "public-import",
            "native-import",
            "post-python-identity",
        ]
        self.assertEqual(observations, expected)

    def test_w01_output_admission_mutants_fail_after_build_before_venv(self) -> None:
        mutations = (
            "control-absent",
            "control-empty",
            "control-newline",
            "control-other-byte",
            "control-directory",
            "control-symlink",
            "control-fifo",
            "control-hard-link",
            "control-renamed",
            "extra-hidden-entry",
            "extra-regular-entry",
            "nested-entry",
            "second-wheel",
            "wheel-symlink",
            "wheel-fifo",
            "wheel-hard-link",
            "zero-wheels",
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                result, observations = self._run(w_mutant=mutation)
                self.assertNotEqual(result.returncode, 0, result.stderr)
                self.assertNotEqual(result.returncode, POST_IDENTITY_SENTINEL)
                self.assertIn("build-ok", observations)
                self.assertNotIn("venv", observations, observations)

    def test_w01_transition_mutants_fail_after_venv_before_install(self) -> None:
        for mutation in (
            "control-transition-to-symlink",
            "control-transition-to-regular",
            "wheel-transition-to-symlink",
            "wheel-transition-to-regular",
        ):
            with self.subTest(mutation=mutation):
                result, observations = self._run(w_mutant=mutation)
                self.assertNotEqual(result.returncode, 0, result.stderr)
                self.assertNotEqual(result.returncode, POST_IDENTITY_SENTINEL)
                self.assertIn("build-ok", observations)
                self.assertIn("venv", observations)
                self.assertIn("transition-applied", observations)
                self.assertNotIn("install", observations, observations)

    def test_p01_identity_mutants_reach_installed_public_package_gate(self) -> None:
        mutations_without_local_import = {"top-level-only", "missing-native"}
        mutations = (
            "top-level-only",
            "both-top-level-and-package-local",
            "top-level-unrelated-import-failure",
            "missing-native",
            "wrong-package-version",
            "wrong-native-version",
            "wrong-distribution-version",
            "origin-source",
            "origin-user-site",
            "origin-system-site",
            "origin-other-venv",
            "package-symlink-escape",
            "native-symlink-escape",
            "distribution-root-escape",
            "wrong-module-name",
            "non-native-stand-in",
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                result, observations = self._run(p_mutant=mutation)
                self.assertNotEqual(result.returncode, 0, result.stderr)
                self.assertNotEqual(result.returncode, POST_IDENTITY_SENTINEL)
                for reached in ("build-ok", "venv", "install", "public-import"):
                    self.assertIn(reached, observations, observations)
                if mutation not in mutations_without_local_import:
                    self.assertIn("native-import", observations, observations)

    def test_p01_exception_name_is_checked_after_absent_spec_and_positives(
        self,
    ) -> None:
        result, observations = self._run(
            p_mutant="top-level-wrong-exception-name-after-positives"
        )
        self.assertNotEqual(result.returncode, 0, result.stderr)
        self.assertNotEqual(result.returncode, POST_IDENTITY_SENTINEL)
        positives = (
            "positive-public-version",
            "positive-public-origin",
            "positive-native-version",
            "positive-native-origin",
            "positive-distribution-version",
            "positive-distribution-origin",
        )
        for reached in (
            "build-ok",
            "venv",
            "install",
            "public-import",
            "native-import",
        ):
            self.assertIn(reached, observations, observations)
        for reached in positives:
            self.assertIn(reached, observations, observations)
        absent_spec = observations.index("namespace-spec-absence-after-positives")
        wrong_name = observations.index("namespace-import-wrong-name-after-positives")
        self.assertLess(
            max(observations.index(value) for value in positives), absent_spec
        )
        self.assertLess(absent_spec, wrong_name)

    def test_p01_namespace_closure_follows_version_and_origin_positives(
        self,
    ) -> None:
        result, observations = self._run(p_mutant="top-level-closure-before-positives")
        self.assertNotEqual(result.returncode, 0, result.stderr)
        self.assertNotEqual(result.returncode, POST_IDENTITY_SENTINEL)
        positives = (
            "positive-public-version",
            "positive-public-origin",
            "positive-native-version",
            "positive-native-origin",
            "positive-distribution-version",
            "positive-distribution-origin",
        )
        for reached in (
            "build-ok",
            "venv",
            "install",
            "public-import",
            "native-import",
        ):
            self.assertIn(reached, observations, observations)
        for reached in positives:
            self.assertIn(reached, observations, observations)
        closure = observations.index("namespace-order-after-positives")
        self.assertLess(max(observations.index(value) for value in positives), closure)
        self.assertNotIn("namespace-order-before-positives", observations)


if __name__ == "__main__":
    unittest.main()
