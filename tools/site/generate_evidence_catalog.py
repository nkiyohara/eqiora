#!/usr/bin/env python3
"""Project a complete eqiora-verify JSON index into deterministic site MDX."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path, PurePosixPath
from typing import Any

SCHEMA = "eqiora.capability-evidence-index/v3"
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
    """The input is not the complete successful capability-index contract."""


def _closed_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise CatalogError(f"JSON object repeats key {key!r}")
        result[key] = value
    return result


def _parse_json(source: str) -> Any:
    return json.loads(
        source,
        object_pairs_hook=_closed_json_object,
        parse_constant=lambda value: (_ for _ in ()).throw(
            CatalogError(f"JSON constant {value!r} is not supported")
        ),
    )


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
        ord(character) < 0x20 or 0x7F <= ord(character) <= 0x9F
        for character in value
    ):
        raise CatalogError(f"{context} must be non-empty text without controls")
    return value


def _text_list(value: Any, context: str) -> list[str]:
    if not isinstance(value, list):
        raise CatalogError(f"{context} must be an array")
    result = [_text(item, f"{context}[]") for item in value]
    if len(result) != len(set(result)):
        raise CatalogError(f"{context} must not contain duplicates")
    return result


def _manifest(value: Any, case: str, context: str) -> str:
    manifest = _text(value, context)
    path = PurePosixPath(manifest)
    if (
        path.is_absolute()
        or manifest != path.as_posix()
        or ".." in path.parts
        or len(path.parts) < 4
        or path.parts[0] != "verify"
        or path.name != "case.toml"
    ):
        raise CatalogError(f"{context} must name a normalized verify/**/case.toml path")

    case_parts = case.split(".", 1)
    if len(case_parts) != 2 or tuple(path.parts[-3:-1]) != tuple(case_parts):
        raise CatalogError(f"{context} is retargeted away from case {case!r}")
    return manifest


def _evidence_label(value: Any, context: str) -> str:
    if value is None:
        return "No executable target"
    if not isinstance(value, dict):
        raise CatalogError(f"{context} must be null or an object")

    environment = _text(value.get("environment", "host-cpu"), f"{context}.environment")
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


_MDX_TEXT_ESCAPES = str.maketrans(
    {
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        "\\": "&#92;",
        "`": "&#96;",
        "*": "&#42;",
        "$": "&#36;",
        "{": "&#123;",
        "}": "&#125;",
        "[": "&#91;",
        "]": "&#93;",
        "|": "&#124;",
        "~": "&#126;",
    }
)


def _cell(value: str) -> str:
    escaped = []
    for offset, character in enumerate(value):
        if character == "_" and not (
            offset > 0
            and offset + 1 < len(value)
            and value[offset - 1].isalnum()
            and value[offset + 1].isalnum()
        ):
            escaped.append("&#95;")
        else:
            escaped.append(character.translate(_MDX_TEXT_ESCAPES))
    return "".join(escaped)


def _jsx_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def render_catalog(document: Any) -> str:
    """Validate and render one complete index without consulting other state."""

    root = _object(document, "index", ROOT_KEYS)
    if root["schema"] != SCHEMA:
        raise CatalogError(f"index.schema must be {SCHEMA!r}")
    if root["selected_capability"] is not None:
        raise CatalogError("index must not be capability-filtered")
    if root["success"] is not True:
        raise CatalogError("index.success must be true")
    if root["errors"] != []:
        raise CatalogError("index.errors must be empty")
    if not isinstance(root["entries"], list) or not root["entries"]:
        raise CatalogError("index.entries must be a non-empty array")

    grouped: dict[str, list[dict[str, str]]] = defaultdict(list)
    seen: set[tuple[str, str]] = set()
    input_order: list[tuple[str, str, str]] = []
    for offset, raw_entry in enumerate(root["entries"]):
        context = f"index.entries[{offset}]"
        entry = _object(raw_entry, context, ENTRY_KEYS)
        capability = _text(entry["capability"], f"{context}.capability")
        case = _text(entry["case"], f"{context}.case")
        manifest = _manifest(entry["manifest"], case, f"{context}.manifest")
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
        input_order.append((capability, case, manifest))
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

    if input_order != sorted(input_order):
        raise CatalogError("index.entries is not in canonical capability/case/manifest order")

    lines = [
        "---",
        "title: Evidence catalog",
        "description: Exact capability-to-case projection; manifests remain authoritative.",
        "---",
        "",
        "import ExactSourceLink from '@components/site/ExactSourceLink.astro';",
        "",
        "{/* Generated by tools/site/generate_evidence_catalog.py; do not edit. */}",
        "",
        f"Schema `{SCHEMA}` · **{len(seen)}** capability-to-case entries.",
        "",
        "This projection does not widen a case claim. The linked manifests remain authoritative.",
        "",
    ]
    for capability in sorted(grouped):
        lines.extend(
            [
                f"## `{_cell(capability)}`",
                "",
                "| Case | Status | Reference | Conformance kits | Target |",
                "| --- | --- | --- | --- | --- |",
            ]
        )
        for entry in sorted(
            grouped[capability], key=lambda item: (item["case"], item["manifest"])
        ):
            case_link = (
                f"<ExactSourceLink path={{{_jsx_string(entry['manifest'])}}} "
                f"kind=\"blob\"><code>{_cell(entry['case'])}</code></ExactSourceLink>"
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


def _write_atomic(output: Path, rendered: bytes) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    descriptor = -1
    temporary: Path | None = None
    try:
        descriptor, name = tempfile.mkstemp(
            prefix=f".{output.name}.", suffix=".tmp", dir=output.parent
        )
        temporary = Path(name)
        with os.fdopen(descriptor, "wb") as destination:
            descriptor = -1
            destination.write(rendered)
            destination.flush()
            os.fsync(destination.fileno())
            os.fchmod(destination.fileno(), 0o644)
        os.replace(temporary, output)
        temporary = None
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--input", type=Path, help="complete index JSON")
    source.add_argument(
        "--repository", type=Path, help="derive the index with eqiora-verify"
    )
    parser.add_argument("--output", required=True, type=Path, help="canonical MDX output")
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare exact bytes without creating or changing the output",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        source = (
            args.input.read_text(encoding="utf-8")
            if args.input is not None
            else subprocess.check_output(
                [
                    "cargo",
                    "run",
                    "--locked",
                    "-p",
                    "eqiora-verify",
                    "--",
                    "index",
                    "--format",
                    "json",
                ],
                cwd=args.repository,
                encoding="utf-8",
            )
        )
        rendered = render_catalog(_parse_json(source)).encode("utf-8")
        if args.check:
            tracked = args.output.read_bytes()
            if tracked != rendered:
                raise CatalogError(f"{args.output} is not the canonical evidence catalog")
        else:
            _write_atomic(args.output, rendered)
    except (
        OSError,
        UnicodeError,
        json.JSONDecodeError,
        CatalogError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"evidence catalog: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
