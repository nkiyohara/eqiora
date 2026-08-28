#!/usr/bin/env python3
"""Product checks for the unverified transient-cylinder gallery sources."""

from __future__ import annotations

import ast
import json
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
PLAIN = ROOT / "examples/python/transient_cylinder_wake.py"
MARIMO = ROOT / "examples/python/transient_cylinder_wake_marimo.py"
NOTEBOOK = ROOT / "examples/python/transient_cylinder_wake_jupyter.ipynb"

PRESENTATION_CALLS = {
    "eqiora.geometry.GeometryGraph": 1,
    "geometry_graph.rectangle": 1,
    "geometry_graph.circle": 1,
    "geometry_graph.subtract": 1,
    "geometry_graph.build": 1,
    "eqiora.meshing.GmshMesher": 1,
    "eqiora.meshing.resolve": 1,
    "eqiora.meshing.generate": 1,
    "eqiora.compile": 2,
    "eqiora.solve.Linear": 1,
    "eqiora.fem.MiniP1": 2,
    "eqiora.resolve": 2,
    "eqiora.run": 2,
    "eqiora.time.BackwardEuler": 1,
    "eqiora.solve.Newton": 1,
    "eqiora.fluid.IncompressibleScaling": 1,
    "steady_result.output": 2,
    "eqiora.State.initial": 1,
    "eqiora.InitialField": 2,
    "result.trajectory.state": 1,
    "accepted.curl": 1,
    "geometry.selection": 1,
    "accepted.boundary_force": 1,
    "accepted.sample": 2,
    "vorticity.values": 1,
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


class TransientCylinderWakeGalleryProduct(unittest.TestCase):
    def setUp(self) -> None:
        if not PLAIN.is_file() or not MARIMO.is_file() or not NOTEBOOK.is_file():
            self.skipTest("consumer tree does not carry the checked-in wake sources")
        self.plain_source = PLAIN.read_text(encoding="utf-8")
        self.marimo_source = MARIMO.read_text(encoding="utf-8")
        self.notebook = json.loads(NOTEBOOK.read_text(encoding="utf-8"))
        self.notebook_source = notebook_source(self.notebook)

    def test_notebook_is_clean_valid_python(self) -> None:
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

    def test_three_sources_use_only_the_current_public_route(self) -> None:
        for source in (
            self.plain_source,
            self.marimo_source,
            self.notebook_source,
        ):
            for retired in (
                "CadAuthoredGraph",
                "MeshRequest",
                "TransientNavierStokesReference2d",
                "fluid.resolve",
                "steady-flow-past-cylinder.model.json",
                "_repr_mimebundle_",
                "IPython.display",
                "from examples",
            ):
                self.assertNotIn(retired, source)
            self.assertEqual(source.count("steady-flow-past-cylinder.eqi"), 1)
            self.assertEqual(source.count("transient-flow-past-cylinder.eqi"), 1)
            self.assertEqual(source.count("files(eqiora)"), 1)
            self.assertIn("UNVERIFIED PRODUCT EXAMPLE", source)

    def test_jupyter_and_marimo_share_the_public_composition(self) -> None:
        notebook_calls = call_inventory(self.notebook_source, NOTEBOOK.as_posix())
        marimo_calls = call_inventory(self.marimo_source, MARIMO.as_posix())
        for call, count in PRESENTATION_CALLS.items():
            self.assertEqual(notebook_calls[call], count, call)
            self.assertEqual(marimo_calls[call], count, call)

        ordered_markers = (
            "GeometryGraph()",
            ".subtract(",
            ".build(",
            "GmshMesher(",
            "meshing.resolve(",
            "meshing.generate(",
            "steady_model = eqiora.compile(",
            "steady_plan = eqiora.resolve(",
            "steady_result = eqiora.run(",
            "model = eqiora.compile(",
            "plan = eqiora.resolve(",
            "State.initial(",
            "result = eqiora.run(",
            ".curl(",
            ".boundary_force(",
            ".sample(",
            "plot_scalar_field(",
        )
        for source in (self.notebook_source, self.marimo_source):
            self.assertIn("maximum_target_size=0.05", source)
            cursor = 0
            for marker in ordered_markers:
                cursor = source.index(marker, cursor) + len(marker)

    def test_plain_script_contains_the_same_computational_path(self) -> None:
        self.assertIn("maximum_target_size=0.05", self.plain_source)
        plain_calls = call_inventory(self.plain_source, PLAIN.as_posix())
        for call, count in PRESENTATION_CALLS.items():
            expected = 2 if call == "result.trajectory.state" else count
            self.assertEqual(plain_calls[call], expected, call)


if __name__ == "__main__":
    unittest.main()
