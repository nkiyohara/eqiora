"""Private Starlight shell, route, and child-policy composition."""

from __future__ import annotations

import re
import sys
from pathlib import Path

try:
    import tools.site.check_site_html as _html
    import tools.site.check_site_sitemap as _sitemap
    import tools.site.check_site_starlight_content as _content
except ModuleNotFoundError as error:
    if error.name not in {
        "tools",
        "tools.site",
        "tools.site.check_site_html",
        "tools.site.check_site_sitemap",
        "tools.site.check_site_starlight_content",
    }:
        raise
    import check_site_html as _html
    import check_site_sitemap as _sitemap
    import check_site_starlight_content as _content


def _require(module: object, filename: str, exports: tuple[str, ...]) -> None:
    expected = Path(__file__).with_name(filename).resolve()
    registered = sys.modules.get(f"tools.site.{filename.removesuffix('.py')}")
    if registered is not None and registered is not module:
        raise ImportError(
            f"site checker {filename} did not resolve to its exact sibling"
        )
    if Path(module.__file__ or "").resolve() != expected:
        raise ImportError(
            f"site checker {filename} did not resolve to its exact sibling"
        )
    if module.__all__ != exports:
        raise ImportError(f"site checker {filename} exposes an unexpected interface")


_require(_html, "check_site_html.py", ("HtmlInspection", "normalize", "read_html"))
_require(_sitemap, "check_site_sitemap.py", ("SITEMAP_ROUTES", "check_sitemap"))
_require(
    _content,
    "check_site_starlight_content.py",
    (
        "PRESSURE_ALT",
        "PRESSURE_CAPTION",
        "CASE_SOURCE_PATHS",
        "CASE_EVIDENCE_PATHS",
        "check_starlight_content",
    ),
)

__all__ = (
    "STARLIGHT_ROUTES",
    "SITEMAP_ROUTES",
    "PRESSURE_ALT",
    "PRESSURE_CAPTION",
    "CASE_SOURCE_PATHS",
    "CASE_EVIDENCE_PATHS",
    "check_starlight",
)

SITEMAP_ROUTES = _sitemap.SITEMAP_ROUTES
PRESSURE_ALT = _content.PRESSURE_ALT
PRESSURE_CAPTION = _content.PRESSURE_CAPTION
CASE_SOURCE_PATHS = _content.CASE_SOURCE_PATHS
CASE_EVIDENCE_PATHS = _content.CASE_EVIDENCE_PATHS

SITE_ORIGIN = "https://eqiora.org"
STARLIGHT_ROUTES = {
    "/": "index.html",
    "/get-started/": "get-started/index.html",
    "/gallery/": "gallery/index.html",
    "/gallery/exact-cylinder-steady-stokes/": "gallery/exact-cylinder-steady-stokes/index.html",
    "/reference/": "reference/index.html",
    "/reference/python/eqiora/": "reference/python/eqiora/index.html",
    "/reference/rust/": "reference/rust/index.html",
    "/reference/cli/": "reference/cli/index.html",
    "/reference/control-v2/": "reference/control-v2/index.html",
    "/reference/mcp/": "reference/mcp/index.html",
    "/examples/": "examples/index.html",
    "/evidence/": "evidence/index.html",
    "/capabilities/": "capabilities/index.html",
    "/404.html": "404.html",
}
TOP_NAV = (
    ("Docs", "/get-started/"),
    ("Gallery", "/gallery/"),
    ("Reference", "/reference/"),
    ("Evidence", "/evidence/"),
    ("GitHub", "https://github.com/nkiyohara/eqiora"),
)


