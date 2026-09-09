"""Uniform spatial coefficients require an explicit existing Cartesian frame."""

import json

import pytest

import eqiora

q = eqiora.lang


def literal_source(value):
    if isinstance(value, tuple):
        return "[" + ", ".join(literal_source(item) for item in value) + "]"
    if isinstance(value, complex):
        return f"math.complex({value.real!r}, {value.imag!r})"
    return repr(value)


def assert_inlined_coefficient(model, kind, values):
    # Component defaults specialize into the Relation. Inspect its actual RHS,
    # not a Parameter node that the selected Component does not retain.
    nodes = json.loads(model.to_bytes())["nodes"]
    definitions = [node["definition"] for node in nodes]
    relations = [node for node in definitions if node["kind"] == "relation"]
    assert len(relations) == 1
    expression = relations[0]["expression"]
    assert len(expression["roots"]) == 2
    fields = [node for node in definitions if node["kind"] == "field"]
    assert len(fields) == 1
    value_type = fields[0]["value_type"]
    assert value_type["domain"] == kind.scalar_domain
    assert value_type["shape"] == list(kind.shape)
    assert value_type["array_rank"] == kind.array_rank
    assert value_type["frame"] == "spatial-cartesian"
    assert value_type["dimension"] == [[0, 1]] * 7

    def flatten(value):
        if isinstance(value, tuple):
            return [component for child in value for component in flatten(child)]
        number = complex(value)
        return [[number.real, number.imag]]

    # Only declared outer channel axes are Array operations. Spatial axes stay
    # inside each complete constant's payload, preserving vector/tensor meaning.
    leaves = [(expression["roots"][1], values)]
    for extent in kind.shape[:kind.array_rank]:
        children = []
        for node_id, expected in leaves:
            array = expression["nodes"][node_id]
            assert array["op"] == "array"
            assert len(array["elements"]) == len(expected) == extent
            children.extend(zip(array["elements"], expected))
        leaves = children
    spatial_type = dict(value_type, shape=list(kind.shape[kind.array_rank:]), array_rank=0)
    assert leaves
    for node_id, expected in leaves:
        constant = expression["nodes"][node_id]
        assert constant["op"] == "constant"
        literal = constant["value"]
        assert literal["value_type"] == spatial_type
        assert literal["components"] == {"kind": "dense", "values": flatten(expected)}
    field_id = next(node["id"] for node in nodes if node["definition"]["kind"] == "field")
    assert expression["nodes"][expression["roots"][0]] == {
        "op": "symbol", "symbol": {"kind": "field", "id": field_id},
    }


CASES = (
    (eqiora.ValueType.vector(eqiora.ValueType.real(), 2), (2.0, -3.0), (4.0, -1.0)),
    (eqiora.ValueType.tensor(eqiora.ValueType.real(), 2, 2),
     ((2.0, 1.0), (3.0, 5.0)), ((2.0, 4.0), (3.0, 5.0))),
    (eqiora.ValueType.vector(eqiora.ValueType.complex(), 2),
     (1 + 2j, 3 - 4j), (1 + 7j, 3 - 4j)),
    (eqiora.ValueType.tensor(eqiora.ValueType.complex(), 2, 2),
     ((1 + 2j, 3 - 4j), (5 + 6j, 7 - 8j)),
     ((1 + 2j, 3 - 9j), (5 + 6j, 7 - 8j))),
)


