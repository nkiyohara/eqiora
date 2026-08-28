#!/usr/bin/env python3
"""Product checks for the exact-cylinder Jupyter source."""

from __future__ import annotations

import ast
import json
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
NOTEBOOK = ROOT / "examples/python/exact_cylinder_stokes_jupyter.ipynb"
MARIMO = ROOT / "examples/python/exact_cylinder_stokes_marimo.py"

PUBLIC_CALLS = {
    "eqiora.geometry.GeometryGraph": 1,
    "geometry_graph.rectangle": 1,
    "geometry_graph.circle": 1,
    "geometry_graph.subtract": 1,
    "geometry_graph.build": 1,
    "eqiora.meshing.GmshMesher": 1,
    "eqiora.meshing.resolve": 1,
    "eqiora.meshing.generate": 1,
    "eqiora.compile": 1,
    "eqiora.solve.Linear": 1,
    "eqiora.fem.MiniP1": 1,
    "eqiora.resolve": 1,
    "eqiora.run": 1,
    "result.output": 1,
    "eqplot.plot_scalar_field": 1,
}


def notebook_source(notebook: dict[str, object]) -> str:
    cells = notebook["cells"]
    assert isinstance(cells, list)
    return "\n\n".join(
        "".join(cell["source"])
        for cell in cells
        if isinstance(cell, dict) and cell.get("cell_type") == "code"
    )


def qualified_name(node: ast.expr) -> str | None:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        owner = qualified_name(node.value)
        return f"{owner}.{node.attr}" if owner is not None else None
    return None


def call_inventory(source: str, filename: str) -> Counter[str]:
    tree = ast.parse(source, filename=filename)
    names = (
        qualified_name(node.func)
        for node in ast.walk(tree)
        if isinstance(node, ast.Call)
    )
    return Counter(name for name in names if name is not None)


class ExactCylinderStokesJupyterProduct(unittest.TestCase):
    def setUp(self) -> None:
        if not NOTEBOOK.is_file() or not MARIMO.is_file():
            self.skipTest("consumer tree does not carry the checked-in notebook sources")
        self.notebook = json.loads(NOTEBOOK.read_text(encoding="utf-8"))
        self.source = notebook_source(self.notebook)

    def test_notebook_is_clean_valid_python_without_retired_routes(self) -> None:
        self.assertEqual(self.notebook["nbformat"], 4)
        self.assertEqual(self.notebook["nbformat_minor"], 5)
        self.assertEqual(
            self.notebook["metadata"]["kernelspec"]["name"],  # type: ignore[index]
            "python3",
        )
        cell_ids = [cell["id"] for cell in self.notebook["cells"]]  # type: ignore[union-attr]
        self.assertEqual(len(cell_ids), len(set(cell_ids)))
        for cell in self.notebook["cells"]:  # type: ignore[union-attr]
            if cell["cell_type"] != "code":
                continue
            self.assertIsNone(cell["execution_count"])
            self.assertEqual(cell["outputs"], [])
            compile("".join(cell["source"]), NOTEBOOK.as_posix(), "exec")

        for retired in (
            "CadAuthoredGraph",
            "MeshRequest",
            "SteadyStokesPlan",
            "fluid.resolve",
            "fluid.SteadyStokes",
            "steady-flow-past-cylinder.model.json",
            "_repr_mimebundle_",
            "IPython.display",
            "from examples",
        ):
            self.assertNotIn(retired, self.source)

        self.assertEqual(self.source.count("steady-flow-past-cylinder.eqi"), 1)
        self.assertEqual(self.source.count("files(eqiora)"), 1)
        self.assertIn("result.plan_key == stokes_plan.identity", self.source)

    def test_jupyter_and_marimo_share_the_public_composition(self) -> None:
        notebook_calls = call_inventory(self.source, NOTEBOOK.as_posix())
        marimo_calls = call_inventory(
            MARIMO.read_text(encoding="utf-8"), MARIMO.as_posix()
        )
        for call, count in PUBLIC_CALLS.items():
            self.assertEqual(notebook_calls[call], count, call)
            self.assertEqual(marimo_calls[call], count, call)

        ordered_markers = (
            "GeometryGraph()",
            ".subtract(",
            ".build(",
            "GmshMesher(",
            "meshing.resolve(",
            "meshing.generate(",
            "eqiora.compile(",
            "eqiora.resolve(",
            "eqiora.run(",
            "result.output(",
            "plot_scalar_field(",
        )
        positions = [self.source.index(marker) for marker in ordered_markers]
        self.assertEqual(positions, sorted(positions))


if __name__ == "__main__":
    unittest.main()
