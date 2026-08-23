"""Contract tests for the installed-wheel gallery evidence gate."""

from __future__ import annotations

import os
import sys
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import python_gallery_gate  # noqa: E402


class PythonGalleryGateTests(unittest.TestCase):
    def test_uv_command_installs_exact_renderer_and_noneditable_project(self) -> None:
        command = python_gallery_gate.uv_gate_command("uv")
        self.assertIn("--isolated", command)
        self.assertIn("--no-editable", command)
        extras = [
            command[index + 1]
            for index, value in enumerate(command)
            if value == "--extra"
        ]
        self.assertEqual(extras, ["gmsh", "matplotlib"])
        self.assertIn("matplotlib==3.11.1", command)
        self.assertEqual(command[command.index("--python") + 1], "3.13")
        self.assertTrue(command[-1].endswith("tools/ci/python_gallery_gate.py"))

    def test_child_environment_is_headless_and_reproducible(self) -> None:
        previous = {
            key: os.environ.get(key)
            for key in ("DISPLAY", "MATPLOTLIBRC", "PYTHONPATH")
        }
        try:
            os.environ.update(
                {
                    "DISPLAY": ":99",
                    "MATPLOTLIBRC": "/host/matplotlibrc",
                    "PYTHONPATH": "/host/python",
                }
            )
            environment = python_gallery_gate.child_environment(
                mplconfig=Path("/tmp/gallery-mpl"),
                inner=True,
                uv_cache=Path("/tmp/gallery-uv-cache"),
            )
        finally:
            for key, value in previous.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value
        self.assertNotIn("DISPLAY", environment)
        self.assertNotIn("MATPLOTLIBRC", environment)
        self.assertNotIn("PYTHONPATH", environment)
        self.assertEqual(environment["SOURCE_DATE_EPOCH"], "0")
        self.assertEqual(environment["TZ"], "UTC")
        self.assertEqual(environment["LC_ALL"], "C")
        self.assertEqual(environment["PYTHONHASHSEED"], "0")
        self.assertEqual(environment["UV_CACHE_DIR"], "/tmp/gallery-uv-cache")
        self.assertEqual(environment[python_gallery_gate.INNER], "1")


if __name__ == "__main__":
    unittest.main()
