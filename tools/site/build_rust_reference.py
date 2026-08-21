#!/usr/bin/env python3
"""Validate and stage the all-features Eqiora facade rustdoc tree."""

from __future__ import annotations

import argparse
import difflib
import hashlib
import html
import json
import shutil
import sys
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path, PurePosixPath
from typing import Any
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[2]
FACADE = ROOT / "api/eqiora-facade-v1.json"
LANDING = ROOT / "docs/site/src/content/docs/reference/rust/index.mdx"
FACADE_SCHEMA = "eqiora.facade-inventory/v1"
FACADE_SHA256 = "101a1292c8c2195b8dfb17e542c548934b59a735c6dbf077aec347a0192539f6"
EXPECTED_COUNTS = {
    "modules": 24,
    "stable_modules": 3,
    "transitional_modules": 21,
    "items": 182,
    "stable_items": 48,
    "transitional_items": 134,
}
PUBLIC_RUSTDOC_PREFIX = "/reference/rust/api/eqiora/"
ALLOWED_SITE_LINKS = {"/favicon.svg", "/reference/rust/"}


class RustReferenceError(ValueError):
    """The Rust reference cannot be generated or staged safely."""


@dataclass(frozen=True, order=True)
class FacadePath:
    path: str
    classification: str
    provider: str | None = None


