#!/usr/bin/env python3
"""Generate the public Python API reference from shipped type stubs.

The generator deliberately parses source text with the Python standard
library. It never imports Eqiora, NumPy, PyTorch, or JAX, so generating the
reference cannot initialize the native module or an optional framework.
"""

from __future__ import annotations

import argparse
import ast
import difflib
import sys
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs/python/api.md"
GITHUB_BLOB = "https://github.com/nkiyohara/eqiora/blob/main"
MODULES = (
    ("eqiora", Path("bindings/python/python/eqiora/__init__.pyi")),
    (
        "eqiora.compatibility",
        Path("bindings/python/python/eqiora/compatibility.pyi"),
    ),
    ("eqiora.geometry", Path("bindings/python/python/eqiora/geometry.pyi")),
    ("eqiora.diff", Path("bindings/python/python/eqiora/diff.pyi")),
    ("eqiora.torch", Path("bindings/python/python/eqiora/torch.pyi")),
    ("eqiora.jax", Path("bindings/python/python/eqiora/jax.pyi")),
)


@dataclass
class Entry:
    """One public top-level declaration, preserving stub source order."""

    kind: str
    name: str
    nodes: list[ast.AST]


def visible_member(name: str) -> bool:
    """Return whether a class member belongs in the public signature index."""
    return not name.startswith("_") or (
        name.startswith("__") and name.endswith("__")
    )


def expression(node: ast.AST | None) -> str:
    if node is None:
        return "None"
    return ast.unparse(node)


def decorator_lines(
    decorators: list[ast.expr],
    *,
    indent: str = "",
) -> list[str]:
    return [f"{indent}@{expression(decorator)}" for decorator in decorators]


def function_signature(
    node: ast.FunctionDef | ast.AsyncFunctionDef,
    *,
    indent: str = "",
) -> list[str]:
    lines = decorator_lines(node.decorator_list, indent=indent)
    prefix = "async " if isinstance(node, ast.AsyncFunctionDef) else ""
    returns = (
        f" -> {expression(node.returns)}"
        if node.returns is not None
        else ""
    )
    lines.append(
        f"{indent}{prefix}def {node.name}({expression(node.args)})"
        f"{returns}: ..."
    )
    return lines


def assignment_signature(
    node: ast.Assign | ast.AnnAssign,
    *,
    indent: str = "",
) -> list[str]:
    if isinstance(node, ast.AnnAssign):
        target = expression(node.target)
        value = (
            f" = {expression(node.value)}"
            if node.value is not None
            else ""
        )
        return [
            f"{indent}{target}: {expression(node.annotation)}{value}"
        ]
    targets = " = ".join(expression(target) for target in node.targets)
    return [f"{indent}{targets} = {expression(node.value)}"]


def assignment_names(node: ast.Assign | ast.AnnAssign) -> list[str]:
    targets: list[ast.expr]
    if isinstance(node, ast.AnnAssign):
        targets = [node.target]
    else:
        targets = node.targets
    return [target.id for target in targets if isinstance(target, ast.Name)]


