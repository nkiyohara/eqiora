#!/usr/bin/env python3
"""Verify and serve the bounded Eqiora static-site artifact without network access."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import mimetypes
import os
import re
import stat
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path, PurePosixPath
from typing import Iterable
from urllib.parse import unquote, urlsplit
from xml.etree import ElementTree

try:
    import tools.site.check_site_html as _site_html
except ModuleNotFoundError as error:
    if error.name not in {"tools", "tools.site", "tools.site.check_site_html"}:
        raise
    import check_site_html as _site_html

_EXPECTED_HTML_HELPER = Path(__file__).with_name("check_site_html.py").resolve()
if Path(_site_html.__file__ or "").resolve() != _EXPECTED_HTML_HELPER:
    raise ImportError("site checker HTML observer did not resolve to its exact sibling")
if _site_html.__all__ != ("HtmlInspection", "normalize", "read_html"):
    raise ImportError("site checker HTML observer exposes an unexpected interface")
HtmlInspection = _site_html.HtmlInspection
normalize = _site_html.normalize
read_html = _site_html.read_html

SITE_ORIGIN = "https://eqiora.org"
SOURCE_SHA = re.compile(r"^[0-9a-f]{40}$")
ACTION_USE = re.compile(r"^\s*uses:\s*([^@\s]+)@([^\s#]+)", re.MULTILINE)
MARKDOWN_LINK = re.compile(r"!?\[[^\]]*]\(([^)\n]+)\)")
HTML_REFERENCE = re.compile(r"""(?:href|src)=["']([^"']+)["']""", re.IGNORECASE)
CSS_URL = re.compile(r"url\(\s*([\"']?)([^\"')]+)\1\s*\)", re.IGNORECASE)

# These are verifier read bounds, not product-performance claims.
MAX_FILES = 20_000
MAX_FILE_BYTES = 32 * 1024 * 1024
MAX_TOTAL_BYTES = 512 * 1024 * 1024
MAX_HTML_BYTES = 4 * 1024 * 1024
PRESSURE_SHA256 = "5e9a694b4a6620d5548f259875b1a9dea1637c37798aa1c1b8b2ab53cb314376"
PUBLICATION_SHA256 = "db88a9a60926f52fc34b4106a29137b2fd8afbd5cc83b4eb797619432a744d33"
SOCIAL_SHA256 = "26c3987ad5e0e7b094100ce670d42062c51329a71f2859ddc0ccdfb8a21a0329"
FAVICON_SHA256 = "6c7ae182102b29ed48281c56434f4d57fe37117dc7df3fa0de18fd79215c9598"
APPLE_TOUCH_SHA256 = "3f7349745502fc3b6f09b79dc989ef6d5d2c820b7300e61819aeb3da44803169"
OLD_SOCIAL_SHA256 = "3b9be694357a6db29674e82eabfdb63738d0e40bf70b3f00163737b490b9128b"
OLD_SOCIAL_LINE = "Open-source computational engineering · Alpha 0.1.0a1"
PRESSURE_ALT = (
    "Pressure in pascals for the frozen 2D steady-Stokes exact-cylinder "
    "demonstration, shown with a viridis color scale and the 104-triangle "
    "affine mesh overlaid. Presentation image only; linked Result evidence "
    "carries the numerical claim."
)
PRESSURE_REVISION = "c6b7a21f52ae1acf941d26319d2499ed89152c15"
PRESSURE_CAPTION = (
    "Pressure (Pa), frozen exact-cylinder steady-Stokes demonstration at "
    f"{PRESSURE_REVISION}; presentation only, not validation."
)


@dataclass(frozen=True)
class SiteIdentities:
    """Exact admitted input identities. CLI callers cannot replace these."""

    pressure: str = PRESSURE_SHA256
    publication: str = PUBLICATION_SHA256
    social: str = SOCIAL_SHA256
    favicon: str = FAVICON_SHA256
    apple_touch: str = APPLE_TOUCH_SHA256


@dataclass(frozen=True)
class ReleaseIdentity:
    cargo: str
    python: str


PRODUCTION_IDENTITIES = SiteIdentities()
ROUTES = {
    "/": "index.html",
    "/gallery/": "gallery/index.html",
    "/gallery/exact-cylinder-steady-stokes/": (
        "gallery/exact-cylinder-steady-stokes/index.html"
    ),
    "/reference/": "reference/index.html",
    "/reference/python/eqiora/": "reference/python/eqiora/index.html",
    "/reference/rust/": "reference/rust/index.html",
    "/reference/rust/api/eqiora/struct.Diagnostic.html": (
        "reference/rust/api/eqiora/struct.Diagnostic.html"
    ),
    "/reference/cli/": "reference/cli/index.html",
    "/reference/control-v2/": "reference/control-v2/index.html",
    "/reference/mcp/": "reference/mcp/index.html",
    "/examples/": "examples/index.html",
    "/404.html": "404.html",
}
SITEMAP_ROUTES = tuple(
    route for route in ROUTES if "rust/api" not in route and route != "/404.html"
)
TOP_NAV = (
    ("Docs", "/get-started/"),
    ("Gallery", "/gallery/"),
    ("Reference", "/reference/"),
    ("Evidence", "/evidence/"),
    ("GitHub", "https://github.com/nkiyohara/eqiora"),
)
STAGES = (
    "Problem setup",
    "Eqiora model definition",
    "Mesh and boundaries",
    "Submit and result",
    "Pressure visualization",
    "Verified and not claimed",
)
HOME_COPY = (
    "Model meaning once. Realize it many ways.",
    "Eqiora is an open-source, meaning-first foundation for scientific modeling, simulation, differentiation, and execution.",
    "Its central boundary is simple:",
    "A model states typed mathematical relations. A realization chooses how those relations are discretized, solved, and executed.",
    "That separation lets block diagrams, acausal physical networks, PDE fields, hybrid dynamics, and reusable components share one canonical meaning without making a numerical method or hardware backend part of the model.",
    "Get started",
    "Explore gallery",
    "Featured walkthrough",
    "Exact-cylinder steady Stokes",
    "Follow one frozen 2D steady-Stokes problem from model definition and named boundaries through one submit/Result path to an independently admitted static pressure image.",
    "Python",
    "2D",
    "steady Stokes",
    "View the static walkthrough",
    "Docs",
    "Learn the Model–Realization boundary and start from bounded examples.",
    "Reference",
    "Browse exact-commit Python, Rust, CLI, control-v2, and MCP surfaces. API presence is not verification or maturity.",
    "Evidence",
    "Inspect the generated capability-to-case index and the manifests that own each bounded claim.",
    "{release_identity}",
    "Eqiora is alpha research software under active development. The capability matrix and generated evidence catalog bound what is currently supported; this site does not widen those claims.",
    "One source of truth",
    "This website is a curated projection, not a parallel specification. Detailed contracts remain in the repository's architecture, RFCs, capability matrix, and validated verify manifests.",
)
CASE_SOURCE_PATHS = (
    "examples/python/exact_cylinder_stokes_marimo.py",
    "examples/python/exact_cylinder_stokes.py",
    "examples/python/exact_cylinder_geometry.py",
    "examples/python/exact_cylinder_mesh.py",
    "verify/fluid/packaged-steady-stokes-2d/models/direct.eqi",
    "verify/fluid/packaged-steady-stokes-2d/package-v0.1.0/src/incompressible.eqi",
    "packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi",
)
CASE_EVIDENCE_PATHS = (
    "verify/artifacts/current-model-canonical-identity/README.md",
    "verify/fluid/packaged-steady-stokes-2d/README.md",
    "verify/fluid/exact-circular-hole-stokes-2d/README.md",
    "verify/geometry/exact-circular-hole-geometry/README.md",
    "verify/geometry/circular-hole-chordal-realization-binding/README.md",
    "verify/geometry/circular-hole-chordal-reference-mesh/README.md",
    "verify/interfaces/python-exact-circular-hole-geometry/README.md",
    "verify/interfaces/python-circular-hole-chordal-mesh/README.md",
    "verify/interfaces/python-exact-cylinder-stokes-result/README.md",
    "verify/interfaces/python-exact-cylinder-pressure-still/README.md",
    "verify/interfaces/python-exact-cylinder-stokes-marimo/README.md",
)
DIRECT_PINS = {
    "astro": "7.2.3",
    "@astrojs/starlight": "0.41.7",
    "@astrojs/mdx": "7.0.6",
    "@astrojs/markdown-satteri": "0.3.6",
    "satteri": "0.9.5",
    "katex": "0.18.4",
    "@playwright/test": "1.62.1",
    "@axe-core/playwright": "4.13.0",
}
RUNTIME_PINS = {
    name: version
    for name, version in DIRECT_PINS.items()
    if name not in {"@playwright/test", "@axe-core/playwright"}
}
DEVELOPMENT_PINS = {
    name: version for name, version in DIRECT_PINS.items() if name not in RUNTIME_PINS
}
ROOT_DEPENDENCY_SECTIONS = {
    "dependencies": RUNTIME_PINS,
    "devDependencies": DEVELOPMENT_PINS,
    "optionalDependencies": {},
    "peerDependencies": {},
}
FORBIDDEN_CLIENT_PACKAGES = {"react", "react-dom"}
PROVIDER_PATHS = (
    "docs/site/src/components/site/ExactSourceLink.astro",
    "docs/site/src/components/site/ReleaseIdentity.astro",
)
CURRENT_VERSION = re.compile(
    r"(?<![A-Za-z0-9_.-])(?:\d+\.\d+\.\d+-alpha\.\d+|\d+\.\d+\.\d+a\d+)(?![A-Za-z0-9_.-])"
)
CURRENT_VERSION_SOURCE_EXCEPTIONS = {
    "docs/site/src/content/docs/reference/cli/index.mdx",
    "docs/site/src/content/docs/reference/mcp/index.mdx",
}
EXECUTION_CONTROL_LABEL = re.compile(
    r"\b(?:run|submit|reset|start|begin|try|solv\w*|execut\w*|simulat\w*|comput\w*|calculat\w*|launch\w*|evaluat\w*|process\w*|generat\w*|analy[sz]\w*|predict\w*)\b",
    re.IGNORECASE,
)
REQUIRED_TRIGGER_PATTERNS = {
    ".gitattributes",
    ".github/workflows/pages.yml",
    ".cargo/config.toml",
    "CHANGELOG.md",
    "Cargo.lock",
    "Cargo.toml",
    "api/eqiora-facade-v1.json",
    "bindings/python/python/eqiora/**/*.py",
    "bindings/python/python/eqiora/**/*.pyi",
    "crates/**",
    "docs/capability-matrix.md",
    "docs/python/api.md",
    "docs/site/**",
    "docs/verification/gallery/README.md",
    "examples/python/exact_cylinder_geometry.py",
    "examples/python/exact_cylinder_mesh.py",
    "examples/python/exact_cylinder_stokes.py",
    "examples/python/exact_cylinder_stokes_marimo.py",
    "examples/steady-flow-past-cylinder.eqi",
    "examples/steady-flow-past-cylinder.geometry.json",
    "examples/steady-flow-past-cylinder.model.json",
    "mise.lock",
    "mise.toml",
    "mkdocs.yml",
    "packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi",
    "pyproject.toml",
    "rust-toolchain.toml",
    "schemas/control/compile-v2.schema.json",
    "tools/docs/**",
    "tools/release/python_candidate_common.py",
    "tools/site/**",
    "uv.lock",
    "verify/artifacts/current-model-canonical-identity/**",
    "verify/fluid/exact-circular-hole-stokes-2d/**",
    "verify/fluid/packaged-steady-stokes-2d/**",
    "verify/geometry/circular-hole-chordal-realization-binding/**",
    "verify/geometry/circular-hole-chordal-reference-mesh/**",
    "verify/geometry/exact-circular-hole-geometry/**",
    "verify/interfaces/cli-compile-check/**",
    "verify/interfaces/control-plane-compile-check/**",
    "verify/interfaces/mcp-stdio-compile-check/README.md",
    "verify/interfaces/mcp-stdio-compile-check/case.toml",
    "verify/interfaces/mcp-stdio-compile-check/**",
    "verify/interfaces/python-circular-hole-chordal-mesh/**",
    "verify/interfaces/python-exact-circular-hole-geometry/**",
    "verify/interfaces/python-exact-cylinder-pressure-still/**",
    "verify/interfaces/python-exact-cylinder-stokes-marimo/**",
    "verify/interfaces/python-exact-cylinder-stokes-result/**",
    "verify/interfaces/studio-exact-cylinder-stokes-demo/**",
    "verify/**",
}
TRIGGER_REPRESENTATIVES = {
    "archive attributes": ".gitattributes",
    "workflow": ".github/workflows/pages.yml",
    "site config": "docs/site/astro.config.mjs",
    "old social deletion": "docs/site/assets/social-card.svg",
    "timeless social target": "docs/site/public/social-card.svg",
    "admitted media": "docs/site/src/assets/gallery/exact-cylinder-pressure.png",
    "publication record": "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json",
    "Python stub": "bindings/python/python/eqiora/geometry.pyi",
    "Python runtime": "bindings/python/python/eqiora/geometry.py",
    "Rust facade": "crates/eqiora/src/lib.rs",
    "Rust provider": "crates/eqiora-core/src/diagnostic.rs",
    "facade inventory": "api/eqiora-facade-v1.json",
    "geometry snippet": "examples/python/exact_cylinder_geometry.py",
    "mesh snippet": "examples/python/exact_cylinder_mesh.py",
    "plain snippet": "examples/python/exact_cylinder_stokes.py",
    "Marimo snippet": "examples/python/exact_cylinder_stokes_marimo.py",
    "EQI owner": "examples/steady-flow-past-cylinder.eqi",
    "geometry owner": "examples/steady-flow-past-cylinder.geometry.json",
    "model owner": "examples/steady-flow-past-cylinder.model.json",
    "formula owner": "packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi",
    "gallery contract": "docs/verification/gallery/README.md",
    "schema": "schemas/control/compile-v2.schema.json",
    "MCP case": "verify/interfaces/mcp-stdio-compile-check/case.toml",
    "MCP prose": "verify/interfaces/mcp-stdio-compile-check/README.md",
    "capability matrix": "docs/capability-matrix.md",
    "uv lock": "uv.lock",
    "mise lock": "mise.lock",
    "Rust toolchain": "rust-toolchain.toml",
    "Cargo config": ".cargo/config.toml",
    "Cargo root": "Cargo.toml",
    "Cargo lock": "Cargo.lock",
    "site tooling": "tools/site/check_site.py",
    "docs tooling": "tools/docs/generate_python_api.py",
    "evidence": "verify/interfaces/python-exact-cylinder-stokes-result/case.toml",
    "changelog": "CHANGELOG.md",
}
FULL_CHROMIUM_VERSION_STDOUT_HEX = (
    "476f6f676c65204368726f6d6520666f722054657374696e67203135312e302e373932322e3334200a"
)
FULL_CHROMIUM_VERSION_STDOUT = bytes.fromhex(FULL_CHROMIUM_VERSION_STDOUT_HEX)
DIRECT_SOURCE_ARCHIVE_COMMAND = (
    'git archive --format=tar "$GITHUB_SHA" | tar -xf - -C "$scratch/source"'
)
EXACT_LINK_PAYLOAD_SHA256 = (
    "a54ff182c7e8acf56acfd6e4b9c3ff41e2c41a31c9b211b2deb9df75d9a478f9"
)
EXACT_TREE_LINK_COMMAND = (
    "git cat-file blob \"$GITHUB_SHA:CLAUDE.md\" | sha256sum | cut -d ' ' -f 1"
)
EXACT_EXTRACTED_LINK_COMMAND = (
    "readlink -n \"$scratch/source/CLAUDE.md\" | sha256sum | cut -d ' ' -f 1"
)
OFFLINE_WORKFLOW_TOKENS = (
    "ubuntu-24.04",
    "eqiora-pw-1.62.1-r1234",
    "playwright install --with-deps chromium",
    "chromium.executablePath()",
    "chromium-1234/chrome-linux64/chrome",
    'sha256sum "$browser_path"',
    'stat -c %s "$browser_path"',
    "EQIORA_SITE_BROWSER_SHA256=",
    "EQIORA_SITE_BROWSER_BYTES=",
    '--expected-executable-sha256 "$EQIORA_SITE_BROWSER_SHA256"',
    '--expected-executable-bytes "$EQIORA_SITE_BROWSER_BYTES"',
    FULL_CHROMIUM_VERSION_STDOUT_HEX,
    "check_site.py browser-supply",
    "unshare --net",
    "ip link set lo up",
    "setpriv",
    "npm_config_offline=true",
    "CARGO_NET_OFFLINE=true",
    "UV_OFFLINE=1",
    DIRECT_SOURCE_ARCHIVE_COMMAND,
    "EQIORA_SITE_SOURCE_ROOT=$scratch/source",
    'test "$(git rev-parse HEAD)" = "$GITHUB_SHA"',
    'git ls-tree -r "$GITHUB_SHA"',
    'git ls-tree "$GITHUB_SHA" -- AGENTS.md',
    EXACT_TREE_LINK_COMMAND,
    'git cat-file blob "$GITHUB_SHA:AGENTS.md"',
    EXACT_EXTRACTED_LINK_COMMAND,
    EXACT_LINK_PAYLOAD_SHA256,
    'cmp "$scratch/source/AGENTS.md" "$scratch/expected-AGENTS.md"',
    "120000",
    "100644",
    "AGENTS.md",
    "source_links=",
    'case "$source_links" in',
    'if test -n "$source_links"; then',
    'if test -L "$scratch/source/CLAUDE.md"; then',
)
FORBIDDEN_WORKFLOW_TOKENS = (
    "playwright install --with-deps --only-shell chromium",
    "HeadlessChrome 151.0.7922.34",
    "tar --exclude",
    "--dereference",
    "cp -L",
    "rsync -L",
    "tar -h",
    'rm "$scratch/source/CLAUDE.md"',
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def check_source_topology(
    root: Path, expected_agents_sha256: str | None = None
) -> list[str]:
    errors: list[str] = []
    if not root.is_dir() or root.is_symlink():
        return ["site source root must be a real directory"]
    resolved_root = root.resolve()
    pruned = root / "docs/site/node_modules"
    links: list[Path] = []
    for current, directories, filenames in os.walk(root, topdown=True):
        current_path = Path(current)
        kept_directories: list[str] = []
        for name in directories:
            candidate = current_path / name
            if candidate == pruned:
                continue
            if candidate.is_symlink():
                links.append(candidate)
            else:
                kept_directories.append(name)
        directories[:] = kept_directories
        links.extend(
            candidate
            for name in filenames
            if (candidate := current_path / name).is_symlink()
        )

    claude = root / "CLAUDE.md"
    if not links:
        return errors
    if links != [claude]:
        observed = sorted(str(path.relative_to(root)) for path in links)
        return [f"site source has unadmitted symlinks: {observed}"]
    try:
        payload = os.readlink(claude)
    except OSError as error:
        return [f"site source CLAUDE.md link is unreadable: {error}"]
    if payload != "AGENTS.md":
        errors.append("site source CLAUDE.md must contain exact link payload AGENTS.md")

    agents = root / "AGENTS.md"
    try:
        agents_mode = agents.lstat().st_mode
    except OSError as error:
        errors.append(f"site source AGENTS.md target is unavailable: {error}")
        return errors
    if agents.is_symlink() or not stat.S_ISREG(agents_mode):
        errors.append("site source AGENTS.md target must be a regular non-symlink file")
        return errors
    try:
        if agents.resolve(strict=True) != resolved_root / "AGENTS.md":
            errors.append("site source AGENTS.md target escapes the authenticated root")
    except OSError as error:
        errors.append(f"site source AGENTS.md target cannot be resolved: {error}")
    if expected_agents_sha256 is not None:
        if not re.fullmatch(r"[0-9a-f]{64}", expected_agents_sha256):
            errors.append("expected AGENTS.md SHA-256 is malformed")
        elif sha256(agents) != expected_agents_sha256:
            errors.append("site source AGENTS.md differs from the same-commit Git blob")
    return errors


def check_runner_source_topology_text(text: str) -> list[str]:
    errors: list[str] = []
    invocation = "check_site.py source-topology"
    if text.count(invocation) != 2:
        errors.append(
            "offline runner must check source topology exactly before and after"
        )
    if re.search(
        r'find\s+"\$EQIORA_SITE_SOURCE_ROOT"[\s\S]{0,256}-type l -print -quit',
        text,
    ):
        errors.append("offline runner retains blanket source-symlink rejection")
    return errors


def check_browser_supply(
    site_root: Path,
    browser_cache: Path,
    expected_executable_sha256: str,
    expected_executable_bytes: int,
    environment: dict[str, str] | None = None,
) -> list[str]:
    errors: list[str] = []
    if not re.fullmatch(r"[0-9a-f]{64}", expected_executable_sha256):
        errors.append("online full Chromium SHA-256 identity is malformed")
    if expected_executable_bytes <= 0:
        errors.append("online full Chromium byte identity is malformed")
    if not site_root.is_dir() or site_root.is_symlink():
        return ["Playwright site root must be a real directory"]
    if browser_cache.name != "eqiora-pw-1.62.1-r1234":
        errors.append("browser cache must use exact Playwright/revision identity")
    if not browser_cache.is_dir() or browser_cache.is_symlink():
        errors.append("browser cache must be a real directory")
    elif not browser_cache.is_absolute() or browser_cache.resolve() != browser_cache:
        errors.append("browser cache must be an absolute canonical path")

    try:
        lock = json.loads((site_root / "package-lock.json").read_text(encoding="utf-8"))
        packages = lock["packages"]
        browsers = json.loads(
            (site_root / "node_modules/playwright-core/browsers.json").read_text(
                encoding="utf-8"
            )
        )["browsers"]
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        return [*errors, f"locked Playwright metadata is unavailable: {error}"]
    if not isinstance(packages, dict) or not isinstance(browsers, list):
        return [*errors, "locked Playwright metadata has the wrong shape"]
    for package in ("@playwright/test", "playwright", "playwright-core"):
        entry = packages.get(f"node_modules/{package}")
        if not isinstance(entry, dict) or entry.get("version") != "1.62.1":
            errors.append(f"browser supply must retain locked {package} 1.62.1")
        try:
            installed = json.loads(
                (site_root / "node_modules" / package / "package.json").read_text(
                    encoding="utf-8"
                )
            )
        except (OSError, json.JSONDecodeError) as error:
            errors.append(f"installed {package} metadata is unavailable: {error}")
        else:
            if not isinstance(installed, dict) or installed.get("version") != "1.62.1":
                errors.append(f"browser supply must retain installed {package} 1.62.1")
    chromium = [
        entry
        for entry in browsers
        if isinstance(entry, dict) and entry.get("name") == "chromium"
    ]
    if len(chromium) != 1 or (
        chromium[0].get("revision"),
        chromium[0].get("browserVersion"),
    ) != ("1234", "151.0.7922.34"):
        errors.append("browser supply metadata must select Chromium 1234/151.0.7922.34")

    supplied_environment = dict(os.environ if environment is None else environment)
    supplied_environment["PLAYWRIGHT_BROWSERS_PATH"] = str(browser_cache)
    try:
        resolved = subprocess.run(
            [
                "node",
                "-e",
                "const {chromium}=require('playwright');"
                "process.stdout.write(chromium.executablePath())",
            ],
            cwd=site_root,
            check=False,
            env=supplied_environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return [*errors, f"locked Playwright executable resolution failed: {error}"]
    expected = browser_cache / "chromium-1234/chrome-linux64/chrome"
    if (
        resolved.returncode != 0
        or resolved.stderr
        or resolved.stdout != os.fsencode(expected)
    ):
        errors.append("locked Playwright did not resolve the exact full Chromium path")

    executable = Path(os.fsdecode(resolved.stdout)) if resolved.stdout else expected
    try:
        mode = executable.lstat().st_mode
    except OSError as error:
        return [*errors, f"full Chromium executable is unavailable: {error}"]
    if executable.is_symlink() or not stat.S_ISREG(mode):
        errors.append("full Chromium executable must be a regular non-symlink file")
        return errors
    if not os.access(executable, os.X_OK):
        errors.append("full Chromium executable is not executable")
    if executable.stat().st_size != expected_executable_bytes:
        errors.append("full Chromium byte length changed after online verification")
    if sha256(executable) != expected_executable_sha256:
        errors.append("full Chromium SHA-256 changed after online verification")
    try:
        with executable.open("rb") as browser:
            magic = browser.read(4)
        if magic != b"\x7fELF":
            errors.append(
                "full Chromium executable must be the acquired binary, not a shim"
            )
    except OSError as error:
        errors.append(f"full Chromium executable cannot be read: {error}")
        return errors
    if errors:
        return errors
    version_environment = {**supplied_environment, "LC_ALL": "C", "TZ": "UTC"}
    try:
        version = subprocess.run(
            [str(executable), "--version"],
            check=False,
            env=version_environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        errors.append(f"full Chromium version observation failed: {error}")
        return errors
    if (
        version.returncode != 0
        or version.stderr
        or version.stdout != FULL_CHROMIUM_VERSION_STDOUT
    ):
        errors.append("full Chromium must emit the exact 41-byte locked version stdout")
    return errors


def check_runner_browser_supply_text(text: str) -> list[str]:
    errors: list[str] = []
    if text.count("check_site.py browser-supply") != 1:
        errors.append("offline runner must verify the locked full browser exactly once")
    for token in (
        '--expected-executable-sha256 "$EQIORA_SITE_BROWSER_SHA256"',
        '--expected-executable-bytes "$EQIORA_SITE_BROWSER_BYTES"',
    ):
        if token not in text:
            errors.append(f"offline runner omits online browser identity {token!r}")
    if "HeadlessChrome 151.0.7922.34" in text:
        errors.append("offline runner retains obsolete headless-shell identity")
    return errors


def _destination(raw: str) -> str:
    destination = raw.strip()
    if destination.startswith("<") and ">" in destination:
        return destination[1 : destination.index(">")]
    return destination.split(maxsplit=1)[0]


def check_markdown_links(site_root: Path) -> list[str]:
    """Retained source-link helper used by the pre-successor unit suite."""

    errors: list[str] = []
    resolved_site = site_root.resolve()
    for document in sorted(site_root.rglob("*.md")):
        text = document.read_text(encoding="utf-8")
        destinations = [
            _destination(match.group(1)) for match in MARKDOWN_LINK.finditer(text)
        ]
        destinations.extend(match.group(1) for match in HTML_REFERENCE.finditer(text))
        for destination in destinations:
            if not destination or destination.startswith("#"):
                continue
            parsed = urlsplit(destination)
            if parsed.scheme:
                if parsed.scheme not in {"https", "mailto"}:
                    errors.append(
                        f"{document}: unsupported link scheme {parsed.scheme!r}"
                    )
                continue
            if destination.startswith("//"):
                errors.append(f"{document}: scheme-relative URL is not allowed")
                continue
            path_text = unquote(parsed.path)
            if not path_text:
                continue
            candidate = (
                site_root / path_text.lstrip("/")
                if path_text.startswith("/")
                else document.parent / path_text
            )
            candidate = candidate.resolve()
            try:
                candidate.relative_to(resolved_site)
            except ValueError:
                errors.append(
                    f"{document}: local link escapes docs/site: {destination}"
                )
                continue
            if candidate.is_dir():
                candidate = candidate / "index.md"
            elif candidate.suffix == "":
                candidate = candidate.with_suffix(".md")
            if not candidate.exists():
                errors.append(f"{document}: missing local link target: {destination}")
    return errors


def _ordered(text: str, fragments: Iterable[str], label: str) -> list[str]:
    errors: list[str] = []
    position = 0
    for fragment in fragments:
        found = text.find(normalize(fragment), position)
        if found < 0:
            errors.append(f"{label}: missing or out-of-order visible text {fragment!r}")
        else:
            position = found + len(normalize(fragment))
    return errors


def _route_file(artifact: Path, route: str) -> Path | None:
    relative = route.lstrip("/")
    if not relative:
        relative = "index.html"
    elif route.endswith("/"):
        relative += "index.html"
    candidate = artifact / relative
    return candidate if candidate.is_file() else None


def _local_reference(
    artifact: Path, page: Path, reference: str
) -> tuple[Path | None, str]:
    parsed = urlsplit(reference)
    if parsed.scheme or reference.startswith("//"):
        if (
            parsed.scheme in {"http", "https"}
            and f"{parsed.scheme}://{parsed.netloc}" == SITE_ORIGIN
        ):
            raw_path = unquote(parsed.path)
        else:
            return None, parsed.fragment
    else:
        raw_path = unquote(parsed.path)
    if not raw_path:
        return page, parsed.fragment
    target = (
        artifact / raw_path.lstrip("/")
        if raw_path.startswith("/")
        else page.parent / raw_path
    )
    target = target.resolve()
    try:
        target.relative_to(artifact.resolve())
    except ValueError:
        return Path("/__escape__"), parsed.fragment
    if target.is_dir():
        target /= "index.html"
    elif target.suffix == "" and not target.exists():
        target /= "index.html"
    return target, parsed.fragment


def _runtime_reference(tag: str, attribute: str, value: str) -> bool:
    if tag in {
        "script",
        "img",
        "source",
        "iframe",
        "video",
        "audio",
        "track",
        "embed",
        "object",
    }:
        return attribute in {"src", "srcset", "poster", "data"}
    if tag == "link" and attribute == "href":
        return True
    if tag == "form" and attribute == "action":
        return True
    return False


def _artifact_inventory(artifact: Path) -> tuple[list[Path], list[str]]:
    files: list[Path] = []
    errors: list[str] = []
    total = 0
    if not artifact.is_dir() or artifact.is_symlink():
        return [], [f"artifact must be a real directory: {artifact}"]
    for current, directories, names in os.walk(artifact, followlinks=False):
        directories.sort()
        names.sort()
        for name in [*directories, *names]:
            path = Path(current) / name
            details = path.lstat()
            if stat.S_ISLNK(details.st_mode):
                errors.append(
                    f"artifact contains symlink: {path.relative_to(artifact)}"
                )
        for name in names:
            path = Path(current) / name
            details = path.lstat()
            if not stat.S_ISREG(details.st_mode):
                errors.append(
                    f"artifact contains non-regular file: {path.relative_to(artifact)}"
                )
                continue
            if details.st_size > MAX_FILE_BYTES:
                errors.append(
                    f"artifact file exceeds read cap: {path.relative_to(artifact)}"
                )
            total += details.st_size
            files.append(path)
            if len(files) > MAX_FILES:
                errors.append(f"artifact exceeds {MAX_FILES} files")
                return files, errors
            if total > MAX_TOTAL_BYTES:
                errors.append(f"artifact exceeds {MAX_TOTAL_BYTES} bytes")
                return files, errors
    return files, errors


def _exact_source(path: Path, expected: str, label: str) -> list[str]:
    if not path.is_file() or path.is_symlink():
        return [f"missing exact {label}: {path}"]
    observed = sha256(path)
    return (
        []
        if observed == expected
        else [f"{label} digest mismatch: expected {expected}, got {observed}"]
    )


def derive_release_identity(
    root: Path,
) -> tuple[ReleaseIdentity | None, list[str]]:
    cargo_path = root / "Cargo.toml"
    mapper_path = root / "tools/release/python_candidate_common.py"
    if (
        not cargo_path.is_file()
        or cargo_path.is_symlink()
        or not mapper_path.is_file()
        or mapper_path.is_symlink()
    ):
        return None, ["Cargo version or accepted Python release mapper is absent"]
    try:
        cargo_document = tomllib.loads(cargo_path.read_text(encoding="utf-8"))
        cargo_version = cargo_document["workspace"]["package"]["version"]
        module_name = (
            "_eqiora_site_release_mapper_"
            + hashlib.sha256(str(mapper_path).encode("utf-8")).hexdigest()
        )
        specification = importlib.util.spec_from_file_location(module_name, mapper_path)
        if specification is None or specification.loader is None:
            raise ValueError("release mapper cannot be loaded")
        module = importlib.util.module_from_spec(specification)
        sys.modules[module_name] = module
        previous_bytecode_policy = sys.dont_write_bytecode
        sys.dont_write_bytecode = True
        try:
            specification.loader.exec_module(module)
        finally:
            sys.dont_write_bytecode = previous_bytecode_policy
        python_version = module.python_distribution_version(cargo_version)
    except Exception as error:  # noqa: BLE001 - a source mapper failure is a denial
        return None, [f"release identity derivation failed: {error}"]
    finally:
        if "module_name" in locals():
            sys.modules.pop(module_name, None)
    if not isinstance(cargo_version, str) or not re.fullmatch(
        r"\d+\.\d+\.\d+-alpha\.\d+", cargo_version
    ):
        return None, ["Cargo version is not the required alpha SemVer identity"]
    if not isinstance(python_version, str) or not re.fullmatch(
        r"\d+\.\d+\.\d+a\d+", python_version
    ):
        return None, ["Python mapper did not produce the required alpha identity"]
    return ReleaseIdentity(cargo_version, python_version), []


def _workflow_paths(text: str, event: str) -> list[str]:
    lines = text.splitlines()
    event_indent = None
    paths_indent = None
    result: list[str] = []
    in_event = False
    for line in lines:
        stripped = line.strip()
        indent = len(line) - len(line.lstrip())
        if stripped == f"{event}:" and indent == 2:
            in_event = True
            event_indent = indent
            continue
        if in_event and stripped and indent <= (event_indent or 0):
            break
        if in_event and stripped == "paths:":
            paths_indent = indent
            continue
        if paths_indent is not None:
            if stripped and indent <= paths_indent:
                break
            match = re.match(r"\s*-\s+[\"']?([^\"']+?)[\"']?\s*$", line)
            if match:
                result.append(match.group(1))
    return result


def _glob_regex(pattern: str) -> re.Pattern[str]:
    output = ""
    offset = 0
    while offset < len(pattern):
        if pattern.startswith("**/", offset):
            output += "(?:.*/)?"
            offset += 3
        elif pattern.startswith("**", offset):
            output += ".*"
            offset += 2
        elif pattern[offset] == "*":
            output += "[^/]*"
            offset += 1
        elif pattern[offset] == "?":
            output += "[^/]"
            offset += 1
        else:
            output += re.escape(pattern[offset])
            offset += 1
    return re.compile(f"^{output}$")


def selected_by_paths(patterns: Iterable[str], changed_path: str) -> bool:
    return any(_glob_regex(pattern).fullmatch(changed_path) for pattern in patterns)


def check_workflow_text(text: str) -> list[str]:
    errors: list[str] = []
    pull = _workflow_paths(text, "pull_request")
    push = _workflow_paths(text, "push")
    if not pull or not push:
        errors.append(
            "Pages workflow must define PR and protected-base push path filters"
        )
        return errors
    if pull != push:
        errors.append("Pages PR and push path filters differ")
    if len(pull) != len(set(pull)):
        errors.append("Pages path filters contain duplicates")
    missing = sorted(REQUIRED_TRIGGER_PATTERNS - set(pull))
    if missing:
        errors.append(f"Pages path filters omit exact authorities: {missing}")
    for label, changed in TRIGGER_REPRESENTATIVES.items():
        if changed == ".gitattributes" and changed in missing:
            continue
        if not selected_by_paths(pull, changed):
            errors.append(f"Pages does not select representative {label}: {changed}")
    if selected_by_paths(pull, "notes/unrelated.txt"):
        errors.append("Pages selects a docs-unrelated negative path")
    for action, revision in ACTION_USE.findall(text):
        if not re.fullmatch(r"[0-9a-f]{40}", revision):
            errors.append(f"Pages action {action} is not pinned to a full SHA")
    for token in OFFLINE_WORKFLOW_TOKENS:
        if token not in text:
            errors.append(f"Pages workflow omits offline/supply boundary {token!r}")
    for token in FORBIDDEN_WORKFLOW_TOKENS:
        if token in text:
            errors.append(
                f"Pages workflow uses forbidden supply substitution {token!r}"
            )
    archive = text.find(DIRECT_SOURCE_ARCHIVE_COMMAND)
    source_export = text.find('echo "EQIORA_SITE_SOURCE_ROOT=', archive + 1)
    before_archive = (
        'git ls-tree -r "$GITHUB_SHA"',
        "source_links=",
        'case "$source_links" in',
        'if test -n "$source_links"; then',
        EXACT_TREE_LINK_COMMAND,
        'git ls-tree "$GITHUB_SHA" -- AGENTS.md',
        'git cat-file blob "$GITHUB_SHA:AGENTS.md"',
    )
    after_archive = (
        'if test -n "$source_links"; then',
        '\n            test -L "$scratch/source/CLAUDE.md"\n',
        EXACT_EXTRACTED_LINK_COMMAND,
        'cmp "$scratch/source/AGENTS.md" "$scratch/expected-AGENTS.md"',
        'elif test -e "$scratch/source/CLAUDE.md" || test -L "$scratch/source/CLAUDE.md"; then',
    )
    before_positions = [text.find(token) for token in before_archive]
    after_positions = [text.find(token, archive + 1) for token in after_archive]
    archive_window = text[archive:source_export] if 0 <= archive < source_export else ""
    archive_bound = (
        archive >= 0
        and source_export > archive
        and all(0 <= position < archive for position in before_positions)
        and before_positions == sorted(before_positions)
        and all(archive < position < source_export for position in after_positions)
        and after_positions == sorted(after_positions)
        and 'unlink "$scratch/source/CLAUDE.md"' not in archive_window
        and 'rm -f "$scratch/source/CLAUDE.md"' not in archive_window
    )
    if not archive_bound:
        errors.append("Pages archive must bind the tracked link after extraction")
    browser_admission = (
        'expected_browser_sha256="0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71"',
        'expected_browser_bytes="290614600"',
        'browser_sha256="$(sha256sum "$browser_path" | cut -d \' \' -f 1)"',
        'browser_bytes="$(stat -c %s "$browser_path")"',
        'test "$browser_sha256" = "$expected_browser_sha256"',
        'test "$browser_bytes" = "$expected_browser_bytes"',
        'EQIORA_SITE_BROWSER_SHA256="$expected_browser_sha256"',
        'EQIORA_SITE_BROWSER_BYTES="$expected_browser_bytes"',
        "export EQIORA_SITE_BROWSER_SHA256 EQIORA_SITE_BROWSER_BYTES",
        "check_site.py browser-supply",
        'version_hex="$("$browser_path" --version | od -An -tx1 | tr -d \'[:space:]\')"',
        'test "$version_hex" = "$expected_browser_version_hex"',
    )
    browser_positions = [text.find(token) for token in browser_admission]
    if any(
        position < 0 for position in browser_positions
    ) or browser_positions != sorted(browser_positions):
        errors.append("Pages browser identity must precede execution and propagation")
    ordered = (
        'git ls-tree -r "$GITHUB_SHA"',
        DIRECT_SOURCE_ARCHIVE_COMMAND,
        EXACT_EXTRACTED_LINK_COMMAND,
        "playwright install --with-deps chromium",
        "chromium.executablePath()",
        FULL_CHROMIUM_VERSION_STDOUT_HEX,
        "check_site.py browser-supply",
        "unshare --net",
    )
    positions = [text.find(token) for token in ordered]
    if all(position >= 0 for position in positions) and positions != sorted(positions):
        errors.append("Pages archive/browser supply checks are out of causal order")
    return errors


def check_source(
    root: Path,
    identities: SiteIdentities = PRODUCTION_IDENTITIES,
    release_identity: ReleaseIdentity | None = None,
) -> list[str]:
    errors: list[str] = []
    if release_identity is None:
        release_identity, release_errors = derive_release_identity(root)
        errors.extend(release_errors)
    site = root / "docs/site"
    for obsolete in (root / "mkdocs.yml", site / "assets/social-card.svg"):
        if obsolete.exists() or obsolete.is_symlink():
            errors.append(
                f"obsolete successor source remains: {obsolete.relative_to(root)}"
            )
    errors.extend(
        _exact_source(
            site / "src/assets/gallery/exact-cylinder-pressure.png",
            identities.pressure,
            "admitted pressure media",
        )
    )
    errors.extend(
        _exact_source(
            site / "src/data/gallery/exact-cylinder-steady-stokes.publication.json",
            identities.publication,
            "admitted publication record",
        )
    )
    errors.extend(
        _exact_source(
            site / "public/social-card.svg", identities.social, "timeless social card"
        )
    )
    errors.extend(
        _exact_source(site / "public/favicon.svg", identities.favicon, "favicon")
    )
    errors.extend(
        _exact_source(
            site / "public/apple-touch-icon.png",
            identities.apple_touch,
            "apple touch icon",
        )
    )
    provider_requirements = {
        site / "src/components/site/ExactSourceLink.astro": (
            "EQIORA_SITE_SOURCE_SHA",
            "blob",
            "tree",
        ),
        site / "src/components/site/ReleaseIdentity.astro": (
            "EQIORA_SITE_CARGO_VERSION",
            "EQIORA_SITE_PYTHON_VERSION",
        ),
    }
    for provider, tokens in provider_requirements.items():
        try:
            if provider.is_symlink() or not provider.is_file():
                raise OSError("not a regular file")
            provider_text = provider.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"accepted provider is missing or invalid: {provider.relative_to(root)}: {error}"
            )
            continue
        for token in tokens:
            if token not in provider_text:
                errors.append(
                    f"accepted provider {provider.relative_to(root)} omits {token!r}"
                )

    provider_consumers = {
        site / "src/content/docs/index.mdx": (
            "@components/site/ExactSourceLink.astro",
            "@components/site/ReleaseIdentity.astro",
            "<ExactSourceLink",
            "<ReleaseIdentity",
        ),
        site / "src/content/docs/evidence/index.mdx": (
            "@components/site/ExactSourceLink.astro",
            "<ExactSourceLink",
        ),
        site / "astro.config.mjs": (
            "src/components/site/ExactSourceLink.astro",
            "src/components/site/ReleaseIdentity.astro",
        ),
    }
    for consumer, tokens in provider_consumers.items():
        try:
            if consumer.is_symlink() or not consumer.is_file():
                raise OSError("not a regular file")
            consumer_text = consumer.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"provider consumer is missing or invalid: {consumer.relative_to(root)}: {error}"
            )
            continue
        for token in tokens:
            if token not in consumer_text:
                errors.append(
                    f"provider consumer {consumer.relative_to(root)} omits {token!r}"
                )

    package = site / "package.json"
    lock = site / "package-lock.json"
    try:
        package_data = json.loads(package.read_text(encoding="utf-8"))
        lock_data = json.loads(lock.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        errors.append(f"site package/lock is missing or invalid: {error}")
    else:
        dependencies = package_data.get("dependencies")
        development = package_data.get("devDependencies")
        lock_packages = lock_data.get("packages", {})
        lock_root = lock_packages.get("", {})
        for label, document in (("package", package_data), ("lock root", lock_root)):
            for section, expected in ROOT_DEPENDENCY_SECTIONS.items():
                if document.get(section, {}) != expected:
                    errors.append(
                        f"site {label} {section} differs from the exact direct set"
                    )
        declared = {
            **(dependencies if isinstance(dependencies, dict) else {}),
            **(development if isinstance(development, dict) else {}),
        }
        for name, expected in DIRECT_PINS.items():
            if declared.get(name) != expected:
                errors.append(f"site package must pin {name} exactly to {expected}")
            entry = lock_packages.get(f"node_modules/{name}", {})
            if entry.get("version") != expected:
                errors.append(f"site lock must resolve {name} exactly to {expected}")
            if not isinstance(entry.get("integrity"), str) or not entry[
                "integrity"
            ].startswith("sha512-"):
                errors.append(
                    f"site lock lacks registry integrity for {name}@{expected}"
                )
        if package_data.get("engines") != {"node": "24.18.1", "npm": "11.16.0"}:
            errors.append("site package must pin Node 24.18.1 and npm 11.16.0")
        if lock_root.get("engines") != package_data.get("engines"):
            errors.append("site lock root engines differ from package.json")
        for package_path, entry in lock_packages.items():
            if re.search(r"(?:^|/)node_modules/(?:react|react-dom)$", package_path):
                errors.append(f"site lock realizes client framework {package_path!r}")
            for section in ROOT_DEPENDENCY_SECTIONS:
                edges = entry.get(section, {})
                if isinstance(edges, dict) and any(
                    name in FORBIDDEN_CLIENT_PACKAGES
                    or str(specification).startswith(("npm:react@", "npm:react-dom@"))
                    for name, specification in edges.items()
                ):
                    errors.append(
                        f"site lock {package_path!r} has forbidden {section} edge"
                    )

    workflow = root / ".github/workflows/pages.yml"
    try:
        workflow_text = workflow.read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"missing Pages workflow: {error}")
    else:
        errors.extend(check_workflow_text(workflow_text))

    runner = root / "tools/site/run_offline_site_checks.sh"
    try:
        runner_text = runner.read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"missing offline site runner: {error}")
    else:
        errors.extend(check_runner_source_topology_text(runner_text))
        errors.extend(check_runner_browser_supply_text(runner_text))
        runner_tokens = (
            "generate_interface_reference.py",
            "--repository",
            "--eqiora-binary",
            "--mcp-binary",
            "--check",
            "build_rust_reference.py",
            "--rustdoc-root",
            "--output",
            "generate_evidence_catalog.py",
            "check_site.py check",
            "check_site.py serve",
            "npm_config_offline",
            "CARGO_NET_OFFLINE",
            "UV_OFFLINE",
            "EQIORA_SITE_CARGO_VERSION",
            "EQIORA_SITE_PYTHON_VERSION",
            "tools.site.tests.test_site_tools",
        )
        for token in runner_tokens:
            if token not in runner_text:
                errors.append(
                    f"offline runner omits frozen orchestration token {token!r}"
                )

    source_files = [
        source
        for suffix in (
            "*.astro",
            "*.css",
            "*.js",
            "*.json",
            "*.md",
            "*.mdx",
            "*.mjs",
            "*.svg",
            "*.ts",
        )
        for source in (site / "src").rglob(suffix)
    ]
    for source in sorted(source_files):
        text = source.read_text(encoding="utf-8")
        relative = source.relative_to(root).as_posix()
        is_release_history = "release-notes" in source.relative_to(site).parts
        if not is_release_history and relative not in CURRENT_VERSION_SOURCE_EXCEPTIONS:
            for forbidden_version in CURRENT_VERSION.findall(text):
                errors.append(
                    f"site source hard-codes product version {forbidden_version!r}: "
                    f"{source.relative_to(root)}"
                )
            if OLD_SOCIAL_LINE in text:
                errors.append(
                    f"site source retains deprecated social-card copy: {source.relative_to(root)}"
                )
        if re.search(r"\bfetch\s*\(", text):
            errors.append(
                f"build/runtime content fetch is forbidden: {source.relative_to(root)}"
            )
        if re.search(r"(?:src|poster)\s*=\s*[\"']https?://", text):
            errors.append(
                f"external executable asset reference: {source.relative_to(root)}"
            )
    return errors


def check_artifact(
    artifact: Path,
    source_sha: str,
    expected_python_version: str,
    identities: SiteIdentities = PRODUCTION_IDENTITIES,
) -> list[str]:
    errors: list[str] = []
    artifact = artifact.resolve()
    if not SOURCE_SHA.fullmatch(source_sha):
        return ["source SHA must be exactly 40 lowercase hexadecimal characters"]
    files, inventory_errors = _artifact_inventory(artifact)
    errors.extend(inventory_errors)
    if inventory_errors:
        return errors
    file_digests = {path: sha256(path) for path in files}
    digest_paths: dict[str, list[Path]] = {}
    for path, digest in file_digests.items():
        digest_paths.setdefault(digest, []).append(path)

    exact_public = {
        "social-card.svg": identities.social,
        "favicon.svg": identities.favicon,
        "apple-touch-icon.png": identities.apple_touch,
    }
    for relative, expected in exact_public.items():
        path = artifact / relative
        if not path.is_file() or file_digests.get(path) != expected:
            errors.append(
                f"public asset {relative} does not have admitted digest {expected}"
            )
    if len(digest_paths.get(identities.social, [])) != 1:
        errors.append(
            "assembled site must expose exactly one timeless social-card byte identity"
        )
    if digest_paths.get(OLD_SOCIAL_SHA256):
        errors.append("assembled site exposes the deprecated social-card bytes")
    if len(digest_paths.get(identities.pressure, [])) != 1:
        errors.append("assembled site must expose exactly one admitted pressure image")
    for path in files:
        if path.stat().st_size <= MAX_HTML_BYTES and path.suffix.lower() in {
            ".html",
            ".svg",
            ".xml",
            ".json",
            ".txt",
            ".css",
            ".js",
        }:
            try:
                text = path.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                continue
            if OLD_SOCIAL_LINE in text:
                errors.append(
                    f"assembled site exposes deprecated social-card copy: {path.relative_to(artifact)}"
                )
            if "assets/social-card.svg" in text:
                errors.append(
                    f"assembled site references the legacy social-card route: {path.relative_to(artifact)}"
                )

    inspections: dict[Path, tuple[str, HtmlInspection]] = {}
    for path in files:
        if path.suffix.lower() != ".html":
            continue
        try:
            inspections[path] = read_html(path, MAX_HTML_BYTES)
        except (OSError, UnicodeDecodeError, ValueError) as error:
            errors.append(f"invalid HTML {path.relative_to(artifact)}: {error}")

    for route, relative in ROUTES.items():
        path = artifact / relative
        if not path.is_file():
            errors.append(f"missing required route {route}: {relative}")

    for route, relative in ROUTES.items():
        path = artifact / relative
        if path not in inspections or "rust/api" in route:
            continue
        _, page = inspections[path]
        canonicals = [
            link.get("href", "")
            for link in page.links
            if "canonical" in link.get("rel", "").split()
        ]
        expected = f"{SITE_ORIGIN}{route}"
        if canonicals != [expected]:
            errors.append(
                f"{route}: canonical must be exactly {expected!r}, got {canonicals!r}"
            )
        properties = {
            (meta.get("property") or meta.get("name"), meta.get("content"))
            for meta in page.metas
        }
        if ("og:image", f"{SITE_ORIGIN}/social-card.svg") not in properties:
            errors.append(f"{route}: missing exact same-origin Open Graph image")
        rels = {(link.get("rel", ""), link.get("href", "")) for link in page.links}
        if not any(
            "icon" in rel.split() and href == "/favicon.svg" for rel, href in rels
        ):
            errors.append(f"{route}: missing exact favicon link")
        if not any(
            "apple-touch-icon" in rel.split() and href == "/apple-touch-icon.png"
            for rel, href in rels
        ):
            errors.append(f"{route}: missing exact apple-touch-icon link")
        labels_and_hrefs = [(label, href) for href, label in page.anchors]
        cursor = 0
        for expected_link in TOP_NAV:
            try:
                cursor = labels_and_hrefs.index(expected_link, cursor) + 1
            except ValueError:
                errors.append(
                    f"{route}: top navigation omits or reorders {expected_link!r}"
                )
                break

    home_path = artifact / ROUTES["/"]
    if home_path in inspections:
        home = inspections[home_path][1]
        home_copy = tuple(
            fragment.format(release_identity=f"Alpha {expected_python_version}")
            for fragment in HOME_COPY
        )
        errors.extend(_ordered(home.visible_text, home_copy, "/"))
        featured_start = home.visible_text.find("Featured walkthrough")
        featured_end = home.visible_text.find("Docs", featured_start + 1)
        featured_text = home.visible_text[featured_start:featured_end].casefold()
        for widening in (
            "flagship",
            "validated flow",
            "production ready",
            "general solver",
            "all backends",
            "benchmark",
            "interactive",
            "run now",
        ):
            if widening in featured_text:
                errors.append(
                    f"/: featured walkthrough widens its claim with {widening!r}"
                )
        brand_marks = []
        featured_pressure = []
        for image in home.images:
            target, _ = _local_reference(artifact, home_path, image.get("src", ""))
            if (
                target is not None
                and file_digests.get(target) == identities.favicon
                and image.get("_ancestor_href") == "/"
            ):
                brand_marks.append(image)
            if (
                target is not None
                and file_digests.get(target) == identities.pressure
                and image.get("alt") == PRESSURE_ALT
            ):
                featured_pressure.append(image)
        if not brand_marks:
            errors.append("/: header does not link the exact Eqiora mark home")
        if ("/", "Eqiora") not in home.anchors:
            errors.append("/: header does not expose the visible Eqiora home link")
        if len(featured_pressure) != 1:
            errors.append(
                "/: featured walkthrough must expose the admitted pressure image with exact alt text"
            )

    case_path = artifact / ROUTES["/gallery/exact-cylinder-steady-stokes/"]
    if case_path in inspections:
        raw_case, case = inspections[case_path]
        stage_headings = [heading for _, heading in case.headings if heading in STAGES]
        if stage_headings != list(STAGES):
            errors.append(
                f"Cylinder route must expose six ordered semantic stages, got {stage_headings!r}"
            )
        math_block = any(
            display == "block" and wrapper for display, wrapper in case.math
        )
        math_inline = any(
            display != "block" and not wrapper for display, wrapper in case.math
        )
        if not math_block or not math_inline:
            errors.append(
                "Cylinder route must contain distinct block and inline MathML/KaTeX output"
            )
        if "katex-mathml" not in raw_case or "katex-html" not in raw_case:
            errors.append(
                "Cylinder route must retain both KaTeX MathML and HTML output"
            )
        if any(
            delimiter in case.visible_text
            for delimiter in ("$$", "\\[", "\\]", "\\(", "\\)")
        ):
            errors.append("Cylinder route exposes raw target math delimiters")
        public_claim = (
            "one frozen 2D steady incompressible Stokes exact-cylinder demonstration, "
            "rendered from its accepted public Result path and linked evidence."
        )
        if public_claim not in case.visible_text:
            errors.append("Cylinder route omits the exact bounded public claim")
        for fallback in (
            "Eqiora source form",
            "sigma(u,p) = 2 mu sym(grad(u)) - p I",
            "-div(sigma(u,p)) - grad(phi) = 0",
            "div(u) = 0",
        ):
            if fallback not in case.visible_text:
                errors.append(
                    f"Cylinder route omits readable math fallback {fallback!r}"
                )
        pressure_images = []
        for image in case.images:
            target, _ = _local_reference(artifact, case_path, image.get("src", ""))
            if target is not None and file_digests.get(target) == identities.pressure:
                pressure_images.append(image)
        if len(pressure_images) != 1 or pressure_images[0].get("alt") != PRESSURE_ALT:
            errors.append(
                "Cylinder route must expose the admitted pressure bytes once with exact alt text"
            )
        for href, label in case.anchors:
            if href.strip().casefold() in {"", "#", "javascript:void(0)"}:
                errors.append(
                    f"Cylinder route contains a fake link control labelled {label!r}"
                )
        for _, attrs, label in case.interactives:
            labelled = " ".join(
                normalize("".join(case.id_text.get(item, [])))
                for item in attrs.get("aria-labelledby", "").split()
            )
            accessible = normalize(
                labelled or attrs.get("aria-label") or label or attrs.get("title", "")
            )
            if any(
                EXECUTION_CONTROL_LABEL.search(item) for item in (label, accessible)
            ):
                errors.append(
                    f"Cylinder route contains an uncontracted execution control {(label, accessible)!r}"
                )
        if (
            PRESSURE_CAPTION not in case.visible_text
            or "Result evidence" not in case.visible_text
            or "Pressure-still presentation case" not in case.visible_text
        ):
            errors.append(
                "Cylinder route omits the exact admitted caption or its two visible evidence links"
            )
        required_boundary = (
            "no curved elements",
            "no mesh/PDE convergence",
            "no drag/lift coefficient, scaled or mesh-independent force, or DFG value",
            "no transient or Navier–Stokes behavior",
            "no vortex shedding",
            "no 3D",
            "no production mesher",
            "no performance claim",
            "no cross-platform byte reproducibility",
            "pixels are not validation",
            "all 104 vertices are on the boundary",
            "only the outlet midpoint velocity vertex is free",
            "API presence is neither verification nor maturity",
        )
        folded = case.visible_text.casefold()
        for phrase in required_boundary:
            if phrase.casefold() not in folded:
                errors.append(f"Cylinder claim boundary omits nonclaim {phrase!r}")
        hrefs = {href for href, _ in case.anchors}
        for relative in (*CASE_SOURCE_PATHS, *CASE_EVIDENCE_PATHS):
            expected = (
                f"https://github.com/nkiyohara/eqiora/blob/{source_sha}/{relative}"
            )
            if expected not in hrefs:
                errors.append(
                    f"Cylinder route omits exact-head source/evidence link {relative}"
                )

    reference_path = artifact / "reference/index.html"
    if reference_path in inspections:
        reference = inspections[reference_path][1]
        for phrase in (
            "Python",
            "Rust",
            "CLI",
            "control-v2",
            "MCP",
            "API presence is not verification or maturity.",
        ):
            if phrase not in reference.visible_text:
                errors.append(f"reference landing omits {phrase!r}")
    else:
        errors.append("missing required route /reference/")

    pagefind = artifact / "pagefind/pagefind.js"
    if not pagefind.is_file():
        errors.append("Pagefind JavaScript entry is missing")
    robots = artifact / "robots.txt"
    try:
        robots_text = robots.read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"robots.txt is missing: {error}")
    else:
        if (
            robots_text
            != "User-agent: *\nAllow: /\nSitemap: https://eqiora.org/sitemap-index.xml\n"
        ):
            errors.append("robots.txt differs from the exact public crawl boundary")
    sitemap = artifact / "sitemap-index.xml"
    try:
        sitemap_text = sitemap.read_text(encoding="utf-8")
        sitemap_urls = {
            node.text
            for node in ElementTree.fromstring(sitemap_text).iter()
            if node.tag.endswith("loc")
        }
    except (OSError, UnicodeDecodeError, ElementTree.ParseError) as error:
        errors.append(f"sitemap-index.xml is missing or invalid: {error}")
    else:
        for route in SITEMAP_ROUTES:
            if f"{SITE_ORIGIN}{route}" not in sitemap_urls:
                errors.append(f"sitemap omits required route {route}")

    parsed_html: dict[Path, HtmlInspection] = {
        path: value[1] for path, value in inspections.items()
    }
    for page_path, parser in sorted(parsed_html.items()):
        if parser.inline_handlers:
            errors.append(
                f"{page_path.relative_to(artifact)}: inline event handlers are forbidden"
            )
        if page_path == case_path and parser.forms:
            errors.append(
                f"{page_path.relative_to(artifact)}: forms imply an uncontracted case interaction"
            )
        for tag, _, value in parser.references:
            if not value or value.startswith(("data:", "mailto:", "tel:")):
                continue
            parsed = urlsplit(value)
            if parsed.scheme not in {"", "http", "https"} or value.startswith("//"):
                errors.append(
                    f"{page_path.relative_to(artifact)}: unsafe reference {value!r}"
                )
                continue
            if parsed.scheme and f"{parsed.scheme}://{parsed.netloc}" != SITE_ORIGIN:
                if re.match(
                    r"^/nkiyohara/eqiora/(?:blob|tree)/", parsed.path
                ) and not re.match(
                    rf"^https://github\.com/nkiyohara/eqiora/(?:blob|tree)/{source_sha}/",
                    value,
                ):
                    errors.append(
                        f"{page_path.relative_to(artifact)}: repository source link does not use the exact asserted SHA: {value!r}"
                    )
                if _runtime_reference(tag, _, value):
                    errors.append(
                        f"{page_path.relative_to(artifact)}: external runtime request {value!r}"
                    )
                continue
            target, fragment = _local_reference(artifact, page_path, value)
            if target is None:
                continue
            if target == Path("/__escape__") or not target.is_file():
                errors.append(
                    f"{page_path.relative_to(artifact)}: broken or escaping link {value!r}"
                )
                continue
            if fragment and target.suffix == ".html":
                target_parser = parsed_html.get(target)
                if target_parser is None:
                    try:
                        _, target_parser = read_html(target, MAX_HTML_BYTES)
                    except (OSError, UnicodeDecodeError, ValueError):
                        target_parser = None
                if (
                    target_parser is None
                    or unquote(fragment) not in target_parser.id_text
                ):
                    errors.append(
                        f"{page_path.relative_to(artifact)}: missing fragment target {value!r}"
                    )

    katex_woff2_count = 0
    for stylesheet in sorted(path for path in files if path.suffix == ".css"):
        try:
            css = stylesheet.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for match in CSS_URL.finditer(css):
            value = match.group(2)
            target, _ = _local_reference(artifact, stylesheet, value)
            if target is None or not target.is_file():
                errors.append(
                    f"{stylesheet.relative_to(artifact)}: CSS asset is external or missing: {value!r}"
                )
            elif target.suffix == ".woff2" and "katex" in css.casefold():
                katex_woff2_count += 1
    if katex_woff2_count == 0:
        errors.append("assembled KaTeX CSS exposes no resolvable local WOFF2 fonts")

    for path in files:
        if path.suffix.lower() not in {".html", ".css", ".js", ".xml", ".json"}:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        if (
            "/blob/main/" in text
            or "/tree/main/" in text
            or "EQIORA_SITE_SOURCE_SHA" in text
        ):
            errors.append(
                f"{path.relative_to(artifact)}: unresolved or branch-relative source identity"
            )
    return errors


def check_site(
    root: Path,
    artifact: Path,
    source_sha: str,
    identities: SiteIdentities = PRODUCTION_IDENTITIES,
) -> list[str]:
    release_identity, _ = derive_release_identity(root)
    return [
        *check_source(root, identities),
        *check_artifact(
            artifact,
            source_sha,
            release_identity.python if release_identity is not None else "",
            identities,
        ),
    ]


class SiteRequestHandler(BaseHTTPRequestHandler):
    artifact: Path

    def do_HEAD(self) -> None:  # noqa: N802
        self._send(send_body=False)

    def do_GET(self) -> None:  # noqa: N802
        self._send(send_body=True)

    def _send(self, *, send_body: bool) -> None:
        parsed = urlsplit(self.path)
        relative = PurePosixPath(unquote(parsed.path).lstrip("/"))
        unsafe = relative.is_absolute() or ".." in relative.parts
        if unsafe:
            self.send_error(400)
            return
        candidate = self.artifact.joinpath(*relative.parts)
        if parsed.path.endswith("/") or not relative.parts:
            candidate /= "index.html"
        elif candidate.is_dir():
            self.send_response(308)
            self.send_header("Location", parsed.path + "/")
            self.end_headers()
            return
        status_code = 200
        if not candidate.is_file() or candidate.is_symlink():
            candidate = self.artifact / "404.html"
            status_code = 404
        try:
            resolved = candidate.resolve(strict=True)
            resolved.relative_to(self.artifact)
            payload = resolved.read_bytes()
        except (OSError, ValueError):
            self.send_error(404)
            return
        content_type = (
            mimetypes.guess_type(resolved.name)[0] or "application/octet-stream"
        )
        if content_type.startswith("text/") or content_type in {
            "application/javascript",
            "application/json",
            "image/svg+xml",
        }:
            content_type += "; charset=utf-8"
        self.send_response(status_code)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        if send_body:
            self.wfile.write(payload)

    def log_message(self, format: str, *args: object) -> None:
        print(f"site server: {format % args}", file=sys.stderr)


def serve(artifact: Path, host: str, port: int) -> int:
    resolved = artifact.resolve(strict=True)
    if not resolved.is_dir() or artifact.is_symlink():
        raise ValueError("artifact must be a real directory")
    handler = type(
        "BoundSiteRequestHandler", (SiteRequestHandler,), {"artifact": resolved}
    )
    server = ThreadingHTTPServer((host, port), handler)
    print(f"site server: http://{host}:{server.server_port}", flush=True)
    server.serve_forever()
    return 0


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    check = subparsers.add_parser("check")
    check.add_argument("--root", type=Path, required=True)
    check.add_argument("--artifact", type=Path, required=True)
    check.add_argument("--source-sha", required=True)
    server = subparsers.add_parser("serve")
    server.add_argument("--artifact", type=Path, required=True)
    server.add_argument("--host", default="127.0.0.1")
    server.add_argument("--port", type=int, default=4173)
    topology = subparsers.add_parser("source-topology")
    topology.add_argument("--root", type=Path, required=True)
    topology.add_argument("--expected-agents-sha256")
    browser = subparsers.add_parser("browser-supply")
    browser.add_argument("--site-root", type=Path, required=True)
    browser.add_argument("--browser-cache", type=Path, required=True)
    browser.add_argument("--expected-executable-sha256", required=True)
    browser.add_argument("--expected-executable-bytes", type=int, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if args.command == "serve":
        try:
            return serve(args.artifact, args.host, args.port)
        except (OSError, ValueError) as error:
            print(f"site server: {error}", file=sys.stderr)
            return 1
    if args.command == "source-topology":
        errors = check_source_topology(args.root, args.expected_agents_sha256)
        if errors:
            print(
                "\n".join(f"site source: {error}" for error in errors), file=sys.stderr
            )
            return 1
        print("site source: exact optional CLAUDE.md topology admitted")
        return 0
    if args.command == "browser-supply":
        errors = check_browser_supply(
            args.site_root,
            args.browser_cache,
            args.expected_executable_sha256,
            args.expected_executable_bytes,
        )
        if errors:
            print(
                "\n".join(f"site browser: {error}" for error in errors), file=sys.stderr
            )
            return 1
        print("site browser: exact locked full Chromium supply admitted")
        return 0
    errors = check_site(args.root.resolve(), args.artifact.resolve(), args.source_sha)
    if errors:
        print("\n".join(f"site check: {error}" for error in errors), file=sys.stderr)
        return 1
    print("site check: exact static artifact satisfies the bounded public contract")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
