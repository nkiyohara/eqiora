"""Exact static extents and nonempty half-open slices cross the public Python API."""

import json

import pytest

import eqiora


def test_source_builder_static_slices_and_binders_retain_scope_and_edit_dependencies():
    q = eqiora.lang
    source = q.Source()
    sibling = source.component("Other")
    owner = source.model("BuilderSlices")
    integer = eqiora.ValueType.integer()
    stop = owner.parameter("stop", value_type=integer)
    owner.set_default(stop, 2)
    tick = owner.clock("tick", period_s=1)
    result = owner.output("result", value_type=integer, at=tick)
    data = q.array(tuple(q.to_integer(value) for value in (2, 3, 5)))
    rows = owner.index_set("Rows", extent=3)
    # All three one-wide slices sum to 10, and data[stop] contributes 5.
    total = owner.sum(lambda i: data[q.ordinal(i):q.ordinal(i) + 1][0], over=rows)
    owner.relation("emit", at=tick, left=result, right=total + data[stop] + data[0:stop][1])
    foreign = sibling.parameter("foreign", value_type=integer)
    sibling.set_default(foreign, 2)
    with pytest.raises(q.SourceError, match="different"):
        data[stop:foreign]
    escaped = []
    owner.sum(lambda i: escaped.append(i) or 1, over=rows)
    with pytest.raises(q.SourceError, match="binder"):
        owner.let_alias("escape", data[q.ordinal(escaped[0]):2])
    compiled = eqiora.compile(source=source, entry="BuilderSlices")
    run = compiled.execution_session(end_time_s=0.1, max_step_s=0.1, inputs={})
    assert run.advance_ticks(1) == 1
    assert run.output("result", 0)[1] == 18
    with pytest.raises(eqiora.EqioraError, match="(?i)(structural|static|topology)"):
        compiled.preview_value_edit(compiled.parameter("stop").id, 1)


@pytest.mark.parametrize("bound", (slice(None, 2), slice(0, None), slice(0, 2, 1),
                                   slice(False, 2), slice(0, 1.5), slice(-1, 2)))
def test_source_builder_slice_bounds_are_raw_explicit_integers(bound):
    with pytest.raises((TypeError, eqiora.lang.SourceError)):
        eqiora.lang.array((2, 3, 5))[bound]


def field_types(model):
    return [node["definition"]["value_type"]
            for node in json.loads(model.to_bytes())["nodes"]
            if node["definition"]["kind"] == "field"]


