#!/usr/bin/env python3
"""Verify and serve the bounded Eqiora static-site artifact without network access."""

from __future__ import annotations

import argparse
import hashlib
import json
import mimetypes
import os
import re
import stat
import sys
from dataclasses import dataclass
from html.parser import HTMLParser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path, PurePosixPath
from typing import Iterable
from urllib.parse import unquote, urlsplit
from xml.etree import ElementTree

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
    "Alpha 0.1.0a1",
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
REQUIRED_TRIGGER_PATTERNS = {
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
OFFLINE_WORKFLOW_TOKENS = (
    "ubuntu-24.04",
    "eqiora-pw-1.62.1-r1234",
    "playwright install --with-deps --only-shell chromium",
    "HeadlessChrome 151.0.7922.34",
    "unshare --net",
    "ip link set lo up",
    "setpriv",
    "npm_config_offline=true",
    "CARGO_NET_OFFLINE=true",
    "UV_OFFLINE=1",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def normalize(text: str) -> str:
    return " ".join(text.split())


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


class HtmlInspection(HTMLParser):
    """Collect semantic HTML observations without assuming component DOM."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.stack: list[tuple[str, dict[str, str], bool]] = []
        self.collectors: list[list[object]] = []
        self.text: list[str] = []
        self.headings: list[tuple[int, str]] = []
        self.anchors: list[tuple[str, str]] = []
        self.interactives: list[tuple[str, dict[str, str], str]] = []
        self.images: list[dict[str, str]] = []
        self.metas: list[dict[str, str]] = []
        self.links: list[dict[str, str]] = []
        self.references: list[tuple[str, str, str]] = []
        self.ids: set[str] = set()
        self.math: list[tuple[str, bool]] = []
        self.inline_handlers: list[str] = []
        self.forms = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = {name.lower(): value or "" for name, value in attrs}
        classes = set(values.get("class", "").split())
        parent_hidden = self.stack[-1][2] if self.stack else False
        hidden = (
            parent_hidden
            or tag in {"script", "style", "template", "svg"}
            or "hidden" in values
            or values.get("aria-hidden") == "true"
        )
        self.stack.append((tag, values, hidden))
        if identifier := values.get("id"):
            self.ids.add(identifier)
        for name in values:
            if name.startswith("on"):
                self.inline_handlers.append(f"{tag}[{name}]")
        if tag in {"h1", "h2", "h3", "h4", "h5", "h6", "a", "button"} or values.get(
            "role"
        ) in {"button", "link"}:
            self.collectors.append([tag, values, []])
        if tag == "img":
            for ancestor, ancestor_values, _ in reversed(self.stack[:-1]):
                if ancestor == "a":
                    values["_ancestor_href"] = ancestor_values.get("href", "")
                    break
            self.images.append(values)
        if tag == "meta":
            self.metas.append(values)
        if tag == "link":
            self.links.append(values)
        if tag == "form":
            self.forms += 1
        if tag == "math":
            in_display = any(
                "katex-display" in item[1].get("class", "").split()
                for item in self.stack[:-1]
            )
            self.math.append((values.get("display", "inline"), in_display))
        for attribute in ("href", "src", "poster", "action"):
            if attribute in values:
                self.references.append((tag, attribute, values[attribute]))
        if "srcset" in values:
            for item in values["srcset"].split(","):
                self.references.append((tag, "srcset", item.strip().split()[0]))
        if "katex-display" in classes and tag not in {"div", "span"}:
            self.references.append((tag, "invalid-katex-display-owner", ""))

    def handle_startendtag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.handle_starttag(tag, attrs)
        self.handle_endtag(tag)

    def handle_endtag(self, tag: str) -> None:
        for index in range(len(self.collectors) - 1, -1, -1):
            collector = self.collectors[index]
            if collector[0] != tag:
                continue
            _, attrs, chunks = self.collectors.pop(index)
            collected = normalize("".join(chunks))
            if tag.startswith("h") and len(tag) == 2 and tag[1].isdigit():
                self.headings.append((int(tag[1]), collected))
            elif tag == "a":
                self.anchors.append((str(attrs.get("href", "")), collected))
            else:
                self.interactives.append((tag, attrs, collected))
            break
        for index in range(len(self.stack) - 1, -1, -1):
            if self.stack[index][0] == tag:
                del self.stack[index:]
                break

    def handle_data(self, data: str) -> None:
        if not (self.stack and self.stack[-1][2]):
            self.text.append(data)
            for collector in self.collectors:
                collector[2].append(data)

    @property
    def visible_text(self) -> str:
        return normalize("".join(self.text))


def _read_html(path: Path) -> tuple[str, HtmlInspection]:
    if path.stat().st_size > MAX_HTML_BYTES:
        raise ValueError(f"HTML exceeds {MAX_HTML_BYTES} bytes")
    text = path.read_text(encoding="utf-8")
    parser = HtmlInspection()
    parser.feed(text)
    parser.close()
    return text, parser


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
    return errors


def check_source(
    root: Path, identities: SiteIdentities = PRODUCTION_IDENTITIES
) -> list[str]:
    errors: list[str] = []
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

    package = site / "package.json"
    lock = site / "package-lock.json"
    try:
        package_data = json.loads(package.read_text(encoding="utf-8"))
        lock_data = json.loads(lock.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        errors.append(f"site package/lock is missing or invalid: {error}")
    else:
        declared = {
            **package_data.get("dependencies", {}),
            **package_data.get("devDependencies", {}),
        }
        for name, expected in DIRECT_PINS.items():
            if declared.get(name) != expected:
                errors.append(f"site package must pin {name} exactly to {expected}")
            entry = lock_data.get("packages", {}).get(f"node_modules/{name}", {})
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
        is_release_history = "release-notes" in source.relative_to(site).parts
        if not is_release_history:
            for forbidden_version in ("0.1.0a1", "0.1.0a2", "0.1.0-alpha.1"):
                if forbidden_version in text:
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
    identities: SiteIdentities = PRODUCTION_IDENTITIES,
) -> list[str]:
    errors: list[str] = []
    artifact = artifact.resolve()
    if not SOURCE_SHA.fullmatch(source_sha):
        return ["source SHA must be exactly 40 lowercase hexadecimal characters"]
    files, inventory_errors = _artifact_inventory(artifact)
    errors.extend(inventory_errors)
    if inventory_errors and not files:
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
            inspections[path] = _read_html(path)
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
        errors.extend(_ordered(home.visible_text, HOME_COPY, "/"))
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
        for _, _, label in case.interactives:
            if re.search(r"\b(?:run|submit|solve|execute)\b", label, re.IGNORECASE):
                errors.append(
                    f"Cylinder route contains an uncontracted execution control {label!r}"
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
                        _, target_parser = _read_html(target)
                    except (OSError, UnicodeDecodeError, ValueError):
                        target_parser = None
                if target_parser is None or unquote(fragment) not in target_parser.ids:
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
    return [
        *check_source(root, identities),
        *check_artifact(artifact, source_sha, identities),
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
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if args.command == "serve":
        try:
            return serve(args.artifact, args.host, args.port)
        except (OSError, ValueError) as error:
            print(f"site server: {error}", file=sys.stderr)
            return 1
    errors = check_site(args.root.resolve(), args.artifact.resolve(), args.source_sha)
    if errors:
        for error in errors:
            print(f"site check: {error}", file=sys.stderr)
        return 1
    print("site check: exact static artifact satisfies the bounded public contract")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