def _check_shell(route: str, raw: str) -> list[str]:
    errors: list[str] = []
    match = re.search(r"<header\b[^>]*>(.*?)</header>", raw, re.DOTALL | re.IGNORECASE)
    header = match.group(1) if match else ""
    match = re.search(
        r'<nav\b[^>]*aria-label=["\']Primary["\'][^>]*>(.*?)</nav>',
        header,
        re.DOTALL | re.IGNORECASE,
    )
    if not match:
        errors.append(f"{route}: primary navigation must be in the page banner")
    navigation = match.group(1) if match else ""
    positions = [navigation.find(f'href="{href}"') for _, href in TOP_NAV]
    if any(position < 0 for position in positions) or positions != sorted(positions):
        errors.append(f"{route}: primary navigation has the wrong link order")
    title = re.findall(r'<a\b[^>]*class="site-title"[^>]*>', header, re.IGNORECASE)
    if title:
        if (
            len(title) != 1
            or 'href="/"' not in title[0]
            or 'aria-label="Eqiora"' not in title[0]
        ):
            errors.append(
                f"{route}: header home link accessible name must be exactly 'Eqiora'"
            )
    return errors


def _check_head(route: str, parser: object) -> list[str]:
    errors: list[str] = []
    canonicals = [
        link.get("href", "")
        for link in parser.links
        if "canonical" in link.get("rel", "").split()
    ]
    expected = f"{SITE_ORIGIN}{route}"
    if canonicals != [expected]:
        errors.append(
            f"{route}: canonical must be exactly {expected!r}, got {canonicals!r}"
        )
    properties = {
        (meta.get("property") or meta.get("name"), meta.get("content"))
        for meta in parser.metas
    }
    if ("og:image", f"{SITE_ORIGIN}/social-card.svg") not in properties:
        errors.append(f"{route}: missing exact same-origin Open Graph image")
    rels = {(link.get("rel", ""), link.get("href", "")) for link in parser.links}
    if not any("icon" in rel.split() and href == "/favicon.svg" for rel, href in rels):
        errors.append(f"{route}: missing exact favicon link")
    if not any(
        "apple-touch-icon" in rel.split() and href == "/apple-touch-icon.png"
        for rel, href in rels
    ):
        errors.append(f"{route}: missing exact apple-touch-icon link")
    return errors


def check_starlight(
    artifact: Path,
    inspections: dict[Path, tuple[str, object]],
    file_digests: dict[Path, str],
    pressure_digest: str,
    favicon_digest: str,
    source_sha: str,
    expected_python_version: str,
    maximum_sitemap_urls: int,
) -> list[str]:
    errors: list[str] = []
    expected_paths = {artifact / relative for relative in STARLIGHT_ROUTES.values()}
    for path in sorted(set(inspections) - expected_paths):
        errors.append(
            f"{path.relative_to(artifact)}: Starlight page outside exact Rustdoc root"
        )
    for route, relative in STARLIGHT_ROUTES.items():
        path = artifact / relative
        if path not in inspections:
            errors.append(f"missing required route {route}: {relative}")
            continue
        raw, parser = inspections[path]
        errors.extend(_check_head(route, parser))
        errors.extend(_check_shell(route, raw))
        if parser.inline_handlers:
            errors.append(f"{relative}: inline event handlers are forbidden")
    case = inspections.get(artifact / "gallery/exact-cylinder-steady-stokes/index.html")
    if case and "<form" in case[0].casefold():
        errors.append(
            "gallery/exact-cylinder-steady-stokes/index.html: forms imply an uncontracted case interaction"
        )
    errors.extend(
        _content.check_starlight_content(
            artifact,
            inspections,
            file_digests,
            pressure_digest,
            favicon_digest,
            source_sha,
            expected_python_version,
        )
    )
    if not (artifact / "pagefind/pagefind.js").is_file():
        errors.append("Pagefind JavaScript entry is missing")
    try:
        robots = (artifact / "robots.txt").read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"robots.txt is missing: {error}")
    else:
        if (
            robots
            != "User-agent: *\nAllow: /\nSitemap: https://eqiora.org/sitemap-index.xml\n"
        ):
            errors.append("robots.txt differs from the exact public crawl boundary")
    errors.extend(_sitemap.check_sitemap(artifact, maximum_sitemap_urls))
    return errors
