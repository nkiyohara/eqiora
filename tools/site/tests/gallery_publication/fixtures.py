from __future__ import annotations

import binascii
import copy
import functools
import hashlib
import json
import os
import struct
import subprocess
import zlib
from pathlib import Path

ENTRY_ID = "exact-cylinder-steady-stokes"
RECORD_SCHEMA = "eqiora.site.gallery-publication/v1"
PREDICATE_SCHEMA = "eqiora.site.gallery-publication-predicate/v1"
RECEIPT_SCHEMA = "eqiora.site.gallery-publication-receipt/v1"
RECEIPT_ID = "exact-cylinder-steady-stokes-publication-admission-v1"
FINAL_RECORD = "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json"
FINAL_MEDIA = "docs/site/src/assets/gallery/exact-cylinder-pressure.png"
WIDTH = 1280
HEIGHT = 832
DPI = 160
PNG_SOFTWARE = "Eqiora exact-cylinder gallery publication v1"

ALT_TEXT = (
    "Pressure in pascals for the frozen 2D steady-Stokes exact-cylinder "
    "demonstration, shown with a viridis color scale and the 104-triangle "
    "affine mesh overlaid. Presentation image only; linked Result evidence "
    "carries the numerical claim."
)
PUBLIC_CLAIM = (
    "one frozen 2D steady incompressible Stokes exact-cylinder demonstration, "
    "rendered from its accepted public Result path and linked evidence."
)
NONCLAIMS = [
    "no curved elements",
    "no mesh/PDE convergence",
    "no drag/lift coefficient, scaled or mesh-independent force, or DFG value",
    "no transient or Navier–Stokes behavior",
    "no vortex shedding",
    "no 3D",
    "no production mesher",
    "no performance claim",
    "no cross-platform/byte-reproducible result",
    "no pixel validation",
    "API presence is neither verification nor maturity",
]
SOURCE_ROLES = {
    "bindings/python/python/eqiora/matplotlib.py": ["plotting-adapter"],
    "examples/python/exact_cylinder_geometry.py": ["geometry-adapter"],
    "examples/python/exact_cylinder_mesh.py": ["mesh-realization-owner"],
    "examples/python/exact_cylinder_stokes.py": ["plain-python-snippet"],
    "examples/python/exact_cylinder_stokes_marimo.py": ["canonical-marimo-snippet"],
    "examples/steady-flow-past-cylinder.eqi": ["example-formula-owner"],
    "examples/steady-flow-past-cylinder.geometry.json": ["geometry"],
    "examples/steady-flow-past-cylinder.model.json": ["model", "scientific-formula"],
    "packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi": ["current-package-formula-owner"],
    "tools/site/produce_exact_cylinder_pressure.py": ["producer-command"],
    "verify/fluid/packaged-steady-stokes-2d/models/direct.eqi": ["packaged-stokes-formula"],
    "verify/fluid/packaged-steady-stokes-2d/package-v0.1.0/src/incompressible.eqi": ["package-formula-owner"],
}
CASE_ROLES = {
    "artifacts.current-model-canonical-identity": "evidence",
    "fluid.exact-circular-hole-stokes-2d": "evidence",
    "fluid.packaged-steady-stokes-2d": "evidence",
    "geometry.circular-hole-chordal-realization-binding": "evidence",
    "geometry.circular-hole-chordal-reference-mesh": "evidence",
    "geometry.exact-circular-hole-geometry": "evidence",
    "interfaces.python-circular-hole-chordal-mesh": "evidence",
    "interfaces.python-exact-circular-hole-geometry": "evidence",
    "interfaces.python-exact-cylinder-pressure-still": "presentation-only",
    "interfaces.python-exact-cylinder-stokes-marimo": "evidence",
    "interfaces.python-exact-cylinder-stokes-result": "evidence",
}
RECEIPT_CHECKS = [
    "canonical-payload-and-wrapper",
    "source-revision-tree-and-source-file-digests",
    "registered-case-identities-and-presentation-only-boundary",
    "model-realization-run-result-pressure-lineage",
    "png-structure-crc-decode-dimensions-and-digests",
    "exact-alt-caption-and-link-digests",
    "renderer-scene-profile-and-environment-identities",
    "claim-nonclaims-and-evidence-routes",
]
RECEIPT_NONCHECKS = [
    "image pixels are not scientific validation",
    "no cross-platform or byte-reproducible Result claim",
    "no new scientific oracle or equality",
]


def canonical_value(value) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def canonical_file(value) -> bytes:
    return canonical_value(value) + b"\n"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def identity(label: str) -> str:
    return sha(label.encode("utf-8"))


def chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", binascii.crc32(kind + data) & 0xFFFFFFFF)


@functools.cache
def png_bytes() -> tuple[bytes, bytes, tuple[str, ...]]:
    first = bytes((35, 90, 80, 255))
    second = bytes((245, 120, 75, 255))
    row_a = first * (WIDTH // 2) + second * (WIDTH - WIDTH // 2)
    row_b = second * (WIDTH // 2) + first * (WIDTH - WIDTH // 2)
    rows = [row_a if index % 2 == 0 else row_b for index in range(HEIGHT)]
    decoded = b"".join(rows)
    filtered = b"".join(b"\0" + row for row in rows)
    pixels_per_metre = round(DPI / 0.0254)
    parts = [
        b"\x89PNG\r\n\x1a\n",
        chunk(b"IHDR", struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 6, 0, 0, 0)),
        chunk(b"tEXt", b"Software\0" + PNG_SOFTWARE.encode("ascii")),
        chunk(b"pHYs", struct.pack(">IIB", pixels_per_metre, pixels_per_metre, 1)),
        chunk(b"IDAT", zlib.compress(filtered, 1)),
        chunk(b"IEND", b""),
    ]
    return b"".join(parts), decoded, ("IHDR", "tEXt", "pHYs", "IDAT", "IEND")


def case_path(case_id: str) -> str:
    area, name = case_id.split(".", 1)
    return f"verify/{area}/{name}/case.toml"


def dossier_route(case_id: str, revision: str) -> str:
    return (
        f"https://github.com/nkiyohara/eqiora/blob/{revision}/"
        f"{Path(case_path(case_id)).with_name('README.md').as_posix()}"
    )


class PublicationFixture:
    def __init__(self, root: Path):
        self.root = root
        self.candidate = root.parent / "candidate" / "exact-cylinder-pressure.png"
        self.receipt_path = root.parent / "admission" / "receipt.json"
        self.external_record = root.parent / "admission" / "publication.json"
        self.installed_record = root / FINAL_RECORD
        self.installed_media = root / FINAL_MEDIA
        self._create_source_revision()
        self.png, self.decoded_pixels, chunk_types = png_bytes()
        self.chunk_types = list(chunk_types)
        self.candidate.parent.mkdir(parents=True)
        self.candidate.write_bytes(self.png)
        self.payload = self._payload()
        self.receipt = self._receipt()
        self.wrapper = self._wrapper()
        self.write_external()

    def _create_source_revision(self) -> None:
        for index, path in enumerate(sorted(SOURCE_ROLES)):
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(f"synthetic source {index}: {path}\n".encode())
        for case_id in sorted(CASE_ROLES):
            target = self.root / case_path(case_id)
            target.parent.mkdir(parents=True, exist_ok=True)
            body = f'id = "{case_id}"\nstatus = "verified"\n'
            if case_id == "interfaces.python-exact-cylinder-pressure-still":
                body += (
                    "\n[claim_boundary]\n"
                    "media_admission = false\n"
                    "durable_image_provenance = false\n"
                    "reproducible_image_bytes = false\n"
                    "scientific_validation_from_pixels = false\n"
                )
            target.write_text(body, encoding="utf-8")
            target.with_name("README.md").write_text(f"# Synthetic dossier for {case_id}\n", encoding="utf-8")
        subprocess.run(["git", "init", "-q", self.root], check=True)
        subprocess.run(["git", "-C", self.root, "add", "."], check=True)
        environment = os.environ.copy()
        environment.update(
            {
                "GIT_AUTHOR_NAME": "Independent Fixture",
                "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
                "GIT_COMMITTER_NAME": "Independent Fixture",
                "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
            }
        )
        subprocess.run(
            [
                "git",
                "-C",
                self.root,
                "commit",
                "-q",
                "-m",
                "synthetic publication authority",
            ],
            check=True,
            env=environment,
        )
        self.revision = subprocess.check_output(["git", "-C", self.root, "rev-parse", "HEAD"], text=True).strip()
        self.tree = subprocess.check_output(["git", "-C", self.root, "rev-parse", "HEAD^{tree}"], text=True).strip()

    def _payload(self):
        source_files = []
        for path, roles in sorted(SOURCE_ROLES.items()):
            source_files.append(
                {
                    "path": path,
                    "roles": roles,
                    "sha256": sha((self.root / path).read_bytes()),
                }
            )
        cases = []
        for case_id, role in sorted(CASE_ROLES.items(), key=lambda item: case_path(item[0])):
            manifest = case_path(case_id)
            cases.append(
                {
                    "dossier_route": dossier_route(case_id, self.revision),
                    "id": case_id,
                    "manifest_path": manifest,
                    "manifest_sha256": sha((self.root / manifest).read_bytes()),
                    "role": role,
                }
            )
        digests = {
            "correspondence_digest": identity("correspondence"),
            "evidence_run_digest": identity("evidence-run"),
            "geometry_digest": identity("geometry"),
            "mesh_digest": identity("mesh"),
            "model_digest": identity("model"),
            "realization_digest": identity("realization"),
            "run_manifest_digest": identity("run-manifest"),
        }
        methods = {key: f"accepted.{key}" for key in digests}
        methods.update(
            {
                "pressure_blocks": "Result.field(FieldRef).ordered_block_digests",
                "pressure_output": "Result.field(FieldRef).ordered_output_digests",
                "pressure_snapshot": "Result.field(FieldRef).digest",
            }
        )
        caption = (
            "Pressure (Pa), frozen exact-cylinder steady-Stokes demonstration at "
            f"{self.revision}; presentation only, not validation."
        )
        return {
            "claim": {
                "case_dossier_routes": [dossier_route(case_id, self.revision) for case_id in sorted(CASE_ROLES)],
                "evidence_route": "/evidence/",
                "nonclaims": NONCLAIMS,
                "pixels_are_validation": False,
                "public_claim": PUBLIC_CLAIM,
            },
            "evidence_cases": cases,
            "lineage": {
                "chain": [
                    {
                        "from": digests["model_digest"],
                        "kind": "Model→Geometry",
                        "to": digests["geometry_digest"],
                    },
                    {
                        "from": digests["geometry_digest"],
                        "kind": "Geometry→Correspondence",
                        "to": digests["correspondence_digest"],
                    },
                    {
                        "from": digests["correspondence_digest"],
                        "kind": "Correspondence→Mesh",
                        "to": digests["mesh_digest"],
                    },
                    {
                        "from": digests["model_digest"],
                        "kind": "Model→Realization",
                        "to": digests["realization_digest"],
                    },
                    {
                        "from": digests["mesh_digest"],
                        "kind": "Mesh→Realization",
                        "to": digests["realization_digest"],
                    },
                    {
                        "from": digests["realization_digest"],
                        "kind": "Realization→Run",
                        "to": digests["run_manifest_digest"],
                    },
                    {
                        "from": digests["run_manifest_digest"],
                        "kind": "Run→ResultEvidence",
                        "to": digests["evidence_run_digest"],
                    },
                    {
                        "from": digests["run_manifest_digest"],
                        "kind": "Result→PressureSnapshot",
                        "to": identity("pressure-snapshot"),
                    },
                ],
                "identities": digests,
                "methods": methods,
                "pressure": {
                    "association": "vertex scalar",
                    "display_unit": "Pa",
                    "field": "pressure",
                    "frame_selection": "single steady result; temporal interval not applicable",
                    "mesh_digest": digests["mesh_digest"],
                    "ordered_block_digests": [identity("block-0")],
                    "ordered_output_digests": [identity("output-0")],
                    "snapshot_digest": identity("pressure-snapshot"),
                    "source_unit": "kg/(m*s^2)",
                    "value_range": {"maximum": 0.25, "minimum": -0.125},
                },
                "source_result": {
                    "digest": digests["run_manifest_digest"],
                    "digest_kind": "Result.run_manifest().digest",
                },
            },
            "media": {
                "bit_depth": 8,
                "byte_size": len(self.png),
                "chunk_types": self.chunk_types,
                "color_type": 6,
                "decoded_pixel_sha256": sha(self.decoded_pixels),
                "height": HEIGHT,
                "interlace": 0,
                "mime": "image/png",
                "nonblank": True,
                "path": FINAL_MEDIA,
                "pixel_mode": "RGBA",
                "sha256": sha(self.png),
                "width": WIDTH,
            },
            "renderer": {
                "backend": "matplotlib/Agg",
                "encoder": "matplotlib PNG",
                "environment": {
                    "architecture": "x86_64",
                    "locale": "C.UTF-8",
                    "os_name": "Synthetic Linux",
                    "os_version": "1",
                    "resolved_inputs": [
                        {
                            "kind": "native-library",
                            "name": "FreeType",
                            "sha256": identity("freetype"),
                            "version": "2.13.3",
                        },
                        {
                            "kind": "runtime",
                            "name": "Python",
                            "sha256": identity("python"),
                            "version": "3.13.14",
                        },
                        {
                            "kind": "native-library",
                            "name": "libpng",
                            "sha256": identity("libpng"),
                            "version": "1.6.47",
                        },
                        {
                            "kind": "wheel",
                            "name": "matplotlib",
                            "sha256": identity("matplotlib"),
                            "version": "3.11.1",
                        },
                        {
                            "kind": "wheel",
                            "name": "numpy",
                            "sha256": identity("numpy"),
                            "version": "2.4.0",
                        },
                    ],
                    "timezone": "UTC",
                },
                "producer_command": {
                    "argv_sha256": identity("producer argv"),
                    "path": "tools/site/produce_exact_cylinder_pressure.py",
                    "sha256": next(
                        item["sha256"]
                        for item in source_files
                        if item["path"] == "tools/site/produce_exact_cylinder_pressure.py"
                    ),
                },
            },
            "scene_profile": {
                "bounds_m": {"x": [0.0, 2.2], "y": [0.0, 0.41]},
                "colorbar_label": "Pressure (Pa)",
                "colormap": "viridis",
                "constant_metadata": {"Software": PNG_SOFTWARE},
                "dpi": DPI,
                "facecolor": "white",
                "figure_inches": [8.0, 5.2],
                "format": "png",
                "mesh_overlay": True,
                "no_crop_resize_reencode": True,
                "normalization": "bound pressure snapshot minimum/maximum",
                "plot": "tripcolor",
                "shading": "gouraud",
                "title": "Exact-cylinder steady Stokes pressure",
                "triangulation": "accepted Result mesh explicit triangle connectivity",
            },
            "source_files": source_files,
            "text": {
                "alt": ALT_TEXT,
                "alt_sha256": sha(ALT_TEXT.encode()),
                "caption": caption,
                "caption_links": [
                    {
                        "case_id": "interfaces.python-exact-cylinder-stokes-result",
                        "label": "Result evidence",
                        "route": dossier_route(
                            "interfaces.python-exact-cylinder-stokes-result",
                            self.revision,
                        ),
                    },
                    {
                        "case_id": "interfaces.python-exact-cylinder-pressure-still",
                        "label": "Pressure-still presentation case",
                        "route": dossier_route(
                            "interfaces.python-exact-cylinder-pressure-still",
                            self.revision,
                        ),
                    },
                ],
                "caption_sha256": sha(caption.encode()),
            },
        }

    def _receipt(self):
        return {
            "alt_sha256": self.payload["text"]["alt_sha256"],
            "caption_sha256": self.payload["text"]["caption_sha256"],
            "checks": RECEIPT_CHECKS,
            "claim_sha256": sha(canonical_value(self.payload["claim"])),
            "environment_sha256": sha(canonical_value(self.payload["renderer"]["environment"])),
            "lineage_sha256": sha(canonical_value(self.payload["lineage"])),
            "media_sha256": self.payload["media"]["sha256"],
            "nonchecks": RECEIPT_NONCHECKS,
            "predicate": PREDICATE_SCHEMA,
            "publication_payload_sha256": sha(canonical_value(self.payload)),
            "receipt_id": RECEIPT_ID,
            "renderer_sha256": sha(canonical_value(self.payload["renderer"])),
            "scene_profile_sha256": sha(canonical_value(self.payload["scene_profile"])),
            "schema": RECEIPT_SCHEMA,
            "source_revision": self.revision,
            "status": "accepted",
        }

    def _wrapper(self):
        return {
            "admission": {
                "predicate": PREDICATE_SCHEMA,
                "receipt": {
                    "id": RECEIPT_ID,
                    "sha256": sha(canonical_file(self.receipt)),
                },
                "status": "accepted",
            },
            "entry_id": ENTRY_ID,
            "publication_payload": self.payload,
            "publication_payload_sha256": sha(canonical_value(self.payload)),
            "schema": RECORD_SCHEMA,
            "source_revision": self.revision,
            "source_tree": self.tree,
        }

    def refresh_bindings(self) -> None:
        self.receipt = self._receipt()
        self.wrapper = self._wrapper()

    def write_external(self) -> None:
        self.receipt_path.parent.mkdir(parents=True, exist_ok=True)
        self.receipt_path.write_bytes(canonical_file(self.receipt))
        self.external_record.write_bytes(canonical_file(self.wrapper))

    def refresh_and_write_external(self) -> None:
        self.refresh_bindings()
        self.write_external()

    def install(self, include_receipt: bool = False) -> None:
        self.installed_media.parent.mkdir(parents=True, exist_ok=True)
        self.installed_media.write_bytes(self.candidate.read_bytes())
        self.installed_record.parent.mkdir(parents=True, exist_ok=True)
        self.installed_record.write_bytes(canonical_file(self.wrapper))
        if not include_receipt and self.receipt_path.exists():
            self.receipt_path.unlink()

    def clone_payload(self):
        return copy.deepcopy(self.payload)
