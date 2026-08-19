from __future__ import annotations

import hashlib
import os
import shutil
import stat
import subprocess
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path

from fixture import REPOSITORY, SOURCE_SHA

BENIGN = b"---\ntitle: Invalid math sentinel\n---\n\n$$\n\\frac{1}{2}\n$$\n"
INVALID = b"---\ntitle: Invalid math sentinel\n---\n\n$$\n\\frac{\n$$\n"
WRONG_ERROR = b"---\ntitle: [invalid\n---\n\n$$\n\\frac{1}{2}\n$$\n"
BENIGN_SHA256 = "4372c659870165a44803413edae41758254624a5ad2b9cff8bfc9622c878fc50"
INVALID_SHA256 = "7abc88da255ffa6835ac0bbff900a0865958671862b198fa6f729ffa508a3e8b"
DIAGNOSTIC = (
    "KaTeX parse error: Unexpected end of input in a macro argument, expected '}' "
    "at end of input: \\frac{"
)
ACCEPTED = Path("docs/site/src/content/docs/invalid-math-sentinel.mdx")
FORBIDDEN = Path("docs/site/src/content/docs/__invalid_math_sentinel__.mdx")
ROUTE = Path("invalid-math-sentinel/index.html")
MIB = 1_048_576


@dataclass(frozen=True)
class Build:
    status: int
    log: bytes
    source: Path
    output: Path
    publication: Path
    pristine: dict[str, tuple[object, ...]]


def _sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _manifest(root: Path) -> dict[str, tuple[object, ...]]:
    manifest: dict[str, tuple[object, ...]] = {}
    for directory, names, files in os.walk(root):
        relative = Path(directory).relative_to(root)
        if relative == Path("docs/site") and "node_modules" in names:
            names.remove("node_modules")
        if ".astro" in names:
            names.remove(".astro")
        for name in sorted(files):
            path, key = Path(directory, name), (relative / name).as_posix()
            if path.is_symlink():
                manifest[key] = ("symlink", os.readlink(path))
                continue
            data = path.read_bytes()
            manifest[key] = (
                "file",
                stat.S_IMODE(path.stat().st_mode),
                len(data),
                _sha256(data),
            )
    return manifest


def _copy_source(destination: Path) -> None:
    def ignore(directory: str, names: list[str]) -> set[str]:
        relative = Path(directory).relative_to(REPOSITORY)
        omitted = {".git"} if relative == Path(".") else set()
        if relative == Path("docs/site"):
            omitted.update({"node_modules", ".astro"})
        return omitted.intersection(names)

    shutil.copytree(
        REPOSITORY,
        destination,
        symlinks=True,
        copy_function=os.link,
        ignore=ignore,
    )
    supply = REPOSITORY / "docs/site/node_modules"
    if not supply.is_dir() or supply.is_symlink():
        raise AssertionError("locked docs/site/node_modules supply must be a real directory")
    shutil.copytree(
        supply,
        destination / "docs/site/node_modules",
        symlinks=True,
        copy_function=os.link,
    )


def _preflight(source: Path, output: Path, cache: Path) -> None:
    site = source / "docs/site"
    for path in (site / ".astro", site / ACCEPTED.relative_to("docs/site"), site / FORBIDDEN.relative_to("docs/site")):
        if path.exists() or path.is_symlink():
            raise AssertionError(f"stale discovery input: {path}")
    for path in (output, cache):
        if path.exists():
            if path.is_symlink() or not path.is_dir() or any(path.iterdir()):
                raise AssertionError(f"reused probe output/cache: {path}")


def _targeted_log(status: int, raw: bytes, publication: Path | None = None) -> bool:
    if publication is not None and (publication.exists() or publication.is_symlink()):
        return False
    if status != 1 or not 1 <= len(raw) <= MIB or b"\0" in raw or b"\r" in raw:
        return False
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        return False
    return text.split("\n").count(DIAGNOSTIC) == 1 and DIAGNOSTIC + "\n" in text