def class_signature(node: ast.ClassDef) -> list[str]:
    lines = decorator_lines(node.decorator_list)
    arguments = [expression(base) for base in node.bases]
    arguments.extend(
        f"{keyword.arg}={expression(keyword.value)}"
        for keyword in node.keywords
        if keyword.arg is not None
    )
    suffix = f"({', '.join(arguments)})" if arguments else ""
    lines.append(f"class {node.name}{suffix}:")

    members = 0
    for member in node.body:
        if isinstance(member, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if visible_member(member.name):
                lines.extend(function_signature(member, indent="    "))
                members += 1
        elif isinstance(member, (ast.Assign, ast.AnnAssign)):
            names = assignment_names(member)
            if names and all(visible_member(name) for name in names):
                lines.extend(assignment_signature(member, indent="    "))
                members += 1
    if members == 0:
        lines.append("    ...")
    return lines


def public_entries(tree: ast.Module) -> list[Entry]:
    entries: list[Entry] = []
    positions: dict[tuple[str, str], int] = {}
    for node in tree.body:
        if isinstance(node, ast.ClassDef) and not node.name.startswith("_"):
            key = ("class", node.name)
            if key in positions:
                entries[positions[key]].nodes = [node]
            else:
                positions[key] = len(entries)
                entries.append(Entry("class", node.name, [node]))
        elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.name.startswith("_"):
                continue
            key = ("function", node.name)
            if key in positions:
                entries[positions[key]].nodes.append(node)
            else:
                positions[key] = len(entries)
                entries.append(Entry("function", node.name, [node]))
        elif isinstance(node, (ast.Assign, ast.AnnAssign)):
            names = assignment_names(node)
            public = [name for name in names if not name.startswith("_")]
            if not public or public == ["__all__"]:
                continue
            name = ", ".join(public)
            entries.append(Entry("alias", name, [node]))
    return entries


def render_entry(module: str, entry: Entry) -> list[str]:
    qualified = f"{module}.{entry.name}"
    lines = [f"### `{qualified}`", "", "```python"]
    if entry.kind == "class":
        lines.extend(class_signature(entry.nodes[0]))
    elif entry.kind == "function":
        for index, node in enumerate(entry.nodes):
            if index:
                lines.append("")
            assert isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            lines.extend(function_signature(node))
    else:
        assert len(entry.nodes) == 1
        node = entry.nodes[0]
        assert isinstance(node, (ast.Assign, ast.AnnAssign))
        lines.extend(assignment_signature(node))
    lines.extend(["```", ""])
    return lines


def render() -> str:
    lines = [
        "<!-- Generated by tools/docs/generate_python_api.py. Do not edit. -->",
        "",
        "# Eqiora Python API",
        "",
        "This signature reference is generated deterministically from the public",
        "type-stub modules shipped in the `eqiora` distribution. It does not",
        "import Eqiora or any optional framework. Behavioral guidance lives in",
        f"[the Python guide]({GITHUB_BLOB}/docs/python/README.md).",
        "Leading-underscore names that occur inside a public signature are",
        "typing-only helpers, not additional public runtime objects.",
        "",
        "Regenerate with:",
        "",
        "```console",
        "python3 tools/docs/generate_python_api.py",
        "python3 tools/docs/generate_python_api.py --check",
        "```",
        "",
    ]
    for module, relative_path in MODULES:
        path = ROOT / relative_path
        source = path.read_text(encoding="utf-8")
        tree = ast.parse(source, filename=relative_path.as_posix())
        lines.extend(
            [
                f"## `{module}`",
                "",
                f"Source: [`{relative_path.as_posix()}`]"
                f"({GITHUB_BLOB}/{relative_path.as_posix()})",
                "",
            ]
        )
        for entry in public_entries(tree):
            lines.extend(render_entry(module, entry))
    rendered = "\n".join(lines).rstrip() + "\n"
    if "_eqiora" in rendered:
        raise ValueError("private native module leaked into public API reference")
    return rendered


def check(expected: str) -> int:
    if not OUTPUT.exists():
        print(f"{OUTPUT.relative_to(ROOT)} is missing", file=sys.stderr)
        return 1
    actual = OUTPUT.read_text(encoding="utf-8")
    if actual == expected:
        print(f"{OUTPUT.relative_to(ROOT)} is current")
        return 0
    diff = difflib.unified_diff(
        actual.splitlines(),
        expected.splitlines(),
        fromfile=OUTPUT.relative_to(ROOT).as_posix(),
        tofile="generated",
        lineterm="",
    )
    print("\n".join(diff), file=sys.stderr)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when docs/python/api.md differs from the public stubs",
    )
    arguments = parser.parse_args()
    expected = render()
    if arguments.check:
        return check(expected)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(expected, encoding="utf-8", newline="\n")
    print(f"wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
