"""Finite reductions bind nominal indices without Python iteration or scope capture."""

import pytest

import eqiora

q = eqiora.lang


def owner_and_rows():
    source = q.Source()
    owner = source.model("Reduction")
    return source, owner, owner.index_set("Rows", extent=3)


def test_reduction_callback_is_called_once_and_body_is_symbolic():
    source, owner, rows = owner_and_rows()
    seen = []
    def body(i):
        seen.append(i)
        return (q.ordinal(i) + 1) ** 2
    result = owner.sum(body, over=rows)
    assert len(seen) == 1
    observed = owner.field("observed", role=eqiora.FieldRole.Variable,
                           value_type=eqiora.ValueType.integer())
    owner.relation("observe", left=observed, right=result)
    assert "sum((ordinal(i) + 1) ^ 2, over = (i in Rows))" in source.to_eqi()
    with pytest.raises(AttributeError):
        seen[0].anything = 1
    with pytest.raises(TypeError, match="truth"):
        bool(seen[0])


@pytest.mark.parametrize("operation", ("sum", "product"))
def test_finite_reduction_source_file_and_sampled_execution(operation, tmp_path):
    source, owner, rows = owner_and_rows()
    tick = owner.clock("tick", period_s=1)
    out = owner.output("result", value_type=eqiora.ValueType.integer(), at=tick)
    reduced = getattr(owner, operation)(lambda i: (q.ordinal(i) + 1) ** 2, over=rows)
    owner.relation("emit", at=tick, left=out, right=reduced)
    model = eqiora.compile(source=source, entry="Reduction")
    path = tmp_path / "reduction.eqi"
    source.write_eqi(path)
    assert eqiora.compile(path=path, entry="Reduction").to_bytes() == model.to_bytes()
    session = model.sampled_session(end_time_s=0.1, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    # Three squared positive ordinals are 1,4,9: sum14 and product36.
    assert session.output("result", 0)[1] == (14 if operation == "sum" else 36)


def test_reduction_product_preserves_physical_units():
    source, owner, rows = owner_and_rows()
    tick = owner.clock("tick", period_s=1)
    observed = owner.output("volume", value_type=eqiora.ValueType.real(eqiora.Dimension(length=3)), at=tick)
    owner.relation("volume_value", at=tick, left=observed,
                   right=owner.product(lambda i: q.quantity(2, eqiora.units.m), over=rows))
    model = eqiora.compile(source=source, entry="Reduction")
    session = model.sampled_session(end_time_s=0.1, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    # (2 m)*(2 m)*(2 m) = 8 m^3; output signature checks the physical dimension.
    assert session.output("volume", 0)[1] == 8.0


def test_nested_reduction_retains_outer_binder_until_bound():
    source, owner, rows = owner_and_rows()
    values = q.array((2, 3, 5))
    nested = owner.sum(lambda i: owner.sum(
        lambda j: values[q.ordinal(i)] + q.ordinal(j), over=rows, name="j"), over=rows)
    owner.let_alias("nested", nested)
    text = source.to_eqi()
    assert "[ordinal(i)]" in text
    assert "over = (j in Rows)" in text and "over = (i in Rows)" in text


@pytest.mark.parametrize("wrap", (lambda i: i + 1, lambda i: -i, lambda i: i ** 2,
    lambda i: q.array((i,)), lambda i: q.logical_not(q.equal(i, i)),
    lambda i: q.math.complex(i, 0), lambda i: q.array((1, 2, 3))[q.ordinal(i)],
    lambda i: q.quotient(q.ordinal(i), 1)))
def test_escaped_binders_cannot_be_hidden_by_expression_construction(wrap):
    _, owner, rows = owner_and_rows()
    escaped = []
    owner.sum(lambda i: escaped.append(i) or 1, over=rows)
    value = wrap(escaped[0])
    with pytest.raises(q.SourceError, match="binder"):
        owner.let_alias("escaped", value)
    with pytest.raises(q.SourceError, match="binder"):
        owner.sum(lambda i: value, over=rows)


@pytest.mark.parametrize("sink", ("relation", "initial", "default"))
def test_declaration_sinks_reject_free_binders_before_mutation(sink):
    _, owner, rows = owner_and_rows()
    parameter = owner.parameter("p", value_type=eqiora.ValueType.integer())
    escaped = []
    owner.sum(lambda i: escaped.append(i) or 1, over=rows)
    with pytest.raises(q.SourceError, match="binder"):
        if sink == "relation":
            owner.relation("bad", left=parameter, right=q.ordinal(escaped[0]))
        elif sink == "initial":
            owner.initial(left=parameter, right=q.ordinal(escaped[0]))
        else:
            owner.set_default(parameter, q.ordinal(escaped[0]))
    owner.let_alias("bad", 1)


def test_reduction_rejects_foreign_set_body_and_name_capture():
    source, owner, rows = owner_and_rows()
    sibling = source.component("Sibling")
    foreign = sibling.index_set("Rows", extent=3)
    p = sibling.parameter("p", value_type=eqiora.ValueType.integer())
    with pytest.raises(q.SourceError, match="index set"):
        owner.sum(lambda i: 1, over=foreign)
    with pytest.raises(q.SourceError, match="Component"):
        owner.sum(lambda i: p, over=rows)
    with pytest.raises(q.SourceError, match="capture"):
        owner.sum(lambda i: 1, over=rows, name="Rows")
    with pytest.raises(q.SourceError, match="capture"):
        owner.sum(lambda i: owner.sum(lambda j: q.ordinal(j), over=rows), over=rows)
    owner.sum(lambda i: 1, over=rows)
    with pytest.raises(q.SourceError, match="capture"):
        owner.let_alias("i", 1)


def test_reduction_bounds_and_callback_failure_leave_scope_usable():
    _, owner, rows = owner_and_rows()
    def fail(i):
        raise RuntimeError("body failed")
    with pytest.raises(RuntimeError, match="body failed"):
        owner.sum(fail, over=rows)
    owner.let_alias("ok", owner.sum(lambda i: 1, over=rows))
    with pytest.raises(q.SourceError, match="limit"):
        owner.sum(lambda i: q.array([q.ordinal(i)] * 4096), over=rows)


def test_nested_reduction_executes_without_capturing_inner_index():
    source, owner, rows = owner_and_rows()
    tick = owner.clock("tick", period_s=1)
    out = owner.output("result", value_type=eqiora.ValueType.integer(), at=tick)
    nested = owner.sum(lambda i: owner.sum(lambda j: q.ordinal(i) + q.ordinal(j),
                       over=rows, name="j"), over=rows)
    owner.relation("emit", at=tick, left=out, right=nested)
    model = eqiora.compile(source=source, entry="Reduction")
    session = model.sampled_session(end_time_s=0.1, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    # Each of 0,1,2 occurs three times in each of the two positions.
    assert session.output("result", 0)[1] == 18


def test_reduction_allows_local_parameter_capture_and_rejects_runtime_index():
    _, owner, rows = owner_and_rows()
    p = owner.parameter("p", value_type=eqiora.ValueType.integer())
    owner.let_alias("total", owner.sum(lambda i: p * (q.ordinal(i) + 1), over=rows))
    with pytest.raises(q.SourceError, match="binder"):
        q.array((1, 2, 3))[p]


def test_reduction_expansion_rejects_out_of_bounds_index():
    source, owner, rows = owner_and_rows()
    out = owner.field("result", role=eqiora.FieldRole.Variable,
                      value_type=eqiora.ValueType.integer())
    owner.relation("emit", left=out, right=owner.sum(
        lambda i: q.array((1, 2))[q.ordinal(i)], over=rows))
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=source, entry="Reduction")
    assert any("index" in diagnostic.message.lower() for diagnostic in error.value.diagnostics)
