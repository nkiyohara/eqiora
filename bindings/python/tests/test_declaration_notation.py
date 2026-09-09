from pathlib import Path

import eqiora
import pytest

q = eqiora.lang


def source_with_notation(decorated: bool) -> eqiora.Module:
    source = eqiora.Module("main")
    model = source.model("Main", doc="Physical model.")
    value = model.field("value", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real(), doc="Value.")
    model.relation("law", q.equation(value, 0))
    if decorated:
        source.set_notation("Main", q.Notation(r"@{\mathcal{M}}"))
        model.set_notation("value", q.Notation(r"@{\hat{x}_{ij}}"))
        model.set_notation("law", q.Notation(r"@{L}"))
    return source


def test_notation_is_native_validated_canonical_and_immutable():
    value = q.Notation(r"@{x^2_i}")
    assert value.canonical == r"@{x_{i}^{2}}"
    assert str(value) == value.canonical
    with pytest.raises(AttributeError):
        value.canonical = "unsafe"
    for island in [r"@{\input{secret}}", r"@{\newcommand{x}}", r"@{\text{words}}", "@{$x$}", "@{x+y}", "@{x" + " " * 1024 + "}"]:
        with pytest.raises(ValueError):
            q.Notation(island)


def test_notation_retains_source_docs_and_leaves_model_identity_unchanged(tmp_path: Path):
    plain = source_with_notation(False)
    annotated = source_with_notation(True)
    text = annotated.to_eqi()
    assert r"value @{\hat{x}_{i j}}: 1" in text
    assert "/// Value." in text
    assert "value = 0;" in text
    path = tmp_path / "notation.eqi"
    annotated.write_eqi(path)
    assert path.read_text() == text
    first = eqiora.compile(source=plain)
    second = eqiora.compile(source=annotated)
    reopened = eqiora.compile(path=path)
    assert first.digest == second.digest == reopened.digest
    assert first.structural_fingerprint == second.structural_fingerprint


def test_notation_attachment_is_lexically_owned_and_frozen_with_source():
    source = eqiora.Module("main")
    left, right = source.component("Left"), source.component("Right")
    with pytest.raises(q.ModuleError):
        left.volume("failed", dimensions=1, doc="x" * 16385)
    with pytest.raises(q.ModuleError):
        left.set_notation("failed", q.Notation("@{x}"))
    for component in [left, right]:
        component.parameter("value", value_type=eqiora.ValueType.real())
    left.set_notation("value", q.Notation(r"@{\alpha}"))
    right.set_notation("value", q.Notation(r"@{\beta}"))
    for owner, missing in [(source, "value"), (left, "Right"), (right, "missing")]:
        with pytest.raises(q.ModuleError):
            owner.set_notation(missing, q.Notation("@{x}"))
    with pytest.raises(TypeError):
        left.set_notation("value", "@{x}")
    emitted = source.to_eqi()
    assert emitted.count(r"value @{\alpha}") == 1
    assert emitted.count(r"value @{\beta}") == 1
    with pytest.raises(q.ModuleError):
        left.set_notation("value", q.Notation("@{x}"))
    with pytest.raises(q.ModuleError):
        source.set_notation("Left", q.Notation("@{L}"))
