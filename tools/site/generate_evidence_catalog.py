#!/usr/bin/env python3
"""Render the validated eqiora-verify capability index as stable Markdown."""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path, PurePosixPath
from typing import Any
from urllib.parse import quote

SCHEMA = "eqiora.capability-evidence-index/v3"
REPOSITORY_BLOB = "https://github.com/nkiyohara/eqiora/blob/main/"
ROOT_KEYS = {
    "schema",
    "selected_capability",
    "success",
    "entries",
    "errors",
}
ENTRY_KEYS = {
    "capability",
    "case",
    "manifest",
    "status",
    "reference_kind",
    "conformance_kits",
    "evidence",
}
STATUSES = {"proposed", "specified", "implemented", "verified", "validated"}
ENVIRONMENTS = {"host-cpu", "physical-mpi-cuda"}


class CatalogError(ValueError):
    """The input is not the complete successful capability index contract."""


def _object(value: Any, context: str, keys: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise CatalogError(f"{context} must be an object")
    actual = set(value)
    if actual != keys:
        missing = sorted(keys - actual)
        unknown = sorted(actual - keys)
        raise CatalogError(
            f"{context} has invalid keys; missing={missing}, unknown={unknown}"
        )
    return value


def _text(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value or any(
        ord(character) < 0x20 for character in value
    ):
        raise CatalogError(f"{context} must be non-empty printable text")
    return value


def _text_list(value: Any, context: str) -> list[str]:
    if not isinstance(value, list):
        raise CatalogError(f"{context} must be an array")
    result = [_text(item, f"{context}[]") for item in value]
    if len(result) != len(set(result)):
        raise CatalogError(f"{context} must not contain duplicates")
    return result


def _manifest(value: Any, context: str) -> str:
    manifest = _text(value, context)
    path = PurePosixPath(manifest)
    if (
        path.is_absolute()
        or ".." in path.parts
        or len(path.parts) < 3
        or path.parts[0] != "verify"
        or path.name != "case.toml"
    ):
        raise CatalogError(f"{context} must name a verify/**/case.toml path")
    return manifest


def _evidence_label(value: Any, context: str) -> str:
    if value is None:
        return "No executable target"
    if not isinstance(value, dict):
        raise CatalogError(f"{context} must be null or an object")

    environment = value.get("environment", "host-cpu")
    if environment not in ENVIRONMENTS:
        raise CatalogError(f"{context}.environment is not recognized")

    runner = value.get("runner")
    if runner is not None:
        runner = _text(runner, f"{context}.runner")
        if runner == "python-installed-wheel":
            required = {"runner", "script"}
            optional = {"environment"}
            if not required <= set(value) or not set(value) <= required | optional:
                raise CatalogError(f"{context} is not a Python evidence target")
            script = _text(value["script"], f"{context}.script")
            suffix = "" if environment == "host-cpu" else f" [{environment}]"
            return f"{runner}: {script}{suffix}"
        if runner != "cargo-library-test":
            raise CatalogError(f"{context}.runner is not supported")

    required = {"package", "test"}
    optional = {"features", "table", "environment"}
    if runner is not None:
        required.add("runner")
    actual = set(value)
    if not required <= actual or not actual <= required | optional:
        raise CatalogError(f"{context} is not a Cargo evidence target")
    package = _text(value["package"], f"{context}.package")
    test = _text(value["test"], f"{context}.test")
    details = []
    if "features" in value:
        features = _text_list(value["features"], f"{context}.features")
        if features:
            details.append(f"features: {', '.join(features)}")
    if "table" in value:
        details.append(f"table: {_text(value['table'], f'{context}.table')}")
    if environment != "host-cpu":
        details.append(f"environment: {environment}")
    suffix = f" ({'; '.join(details)})" if details else ""
    label = "Cargo" if runner is None else runner
    return f"{label}: {package}/{test}{suffix}"


def _cell(value: str) -> str:
    return value.replace("\\", "\\\\").replace("|", "\\|").replace("\n", " ")


def _manifest_url(manifest: str) -> str:
    return REPOSITORY_BLOB + quote(manifest, safe="/")


def render_catalog(document: Any) -> str:
    """Validate and render one complete index without consulting repository state."""

    root = _object(document, "index", ROOT_KEYS)
    if root["schema"] != SCHEMA:
        raise CatalogError(f"index.schema must be {SCHEMA!r}")
    if root["selected_capability"] is not None:
        raise CatalogError("index must not be capability-filtered")
    if root["success"] is not True:
        raise CatalogError("index.success must be true")
    if root["errors"] != []:
        raise CatalogError("index.errors must be empty")
    if not isinstance(root["entries"], list):
        raise CatalogError("index.entries must be an array")

    grouped: dict[str, list[dict[str, str]]] = defaultdict(list)
    seen: set[tuple[str, str]] = set()
    for offset, raw_entry in enumerate(root["entries"]):
        context = f"index.entries[{offset}]"
        entry = _object(raw_entry, context, ENTRY_KEYS)
        capability = _text(entry["capability"], f"{context}.capability")
        case = _text(entry["case"], f"{context}.case")
        manifest = _manifest(entry["manifest"], f"{context}.manifest")
        status = _text(entry["status"], f"{context}.status")
        if status not in STATUSES:
            raise CatalogError(f"{context}.status is not recognized")
        reference = _text(entry["reference_kind"], f"{context}.reference_kind")
        kits = _text_list(entry["conformance_kits"], f"{context}.conformance_kits")
        target = _evidence_label(entry["evidence"], f"{context}.evidence")
        identity = (capability, case)
        if identity in seen:
            raise CatalogError(f"duplicate capability/case entry: {identity!r}")
        seen.add(identity)
        grouped[capability].append(
            {
                "case": case,
                "manifest": manifest,
                "status": status,
                "reference": reference,
                "kits": ", ".join(kits) if kits else "—",
                "target": target,
            }
        )

    lines = [
        "# Evidence catalog",
        "",
        "<!-- Generated by tools/site/generate_evidence_catalog.py; do not edit. -->",
        "",
        "This catalog is a deterministic projection of the validated",
        f"`{SCHEMA}` index. It contains **{len(seen)}** capability-to-case",
        "entries. Case manifests and their referenced evidence remain authoritative;",
        "an entry here does not widen its bounded claim.",
        "",
    ]
    if not grouped:
        lines.extend(["No capability evidence is registered.", ""])
        return "\n".join(lines)

    for capability in sorted(grouped):
        lines.extend(
            [
                f"## `{_cell(capability)}`",
                "",
                "| Case | Status | Reference | Conformance kits | Target |",
                "|---|---|---|---|---|",
            ]
        )
        for entry in sorted(
            grouped[capability], key=lambda item: (item["case"], item["manifest"])
        ):
            case_link = (
                f"[`{_cell(entry['case'])}`]"
                f"({_manifest_url(entry['manifest'])})"
            )
            lines.append(
                "| "
                + " | ".join(
                    [
                        case_link,
                        _cell(entry["status"]),
                        _cell(entry["reference"]),
                        _cell(entry["kits"]),
                        _cell(entry["target"]),
                    ]
                )
                + " |"
            )
        lines.append("")
    return "\n".join(lines)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--input",
        type=Path,
        help="index JSON path; stdin when omitted",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Markdown path; stdout when omitted",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        source = args.input.read_text(encoding="utf-8") if args.input else sys.stdin.read()
        rendered = render_catalog(json.loads(source))
    except (OSError, json.JSONDecodeError, CatalogError) as error:
        print(f"evidence catalog: {error}", file=sys.stderr)
        return 1

    if args.output:
        try:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(rendered, encoding="utf-8")
        except OSError as error:
            print(f"evidence catalog: {error}", file=sys.stderr)
            return 1
    else:
        print(rendered, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
