from __future__ import annotations

import json
from pathlib import Path

import pytest

import eqiora


FLUID_MODEL = """model Main {
  domain body = box(0, 4, 0, 2);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  representation space = continuum;
  field velocity on body as space: m / s shape spatial_vector;
  field pressure on body as space: kg / (m * s ^ 2) = 0;
  field force_potential on body as space: kg / (m * s ^ 2) = 0;
  parameter dynamic_viscosity: kg / (m * s) = 2;
  parameter zero_pressure: kg / (m * s ^ 2) = 0;
  relation force_definition continuous on body {
    force_potential - zero_pressure = 0;
  }
  instance governing: fluid.SteadyStokes2d(
    support body = body,
    support exterior = boundaries(x_lower, x_upper, y_lower, y_upper),
    field velocity = velocity,
    field pressure = pressure,
    field force_potential = force_potential,
    dynamic_viscosity = dynamic_viscosity
  );
  instance x_lower_condition: fluid.NoSlip2d(
    support body = body, support face = x_lower
  );
  instance x_upper_condition: fluid.NoSlip2d(
    support body = body, support face = x_upper
  );
  instance y_lower_condition: fluid.NoSlip2d(
    support body = body, support face = y_lower
  );
  instance y_upper_condition: fluid.NoSlip2d(
    support body = body, support face = y_upper
  );
  connect conserving governing.mechanical[boundary = x_lower],
    x_lower_condition.mechanical;
  connect conserving governing.mechanical[boundary = x_upper],
    x_upper_condition.mechanical;
  connect conserving governing.mechanical[boundary = y_lower],
    y_lower_condition.mechanical;
  connect conserving governing.mechanical[boundary = y_upper],
    y_upper_condition.mechanical;
}
"""


def write_fluid_application(root: Path, fluid: eqiora.VendoredStandardPackage) -> None:
    application = root / "application"
    source = application / "src/main.eqi"
    source.parent.mkdir(parents=True)
    source.write_text(FLUID_MODEL, encoding="utf-8")
    manifest = {
        "schema": "eqiora.author-manifest.v1",
        "name": "org.example.VendoredFluid",
        "version": "0.1.0",
        "dependencies": [
            {
                "alias": "fluid",
                "target": {
                    "name": fluid.name,
                    "version": fluid.version,
                    "semantic_digest": fluid.semantic_digest,
                },
            }
        ],
        "bundle": [{"path": "src/main.eqi", "role": "model_source"}],
    }
    (application / "package.json").write_text(
        json.dumps(manifest, separators=(",", ":")),
        encoding="utf-8",
    )


def test_vendored_standard_fluid_resolves_and_compiles_offline(tmp_path: Path) -> None:
    packages = eqiora.vendor_standard_package(tmp_path, "Eqiora.Fluid@0.1.0")
    assert [package.name for package in packages] == [
        "Eqiora.Mechanics.Interfaces",
        "Eqiora.Fluid",
    ]
    mechanics, fluid = packages
    assert len(fluid.semantic_digest) == 64
    assert len(fluid.source_digest) == 64
    assert fluid.path == "packages/Eqiora.Fluid/0.1.0"
    assert eqiora.vendor_standard_package(tmp_path, "Eqiora.Fluid@0.1.0") == packages

    write_fluid_application(tmp_path, fluid)
    (tmp_path / "eqiora.toml").write_text(
        f'''schema = "eqiora.project.v1"
root = "application"

[dependencies]
fluid = "fluid"

[sources.application]
path = "application"

[sources.fluid]
path = "{fluid.path}"

[sources.mechanics]
path = "{mechanics.path}"
''',
        encoding="utf-8",
    )
    store = tmp_path / "store"
    store.mkdir()
    resolution = eqiora.resolve_local_project(tmp_path, store)
    model = eqiora.compile_package(store, resolution, entry_model="Main")
    assert model.revision.number == 1


def test_standard_vendoring_rejects_changed_or_escaping_destinations(
    tmp_path: Path,
) -> None:
    (mechanics, fluid) = eqiora.vendor_standard_package(
        tmp_path, "Eqiora.Fluid@0.1.0"
    )
    fluid_source = tmp_path / fluid.path / "src/fluid.eqi"
    fluid_source.write_text("changed", encoding="utf-8")

    with pytest.raises(eqiora.CompatibilityError, match="different bytes"):
        eqiora.vendor_standard_package(tmp_path, "Eqiora.Fluid@0.1.0")
    assert fluid_source.read_text(encoding="utf-8") == "changed"
    assert (tmp_path / mechanics.path / "src/interfaces.eqi").is_file()

    with pytest.raises(eqiora.CompatibilityError, match="destination is invalid"):
        eqiora.vendor_standard_package(
            tmp_path,
            "Eqiora.Solid@0.2.0",
            destination="../outside",
        )
    assert not (tmp_path.parent / "outside").exists()


def test_standard_solid_is_available_from_the_same_distribution(tmp_path: Path) -> None:
    (solid,) = eqiora.vendor_standard_package(tmp_path, "Eqiora.Solid@0.2.0")
    assert solid.name == "Eqiora.Solid"
    assert solid.version == "0.2.0"
    assert (tmp_path / solid.path / "src/solid.eqi").is_file()
