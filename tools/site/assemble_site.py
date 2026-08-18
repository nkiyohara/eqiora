#!/usr/bin/env python3
"""Assemble the checked Astro, rustdoc, and control-schema projections."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import stat
import sys
import unicodedata
from dataclasses import dataclass
from pathlib import Path, PurePosixPath


class AssemblyError(RuntimeError):
    """A source or destination violates the static-site assembly contract."""


@dataclass(frozen=True)
class CopyEntry:
    source: Path
    destination: PurePosixPath


def _regular_file(path: Path, label: str) -> Path:
    try:
        details = path.lstat()
    except FileNotFoundError as error:
        raise AssemblyError(f"missing {label}: {path}") from error
    if not stat.S_ISREG(details.st_mode) or path.is_symlink():
        raise AssemblyError(f"{label} must be a regular file: {path}")
    return path.resolve(strict=True)


def _directory(path: Path, label: str) -> Path:
    try:
        details = path.lstat()
    except FileNotFoundError as error:
        raise AssemblyError(f"missing {label}: {path}") from error
    if not stat.S_ISDIR(details.st_mode) or path.is_symlink():
        raise AssemblyError(f"{label} must be a real directory: {path}")
    return path.resolve(strict=True)


def _destination(path: PurePosixPath) -> PurePosixPath:
    if path.is_absolute() or not path.parts or any(part in {"", ".", ".."} for part in path.parts):
        raise AssemblyError(f"unsafe assembly destination: {path}")
    return path


def _tree_entries(source: Path, prefix: PurePosixPath) -> list[CopyEntry]:
    entries: list[CopyEntry] = []
    for current, directories, files in os.walk(source, followlinks=False):
        current_path = Path(current)
        directories.sort()
        files.sort()
        for name in [*directories, *files]:
            candidate = current_path / name
            details = candidate.lstat()
            if stat.S_ISLNK(details.st_mode):
                raise AssemblyError(f"assembly input contains a symlink: {candidate}")
            if name in directories:
                if not stat.S_ISDIR(details.st_mode):
                    raise AssemblyError(f"assembly input contains a non-directory: {candidate}")
                continue
            if not stat.S_ISREG(details.st_mode):
                raise AssemblyError(f"assembly input contains a non-regular file: {candidate}")
            relative = PurePosixPath(candidate.relative_to(source).as_posix())
            entries.append(CopyEntry(candidate, _destination(prefix / relative)))
    return entries


def _collision_key(path: PurePosixPath) -> str:
    return unicodedata.normalize("NFC", path.as_posix()).casefold()


def _manifest(entries: list[CopyEntry]) -> list[CopyEntry]:
    exact: dict[PurePosixPath, Path] = {}
    portable: dict[str, PurePosixPath] = {}
    for entry in entries:
        previous = exact.get(entry.destination)
        if previous is not None:
            raise AssemblyError(
                f"output collision at {entry.destination}: {previous} and {entry.source}"
            )
        collision_key = _collision_key(entry.destination)
        other = portable.get(collision_key)
        if other is not None and other != entry.destination:
            raise AssemblyError(f"case/Unicode output collision: {other} and {entry.destination}")
        exact[entry.destination] = entry.source
        portable[collision_key] = entry.destination
    for destination in exact:
        for parent in destination.parents:
            if not parent.parts:
                continue
            if parent in exact:
                raise AssemblyError(f"file/directory output collision: {parent} and {destination}")
            portable_parent = portable.get(_collision_key(parent))
            if portable_parent is not None:
                raise AssemblyError(
                    f"portable file/directory output collision: {portable_parent} and {destination}"
                )
    return sorted(entries, key=lambda entry: entry.destination.as_posix())


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def assemble(
    *,
    astro_root: Path,
    rustdoc_root: Path,
    control_schema: Path,
    output: Path,
    scratch_root: Path,
) -> int:
    astro = _directory(astro_root, "Astro output")
    rustdoc = _directory(rustdoc_root, "rustdoc output")
    schema = _regular_file(control_schema, "control-v2 schema")
    scratch = _directory(scratch_root, "assembly scratch root")

    for required in (
        astro / "index.html",
        astro / "404.html",
        astro / "pagefind/pagefind.js",
        astro / "robots.txt",
        astro / "sitemap-index.xml",
    ):
        _regular_file(required, "required Astro artifact")
    _regular_file(rustdoc / "eqiora/index.html", "Eqiora rustdoc entry")
    if (rustdoc / "eqiora_mcp").exists():
        raise AssemblyError("rustdoc output contains the forbidden eqiora_mcp root")

    try:
        schema_document = json.loads(schema.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AssemblyError(f"control-v2 schema is not canonical UTF-8 JSON: {error}") from error
    if schema_document.get("$id") != "urn:eqiora:schema:control:compile-v2":
        raise AssemblyError("control-v2 schema has the wrong $id")

    output_path = output.resolve(strict=False)
    if output_path.exists() or output_path.is_symlink():
        raise AssemblyError(f"refusing to replace an existing output path: {output_path}")
    output_parent = output_path.parent
    if not output_parent.is_dir() or output_parent.is_symlink():
        raise AssemblyError(f"output parent must be an existing real directory: {output_parent}")
    resolved_parent = output_parent.resolve(strict=True)
    for source in (astro, rustdoc, schema):
        if source == resolved_parent or resolved_parent in source.parents or source in resolved_parent.parents:
            raise AssemblyError(f"assembly source overlaps the output tree: {source}")

    entries = _manifest(
        _tree_entries(astro, PurePosixPath())
        + _tree_entries(rustdoc, PurePosixPath("reference/rust/api"))
        + [
            CopyEntry(
                schema,
                PurePosixPath("reference/control-v2/compile-v2.schema.json"),
            )
        ]
    )

    stage = scratch / "assembled-site"
    if stage.exists() or stage.is_symlink():
        raise AssemblyError(f"assembly scratch child already exists: {stage}")
    if os.stat(scratch).st_dev != os.stat(resolved_parent).st_dev:
        raise AssemblyError("assembly scratch and output must share a filesystem for atomic installation")

    stage.mkdir(mode=0o755)
    try:
        for entry in entries:
            destination = stage.joinpath(*entry.destination.parts)
            destination.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
            with entry.source.open("rb") as source_file, destination.open("xb") as output_file:
                shutil.copyfileobj(source_file, output_file, length=1024 * 1024)
            destination.chmod(0o644)
            if _sha256(destination) != _sha256(entry.source):
                raise AssemblyError(f"copy digest mismatch at {entry.destination}")
        os.replace(stage, output_path)
    except BaseException:
        if stage.is_dir() and not stage.is_symlink():
            shutil.rmtree(stage)
        raise

    print(f"assembled {len(entries)} collision-free files at {output_path}")
    return len(entries)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--astro-root", type=Path, required=True)
    parser.add_argument("--rustdoc-root", type=Path, required=True)
    parser.add_argument("--control-schema", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scratch-root", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        assemble(
            astro_root=args.astro_root,
            rustdoc_root=args.rustdoc_root,
            control_schema=args.control_schema,
            output=args.output,
            scratch_root=args.scratch_root,
        )
    except (AssemblyError, OSError) as error:
        print(f"site assembly: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
