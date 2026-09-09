import eqiora
import pytest


def model_with_collisions(*, parsed=False):
    source = eqiora.Module("main")
    model = source.model("Main")
    left = model.field("left", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real())
    right = model.field("right", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real())
    model.relation("left_law", eqiora.lang.equation(left, 0))
    model.relation("right_law", eqiora.lang.equation(right, 0))
    model.set_notation("left", eqiora.lang.Notation(r"@{x_i}"))
    model.set_notation("right", eqiora.lang.Notation(r"@{\mathbf{x_i}}"))
    return eqiora.compile(source=eqiora.Module.parse("main", source.to_eqi()) if parsed else source)


def test_full_model_labels_and_subviews_share_exact_occurrences():
    model = model_with_collisions()
    full = model.notation_labels()
    assert len(full) == 2
    assert all(isinstance(entry, eqiora.QuantityLabel) for entry in full)
    assert all(repr(entry).startswith("QuantityLabel(selector=") and entry.selector in repr(entry) for entry in full)
    assert len({entry.identity for entry in full}) == 2
    assert len({entry.scope for entry in full}) == 1
    assert {entry.selector for entry in full} == {"left", "right"}
    for profile in ["latex", "mathml", "unicode", "plain", "speech"]:
        rendered = model.notation_labels(profile)
        assert len({entry.label for entry in rendered}) == 2
        selected = model.notation_labels(profile, identities=[full[1].identity, full[1].identity])
        assert len(selected) == 1
        assert selected[0].label == next(entry.label for entry in rendered if entry.identity == full[1].identity)
    assert all(entry.definition_span is None and entry.instance_span is None for entry in full)
    parsed = model_with_collisions(parsed=True)
    assert parsed.structural_fingerprint == model.structural_fingerprint
    assert all(entry.definition_span is not None and entry.instance_span is not None
               for entry in parsed.notation_labels())
    with pytest.raises(AttributeError):
        full[0].label = "changed"
    with pytest.raises(ValueError, match="outside this Model scope"):
        model.notation_labels(identities=["missing"])
    with pytest.raises(ValueError, match="profile"):
        model.notation_labels("tex-engine")


def test_bare_artifact_replay_exposes_only_exact_identity_fallback():
    model = model_with_collisions()
    reopened = eqiora.Model.from_bytes(model.to_bytes())
    assert reopened.digest == model.digest
    entries = reopened.notation_labels()
    assert len(entries) == 2
    assert all(entry.definition_span is None and entry.instance_span is None for entry in entries)
    assert all(entry.graph_id is not None for entry in entries)
    assert len({entry.label for entry in entries}) == 2
    assert {entry.identity for entry in entries}.isdisjoint(entry.identity for entry in model.notation_labels())
