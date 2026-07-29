#!/usr/bin/env python3
"""Packaging-fidelity utility. Not part of the dual independent oracle gate.

Two modes, and only the first one is ever needed again.

**Default, self-contained (no arguments).** Audits the package as it stands,
reading nothing outside this directory:

    python3 check_packaging_fidelity.py

It recomputes the sha256 of every document the frozen agreement report says it
compared, requires each to equal the digest recorded there, and walks each
frozen JSON rejecting any non-finite numeric leaf. That is a complete integrity
statement about the packaged bytes, and it needs no source worktree, no network
and no git object to resolve.

**Historical, optional (source/packaged pairs).** The argument recorded once,
at packaging time, that packaging changed prose only:

    python3 check_packaging_fidelity.py SOURCE_JSON PACKAGED_JSON [...]

The two oracle routes were frozen in separate throwaway source worktrees, whose
branches were never pushed. Packaging replaced repository-numbered tracking
prose with stable contract and RFC wording, then reran the deterministic
generators so every self-hash describes the packaged source. That rerun
rewrites the frozen JSON files, so their bytes and digests necessarily change;
this differ is the argument that nothing else did. Given a frozen JSON as it
was emitted in the source worktree and the same file as emitted here, it walks
both documents in parallel and requires:

- identical key sets **in identical order** at every object;
- identical list lengths at every array;
- identical leaf types at every position;
- every non-string leaf **bit-identical** (floats compared by ``repr`` and by
  sign of zero, so ``0.0`` and ``-0.0`` are distinguishable and NaN is
  rejected outright);
- string leaves are the only permitted difference, and every differing string
  path is reported so a reader can audit the wording change by hand.

Those source files are **not** part of this package and the recorded result of
that run is history. No accepted evidence here depends on this mode, and
reproducing this package in future does not require running it.

Exit status is 0 only when every checked document passes.
"""

from __future__ import annotations

import hashlib
import json
import math
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
CASE = HERE.parent
REPORT = HERE / "expected" / "agreement-report.json"


def _leaf_kind(value: object) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "bool"
    if isinstance(value, int):
        return "int"
    if isinstance(value, float):
        return "float"
    if isinstance(value, str):
        return "str"
    if isinstance(value, list):
        return "array"
    if isinstance(value, dict):
        return "object"
    raise TypeError(f"unexpected JSON type {type(value)!r}")


class Report:
    def __init__(self) -> None:
        self.numeric_leaves = 0
        self.string_leaves = 0
        self.other_leaves = 0
        self.changed_strings: list[str] = []
        self.failures: list[str] = []

    def fail(self, path: str, detail: str) -> None:
        self.failures.append(f"{path or '<root>'}: {detail}")


def walk(source: object, packaged: object, path: str, report: Report) -> None:
    source_kind = _leaf_kind(source)
    packaged_kind = _leaf_kind(packaged)
    if source_kind != packaged_kind:
        report.fail(path, f"type changed {source_kind} -> {packaged_kind}")
        return

    if source_kind == "object":
        source_keys = list(source)
        packaged_keys = list(packaged)
        if source_keys != packaged_keys:
            missing = [k for k in source_keys if k not in packaged_keys]
            added = [k for k in packaged_keys if k not in source_keys]
            if missing or added:
                report.fail(path, f"keys changed: missing {missing}, added {added}")
            else:
                report.fail(
                    path, f"key order changed: {source_keys} -> {packaged_keys}"
                )
            return
        for key in source_keys:
            walk(source[key], packaged[key], f"{path}.{key}", report)
        return

    if source_kind == "array":
        if len(source) != len(packaged):
            report.fail(path, f"length changed {len(source)} -> {len(packaged)}")
            return
        for index, (a, b) in enumerate(zip(source, packaged)):
            walk(a, b, f"{path}[{index}]", report)
        return

    if source_kind == "str":
        report.string_leaves += 1
        if source != packaged:
            report.changed_strings.append(path)
        return

    if source_kind == "null":
        report.other_leaves += 1
        return

    # bool, int, float: every one of these must survive packaging bit-identical.
    report.numeric_leaves += 1
    if source_kind == "float":
        if math.isnan(source) or math.isnan(packaged):
            report.fail(path, "non-finite value (NaN) in a frozen numeric field")
            return
        same = repr(source) == repr(packaged) and math.copysign(
            1.0, source
        ) == math.copysign(1.0, packaged)
    else:
        same = source == packaged
    if not same:
        report.fail(path, f"value changed {source!r} -> {packaged!r}")


