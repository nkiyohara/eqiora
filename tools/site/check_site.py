#!/usr/bin/env python3
"""Verify and serve the bounded Eqiora static-site artifact without network access."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import mimetypes
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path, PurePosixPath
from typing import Iterable
from urllib.parse import unquote, urlsplit

try:
    import tools.site.check_site_artifact as _site_artifact
    import tools.site.check_site_html as _site_html
    import tools.site.check_site_supply as _site_supply
except ModuleNotFoundError as error:
    if error.name not in {
        "tools",
        "tools.site",
        "tools.site.check_site_artifact",
        "tools.site.check_site_html",
        "tools.site.check_site_supply",
    }:
        raise
    import check_site_artifact as _site_artifact
    import check_site_html as _site_html
    import check_site_supply as _site_supply

_ARTIFACT_EXPORTS = (
    "MAX_FILES",
    "MAX_FILE_BYTES",
    "MAX_TOTAL_BYTES",
    "MAX_HTML_BYTES",
    "PRESSURE_SHA256",
    "PUBLICATION_SHA256",
    "SOCIAL_SHA256",
    "FAVICON_SHA256",
    "APPLE_TOUCH_SHA256",
    "OLD_SOCIAL_SHA256",
    "OLD_SOCIAL_LINE",
    "SiteIdentities",
    "PRODUCTION_IDENTITIES",
    "ROUTES",
    "SITEMAP_ROUTES",
    "PRESSURE_ALT",
    "PRESSURE_CAPTION",
    "CASE_SOURCE_PATHS",
    "CASE_EVIDENCE_PATHS",
    "sha256",
    "check_exact_source",
    "check_artifact",
)
_SUPPLY_EXPORTS = (
    "FULL_CHROMIUM_VERSION_STDOUT_HEX",
    "FULL_CHROMIUM_VERSION_STDOUT",
    "DIRECT_SOURCE_ARCHIVE_COMMAND",
    "EXACT_LINK_PAYLOAD_SHA256",
    "EXACT_TREE_LINK_COMMAND",
    "EXACT_EXTRACTED_LINK_COMMAND",
    "OFFLINE_WORKFLOW_TOKENS",
    "FORBIDDEN_WORKFLOW_TOKENS",
    "check_source_topology",
    "check_runner_source_topology_text",
    "check_browser_supply",
    "check_runner_browser_supply_text",
)
for module, filename, exports in (
    (_site_artifact, "check_site_artifact.py", _ARTIFACT_EXPORTS),
    (_site_html, "check_site_html.py", ("HtmlInspection", "normalize", "read_html")),
    (_site_supply, "check_site_supply.py", _SUPPLY_EXPORTS),
):
    registered = sys.modules.get(f"tools.site.{filename.removesuffix('.py')}")
    if registered is not None and registered is not module:
        raise ImportError(
            f"site checker {filename} did not resolve to its exact sibling"
        )
    if (
        Path(module.__file__ or "").resolve()
        != Path(__file__).with_name(filename).resolve()
    ):
        raise ImportError(
            f"site checker {filename} did not resolve to its exact sibling"
        )
    if module.__all__ != exports:
        raise ImportError(f"site checker {filename} exposes an unexpected interface")
if _site_supply.subprocess is not subprocess:
    raise ImportError(
        "site checker subprocess facade does not share its supply binding"
    )

HtmlInspection = _site_html.HtmlInspection
normalize = _site_html.normalize
read_html = _site_html.read_html
MAX_FILES = _site_artifact.MAX_FILES
MAX_FILE_BYTES = _site_artifact.MAX_FILE_BYTES
MAX_TOTAL_BYTES = _site_artifact.MAX_TOTAL_BYTES
MAX_HTML_BYTES = _site_artifact.MAX_HTML_BYTES
PRESSURE_SHA256 = _site_artifact.PRESSURE_SHA256
PUBLICATION_SHA256 = _site_artifact.PUBLICATION_SHA256
SOCIAL_SHA256 = _site_artifact.SOCIAL_SHA256
FAVICON_SHA256 = _site_artifact.FAVICON_SHA256
APPLE_TOUCH_SHA256 = _site_artifact.APPLE_TOUCH_SHA256
OLD_SOCIAL_SHA256 = _site_artifact.OLD_SOCIAL_SHA256
OLD_SOCIAL_LINE = _site_artifact.OLD_SOCIAL_LINE
PRESSURE_ALT = _site_artifact.PRESSURE_ALT
PRESSURE_CAPTION = _site_artifact.PRESSURE_CAPTION
SiteIdentities = _site_artifact.SiteIdentities
PRODUCTION_IDENTITIES = _site_artifact.PRODUCTION_IDENTITIES
ROUTES = _site_artifact.ROUTES
SITEMAP_ROUTES = _site_artifact.SITEMAP_ROUTES
CASE_SOURCE_PATHS = _site_artifact.CASE_SOURCE_PATHS
CASE_EVIDENCE_PATHS = _site_artifact.CASE_EVIDENCE_PATHS
sha256 = _site_artifact.sha256
FULL_CHROMIUM_VERSION_STDOUT_HEX = _site_supply.FULL_CHROMIUM_VERSION_STDOUT_HEX
FULL_CHROMIUM_VERSION_STDOUT = _site_supply.FULL_CHROMIUM_VERSION_STDOUT
DIRECT_SOURCE_ARCHIVE_COMMAND = _site_supply.DIRECT_SOURCE_ARCHIVE_COMMAND
EXACT_LINK_PAYLOAD_SHA256 = _site_supply.EXACT_LINK_PAYLOAD_SHA256
EXACT_TREE_LINK_COMMAND = _site_supply.EXACT_TREE_LINK_COMMAND
EXACT_EXTRACTED_LINK_COMMAND = _site_supply.EXACT_EXTRACTED_LINK_COMMAND
OFFLINE_WORKFLOW_TOKENS = _site_supply.OFFLINE_WORKFLOW_TOKENS
FORBIDDEN_WORKFLOW_TOKENS = _site_supply.FORBIDDEN_WORKFLOW_TOKENS
check_source_topology = _site_supply.check_source_topology
check_runner_source_topology_text = _site_supply.check_runner_source_topology_text
check_browser_supply = _site_supply.check_browser_supply
check_runner_browser_supply_text = _site_supply.check_runner_browser_supply_text

ACTION_USE = re.compile(r"^\s*uses:\s*([^@\s]+)@([^\s#]+)", re.MULTILINE)
MARKDOWN_LINK = re.compile(r"!?\[[^\]]*]\(([^)\n]+)\)")
HTML_REFERENCE = re.compile(r"""(?:href|src)=["']([^"']+)["']""", re.IGNORECASE)


@dataclass(frozen=True)
class ReleaseIdentity:
    cargo: str
    python: str


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
        _site_artifact.check_exact_source(
            site / "src/assets/gallery/exact-cylinder-pressure.png",
            identities.pressure,
            "admitted pressure media",
        )
    )
    record = site / "src/data/gallery" / "exact-cylinder-steady-stokes.publication.json"
    publication_label = "admitted publication record"
    record_errors = _site_artifact.check_exact_source(
        record, identities.publication, publication_label
    )
    errors.extend(record_errors)
    fixed = identities.publication == PUBLICATION_SHA256 and not record_errors
    errors.extend(
        _site_artifact.check_exact_source(
            site / "public/social-card.svg", identities.social, "timeless social card"
        )
    )
    errors.extend(
        _site_artifact.check_exact_source(
            site / "public/favicon.svg", identities.favicon, "favicon"
        )
    )
    errors.extend(
        _site_artifact.check_exact_source(
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
                if fixed and source == record and forbidden_version == "0.1.0a3":
                    continue
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
    """Retain the checker-level call shape while delegating sole artifact policy."""

    return _site_artifact.check_artifact(
        artifact,
        source_sha,
        expected_python_version,
        identities,
        maximum_files=MAX_FILES,
        maximum_total_bytes=MAX_TOTAL_BYTES,
    )


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
