from __future__ import annotations

from pathlib import Path

import pytest

import eqiora


FLUID_MODEL = """import Eqiora.Fluid.Incompressible.incompressible as fluid;

model Main {
  domain body = box(0, 4, 0, 2);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  representation space = continuum;
  field velocity on body as space: m / s shape spatial_vector;
  field pressure on body as space: kg / (m * s ^ 2) = 0;
  field force_potential on body as space: kg / (m * s ^ 2) = 0;
  field inlet_speed on body as space: m / s = 0;
  parameter dynamic_viscosity: kg / (m * s) = 2;
  parameter zero_pressure: kg / (m * s ^ 2) = 0;
  relation force_definition continuous on body {
    force_potential - zero_pressure = 0;
  }
  instance governing: fluid.SteadyStokesWithPotential2d(
    support body = body,
    field velocity = velocity,
    field pressure = pressure,
    field force_potential = force_potential,
    dynamic_viscosity = dynamic_viscosity
  );
}
"""


def write_fluid_application(root: Path) -> None:
    application = root / "application"
    source = application / "src/main.eqi"
    source.parent.mkdir(parents=True)
    source.write_text(FLUID_MODEL, encoding="utf-8")


def test_vendored_standard_fluid_resolves_and_compiles_offline(tmp_path: Path) -> None:
    packages = eqiora.vendor_standard_package(
        tmp_path, "Eqiora.Fluid.Incompressible@0.2.0"
    )
    assert [package.name for package in packages] == [
        "Eqiora.Mechanics.Interfaces",
        "Eqiora.Fluid.Incompressible",
    ]
    mechanics, fluid = packages
    assert len(fluid.semantic_digest) == 64
    assert len(fluid.source_digest) == 64
    assert fluid.path == "packages/Eqiora.Fluid.Incompressible/0.2.0"
    assert (
        eqiora.vendor_standard_package(
            tmp_path, "Eqiora.Fluid.Incompressible@0.2.0"
        )
        == packages
    )

    write_fluid_application(tmp_path)
    (tmp_path / fluid.path / "eqiora.toml").write_text(
        f'''[package]
name = "{fluid.name}"
version = "{fluid.version}"
source = "src"
entry = "incompressible"

[dependencies."{mechanics.name}"]
version = "{mechanics.version}"
path = "../../{mechanics.name}/{mechanics.version}"
''',
        encoding="utf-8",
    )
    (tmp_path / mechanics.path / "eqiora.toml").write_text(
        f'''[package]
name = "{mechanics.name}"
version = "{mechanics.version}"
source = "src"
entry = "interfaces"
''',
        encoding="utf-8",
    )
    (tmp_path / "eqiora.toml").write_text(
        f'''[package]
name = "org.example.VendoredFluid"
version = "0.1.0"
source = "application/src"
entry = "main"

[dependencies."{fluid.name}"]
version = "{fluid.version}"
path = "{fluid.path}"
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
        tmp_path, "Eqiora.Fluid.Incompressible@0.2.0"
    )
    fluid_source = tmp_path / fluid.path / "src/incompressible.eqi"
    fluid_source.write_text("changed", encoding="utf-8")

    with pytest.raises(eqiora.CompatibilityError, match="different bytes"):
        eqiora.vendor_standard_package(
            tmp_path, "Eqiora.Fluid.Incompressible@0.2.0"
        )
    assert fluid_source.read_text(encoding="utf-8") == "changed"
    assert (tmp_path / mechanics.path / "src/interfaces.eqi").is_file()

    with pytest.raises(eqiora.CompatibilityError, match="destination is invalid"):
        eqiora.vendor_standard_package(
            tmp_path,
            "Eqiora.Solid.LinearElasticity@0.4.0",
            destination="../outside",
        )
    assert not (tmp_path.parent / "outside").exists()


def test_standard_solid_is_available_from_the_same_distribution(tmp_path: Path) -> None:
    mechanics, solid = eqiora.vendor_standard_package(
        tmp_path, "Eqiora.Solid.LinearElasticity@0.4.0"
    )
    assert mechanics.name == "Eqiora.Mechanics.Interfaces"
    assert solid.name == "Eqiora.Solid.LinearElasticity"
    assert solid.version == "0.4.0"
    assert (tmp_path / solid.path / "src/linear_elasticity.eqi").is_file()