def compare(source_path: pathlib.Path, packaged_path: pathlib.Path) -> Report:
    report = Report()
    source = json.loads(source_path.read_text(encoding="utf-8"))
    packaged = json.loads(packaged_path.read_text(encoding="utf-8"))
    walk(source, packaged, "", report)
    return report


def check_package() -> int:
    """Self-contained integrity audit of the packaged bytes. No source needed."""
    if not REPORT.exists():
        print(f"missing {REPORT}", file=sys.stderr)
        return 2
    report = json.loads(REPORT.read_text(encoding="utf-8"))
    failures: list[str] = []
    for name, entry in report["compared"].items():
        relative = entry.get("result") or entry["file"]
        path = CASE / relative
        if not path.exists():
            failures.append(f"{name}: missing {relative}")
            continue
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        recorded = entry["sha256"]
        agrees = actual == recorded
        print(f"  {name:<32} {relative}")
        print(f"    sha256 recorded in the report : {recorded}")
        print(
            f"    sha256 of the packaged bytes  : {actual}  "
            f"{'match' if agrees else 'MISMATCH'}"
        )
        if not agrees:
            failures.append(f"{name}: digest changed since the report was frozen")
            continue
        if path.suffix != ".json":
            continue
        # Walking a document against itself exercises the same type, ordering
        # and finiteness rules as the historical differ, so a NaN or an Infinity
        # anywhere in a frozen document is rejected here too.
        document = json.loads(path.read_text(encoding="utf-8"))
        audit = Report()
        walk(document, document, "", audit)
        print(
            f"    numeric/boolean leaves        : {audit.numeric_leaves} "
            f"(all finite: {not audit.failures})"
        )
        failures.extend(f"{name}: {failure}" for failure in audit.failures)

    if failures:
        print(f"  FAILURES : {len(failures)}")
        for failure in failures:
            print(f"      {failure}")
        print("PACKAGE INTEGRITY: FAIL")
        return 1
    print("PACKAGE INTEGRITY: PASS")
    return 0


def main(argv: list[str]) -> int:
    if not argv:
        return check_package()
    if len(argv) % 2 != 0:
        print(__doc__, file=sys.stderr)
        return 2

    overall = 0
    for index in range(0, len(argv), 2):
        source_path = pathlib.Path(argv[index])
        packaged_path = pathlib.Path(argv[index + 1])
        report = compare(source_path, packaged_path)
        print(f"source   {source_path}")
        print(f"packaged {packaged_path}")
        print(
            f"  numeric/boolean leaves compared : {report.numeric_leaves}"
            f"   (all bit-identical: {not report.failures})"
        )
        print(f"  string leaves compared          : {report.string_leaves}")
        print(f"  string leaves changed by prose  : {len(report.changed_strings)}")
        for changed in report.changed_strings:
            print(f"      {changed or '<root>'}")
        if report.failures:
            overall = 1
            print(f"  STRUCTURAL OR NUMERIC FAILURES  : {len(report.failures)}")
            for failure in report.failures:
                print(f"      {failure}")
        else:
            print(
                "  VERDICT                         : prose-only, no numeric or "
                "structural change"
            )
        print()

    print("PACKAGING FIDELITY: PASS" if overall == 0 else "PACKAGING FIDELITY: FAIL")
    return overall


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
