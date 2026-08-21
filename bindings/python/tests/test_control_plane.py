import subprocess
import sys
from importlib import metadata

import pytest


SOURCE = """
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"""

MALFORMED_RETIRED_SCHEMA_SPECIMEN = b'{"schema":"eqiora.model-envelope/v7"}'


def test_distribution_version_is_native_and_matches_installed_metadata() -> None:
    import eqiora

    assert eqiora.__version__ == metadata.version("eqiora") == "0.1.0a2"


def test_base_import_does_not_load_optional_numerical_frameworks() -> None:
    script = """
import sys
import eqiora

for name in ("numpy", "torch", "jax", "cupy"):
    assert name not in sys.modules, (name, sorted(sys.modules))
"""
    subprocess.run(
        [sys.executable, "-I", "-c", script],
        check=True,
        text=True,
        capture_output=True,
    )


def test_revision_identity_is_exact_across_artifact_replay() -> None:
    import eqiora

    model = eqiora.compile(SOURCE, filename="decay.eqi")
    revision = model.revision
    replay = eqiora.replay(model.to_json())

    assert revision.number == 1
    assert revision.model_id == model.model_id
    assert revision.digest == model.digest
    assert replay.revision == revision
    assert replay == model
    assert hash(replay) == hash(model)
    assert hash(replay.revision) == hash(revision)


def test_current_only_surface_rejects_retired_selectors_and_malformed_replay() -> None:
    import eqiora

    for retired in (
        "compatibility",
        "ExactModelCodec",
        "compile_exact",
        "define_exact",
        "replay_exact",
    ):
        assert not hasattr(eqiora, retired)

    with pytest.raises(eqiora.CompatibilityError) as caught:
        eqiora.replay(MALFORMED_RETIRED_SCHEMA_SPECIMEN)
    assert [diagnostic.code for diagnostic in caught.value.diagnostics] == ["EQ0901"]


def test_value_edit_is_atomic_immutable_and_stale_base_safe() -> None:
    import eqiora

    base = eqiora.compile(SOURCE, filename="decay.eqi")
    edit = base.preview_value_edit("rate", 2.0)
    child = base.commit(edit)

    assert base.revision.number == 1
    assert child.revision.number == 2
    assert child.model_id == base.model_id
    assert child.digest != base.digest
    assert edit.base_digest == base.digest
    assert edit.base_revision == base.revision.number
    assert edit.target_id in base.parameter_ids
    assert edit == base.preview_value_edit("rate", 2.0)
    assert hash(edit) == hash(base.preview_value_edit("rate", 2.0))

    replay = eqiora.replay(child.to_json())
    assert replay == child

    with pytest.raises(eqiora.ValidationError) as caught:
        child.commit(edit)
    assert caught.value.category == "validation"
    assert [diagnostic.code for diagnostic in caught.value.diagnostics] == ["EQ0106"]

    replay_edit = replay.preview_value_edit(replay.parameter_ids[0], 3.0)
    grandchild = replay.commit(replay_edit)
    assert grandchild.revision.number == 3

    sibling = base.commit(base.preview_value_edit("rate", 3.0))
    child_state_edit = child.preview_value_edit("x", 2.0)
    sibling_state_edit = sibling.preview_value_edit("x", 2.0)
    assert child_state_edit.key != sibling_state_edit.key
    assert child_state_edit != sibling_state_edit
    assert len({child_state_edit, sibling_state_edit}) == 2


def test_direct_exception_construction_has_the_stubbed_attributes() -> None:
    import eqiora

    error = eqiora.ValidationError("manually constructed")
    assert error.category == "validation"
    assert error.diagnostics == ()


def test_exception_taxonomy_keeps_structured_diagnostics() -> None:
    import eqiora

    with pytest.raises(eqiora.ValidationError) as validation:
        eqiora.compile("model broken { field ; }", filename="broken.eqi")
    assert isinstance(validation.value, eqiora.EqioraError)
    assert validation.value.category == "validation"
    assert validation.value.diagnostics[0].source_span is not None

    with pytest.raises(eqiora.CompatibilityError) as compatibility:
        eqiora.replay(b"{}")
    assert compatibility.value.category == "compatibility"
    assert compatibility.value.diagnostics[0].code == "EQ0901"

    model = eqiora.compile(SOURCE)
    with pytest.raises(eqiora.ExecutionError) as execution:
        eqiora.run(model, end_time=1.0, max_step=0.0)
    assert execution.value.category == "execution"
    assert execution.value.diagnostics[0].code == "EQ0501"

    assert issubclass(eqiora.CapabilityError, eqiora.EqioraError)
    assert issubclass(eqiora.CancellationError, eqiora.EqioraError)
    assert issubclass(eqiora.InternalError, eqiora.EqioraError)