class _References(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.urls: list[str] = []

    def handle_starttag(
        self, tag: str, attrs: list[tuple[str, str | None]]
    ) -> None:
        del tag
        for name, value in attrs:
            if name in {"href", "src"} and value is not None:
                self.urls.append(value)


def _object(value: Any, context: str, keys: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise RustReferenceError(f"{context} does not match its frozen shape")
    return value


def _text(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value or any(ord(c) < 0x20 for c in value):
        raise RustReferenceError(f"{context} must be non-empty printable text")
    return value


def _exports(value: Any, context: str) -> list[tuple[str, str]]:
    if not isinstance(value, list):
        raise RustReferenceError(f"{context} must be an array")
    result: list[tuple[str, str]] = []
    for offset, raw in enumerate(value):
        entry = _object(raw, f"{context}[{offset}]", {"name", "from"})
        name = _text(entry["name"], f"{context}[{offset}].name")
        provider = _text(entry["from"], f"{context}[{offset}].from")
        if "::" in name or any(character.isspace() for character in name):
            raise RustReferenceError(f"{context}[{offset}].name is not one item name")
        result.append((name, provider))
    return result


def _load_facade() -> tuple[list[FacadePath], list[FacadePath]]:
    try:
        payload = FACADE.read_bytes()
        document = json.loads(payload)
    except (OSError, json.JSONDecodeError) as error:
        raise RustReferenceError(f"cannot read facade inventory: {error}") from error
    digest = hashlib.sha256(payload).hexdigest()
    if digest != FACADE_SHA256:
        raise RustReferenceError(
            f"facade inventory digest changed: expected {FACADE_SHA256}, got {digest}"
        )
    root = _object(
        document,
        "facade",
        {"schema", "crate", "source", "root", "stable_namespaces"},
    )
    if root["schema"] != FACADE_SCHEMA or root["crate"] != "eqiora":
        raise RustReferenceError("facade schema or crate identity changed")
    if root["source"] != "crates/eqiora/src/lib.rs":
        raise RustReferenceError("facade source owner changed")

    root_inventory = _object(
        root["root"],
        "facade.root",
        {"stable_exports", "stable_modules", "transitional_modules"},
    )
    modules: list[FacadePath] = []
    for classification, key in (
        ("stable", "stable_modules"),
        ("transitional", "transitional_modules"),
    ):
        raw_modules = root_inventory[key]
        if not isinstance(raw_modules, list):
            raise RustReferenceError(f"facade.root.{key} must be an array")
        for offset, value in enumerate(raw_modules):
            name = _text(value, f"facade.root.{key}[{offset}]")
            modules.append(FacadePath(f"eqiora::{name}", classification))

    items = [
        FacadePath(f"eqiora::{name}", "stable", provider)
        for name, provider in _exports(
            root_inventory["stable_exports"], "facade.root.stable_exports"
        )
    ]
    namespaces = root["stable_namespaces"]
    if not isinstance(namespaces, list):
        raise RustReferenceError("facade.stable_namespaces must be an array")
    for offset, raw_namespace in enumerate(namespaces):
        context = f"facade.stable_namespaces[{offset}]"
        namespace = _object(
            raw_namespace,
            context,
            {"path", "source_module", "stable_exports", "transitional_exports"},
        )
        path = _text(namespace["path"], f"{context}.path")
        source_module = _text(namespace["source_module"], f"{context}.source_module")
        if path != f"eqiora::{source_module}":
            raise RustReferenceError(f"{context} path/source module disagreement")
        for classification, key in (
            ("stable", "stable_exports"),
            ("transitional", "transitional_exports"),
        ):
            items.extend(
                FacadePath(f"{path}::{name}", classification, provider)
                for name, provider in _exports(namespace[key], f"{context}.{key}")
            )

    modules.sort()
    items.sort()
    if len({entry.path for entry in modules}) != len(modules):
        raise RustReferenceError("facade module paths are not unique")
    if len({entry.path for entry in items}) != len(items):
        raise RustReferenceError("facade item paths are not unique")
    counts = {
        "modules": len(modules),
        "stable_modules": sum(entry.classification == "stable" for entry in modules),
        "transitional_modules": sum(
            entry.classification == "transitional" for entry in modules
        ),
        "items": len(items),
        "stable_items": sum(entry.classification == "stable" for entry in items),
        "transitional_items": sum(
            entry.classification == "transitional" for entry in items
        ),
    }
    if counts != EXPECTED_COUNTS:
        raise RustReferenceError(
            f"facade classification counts changed: expected {EXPECTED_COUNTS}, got {counts}"
        )
    return modules, items


def _resolved_directory(path: Path, context: str) -> Path:
    if path.is_symlink() or not path.is_dir():
        raise RustReferenceError(f"{context} must be a real directory: {path}")
    return path.resolve(strict=True)


def _reject_symlinks(root: Path) -> None:
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise RustReferenceError(f"rustdoc tree contains a symlink: {path}")


def _validate_rustdoc_root(root: Path) -> Path:
    rustdoc = _resolved_directory(root, "--rustdoc-root")
    _reject_symlinks(rustdoc)
    crate = rustdoc / "eqiora"
    if not (crate / "index.html").is_file():
        raise RustReferenceError("rustdoc root is missing eqiora/index.html")
    if (rustdoc / "eqiora_mcp").exists():
        raise RustReferenceError("rustdoc contains the private eqiora_mcp root")
    crate_roots = sorted(
        child.name
        for child in rustdoc.iterdir()
        if child.is_dir() and (child / "index.html").is_file()
    )
    if crate_roots != ["eqiora"]:
        raise RustReferenceError(f"rustdoc contains non-facade crate roots: {crate_roots}")
    _validate_html_references(rustdoc)
    return rustdoc


def _validate_html_references(root: Path) -> None:
    for document in sorted(root.rglob("*.html")):
        parser = _References()
        try:
            parser.feed(document.read_text(encoding="utf-8"))
        except (OSError, UnicodeError) as error:
            raise RustReferenceError(f"cannot parse {document}: {error}") from error
        for raw_url in parser.urls:
            parsed = urlsplit(html.unescape(raw_url))
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            path_text = unquote(parsed.path)
            if path_text.startswith("/"):
                if path_text not in ALLOWED_SITE_LINKS:
                    raise RustReferenceError(
                        f"{document} contains an unowned site-absolute link: {path_text}"
                    )
                continue
            target = (document.parent / path_text).resolve(strict=False)
            try:
                target.relative_to(root)
            except ValueError as error:
                raise RustReferenceError(
                    f"{document} contains a link escaping rustdoc: {raw_url}"
                ) from error
            if target.is_dir():
                target = target / "index.html"
            if not target.is_file():
                relative = target.relative_to(root)
                # Rustdoc emits async implementor-shard references even when a
                # public re-exported trait has no implementors and therefore no
                # shard file. Those sparse shards are optional rustdoc data,
                # not a missing copied asset. Every other local reference is
                # required to resolve.
                if (
                    len(relative.parts) >= 2
                    and relative.parts[0] in {"trait.impl", "type.impl"}
                    and relative.suffix == ".js"
                ):
                    continue
                # Standalone rustdoc emits help/settings navigation to a
                # workspace index that Cargo intentionally does not create for
                # a single selected package. The published entry point is the
                # required eqiora/index.html, never this absent aggregate.
                if document.parent == root and relative == Path("index.html"):
                    continue
                raise RustReferenceError(
                    f"{document} references a missing rustdoc asset: {raw_url}"
                )


def _item_target(crate: Path, item: FacadePath) -> str:
    parts = item.path.split("::")
    if parts[0] != "eqiora" or len(parts) < 2:
        raise RustReferenceError(f"invalid facade item path: {item.path}")
    parent_parts = parts[1:-1]
    name = parts[-1]
    parent = crate.joinpath(*parent_parts)
    module_target = parent / name / "index.html"
    if module_target.is_file():
        return PurePosixPath(*parent_parts, name, "index.html").as_posix()
    candidates = sorted(parent.glob(f"*.{name}.html"))
    if len(candidates) != 1:
        relative = PurePosixPath(*parent_parts).as_posix()
        raise RustReferenceError(
            f"{item.path} has {len(candidates)} rustdoc targets below {relative or '.'}"
        )
    return PurePosixPath(*parent_parts, candidates[0].name).as_posix()


def _module_target(crate: Path, module: FacadePath) -> str:
    parts = module.path.split("::")
    target = crate.joinpath(*parts[1:], "index.html")
    if not target.is_file():
        raise RustReferenceError(f"{module.path} has no rustdoc module target")
    return PurePosixPath(*parts[1:], "index.html").as_posix()


def _classification(value: str) -> str:
    return "**stable**" if value == "stable" else "transitional"


def _link(path: str, target: str) -> str:
    return f"[`{path}`]({PUBLIC_RUSTDOC_PREFIX}{target})"


def _render_landing(
    rustdoc: Path, modules: list[FacadePath], items: list[FacadePath]
) -> str:
    crate = rustdoc / "eqiora"
    lines = [
        "---",
        'title: "Rust API"',
        'description: "All-features library-only rustdoc and classified Eqiora facade index."',
        "---",
        "",
        "import ExactSourceLink from '@components/site/ExactSourceLink.astro';",
        "",
        "{/* Generated by tools/site/build_rust_reference.py; do not edit. */}",
        "",
        "This reference was built with **all facade features enabled for documentation**.",
        "It is the complete public surface/signature projection of the curated `eqiora`",
        "library facade for one exact source commit.",
        "",
        "> API presence is neither capability evidence nor maturity. Enabling features for",
        "> rustdoc does not prove runtime availability, hardware support, native-library",
        "> availability, backend verification, portability, or stability.",
        "",
        "[Open the standalone rustdoc](/reference/rust/api/eqiora/). It retains rustdoc's",
        "own navigation and search; with JavaScript disabled, item and module links remain usable.",
        "",
        "## Coverage",
        "",
        "| Surface | Stable | Transitional | Total |",
        "| --- | ---: | ---: | ---: |",
        "| Public modules | 3 | 21 | 24 |",
        "| Explicit exported items | 48 | 134 | 182 |",
        "| Classified facade paths | 51 | 155 | **206** |",
        "",
        "Classifications come from the checked facade inventory. Rustdoc remains compiler",
        "output; the inventory is the classification authority.",
        "",
        "- <ExactSourceLink kind=\"blob\" path=\"api/eqiora-facade-v1.json\">Facade inventory</ExactSourceLink>",
        "- <ExactSourceLink kind=\"blob\" path=\"crates/eqiora/src/lib.rs\">Facade source</ExactSourceLink>",
        "",
        "## Public modules (24)",
        "",
        "| Module | Classification |",
        "| --- | --- |",
    ]
    for module in modules:
        lines.append(
            f"| {_link(module.path, _module_target(crate, module))} | "
            f"{_classification(module.classification)} |"
        )
    lines.extend(
        [
            "",
            "## Explicit exported items (182)",
            "",
            "| Item | Classification | Provider |",
            "| --- | --- | --- |",
        ]
    )
    for item in items:
        assert item.provider is not None
        lines.append(
            f"| {_link(item.path, _item_target(crate, item))} | "
            f"{_classification(item.classification)} | `{item.provider}` |"
        )
    lines.extend(
        [
            "",
            "## Boundary",
            "",
            "The generated tree contains only the library facade root. It excludes the",
            "`eqiora-mcp` binary and workspace-private crate roots. Missing documentation is",
            "reported by rustdoc but is not relabelled as a false completeness gate.",
            "",
        ]
    )
    return "\n".join(lines)


def _update_or_check_landing(expected: str, update: bool) -> None:
    if update:
        LANDING.parent.mkdir(parents=True, exist_ok=True)
        LANDING.write_text(expected, encoding="utf-8", newline="\n")
        print(f"updated {LANDING.relative_to(ROOT)}")
        return
    if not LANDING.is_file():
        raise RustReferenceError(f"generated landing is missing: {LANDING}")
    actual = LANDING.read_text(encoding="utf-8")
    if actual != expected:
        diff = "\n".join(
            difflib.unified_diff(
                actual.splitlines(),
                expected.splitlines(),
                fromfile=LANDING.relative_to(ROOT).as_posix(),
                tofile="generated",
                lineterm="",
            )
        )
        raise RustReferenceError(f"generated Rust landing is stale:\n{diff}")
    print(f"checked {LANDING.relative_to(ROOT)}")


def _empty_output(path: Path) -> Path:
    output = _resolved_directory(path, "--output")
    if any(output.iterdir()):
        raise RustReferenceError(f"--output must be empty: {output}")
    return output


def _stage(rustdoc: Path, output: Path) -> Path:
    destination = output / "reference" / "rust" / "api"
    destination.parent.mkdir(parents=True, exist_ok=False)
    shutil.copytree(rustdoc, destination, symlinks=False)
    _validate_rustdoc_root(destination)
    return destination


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustdoc-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument(
        "--update-index",
        action="store_true",
        help="update the committed classified facade landing before staging",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        rustdoc = _validate_rustdoc_root(args.rustdoc_root)
        output = _empty_output(args.output)
        modules, items = _load_facade()
        expected = _render_landing(rustdoc, modules, items)
        _update_or_check_landing(expected, args.update_index)
        destination = _stage(rustdoc, output)
    except (OSError, RustReferenceError) as error:
        print(f"Rust reference: {error}", file=sys.stderr)
        return 1
    print(
        "Rust reference: staged complete facade rustdoc at "
        f"{destination} ({len(modules)} modules + {len(items)} items = 206 paths)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