@pytest.mark.parametrize("kind, values, changed_values", CASES)
def test_native_frame_values_match_source_and_preserve_edits_replay(kind, values, changed_values, tmp_path):
    body = eqiora.Domain.box("body", (0.0, 1.0), (0.0, 1.0))
    coefficient = eqiora.Parameter("coefficient", value_type=kind, value=values, frame=body)
    field = eqiora.Field("field", role=eqiora.FieldRole.Variable, value_type=kind, domain=body)
    law = eqiora.Relation("law", domain=body, equations=[(field, coefficient)])
    native = eqiora.compile(source=eqiora.Module("Framed", body, coefficient, field, law))
    text = f"""
model Framed() {{
  domain body = box(0, 1, 0, 1);
  parameter coefficient: {kind.to_eqi()} = tensor_value(frame = body, components = {literal_source(values)});
  variable field: {kind.to_eqi()} on body;
  relation law on body {{ field = coefficient; }}
}}
"""
    source = eqiora.compile(source=text)
    path = tmp_path / "framed.eqi"
    path.write_text(text)
    assert eqiora.compile(path=path).to_bytes() == source.to_bytes()
    assert native.structural_fingerprint == source.structural_fingerprint
    for model in (native, source):
        reference = model.parameter("coefficient")
        assert reference.value == values
        assert reference.value_type == kind
        assert reference.value_type.frame == "spatial_cartesian"
        replayed = eqiora.Model.from_bytes(model.to_bytes())
        assert replayed.digest == model.digest
        assert replayed.parameter(reference.id).value == values
        changed = replayed.commit(replayed.preview_value_edit(reference.id, changed_values))
        assert changed.parameter(reference.id).value == changed_values
        assert changed.parameter(reference.id).value_type == kind
        assert eqiora.Model.from_bytes(changed.to_bytes()).parameter(reference.id).value == changed_values
        assert reference.value == values
        with pytest.raises((TypeError, ValueError)):
            model.preview_value_edit(reference.id, 1)


def geometry():
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
    return graph.build(rectangle, named_topology={
        "body": rectangle.region,
        "left": rectangle.boundaries[0], "right": rectangle.boundaries[1],
        "bottom": rectangle.boundaries[2], "top": rectangle.boundaries[3],
    })


@pytest.mark.parametrize("kind, values, _", CASES)
def test_source_tensor_value_defaults_use_explicit_support_and_file_path(kind, values, _, tmp_path):
    source = q.Source()
    owner = source.component("Framed")
    body = owner.volume("body", dimensions=2)
    coefficient = owner.parameter("coefficient", value_type=kind)
    owner.set_default(coefficient, q.tensor_value(frame=body, components=values))
    field = owner.field("field", role=eqiora.FieldRole.Variable, value_type=kind, on=body)
    owner.relation("law", on=body, left=field, right=coefficient)
    shape = geometry()
    bindings = {"body": shape.selection("body")}
    compiled = eqiora.compile(source=source, entry="Framed", geometry=shape, bindings=bindings)
    assert_inlined_coefficient(compiled, kind, values)
    assert_inlined_coefficient(eqiora.Model.from_bytes(compiled.to_bytes()), kind, values)
    path = tmp_path / "authored.eqi"
    source.write_eqi(path)
    assert eqiora.compile(path=path, entry="Framed", geometry=shape, bindings=bindings).to_bytes() == compiled.to_bytes()


def test_outer_channel_axes_remain_distinct_from_spatial_component_axes():
    body = eqiora.Domain.box("body", (0.0, 1.0), (0.0, 1.0))
    kind = eqiora.ValueType.array(eqiora.ValueType.vector(eqiora.ValueType.real(), 2), 2)
    values = ((1.0, 2.0), (3.0, 4.0))
    coefficient = eqiora.Parameter("channels", value_type=kind, value=values, frame=body)
    observed = eqiora.Field("observed", role=eqiora.FieldRole.Variable, domain=body)
    native = eqiora.compile(source=eqiora.Module("Channels", body, coefficient, observed,
                                eqiora.Relation("observe", domain=body, equations=[(observed, 0)])))
    source = eqiora.compile(source="""
model Channels() {
  domain body = box(0, 1, 0, 1);
  parameter channels: array<vector<1, 2>, 2> = [
    tensor_value(frame = body, components = [1, 2]),
    tensor_value(frame = body, components = [3, 4])
  ];
  variable observed: 1 on body;
  relation observe on body { observed = 0; }
}
""")
    assert native.structural_fingerprint == source.structural_fingerprint
    assert native.parameter("channels").value == values
    assert native.parameter("channels").value_type.array_rank == 1
    assert kind != eqiora.ValueType.tensor(eqiora.ValueType.real(), 2, 2)
    authored = q.Source()
    owner = authored.component("Channels")
    support = owner.volume("body", dimensions=2)
    channels = owner.parameter("channels", value_type=kind)
    owner.set_default(channels, q.array([
        q.tensor_value(frame=support, components=row) for row in values
    ]))
    field = owner.field("observed", on=support, role=eqiora.FieldRole.Variable,
                        value_type=kind)
    owner.relation("observe", on=support, left=field, right=channels)
    shape = geometry()
    compiled = eqiora.compile(source=authored, entry="Channels", geometry=shape,
                              bindings={"body": shape.selection("body")})
    assert_inlined_coefficient(compiled, kind, values)
    assert_inlined_coefficient(eqiora.Model.from_bytes(compiled.to_bytes()), kind, values)


