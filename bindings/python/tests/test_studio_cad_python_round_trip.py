"""Independent installed-wheel oracle for Studio-authored Python exports."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest


ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "verify/interfaces/studio-python-cad-round-trip/models"
V1_DIGEST = "919545f70118840c04da9715829deb2da947460a51311ebabec6a34038c66f36"
V2_DIGEST = "00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47"

INSPECTOR = r"""
import json
from pathlib import Path
import runpy
import sys

import eqiora


def rectangle():
    return eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(-2.0, 3.0),
        y_bounds=(-1.0, 2.0),
        plane_z=0.5,
        depth=4.0,
        modeling_tolerance=1e-9,
    )


def circular_cut():
    return eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(-0.04, 0.04),
        y_bounds=(-0.025, 0.025),
        plane_z=0.0,
        depth=0.02,
        modeling_tolerance=1e-10,
    ).circular_through_cut(
        center=(0.02, 0.0),
        radius=0.008,
        boolean_tolerance=1e-9,
    )


program = Path(sys.argv[1])
expected = rectangle() if sys.argv[2] == "v1" else circular_cut()
namespace = runpy.run_path(str(program))
assert {name for name in namespace if not name.startswith("__")} == {
    "eqiora",
    "authored_graph",
    "_expected_graph_digest",
}
actual = namespace["authored_graph"]

assert type(actual) is eqiora.geometry.CadAuthoredGraph
assert actual == expected
assert actual.canonical_bytes == expected.canonical_bytes
assert actual.graph_digest == expected.graph_digest
for attribute in (
    "x_bounds",
    "y_bounds",
    "plane_z",
    "extrusion_depth",
    "requested_modeling_tolerance",
    "requested_boolean_tolerance",
    "cut_center",
    "cut_radius",
    "bounds",
    "vertex_count",
    "edge_count",
    "face_count",
    "closed_shell_count",
    "body_count",
    "genus",
    "volume",
    "surface_area",
    "repair",
    "selection_names",
):
    assert getattr(actual, attribute) == getattr(expected, attribute), attribute

for name in expected.selection_names:
    actual_handle = actual.face_handle(name)
    expected_handle = expected.face_handle(name)
    assert actual_handle == expected_handle
    assert actual_handle.canonical_bytes == expected_handle.canonical_bytes
    assert actual_handle.graph_digest == actual.graph_digest
    assert actual_handle.provenance_key == name
    assert actual.resolve_face(actual_handle) == name
    assert actual.face_area(actual_handle) == expected.face_area(expected_handle)
    assert actual.face_boundary_loop_count(actual_handle) == expected.face_boundary_loop_count(
        expected_handle
    )
    assert actual.rectangular_face_vertices(
        actual_handle
    ) == expected.rectangular_face_vertices(expected_handle)
    assert actual.rectangular_face_centroid(
        actual_handle
    ) == expected.rectangular_face_centroid(expected_handle)
    assert actual.planar_face_outward_normal(
        actual_handle
    ) == expected.planar_face_outward_normal(expected_handle)

assert actual.build() == expected.build()
print(
    json.dumps(
        {
            "canonical_hex": actual.canonical_bytes.hex(),
            "digest": actual.graph_digest,
            "module_file": eqiora.__file__,
        },
        sort_keys=True,
    )
)
"""


CASES = (
    ("rectangle_extrusion.py", "v1", V1_DIGEST, 731),
    ("circular_through_cut.py", "v2", V2_DIGEST, 1292),
)


@pytest.mark.parametrize(("filename", "history", "digest", "canonical_size"), CASES)
def test_frozen_studio_export_executes_through_only_the_installed_public_api(
    tmp_path: Path,
    filename: str,
    history: str,
    digest: str,
    canonical_size: int,
) -> None:
    source_path = FIXTURES / filename
    source = source_path.read_bytes()

    assert len(source) <= 4096
    assert source.decode("utf-8").encode("utf-8") == source
    assert not source.startswith(b"\xef\xbb\xbf")
    assert b"\r" not in source
    assert b"\0" not in source
    assert source.endswith(b"\n") and not source.endswith(b"\n\n")

    text = source.decode("utf-8")
    assert text.count("import eqiora\n") == 1
    assert text.count("CadAuthoredGraph.rectangle_extrusion(") == 1
    assert text.count(".circular_through_cut(") == (history == "v2")
    assert "authored_graph =" in text
    assert digest in text
    assert str(ROOT) not in text
    for forbidden in (
        "decode_canonical",
        "_eqiora",
        "studio",
        "canonical_bytes",
        "canonical_graph",
        "subprocess",
        "pathlib",
    ):
        assert forbidden not in text

    export_directory = tmp_path / "export"
    working_directory = tmp_path / "empty-working-directory"
    export_directory.mkdir()
    working_directory.mkdir()
    copied = export_directory / "eqiora_authored_cad.py"
    shutil.copyfile(source_path, copied)

    environment = os.environ.copy()
    environment.pop("PYTHONPATH", None)
    environment.pop("PYTHONHOME", None)
    completed = subprocess.run(
        [sys.executable, "-I", "-c", INSPECTOR, str(copied), history],
        cwd=working_directory,
        env=environment,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )

    assert completed.stderr == ""
    result = json.loads(completed.stdout)
    assert result["digest"] == digest
    assert len(bytes.fromhex(result["canonical_hex"])) == canonical_size
    installed_module = Path(result["module_file"]).resolve()
    assert installed_module != ROOT and ROOT not in installed_module.parents
