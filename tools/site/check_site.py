#!/usr/bin/env python3
"""Check the bounded public-site source without fetching external resources."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

REQUIRED_PAGES = {
    "index.md",
    "get-started.md",
    "python/index.md",
    "python/modeling.md",
    "python/execution-and-arrays.md",
    "python/differentiation.md",
    "concepts.md",
    "examples.md",
    "capabilities.md",
    "evidence/index.md",
    "architecture.md",
    "contributing.md",
    "api.md",
    "release-notes.md",
}
MARKDOWN_LINK = re.compile(r"!?\[[^\]]*]\(([^)\n]+)\)")
HTML_REFERENCE = re.compile(r"""(?:href|src)=["']([^"']+)["']""", re.IGNORECASE)
NAV_PAGE = re.compile(r"^\s*-\s+(?:[^:]+:\s+)?([A-Za-z0-9_./-]+\.md)\s*$")
ACTION_USE = re.compile(r"^\s*uses:\s*([^@\s]+)@([^\s#]+)", re.MULTILINE)
PRIVATE_OR_TRACKING = re.compile(
    r"(?i)(?:"
    r"/home/|file://|localhost|127\.0\.0\.1|"
    r"google-analytics|googletagmanager|segment\.com|plausible\.io"
    r")"
)


def _destination(raw: str) -> str:
    destination = raw.strip()
    if destination.startswith("<") and ">" in destination:
        return destination[1 : destination.index(">")]
    return destination.split(maxsplit=1)[0]


def check_markdown_links(site_root: Path) -> list[str]:
    errors: list[str] = []
    resolved_site = site_root.resolve()
    for document in sorted(site_root.rglob("*.md")):
        text = document.read_text(encoding="utf-8")
        destinations = [
            _destination(match.group(1))
            for match in MARKDOWN_LINK.finditer(text)
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
                errors.append(f"{document}: local link escapes docs/site: {destination}")
                continue
            if candidate.is_dir():
                candidate = candidate / "index.md"
            elif candidate.suffix == "":
                candidate = candidate.with_suffix(".md")
            if not candidate.exists():
                errors.append(f"{document}: missing local link target: {destination}")
    return errors


def check_site(root: Path) -> list[str]:
    errors: list[str] = []
    site_root = root / "docs/site"
    config = root / "mkdocs.yml"
    workflow = root / ".github/workflows/pages.yml"

    if not config.is_file():
        return ["missing mkdocs.yml"]
    if not site_root.is_dir():
        return ["missing docs/site"]

    config_text = config.read_text(encoding="utf-8")
    for fragment in (
        "site_url: https://eqiora.org/",
        "docs_dir: docs/site",
        "strict: true",
        "font: false",
    ):
        if fragment not in config_text:
            errors.append(f"mkdocs.yml: missing required setting {fragment!r}")

    nav_pages = {
        match.group(1)
        for line in config_text.splitlines()
        if (match := NAV_PAGE.match(line))
    }
    missing_nav = sorted(REQUIRED_PAGES - nav_pages)
    extra_nav = sorted(nav_pages - REQUIRED_PAGES)
    if missing_nav:
        errors.append(f"mkdocs.yml: pages absent from nav: {missing_nav}")
    if extra_nav:
        errors.append(f"mkdocs.yml: unexpected Markdown nav pages: {extra_nav}")
    for relative in sorted(REQUIRED_PAGES):
        if not (site_root / relative).is_file():
            errors.append(f"missing site page: docs/site/{relative}")
    robots = site_root / "robots.txt"
    if not robots.is_file():
        errors.append("missing docs/site/robots.txt")
    else:
        robots_text = robots.read_text(encoding="utf-8")
        for fragment in ("User-agent: *", "Allow: /", "https://eqiora.org/sitemap.xml"):
            if fragment not in robots_text:
                errors.append(f"{robots}: missing {fragment!r}")

    for source in sorted(site_root.rglob("*")):
        if not source.is_file():
            continue
        text = source.read_text(encoding="utf-8")
        if match := PRIVATE_OR_TRACKING.search(text):
            errors.append(f"{source}: forbidden private/tracking reference {match.group(0)!r}")
        if re.search(r"<script\b", text, re.IGNORECASE):
            errors.append(f"{source}: custom script tags are not allowed")
    errors.extend(check_markdown_links(site_root))

    for cname in (root / "CNAME", site_root / "CNAME"):
        if cname.exists():
            errors.append(
                f"{cname}: Actions Pages keeps custom-domain state in repository settings"
            )

    if not workflow.is_file():
        errors.append("missing .github/workflows/pages.yml")
        return errors
    workflow_text = workflow.read_text(encoding="utf-8")
    uses = ACTION_USE.findall(workflow_text)
    if not uses:
        errors.append(f"{workflow}: no actions found")
    for action, revision in uses:
        if not re.fullmatch(r"[0-9a-f]{40}", revision):
            errors.append(f"{workflow}: {action} is not pinned to a full SHA")
    for fragment in (
        "contents: read",
        "pages: write",
        "id-token: write",
        "name: github-pages",
    ):
        if fragment not in workflow_text:
            errors.append(f"{workflow}: missing {fragment!r}")
    for forbidden in ("write-all", "contents: write"):
        if forbidden in workflow_text:
            errors.append(f"{workflow}: forbidden permission {forbidden!r}")
    return errors


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", nargs="?", type=Path, default=Path("."))
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    errors = check_site(args.root)
    if errors:
        for error in errors:
            print(f"site check: {error}", file=sys.stderr)
        return 1
    print("site check: public site source is internally consistent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