@pytest.mark.parametrize("extent", (2, 3))
def test_static_extent_sampled_values_reopen_and_edit_guards(extent, tmp_path):
    values = (2**53 + 1, 3, 5)[:extent]
    literal = "[" + ", ".join(map(str, values)) + "]"
    source = f"""
model Sized(output seen: array<integer, 2> at tick, output scaled: 1 at tick) {{
  parameter n: integer = {extent};
  parameter gain: 1 = 2;
  parameter data: array<integer, n> = {literal};
  clock tick = periodic(1 [s] / 1, phase = 0 [s] / 1);
  state memory: array<integer, n> at tick;
  initial {{ memory = {literal}; }}
  relation update at tick {{ next(memory) = data; }}
  relation observe at tick {{ seen = pre(memory)[0:2]; }}
  relation scale at tick {{ scaled = gain * to_real(pre(memory)[1]); }}
}}
"""
    model = eqiora.compile(source=source)
    path = tmp_path / "sized.eqi"
    path.write_text(source, encoding="utf-8")
    assert eqiora.compile(path=path).to_bytes() == model.to_bytes()
    n_id = model.parameter("n").id
    gain_id = model.parameter("gain").id
    assert model.parameter("data").value_type == eqiora.ValueType.array(
        eqiora.ValueType.integer(), extent)
    assert model.parameter("data").value == values
    for current in (model, eqiora.Model.from_bytes(model.to_bytes())):
        before = current.to_bytes()
        with pytest.raises(eqiora.EqioraError, match="(?i)(structural|static|topology)"):
            current.preview_value_edit(n_id, extent + 1)
        assert current.to_bytes() == before
        assert current.parameter(n_id).value == extent
        changed = current.commit(current.preview_value_edit(gain_id, 4.0))
        assert current.parameter(gain_id).value == 2.0
        assert changed.parameter(gain_id).value == 4.0
        assert eqiora.Model.from_bytes(changed.to_bytes()).parameter(gain_id).value == 4.0
    # Session names are source aliases; bare artifacts need not retain them.
    session = model.execution_session(end_time_s=1, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    assert session.field("memory") == values
    assert session.output("seen", 0)[1] == values[:2]
    assert all(type(value) is int for value in session.output("seen", 0)[1])
    assert session.output("scaled", 0)[1] == 6.0
    resumed = model.resume_execution(session.checkpoint())
    assert resumed.advance_ticks(1) == 1
    assert resumed.output("seen", 1)[1] == values[:2]
    changed = model.commit(model.preview_value_edit(gain_id, 4.0))
    edited = changed.execution_session(end_time_s=0.1, max_step_s=0.1, inputs={})
    assert edited.advance_ticks(1) == 1
    assert edited.output("scaled", 0)[1] == 12.0


@pytest.mark.parametrize("extent", (2, 3))
def test_selected_component_specializes_defaults_and_binding_order(extent):
    source = """
public component Sized(parameter data: array<integer, n>, parameter n: integer = 3) {
  variable values: array<integer, n>;
  relation copy { values = data; }
}
"""
    data = (2, 3, 5)[:extent]
    left = eqiora.compile(source=source, entry="Sized", bindings={"n": extent, "data": data})
    right = eqiora.compile(source=source, entry="Sized", bindings={"data": data, "n": extent})
    assert left.structural_fingerprint == right.structural_fingerprint
    types = field_types(left)
    assert len(types) == 1
    assert types[0]["shape"] == [extent]
    assert types[0]["array_rank"] == 1
    assert types[0]["domain"] == "integer"
    if extent == 3:
        defaulted = eqiora.compile(source=source, entry="Sized", bindings={"data": data})
        assert field_types(defaulted) == types
        assert defaulted.parameter("data").value == left.parameter("data").value == data
        # A default is a child constant, whereas explicit n is an editable root
        # Parameter with a guarded structural dependency: these graphs differ.


@pytest.mark.parametrize("domain", ("integer", "real"))
def test_native_field_parameter_expression_slices_match_source_types(domain):
    scalar = (eqiora.ValueType.integer() if domain == "integer"
              else eqiora.ValueType.real(eqiora.Dimension(mass=1, length=2, time=-3, current=-1)))
    unit = "integer" if domain == "integer" else "V"
    values = (2**53 + 1, 3, 5) if domain == "integer" else (1.25, -2.5, 4.0)
    kind = eqiora.ValueType.array(scalar, 3)
    short = eqiora.ValueType.array(scalar, 2)
    data = eqiora.Parameter("data", value_type=kind, value=values)
    field = eqiora.Field("values", role=eqiora.FieldRole.Variable, value_type=kind)
    outputs = [eqiora.Field(name, role=eqiora.FieldRole.Variable, value_type=short)
               for name in ("field_cut", "parameter_cut", "expression_cut")]
    native = eqiora.Model.define(
        "Slices", data, field, *outputs,
        eqiora.Relation("copy", equations=[(field, data)]),
        eqiora.Relation("cuts", equations=[
            (outputs[0], field[0:2]),
            (outputs[1], data[0:2]),
            (outputs[2], (-field)[0:2]),
        ]),
    )
    literal = "[" + ", ".join(
        str(value) if domain == "integer" else f"{value} [V]"
        for value in values) + "]"
    source = f"""
model Slices() {{
  parameter data: array<{unit}, 3> = {literal};
  variable values: array<{unit}, 3>;
  variable field_cut: array<{unit}, 2>;
  variable parameter_cut: array<{unit}, 2>;
  variable expression_cut: array<{unit}, 2>;
  relation copy {{ values = data; }}
  relation cuts {{
    field_cut = values[0:2];
    parameter_cut = data[0:2];
    expression_cut = (-values)[0:2];
  }}
}}
"""
    authored = eqiora.compile(source=source)
    assert native.structural_fingerprint == authored.structural_fingerprint
    assert native.parameter("data").value == values
    assert native.parameter("data").value_type == kind
    assert eqiora.Model.from_bytes(native.to_bytes()).structural_fingerprint == native.structural_fingerprint
    # The same half-open source expression also executes, retaining units and
    # exact integer values rather than coercing channels through float64.
    sampled = eqiora.compile(source=f"""
model SliceOutput(output seen: array<{unit}, 2> at tick) {{
  parameter data: array<{unit}, 3> = {literal};
  clock tick = periodic(1 [s] / 1, phase = 0 [s] / 1);
  relation emit at tick {{ seen = data[0:2]; }}
}}
""")
    session = sampled.execution_session(end_time_s=0.1, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    assert session.output("seen", 0)[1] == values[:2]
    assert all(type(value) is (int if domain == "integer" else float)
               for value in session.output("seen", 0)[1])


@pytest.mark.parametrize("bound", (
    slice(0, 2, 1), slice(None, 2), slice(0, None), slice(1, 1),
    slice(2, 1), slice(-1, 2), slice(0, 4), slice(0.5, 2), slice(0, 2.5),
    slice(False, 2), slice(0, True),
))
def test_invalid_python_slices_reject_without_mutating_declarations(bound):
    kind = eqiora.ValueType.array(eqiora.ValueType.integer(), 3)
    parameter = eqiora.Parameter("data", value_type=kind, value=(2, 3, 5))
    output = eqiora.Field("result", role=eqiora.FieldRole.Variable,
                          value_type=eqiora.ValueType.array(eqiora.ValueType.integer(), 2))
    with pytest.raises((TypeError, ValueError, OverflowError, eqiora.ValidationError)):
        eqiora.Model.define("InvalidSlice", parameter, output,
                            eqiora.Relation("cut", equations=[(output, parameter[bound])]))
    assert parameter.value == (2, 3, 5)
    assert parameter.value_type == kind


@pytest.mark.parametrize("bounds", ("0:2:1", ":2", "0:", "1:1", "-1:2", "0:4", "0.5:2", "false:2"))
def test_source_slice_bounds_do_not_normalize_or_clamp(bounds):
    source = f"""
model InvalidSlice() {{
  parameter data: array<integer, 3> = [2, 3, 5];
  variable result: array<integer, 2>;
  relation cut {{ result = data[{bounds}]; }}
}}
"""
    with pytest.raises(eqiora.ValidationError):
        eqiora.compile(source=source)
