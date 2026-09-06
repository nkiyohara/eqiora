"""Ordinary rendering checks; requires the declared plotting extra and ffmpeg."""

import importlib.util
import json
from pathlib import Path
import subprocess
import unittest

import numpy as np
from PIL import Image


ROOT = Path(__file__).resolve().parents[3]
ASSETS = ROOT / "docs/site/src/assets/gallery"
SPEC = importlib.util.spec_from_file_location(
    "startup_media", ROOT / "tools/site/produce_transient_cylinder_startup_media.py"
)
startup = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(startup)


class GalleryMediaTests(unittest.TestCase):
    def test_startup_panels_preserve_domain_and_color_scales(self):
        coordinates = np.array([[0, 0], [2.2, 0], [2.2, 0.41], [0, 0.41]])
        triangles = np.array([[0, 1, 2], [0, 2, 3]])
        figure = startup._render_frame(
            coordinates, triangles, np.array([2.0, -2.0]), np.zeros(2),
            magnitude=3, delta_magnitude=4, step=1, time_s=0.01,
        )
        for axes, scale in ((figure.axes[0], 3), (figure.axes[2], 4)):
            self.assertEqual(axes.get_xlim(), (0, 2.2))
            self.assertEqual(axes.get_ylim(), (0, 0.41))
            self.assertEqual(axes.get_aspect(), 1)
            self.assertEqual(axes.collections[0].get_clim(), (-scale, scale))
            self.assertEqual((axes.get_xlabel(), axes.get_ylabel()), ("x [m]", "y [m]"))
        self.assertGreater(figure.axes[0].get_position().y0, figure.axes[2].get_position().y0)

    def test_published_pngs_decode_and_have_visible_content(self):
        self.assertFalse((ASSETS / "exact-cylinder-pressure.png").exists())
        for path in ASSETS.glob("*.png"):
            with self.subTest(path=path.name), Image.open(path) as image:
                image.load()
                self.assertGreaterEqual(min(image.size), 300)
                self.assertLessEqual(max(image.size), 4096)
                self.assertTrue(any(lo != hi for lo, hi in image.convert("RGB").getextrema()))
        with Image.open(ASSETS / "exact-cylinder-pressure-presentation.png") as image:
            self.assertGreater(image.width / image.height, 3)

    def test_both_videos_decode_ten_frames_and_match_poster_geometry(self):
        with Image.open(ASSETS / "transient-cylinder-startup-poster.png") as poster:
            size = poster.size
        for suffix in ("mp4", "webm"):
            with self.subTest(format=suffix):
                result = subprocess.run([
                    "ffprobe", "-v", "error", "-count_frames", "-select_streams", "v:0",
                    "-show_entries", "stream=width,height,nb_read_frames,r_frame_rate",
                    "-of", "json", str(ASSETS / f"transient-cylinder-startup.{suffix}"),
                ], check=True, capture_output=True, text=True)
                stream, = json.loads(result.stdout)["streams"]
                self.assertEqual((stream["width"], stream["height"]), size)
                self.assertEqual(stream["nb_read_frames"], "10")
                self.assertEqual(stream["r_frame_rate"], "2/1")


if __name__ == "__main__":
    unittest.main()
