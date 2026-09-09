"""Explicit module descriptors retain the source connector's nominal identity."""

import pytest

import eqiora

from test_boundary_families import authored_exterior, bindings, rectangle

q = eqiora.lang
VOLTAGE = eqiora.ValueType.real(eqiora.Dimension(mass=1, length=2, time=-3, current=-1))
CURRENT = eqiora.ValueType.real(eqiora.Dimension(current=1))

LIBRARY = """
public connector Pin { across voltage: kg * m^2 / (s^3 * A); through current: A; }
connector PrivatePin { across voltage: kg * m^2 / (s^3 * A); through current: A; }
public component Terminal(port pin: Pin) {
  relation reference { pin.voltage = 0; pin.current = 0; }
}
component PrivateTerminal(port pin: Pin) {}
"""


def imported_circuit(*, lookalike=False, path=None):
    module = eqiora.Module("main")
    library = (module.import_module("parts", eqiora.Module.parse("parts", LIBRARY))
               if path is None else module.import_module("parts", path=path))
    pin = (module.connector("LocalPin", across=("voltage", VOLTAGE), through=("current", CURRENT))
           if lookalike else library.connector("Pin"))
    local = module.component("Local")
    endpoint = local.port("pin", connector=pin)
    local.relation("reference", q.equation(endpoint.voltage - endpoint.voltage, 0),
                   q.equation(endpoint.current - endpoint.current, 0))
    root = module.model("Main")
    imported = root.instance("imported", component=library.component("Terminal"), bindings={})
    authored = root.instance("authored", component=local, bindings={})
    root.connect(imported["pin"], authored["pin"])
    return module


@pytest.mark.parametrize("route", ("module", "path"))
def test_imported_connector_and_component_endpoints_share_exact_identity(route, tmp_path):
    path = None
    if route == "path":
        path = tmp_path / "parts.eqi"
        path.write_text(LIBRARY)
    module = imported_circuit(path=path)
    model = eqiora.compile(source=module, entry="Main")
    source = module.to_eqi()
    assert "port pin: parts.Pin" in source
    assert "connector Pin" not in source
    assert "connect imported.pin, authored.pin;" in source
    assert eqiora.Model.from_bytes(model.to_bytes()).digest == model.digest


def test_an_imported_connector_is_not_replaced_by_an_equal_local_contract():
    assert eqiora.compile(source=imported_circuit(), entry="Main").digest
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=imported_circuit(lookalike=True), entry="Main")
    assert any("connector" in diagnostic.message.lower() for diagnostic in error.value.diagnostics)


def test_import_descriptors_reject_private_unknown_and_transitive_lookups():
    module = eqiora.Module("main")
    library = module.import_module("parts", eqiora.Module.parse("parts", LIBRARY))
    for name in ("PrivatePin", "Missing"):
        with pytest.raises(q.ModuleError, match="Connector"):
            library.connector(name)
    with pytest.raises(q.ModuleError):
        library.connector("other.Pin")
    with pytest.raises(q.ModuleError, match="public Component"):
        library.component("PrivateTerminal")
    pin = library.connector("Pin")
    with pytest.raises(AttributeError):
        pin._name = "LocalPin"
    foreign = eqiora.Module("foreign")
    with pytest.raises(q.ModuleError, match="Module"):
        foreign.component("Law").port("pin", connector=pin)


def test_imported_field_family_descriptors_keep_exact_set_binding_and_quantity_names():
    library = eqiora.Module.parse("boundaries", """
public connector Boundary {
  trace displacement: m; flux traction: kg / (m * s^2);
  shape spatial_vector; frame spatial;
  pairing euclidean_boundary_duality; orientation parent_outward;
}
public component Exterior(
  support body: volume(ambient_dimension = 2),
  support exterior: complete_exterior(parent = body),
  port mechanical[side in exterior]: Boundary over side
) {
  relation law[side in exterior] on side {
    mechanical[side = side].traction - mechanical[side = side].traction = 0;
  }
}
""")
    module = eqiora.Module("main")
    imported = module.import_module("parts", library)
    connector = imported.connector("Boundary")
    assert isinstance(connector, q.FieldConnector)
    component = module.component("Wrapper")
    body = component.volume("body", dimensions=2)
    exterior = component.complete_exterior("exterior", parent=body)
    member = exterior.member("face")
    port = component.port("mechanical", connector=connector, on=member)
    child = component.instance("child", component=imported.component("Exterior"),
                                bindings={"body": body, "exterior": exterior})
    assert isinstance(child["mechanical"], q.FieldPort)
    component.connect(port[member], child["mechanical"][member], over=member)
    source = module.to_eqi()
    assert "child.mechanical[side = face]" in source
    assert "mechanical[face in exterior]: parts.Boundary over face" in source


def test_imported_boundary_families_compile_with_the_same_separate_geometry_after_source_roundtrip(tmp_path):
    library = authored_exterior(library=True)
    module = eqiora.Module("main")
    imported = module.import_module("parts", library)
    root = module.model("Main")
    body = root.volume("body", dimensions=2)
    faces = {name: root.boundary(name, parent=body) for name in ("left", "right", "bottom", "top")}
    wrapper = root.instance("wrapped", component=imported.component("ExteriorWrapper"), bindings={
        "body": body, "exterior": root.boundaries(*faces.values()),
    })
    for name, face in faces.items():
        terminal = root.instance(name + "_terminal", component=imported.component("BoundaryTerminal"),
                                  bindings={"body": body, "face": face})
        root.connect(wrapper["mechanical"][face], terminal["mechanical"])
    geometry = rectangle()
    direct = eqiora.compile(source=module, geometry=geometry, entry="Main", bindings=bindings(geometry))
    parsed_library = eqiora.Module.parse("parts", library.to_eqi())
    parsed_root = eqiora.Module.parse("main", module.to_eqi())
    parsed_root.import_module("parts", parsed_library)
    replay = eqiora.compile(source=parsed_root, geometry=geometry, entry="Main", bindings=bindings(geometry))
    assert direct.to_bytes() == replay.to_bytes()
    assert eqiora.Model.from_bytes(replay.to_bytes()).digest == direct.digest

    project = tmp_path / "project"
    (project / "src").mkdir(parents=True)
    (project / "eqiora.toml").write_text(
        '[package]\nname="org.example.Exterior"\nversion="1.0.0"\nentry="main"\n')
    module.write_eqi(project / "src/main.eqi")
    library.write_eqi(project / "src/parts.eqi")
    store = tmp_path / "store"
    store.mkdir()
    resolution = eqiora.update_project(project, store)
    packaged = eqiora.compile_package(store, resolution, entry="Main", geometry=geometry,
                                      bindings=bindings(geometry))
    vendor = project / "vendor"
    vendor.mkdir()
    assert eqiora.vendor_project(project, store, vendor) == resolution
    reopened = eqiora.compile_package(vendor, resolution, entry="Main", geometry=geometry,
                                      bindings=bindings(geometry))
    assert packaged.to_bytes() == reopened.to_bytes()
    assert eqiora.Model.from_bytes(reopened.to_bytes()).digest == packaged.digest