def test_frame_handles_reject_foreign_identity_and_invalid_shape():
    body = eqiora.Domain.box("body", (0.0, 1.0), (0.0, 1.0))
    foreign = eqiora.Domain.box("body", (0.0, 1.0), (0.0, 1.0))
    observed = eqiora.Field("observed", role=eqiora.FieldRole.Variable, domain=body)
    law = eqiora.Relation("observe", domain=body, equations=[(observed, 0)])
    vector = eqiora.ValueType.vector(eqiora.ValueType.real(), 2)
    parameter = eqiora.Parameter("coefficient", value_type=vector, value=(1, 2), frame=foreign)
    with pytest.raises(eqiora.ValidationError, match="frame|foreign|omitted|registered"):
        eqiora.compile(source=eqiora.Module("Foreign", body, parameter, observed, law))
    wrong_extent = eqiora.Parameter("coefficient", value_type=eqiora.ValueType.vector(eqiora.ValueType.real(), 3),
                                    value=(1, 2, 3), frame=body)
    with pytest.raises(eqiora.ValidationError, match="frame|extent|dimension|shape"):
        eqiora.compile(source=eqiora.Module("WrongExtent", body, wrong_extent, observed, law))
    for invalid in (1, (1,), ((1, 2), (3, 4)), (True, 2)):
        with pytest.raises((TypeError, ValueError)):
            eqiora.Parameter("invalid", value_type=vector, value=invalid, frame=body)


def test_tensor_value_preserves_component_ownership_and_expression_bounds():
    source = q.Source()
    left, right = source.component("Left"), source.component("Right")
    body = left.volume("body", dimensions=2)
    other = right.volume("body", dimensions=2)
    value = left.parameter("value", value_type=eqiora.ValueType.real())
    expression = q.tensor_value(frame=body, components=(value, 2))
    with pytest.raises(q.SourceError, match="Component|owner"):
        right.let_alias("foreign", expression)
    with pytest.raises(q.SourceError, match="Component"):
        q.tensor_value(frame=other, components=(value, 2))
    with pytest.raises(TypeError, match="Support"):
        q.tensor_value(frame="body", components=(1, 2))
    with pytest.raises(q.SourceError, match="4096"):
        q.tensor_value(frame=body, components=[1] * 4095)
    left.let_alias("valid", expression)
    assert "tensor_value(frame = body, components = [value, 2])" in source.to_eqi()


def test_array_literals_cannot_implicitly_become_spatial_values():
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source="""
model WrongFrame() {
  domain body = box(0, 1, 0, 1);
  parameter coefficient: vector<1, 2> = [1, 2];
  variable observed: 1 on body;
  relation observe on body { observed = 0; }
}
""")
    assert error.value.diagnostics[0].code == "EQ0603"


def test_uniform_zero_keeps_contextual_shape_and_invariant_values_have_no_frame():
    body = eqiora.Domain.box("body", (0.0, 1.0), (0.0, 1.0))
    kind = eqiora.ValueType.vector(eqiora.ValueType.real(), 2)
    zero = eqiora.Parameter("zero", value_type=kind, value=0)
    field = eqiora.Field("field", role=eqiora.FieldRole.Variable, domain=body, value_type=kind)
    law = eqiora.Relation("law", domain=body, equations=[(field, zero)])
    model = eqiora.compile(source=eqiora.Module("Zero", body, zero, field, law))
    assert model.parameter("zero").value == (0.0, 0.0)
    assert model.parameter("zero").value_type == kind
    invariant = eqiora.Parameter("invariant", value=1.0, frame=body)
    with pytest.raises(eqiora.ValidationError, match="frame|invariant"):
        eqiora.compile(source=eqiora.Module("NotSpatial", body, zero, field, law, invariant))
