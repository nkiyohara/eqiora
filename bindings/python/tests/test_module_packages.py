"""Explicit package identity travels through the existing Module graph resolver."""

import pytest
import eqiora

from test_imported_connectors import LIBRARY


def circuit(*, reverse=False, distinct=False):
    root = eqiora.Module("main", package="org.example.Application")
    library = eqiora.Module.parse("ports", LIBRARY, package="org.example.Components")
    entries = [("first", library), ("second", eqiora.Module.parse(
        "ports", LIBRARY, package="org.example.Other") if distinct else library)]
    if reverse:
        entries.reverse()
    imports = {name: root.import_module(name, module) for name, module in entries}
    model = root.model("Main")
    ports = [model.instance(name, component=imports[name].component("Terminal"), bindings={})["pin"]
             for name in ("first", "second")]
    model.connect(*ports)
    return root


def test_explicit_package_imports_retain_exact_namespace_and_discovery_independence():
    first, second = circuit(), circuit(reverse=True)
    assert "import org.example.Components.ports as first;" in first.to_eqi()
    assert eqiora.compile(source=first, entry="Main").to_bytes() == eqiora.compile(
        source=second, entry="Main").to_bytes()


def test_equal_named_connectors_in_different_packages_remain_distinct():
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=circuit(distinct=True), entry="Main")
    assert any("connector" in item.message.lower() for item in error.value.diagnostics)


@pytest.mark.parametrize("package", ("", "../escape", "org..Example", "org.example/Other", "x" * 513))
def test_module_package_names_use_the_canonical_package_validator(package):
    with pytest.raises(eqiora.lang.ModuleError):
        eqiora.Module("main", package=package)


def test_frozen_import_attachment_rejects_a_different_exact_package():
    root = eqiora.Module.parse("main", "import org.example.Library.ports as parts; model Main() { parameter gain:1=1; relation value {gain-1=0;} }",
                               package="org.example.Application")
    with pytest.raises(eqiora.lang.ModuleError, match="exact existing import"):
        root.import_module("parts", eqiora.Module.parse("ports", LIBRARY, package="org.example.Other"))
    root.import_module("parts", eqiora.Module.parse("ports", LIBRARY, package="org.example.Library"))
    assert eqiora.compile(source=root, entry="Main").digest


@pytest.mark.parametrize("reverse", (False, True))
def test_duplicate_module_handles_cannot_hide_conflicting_attached_transitive_sources(reverse):
    package = "org.example.Closure"
    middle_source = "import org.example.Closure.leaf; public component Middle() {}"
    middles = []
    for value in (1, 2):
        leaf = eqiora.Module.parse("leaf", f"public component Leaf() {{parameter gain:1={value};}}", package=package)
        middle = eqiora.Module.parse("middle", middle_source, package=package)
        middle.import_module("leaf", leaf)
        middles.append(middle)
    if reverse:
        middles.reverse()
    root = eqiora.Module("main", package=package)
    root.import_module("first", middles[0])
    root.import_module("second", middles[1])
    root.model("Main")
    with pytest.raises(eqiora.lang.ModuleError, match="conflicting contents"):
        eqiora.compile(source=root, entry="Main")
