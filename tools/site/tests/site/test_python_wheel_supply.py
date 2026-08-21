from __future__ import annotations

import hashlib
import os
import shutil
import stat
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
BROWSER_SHA256 = "0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71"
BROWSER_BYTES = 290_614_600
FULL_BROWSER_STDOUT = b"Google Chrome for Testing 151.0.7922.34 \n"
SOURCE_SUCCESS = "site source: exact optional CLAUDE.md topology admitted"
BROWSER_SUCCESS = "site browser: exact locked full Chromium supply admitted"
TOOLCHAIN_BYTES = 66
TOOLCHAIN_BLOB = "73cb934de4706a914c15e8db2a3c037ce75699d9"
TOOLCHAIN_SHA256 = "a6a0bbd29ffaa8182dc22d1d9149709f1091e47df40ed96eb8a78a711c66a4ce"
MISMATCH_TOOLCHAIN = b'[toolchain]\nchannel = "1.85.0"\n'


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
        browser_root_value = os.environ.get("PLAYWRIGHT_BROWSERS_PATH")
        browser_sha256 = os.environ.get("EQIORA_SITE_BROWSER_SHA256")
        browser_bytes = os.environ.get("EQIORA_SITE_BROWSER_BYTES")
        if not browser_root_value or not browser_sha256 or not browser_bytes:
            raise AssertionError("the official browser identity inputs are required")
        cls._browser_root = Path(browser_root_value)
        cls._browser = cls._browser_root / "chromium-1234/chrome-linux64/chrome"
        if (
            not cls._browser_root.is_absolute()
            or cls._browser_root.resolve() != cls._browser_root
            or cls._browser_root.name != "eqiora-pw-1.62.1-r1234"
            or cls._browser.is_symlink()
            or not cls._browser.is_file()
            or not os.access(cls._browser, os.X_OK)
            or cls._browser.stat().st_size != BROWSER_BYTES
            or hashlib.sha256(cls._browser.read_bytes()).hexdigest() != BROWSER_SHA256
            or browser_sha256 != BROWSER_SHA256
            or browser_bytes != str(BROWSER_BYTES)
        ):
            raise AssertionError("the official full Chromium supply changed")
        version = subprocess.run(
            [str(cls._browser), "--version"],
            check=False,
            env={**os.environ, "LC_ALL": "C", "TZ": "UTC"},
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=10,
        )
        if (
            version.returncode != 0
            or version.stderr
            or version.stdout != FULL_BROWSER_STDOUT
        ):
            raise AssertionError("the official full Chromium version bytes changed")

        for relative in (
            "node_modules/@playwright/test/package.json",
            "node_modules/playwright/package.json",
            "node_modules/playwright-core/package.json",
            "node_modules/playwright-core/browsers.json",
        ):
            if not (REPOSITORY / "docs/site" / relative).is_file():
                raise AssertionError(
                    "the exact locked Playwright packages must be installed first"
                )

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

    @staticmethod
    def _copy_locked_browser_supply(source: Path) -> None:
        site = source / "docs/site"
        site.mkdir(parents=True)
        shutil.copy2(REPOSITORY / "docs/site/package.json", site / "package.json")
        shutil.copy2(
            REPOSITORY / "docs/site/package-lock.json", site / "package-lock.json"
        )
        modules = site / "node_modules"
        modules.mkdir()

        def link_or_copy(origin: str, destination: str) -> str:
            try:
                os.link(origin, destination)
            except OSError:
                shutil.copy2(origin, destination)
            return destination

        for package in ("@playwright/test", "playwright", "playwright-core"):
            destination = modules / package
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(
                REPOSITORY / "docs/site/node_modules" / package,
                destination,
                copy_function=link_or_copy,
            )

    @classmethod
    def tearDownClass(cls) -> None:
        cls._native_root.cleanup()

    def _copy_exact_toolchain(self, source: Path) -> None:
        origin = REPOSITORY / "rust-toolchain.toml"
        self.assertTrue(origin.is_file())
        self.assertFalse(origin.is_symlink())
        payload = origin.read_bytes()
        self.assertEqual(len(payload), TOOLCHAIN_BYTES)
        self.assertEqual(hashlib.sha256(payload).hexdigest(), TOOLCHAIN_SHA256)
        self.assertEqual(
            hashlib.sha1(b"blob 66\0" + payload, usedforsecurity=False).hexdigest(),
            TOOLCHAIN_BLOB,
        )
        destination = source / "rust-toolchain.toml"
        shutil.copyfile(origin, destination)
        destination.chmod(0o644)
        self.assertEqual(destination.read_bytes(), payload)
        self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o644)

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
        }:
            package_source += r"""
import importlib.machinery as _probe_importlib_machinery
import os as _probe_os
import sys as _probe_sys


def _probe_record(value):
    with open(_probe_os.environ["TRACE_FILE"], "a", encoding="utf-8") as stream:
        stream.write(value + "\n")


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
        mutation = _probe_os.environ["P_MUTANT"]
        if mutation != "top-level-wrong-exception-name-after-positives":
            _probe_record("namespace-before-positive-rejection")
            return _probe_importlib_machinery.ModuleSpec(
                fullname, _TopLevelLoader()
            )

        self._calls += 1
        if self._calls == 1:
            _probe_record("namespace-spec-absence-after-positives")
            return None
        return _probe_importlib_machinery.ModuleSpec(
            fullname, _TopLevelLoader(wrong_name=True)
        )


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
        self, root: Path, w_mutant: str, p_mutant: str, upstream_mutant: str
    ) -> tuple[Path, dict[str, str], Path]:
        scratch = root / "scratch"
        source = scratch / "source"
        mocks = root / "mocks"
        trace = root / "trace.log"
        fixture = self._fixture_site(root, p_mutant)

        (scratch / "build").mkdir(parents=True)
        (scratch / "uv-cache").mkdir()
        self._copy_locked_browser_supply(source)
        _write(
            source / "Cargo.toml",
            '[workspace]\nmembers = []\n[workspace.package]\nversion = "0.1.0-alpha.1"\n',
        )
        self._copy_exact_toolchain(source)
        shutil.copy2(REPOSITORY / "AGENTS.md", source / "AGENTS.md")
        (source / "CLAUDE.md").symlink_to("AGENTS.md")
        _write(
            source / "tools/release/python_candidate_common.py",
            "def python_distribution_version(version):\n"
            "    return version.replace('-alpha.', 'a')\n",
        )
        runner = source / "tools/site/run_offline_site_checks.sh"
        runner.parent.mkdir(parents=True)
        shutil.copy2(
            REPOSITORY / "tools/site/check_site.py", source / "tools/site/check_site.py"
        )
        shutil.copy2(
            REPOSITORY / "tools/site/check_site_html.py",
            source / "tools/site/check_site_html.py",
        )
        shutil.copy2(RUNNER, runner)
        runner.chmod(0o755)

        mocks.mkdir()
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

        browser_root = self._browser_root
        browser_sha256 = BROWSER_SHA256
        browser_bytes = str(BROWSER_BYTES)
        if upstream_mutant == "missing-checker":
            (source / "tools/site/check_site.py").unlink()
        elif upstream_mutant == "additional-source-link":
            (source / "extra-link").symlink_to("AGENTS.md")
        elif upstream_mutant == "shell-browser":
            browser_root = root / "wrong-browser/eqiora-pw-1.62.1-r1234"
            browser = browser_root / "chromium-1234/chrome-linux64/chrome"
            _executable(
                browser,
                "#!/bin/sh\nprintf 'Google Chrome for Testing 151.0.7922.34 \\n'\n",
            )
            payload = browser.read_bytes()
            browser_sha256 = hashlib.sha256(payload).hexdigest()
            browser_bytes = str(len(payload))
        elif upstream_mutant == "wrong-browser-digest":
            browser_sha256 = "0" * 64
        elif upstream_mutant == "wrong-browser-bytes":
            browser_bytes = str(BROWSER_BYTES - 1)

        environment = os.environ.copy()
        environment.pop("RUSTUP_TOOLCHAIN", None)
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
                "EQIORA_SITE_BROWSER_SHA256": browser_sha256,
                "EQIORA_SITE_BROWSER_BYTES": browser_bytes,
                "TRACE_FILE": str(trace),
                "W_MUTANT": w_mutant,
                "P_MUTANT": p_mutant,
                "WHEELS_ROOT": str((scratch / "wheels").resolve()),
                "FIXTURE_SITE": str(fixture.resolve()),
                "FIXTURE_EXTERNAL": str((root / f"external-{p_mutant}").resolve()),
            }
        )
        if upstream_mutant == "missing-browser-sha256":
            environment.pop("EQIORA_SITE_BROWSER_SHA256")
        elif upstream_mutant == "missing-browser-bytes":
            environment.pop("EQIORA_SITE_BROWSER_BYTES")
        return runner, environment, trace

    def _run(
        self,
        w_mutant: str = "",
        p_mutant: str = "",
        upstream_mutant: str = "",
    ) -> tuple[subprocess.CompletedProcess[str], list[str]]:
        with tempfile.TemporaryDirectory() as temporary:
            runner, environment, trace = self._layout(
                Path(temporary), w_mutant, p_mutant, upstream_mutant
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

    def _assert_sb_positive(self, result: subprocess.CompletedProcess[str]) -> None:
        self.assertEqual(result.stdout.count(SOURCE_SUCCESS), 1, result.stdout)
        self.assertEqual(result.stdout.count(BROWSER_SUCCESS), 1, result.stdout)
        self.assertLess(
            result.stdout.index(SOURCE_SUCCESS), result.stdout.index(BROWSER_SUCCESS)
        )

    def _assert_toolchain_rejection(
        self,
        result: subprocess.CompletedProcess[str],
        observations: list[str],
        environment: dict[str, str],
    ) -> None:
        self.assertNotIn(result.returncode, (0, POST_IDENTITY_SENTINEL))
        self.assertEqual(observations, [])
        self.assertNotIn(SOURCE_SUCCESS, result.stdout)
        self.assertNotIn(BROWSER_SUCCESS, result.stdout)
        source = Path(environment["EQIORA_SITE_SOURCE_ROOT"])
        selected = subprocess.run(
            ["rustc", "-Vv"],
            check=False,
            cwd=source,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=15,
        )
        stable = subprocess.run(
            ["rustc", "+stable", "-Vv"],
            check=False,
            cwd=source,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=15,
        )
        self.assertEqual((selected.returncode, stable.returncode), (0, 0))
        self.assertEqual((selected.stderr, stable.stderr), (b"", b""))
        self.assertNotEqual(selected.stdout, stable.stdout)

    def test_00_ordinary_supply_reaches_post_identity_boundary_before_mutants(
        self,
    ) -> None:
        result, observations = self._run()
        self.assertEqual(
            result.returncode,
            POST_IDENTITY_SENTINEL,
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}\ntrace:{observations}",
        )
        self._assert_sb_positive(result)
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

        for mutation in ("missing", "mismatch"):
            with (
                self.subTest(toolchain=mutation),
                tempfile.TemporaryDirectory() as temporary,
            ):
                runner, environment, trace = self._layout(Path(temporary), "", "", "")
                toolchain = Path(
                    environment["EQIORA_SITE_SOURCE_ROOT"], "rust-toolchain.toml"
                )
                if mutation == "missing":
                    toolchain.unlink()
                else:
                    toolchain.write_bytes(MISMATCH_TOOLCHAIN)
                    toolchain.chmod(0o644)
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
                    trace.read_text(encoding="utf-8").splitlines()
                    if trace.exists()
                    else []
                )
                self._assert_toolchain_rejection(result, observations, environment)

    def test_01_upstream_supply_mutants_fail_before_python_wheel_work(self) -> None:
        for mutation in (
            "missing-checker",
            "additional-source-link",
            "missing-browser-sha256",
            "missing-browser-bytes",
            "wrong-browser-digest",
            "wrong-browser-bytes",
            "shell-browser",
        ):
            with self.subTest(mutation=mutation):
                result, observations = self._run(upstream_mutant=mutation)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotEqual(result.returncode, POST_IDENTITY_SENTINEL)
                self.assertEqual(observations, [])

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
                self._assert_sb_positive(result)
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
                self._assert_sb_positive(result)
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
                self._assert_sb_positive(result)
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
        self._assert_sb_positive(result)
        self.assertNotEqual(result.returncode, 0, result.stderr)
        self.assertNotEqual(result.returncode, POST_IDENTITY_SENTINEL)
        for reached in (
            "build-ok",
            "venv",
            "install",
            "public-import",
            "native-import",
        ):
            self.assertIn(reached, observations, observations)
        absent_spec = observations.index("namespace-spec-absence-after-positives")
        wrong_name = observations.index("namespace-import-wrong-name-after-positives")
        self.assertLess(absent_spec, wrong_name)

    def test_p01_namespace_closure_follows_version_and_origin_positives(
        self,
    ) -> None:
        for mutation in (
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
        ):
            with self.subTest(mutation=mutation):
                result, observations = self._run(p_mutant=mutation)
                self._assert_sb_positive(result)
                self.assertNotEqual(result.returncode, 0, result.stderr)
                self.assertNotEqual(result.returncode, POST_IDENTITY_SENTINEL)
                for reached in (
                    "build-ok",
                    "venv",
                    "install",
                    "public-import",
                    "native-import",
                ):
                    self.assertIn(reached, observations, observations)
                self.assertNotIn("namespace-before-positive-rejection", observations)


if __name__ == "__main__":
    unittest.main()