class InvalidMathOracleTests(unittest.TestCase):
    def _build(self, root: Path, relative: Path | None, content: bytes | None) -> Build:
        source, output, cache = root / "source", root / "output", root / "npm-cache"
        publication = root / "published"
        _copy_source(source)
        _preflight(source, output, cache)
        pristine = _manifest(source)
        if relative is not None:
            target = source / relative
            target.write_bytes(content if content is not None else b"")
            target.chmod(0o644)
        environment = os.environ.copy()
        environment.update(
            {
                "LC_ALL": "C",
                "TZ": "UTC",
                "npm_config_offline": "true",
                "npm_config_cache": str(cache),
                "EQIORA_SITE_BUILD_PROFILE": "complete",
                "EQIORA_SITE_SOURCE_SHA": environment.get("EQIORA_SITE_SOURCE_SHA", SOURCE_SHA),
                "EQIORA_SITE_CARGO_VERSION": environment.get("EQIORA_SITE_CARGO_VERSION", "0.1.0-alpha.1"),
                "EQIORA_SITE_PYTHON_VERSION": environment.get("EQIORA_SITE_PYTHON_VERSION", "0.1.0a1"),
                "EQIORA_SITE_ASTRO_OUT_DIR": str(output),
            }
        )
        result = subprocess.run(
            ["npm", "--prefix", str(source / "docs/site"), "run", "build"],
            cwd=source,
            check=False,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        log_path = root / "build.log"
        log_path.write_bytes(result.stdout)
        self.assertTrue(log_path.is_file())
        self.assertFalse(log_path.is_symlink())
        return Build(result.returncode, result.stdout, source, output, publication, pristine)

    def _assert_sole_sentinel(self, build: Build, expected: bytes, relative: Path = ACCEPTED) -> None:
        current = _manifest(build.source)
        added = set(current) - set(build.pristine)
        removed = set(build.pristine) - set(current)
        changed = {path for path in current.keys() & build.pristine if current[path] != build.pristine[path]}
        self.assertEqual((added, removed, changed), ({relative.as_posix()}, set(), set()))
        self.assertEqual(
            current[relative.as_posix()],
            ("file", 0o644, len(expected), _sha256(expected)),
        )

    def test_00_real_provider_positive_then_causal_falsifiers(self) -> None:
        self.assertEqual(subprocess.check_output(["node", "--version"], text=True).strip(), "v24.18.1")
        self.assertEqual(subprocess.check_output(["npm", "--version"], text=True).strip(), "11.16.0")
        self.assertEqual((len(BENIGN), _sha256(BENIGN)), (56, BENIGN_SHA256))
        self.assertEqual((len(INVALID), _sha256(INVALID)), (51, INVALID_SHA256))
        self.assertEqual(BENIGN.count(b"1}{2}"), 1)
        self.assertEqual(BENIGN.replace(b"1}{2}", b"", 1), INVALID)
        self.assertEqual(BENIGN.splitlines().count(b"\\frac{"), 0)
        self.assertEqual(INVALID.splitlines().count(b"\\frac{"), 1)
        scratch_root = Path.home() / ".cache/eqiora/invalid-math-oracle"
        scratch_root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="probe.", dir=scratch_root) as temporary:
            root = Path(temporary)
            ordinary = self._build(root / "00-ordinary", None, None)
            self.assertEqual(ordinary.status, 0, ordinary.log.decode("utf-8", "replace"))
            self.assertEqual(_manifest(ordinary.source), ordinary.pristine)
            self.assertFalse((ordinary.output / ROUTE).exists())

            benign = self._build(root / "01-benign", ACCEPTED, BENIGN)
            self.assertEqual(benign.status, 0, benign.log.decode("utf-8", "replace"))
            self._assert_sole_sentinel(benign, BENIGN)
            html = (benign.output / ROUTE).read_text(encoding="utf-8")
            for marker in ('class="katex-display"', 'class="katex-mathml"', 'class="katex-html"', '<math', 'display="block"'):
                self.assertEqual(html.count(marker), 1, marker)

            invalid = self._build(root / "02-invalid", ACCEPTED, INVALID)
            self._assert_sole_sentinel(invalid, INVALID)
            self.assertTrue(_targeted_log(invalid.status, invalid.log, invalid.publication))
            self.assertFalse(invalid.publication.exists())
            benign_manifest, invalid_manifest = _manifest(benign.source), _manifest(invalid.source)
            self.assertEqual(
                {path for path in benign_manifest if benign_manifest[path] != invalid_manifest[path]},
                {ACCEPTED.as_posix()},
            )

            ignored = self._build(root / "03-ignored", FORBIDDEN, INVALID)
            self.assertEqual(ignored.status, 0, ignored.log.decode("utf-8", "replace"))
            self._assert_sole_sentinel(ignored, INVALID, FORBIDDEN)
            self.assertFalse((ignored.output / ROUTE).exists())

            missing = self._build(root / "04-missing", None, None)
            self.assertEqual(missing.status, 0, missing.log.decode("utf-8", "replace"))
            self.assertEqual(_manifest(missing.source), missing.pristine)
            self.assertFalse((missing.output / ROUTE).exists())

            wrong = self._build(root / "05-wrong-error", ACCEPTED, WRONG_ERROR)
            self.assertNotEqual(wrong.status, 0)
            self._assert_sole_sentinel(wrong, WRONG_ERROR)
            self.assertFalse(_targeted_log(wrong.status, wrong.log))
            self.assertIn(b"\\frac{1}{2}", WRONG_ERROR)

            extra = invalid.source / "invalid-math-unrelated-delta"
            extra.write_bytes(b"different source delta\n")
            with self.assertRaises(AssertionError):
                self._assert_sole_sentinel(invalid, INVALID)
            extra.unlink()
            (invalid.source / ACCEPTED).write_bytes(INVALID + b" ")
            with self.assertRaises(AssertionError):
                self._assert_sole_sentinel(invalid, INVALID)
            invalid.publication.mkdir()
            self.assertFalse(_targeted_log(invalid.status, invalid.log, invalid.publication))

            for label, status, log in (
                ("generic cooccurrence", 1, b"invalid-math-sentinel.mdx\nExpected KaTeX\n"),
                ("wrong exit", 2, (DIAGNOSTIC + "\n").encode()),
                ("duplicate line", 1, ((DIAGNOSTIC + "\n") * 2).encode()),
                ("not LF-delimited", 1, ("prefix " + DIAGNOSTIC + " suffix\n").encode()),
                ("not LF-terminated", 1, DIAGNOSTIC.encode()),
                ("NUL", 1, (DIAGNOSTIC + "\n\0").encode()),
                ("invalid UTF-8", 1, (DIAGNOSTIC + "\n").encode() + b"\xff"),
                ("oversize", 1, (DIAGNOSTIC + "\n").encode() + b"x" * MIB),
            ):
                with self.subTest(label=label):
                    self.assertFalse(_targeted_log(status, log))

            for label, mutate in (
                ("astro", lambda source, output, cache: (source / "docs/site/.astro").mkdir()),
                ("output", lambda source, output, cache: (output.mkdir(), (output / "stale").touch())),
                ("cache", lambda source, output, cache: (cache.mkdir(), (cache / "stale").touch())),
            ):
                with self.subTest(label=label):
                    stale_root = root / f"stale-{label}"
                    source, output, cache = stale_root / "source", stale_root / "output", stale_root / "cache"
                    _copy_source(source)
                    mutate(source, output, cache)
                    with self.assertRaises(AssertionError):
                        _preflight(source, output, cache)

    def test_99_runner_binds_the_admitted_nonunderscore_path(self) -> None:
        runner = (REPOSITORY / "tools/site/run_offline_site_checks.sh").read_text(encoding="utf-8")
        self.assertTrue(ACCEPTED.name in runner, "runner remains bound to the ignored basename")
        self.assertTrue(FORBIDDEN.name not in runner, "runner retains the ignored basename")

if __name__ == "__main__":
    unittest.main()
