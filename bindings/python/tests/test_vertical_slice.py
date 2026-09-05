import gc
import json
from pathlib import Path

import numpy as np
import pytest

import eqiora


CONTROL_FIXTURES = (
    Path(__file__).resolve().parents[3]
    / "verify"
    / "interfaces"
    / "control-plane-compile-check"
)
CURRENT_PROFILE_FIXTURE = (
    Path(__file__).resolve().parents[3]
    / "verify"
    / "interfaces"
    / "current-authoring-profile"
    / "expected"
    / "profile.json"
)


def load_control_fixture(relative_path: str) -> dict[str, object]:
    return json.loads((CONTROL_FIXTURES / relative_path).read_text(encoding="utf-8"))


def test_python_authoring_and_replay_use_the_registered_current_profile() -> None:
    profile = json.loads(CURRENT_PROFILE_FIXTURE.read_text(encoding="utf-8"))
    model = eqiora.compile(source=SOURCE, filename="current.eqi")
    assert json.loads(model.to_bytes())["schema"] == profile["modelSchema"]

    state = eqiora.Field("x", initial=1.0)
    hold = eqiora.Relation("hold", residual=eqiora.derivative(state))
    native = eqiora.Model.define("hold", state, hold)
    assert json.loads(native.to_bytes())["schema"] == profile["modelSchema"]
    replayed = eqiora.Model.from_bytes(model.to_bytes())
    assert replayed.to_bytes() == model.to_bytes()
    assert replayed.digest == model.digest


SOURCE = """
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"""

PHYSICAL_SOURCE = """
model physical_pair {
  domain electrical = scalar_physical(
    across = kg * m ^ 2 / (s ^ 3 * A),
    through = A
  );
  port left: conserving on electrical;
  port right: conserving on electrical;
  relation component continuous {
    across(left) = 0;
    through(right) = 0;
  }
  connect conserving left, right;
}
"""

SPATIAL_SOURCE = """
model native_poisson {
  domain interval = box(0, 1);
  domain lower_end = boundary(interval, axis = 0, side = lower);
  domain upper_end = boundary(interval, axis = 0, side = upper);
  representation scalar_space = continuum;
  field potential on interval as scalar_space: 1 = 0;
  parameter source_scale: 1 / m ^ 2 = 1;
  relation balance continuous on interval {
    -div(grad(potential)) - source_scale = 0;
  }
  relation lower_value continuous on lower_end { trace(potential) = 0; }
  relation upper_value continuous on upper_end { trace(potential) = 0; }
}
"""


def test_compile_artifact_run_and_owned_numpy_result() -> None:
    model = eqiora.compile(source=SOURCE, filename="decay.eqi")
    artifact = model.to_bytes()
    assert len(model.digest) == 64
    reconstructed = eqiora.Model.from_bytes(artifact)
    assert reconstructed.to_bytes() == artifact
    assert reconstructed.digest == model.digest

    field = model.field(model.field_ids[0])
    plan = eqiora.resolve(
        model,
        temporal=eqiora.time.Tsitouras45(
            initial_step_s=0.01,
            relative_tolerance=1.0e-9,
            absolute_tolerances={field: 1.0e-11},
        ),
    )
    result = eqiora.run(
        plan,
        state=eqiora.State.initial(plan),
        until_s=0.2,
        output_times_s=(0.1, 0.2),
    )
    series = result.series(field)
    assert isinstance(result.fields, list)
    assert result.fields == [series]
    assert len(result.fields) == 1
    assert not hasattr(result, "snapshots")
    assert series.dimension == (0, 0, 0, 0, 0, 0, 0)
    time = series.time.numpy(copy=False)
    values = series.values.numpy(copy=False)
    assert time is series.time.numpy(copy=False)
    assert values is series.values.numpy(copy=False)
    assert series.values.device == "cpu"
    assert series.values.dtype == "float64"
    assert series.values.shape == (2,)
    assert np.array_equal(time, np.array([0.1, 0.2]))
    assert np.allclose(values, np.exp(-time), rtol=2.0e-8, atol=2.0e-10)
    assert not time.flags.writeable
    assert not values.flags.writeable
    copied = series.values.numpy(copy=True)
    assert copied is not values
    assert copied.flags.writeable

    assert not hasattr(result, "field")
    with pytest.raises(KeyError):
        result.mesh(field)
    assert not hasattr(result, "run_manifest")
    with pytest.raises(eqiora.CapabilityError):
        eqiora.fluid.steady_stokes_evidence(result)

    del series, result, model
    gc.collect()
    assert time[-1] == pytest.approx(0.2)
    assert values[-1] == pytest.approx(np.exp(-0.2), rel=2.0e-8)


def test_diagnostics_are_structured() -> None:
    with pytest.raises(eqiora.EqioraError) as caught:
        eqiora.compile(source="model broken { field ; }", filename="broken.eqi")
    assert caught.value.diagnostics
    diagnostic = caught.value.diagnostics[0]
    assert diagnostic.code.startswith("EQ")
    assert diagnostic.severity == "error"
    assert diagnostic.message
    assert diagnostic.source_span is not None

    with pytest.raises(eqiora.EqioraError) as caught:
        eqiora.time.Tsitouras45(
            initial_step_s=0.0,
            relative_tolerance=1.0e-9,
            absolute_tolerances={eqiora.compile(source=SOURCE).field("x"): 1.0e-11},
        )
    assert caught.value.diagnostics[0].code == "EQ0807"


def test_compile_request_fails_closed_before_entering_the_compiler() -> None:
    with pytest.raises(eqiora.EqioraError) as caught:
        eqiora.compile(source=SOURCE, filename="invalid\nfilename.eqi")

    assert len(caught.value.diagnostics) == 1
    diagnostic = caught.value.diagnostics[0]
    assert diagnostic.code == "EQ0901"
    assert diagnostic.source == "control"
    assert diagnostic.severity == "error"
    assert diagnostic.graph_path is None
    assert diagnostic.source_span is None


def test_shared_compile_check_fixtures_cross_the_python_adapter() -> None:
    contract = load_control_fixture("expected/contract.json")

    accepted_expectation = contract["accepted"]
    assert isinstance(accepted_expectation, dict)
    accepted = load_control_fixture(f"models/{accepted_expectation['request']}")
    assert accepted["requestId"] == accepted_expectation["requestId"]
    assert accepted_expectation["outcome"] == "accepted"
    model = eqiora.compile(
        source=accepted["source"],
        filename=accepted["filename"],
    )
    artifact = json.loads(model.to_bytes())
    assert artifact["schema"] == accepted_expectation["modelSchema"]

    rejected_expectation = next(
        rejection
        for rejection in contract["rejections"]
        if rejection["name"] == "rejected-source"
    )
    assert isinstance(rejected_expectation, dict)
    rejected = load_control_fixture(f"models/{rejected_expectation['request']}")
    assert rejected["requestId"] == rejected_expectation["requestId"]
    assert rejected_expectation["outcome"] == "rejected"
    with pytest.raises(eqiora.EqioraError) as caught:
        eqiora.compile(
            source=rejected["source"],
            filename=rejected["filename"],
        )
    assert [diagnostic.code for diagnostic in caught.value.diagnostics] == [
        rejected_expectation["diagnosticCode"]
    ]
    assert [diagnostic.source for diagnostic in caught.value.diagnostics] == [
        rejected_expectation["diagnosticSource"]
    ]


def test_native_declarations_share_the_canonical_compile_and_run_path() -> None:
    state = eqiora.Field("x", initial=1.0)
    rate = eqiora.Parameter(
        "rate",
        dimension=eqiora.Dimension(time=-1),
        value=1.0,
    )
    flow = eqiora.Relation(
        "flow",
        residual=eqiora.derivative(state) + rate * state,
    )

    model = eqiora.Model.define("decay", state, rate, flow)
    assert json.loads(model.to_bytes())["schema"] == "eqiora.model-envelope/v9"
    field = model.field(model.field_ids[0])
    plan = eqiora.resolve(
        model,
        temporal=eqiora.time.Tsitouras45(
            initial_step_s=0.01,
            relative_tolerance=1.0e-9,
            absolute_tolerances={field: 1.0e-11},
        ),
    )
    result = eqiora.run(
        plan,
        state=eqiora.State.initial(plan),
        until_s=0.2,
        output_times_s=(0.1, 0.2),
    )

    assert model.revision.number == 1
    assert state.dimension == eqiora.Dimension()
    assert result.series(field).dimension == state.dimension.exponents
    assert np.allclose(
        result.series(field).values.numpy(copy=False),
        np.exp(-np.array([0.1, 0.2])),
        rtol=2.0e-8,
        atol=2.0e-10,
    )


def test_source_and_native_models_share_only_structural_identity() -> None:
    source = eqiora.compile(source=SOURCE, filename="source-decay.eqi")
    state = eqiora.Field("state", initial=1.0)
    rate = eqiora.Parameter(
        "coefficient",
        dimension=eqiora.Dimension(time=-1),
        value=1.0,
    )
    balance = eqiora.Relation(
        "balance",
        residual=eqiora.derivative(state) + rate * state,
    )
    native = eqiora.Model.define("native_decay", balance, rate, state)

    assert source.model_id != native.model_id
    assert source.digest != native.digest
    assert source != native
    assert source.structural_fingerprint == native.structural_fingerprint
    assert source.structural_fingerprint.generation == (
        "eqiora.structural-semantic-fingerprint/v4"
    )
    assert len(source.structural_fingerprint.digest) == 64
    assert source.structurally_equivalent(native)

    changed_rate = eqiora.Parameter(
        "coefficient",
        dimension=eqiora.Dimension(time=-1),
        value=2.0,
    )
    changed_balance = eqiora.Relation(
        "balance",
        residual=eqiora.derivative(state) + changed_rate * state,
    )
    changed = eqiora.Model.define("changed", state, changed_rate, changed_balance)
    assert not source.structurally_equivalent(changed)


def test_native_spatial_model_reuses_shared_support_and_operator_semantics() -> None:
    interval = eqiora.Domain.box("interval", (0.0, 1.0))
    lower = interval.boundary(
        "lower_end",
        axis=0,
        side=eqiora.BoundarySide.Lower,
    )
    upper = interval.boundary(
        "upper_end",
        axis=0,
        side=eqiora.BoundarySide.Upper,
    )
    space = eqiora.Representation.continuum("scalar_space")
    potential = eqiora.Field(
        "potential",
        domain=interval,
        representation=space,
    )
    source_scale = eqiora.Parameter(
        "source_scale",
        dimension=eqiora.Dimension(length=-2),
        value=1.0,
    )
    native = eqiora.Model.define(
        "native_poisson",
        source_scale,
        upper,
        interval,
        potential,
        space,
        lower,
        eqiora.Relation(
            "upper_value",
            domain=upper,
            residual=eqiora.trace(potential),
        ),
        eqiora.Relation(
            "balance",
            domain=interval,
            residual=-eqiora.div(eqiora.grad(potential)) - source_scale,
        ),
        eqiora.Relation(
            "lower_value",
            domain=lower,
            residual=eqiora.trace(potential),
        ),
    )
    source = eqiora.compile(source=SPATIAL_SOURCE, filename="source-poisson.eqi")

    assert native.digest != source.digest
    assert native.structural_fingerprint == source.structural_fingerprint
    assert interval.bounds == [(0.0, 1.0)]
    assert lower.parent == interval
    assert lower.side == eqiora.BoundarySide.Lower
    assert potential.domain == interval
    assert potential.representation == space
    assert eqiora.Domain.box("interval", (0.0, 1.0)) != interval
    assert eqiora.Representation.continuum("scalar_space") != space

    with pytest.raises(TypeError, match="both domain= and representation="):
        eqiora.Field("half_scoped", domain=interval)

    invalid = eqiora.Relation(
        "invalid",
        domain=interval,
        residual=eqiora.trace(potential),
    )
    with pytest.raises(eqiora.ValidationError):
        eqiora.Model.define("support_mismatch", interval, space, potential, invalid)


def test_native_declarations_fail_closed_without_python_semantics() -> None:
    included = eqiora.Field("x", initial=1.0)
    foreign = eqiora.Field("x", initial=1.0)
    relation = eqiora.Relation("flow", residual=foreign)

    with pytest.raises(eqiora.EqioraError) as caught:
        eqiora.Model.define("invalid", included, relation)
    diagnostic = caught.value.diagnostics[0]
    assert diagnostic.code == "EQ0603"
    assert diagnostic.graph_path == ["invalid", "flow"]
    assert diagnostic.source_span is None

    with pytest.raises(TypeError, match="no truth value"):
        bool(included + 1.0)
    with pytest.raises(TypeError, match="expected an Expression"):
        eqiora.Relation("invalid", residual=True)


def test_native_declarations_are_frozen_and_keep_typed_compiler_diagnostics() -> None:
    temperature = eqiora.Field(
        "temperature",
        dimension=eqiora.Dimension(temperature=1),
        initial=293.0,
    )
    duration = eqiora.Parameter(
        "duration",
        dimension=eqiora.Dimension(time=1),
        value=1.0,
    )
    relation = eqiora.Relation(
        "invalid",
        residual=temperature + duration,
    )

    with pytest.raises(AttributeError):
        temperature.name = "renamed"
    with pytest.raises(eqiora.EqioraError) as caught:
        eqiora.Model.define("thermal", temperature, duration, relation)
    diagnostic = caught.value.diagnostics[0]
    assert diagnostic.code == "EQ0603"
    assert diagnostic.graph_path == ["thermal", "invalid"]
    assert diagnostic.source_span is None

    non_finite = eqiora.Field("x", initial=float("nan"))
    flow = eqiora.Relation("flow", residual=eqiora.derivative(non_finite))
    with pytest.raises(eqiora.EqioraError, match="must be finite"):
        eqiora.Model.define("invalid", non_finite, flow)


def physical_pair() -> tuple[
    eqiora.PhysicalDomain,
    eqiora.ConservingPort,
    eqiora.ConservingPort,
    eqiora.Relation,
    eqiora.Connection,
]:
    voltage = eqiora.Dimension(mass=1, length=2, time=-3, current=-1)
    current = eqiora.Dimension(current=1)
    electrical = eqiora.PhysicalDomain(
        "electrical",
        across_dimension=voltage,
        through_dimension=current,
    )
    left = eqiora.ConservingPort("left", domain=electrical)
    right = eqiora.ConservingPort("right", domain=electrical)
    component = eqiora.Relation(
        "component",
        residuals=[eqiora.across(left), eqiora.through(right)],
    )
    net = eqiora.connect(left, right)
    return electrical, left, right, component, net


def test_native_physical_declarations_use_current_and_retain_replay() -> None:
    declarations = physical_pair()

    model = eqiora.Model.define("physical_pair", *declarations)
    artifact = model.to_bytes()
    reconstructed = eqiora.Model.from_bytes(artifact)
    assert reconstructed.to_bytes() == artifact
    assert reconstructed.digest == model.digest

    source = eqiora.compile(source=PHYSICAL_SOURCE, filename="physical-source.eqi")
    assert source.digest != model.digest
    assert source.structural_fingerprint == model.structural_fingerprint
    assert source.structurally_equivalent(model)


def test_physical_source_compile_uses_current_without_user_codec_selection() -> None:
    model = eqiora.compile(
        source=PHYSICAL_SOURCE,
        filename="physical_pair.eqi",
    )
    restored = eqiora.Model.from_bytes(model.to_bytes())
    assert restored.digest == model.digest


def test_native_physical_handles_are_frozen_and_nominal() -> None:
    electrical, left, right, component, net = physical_pair()
    assert left.domain.name == electrical.name
    assert len(component.residuals) == 2
    with pytest.raises(AttributeError, match="no unique residual"):
        component.residual
    with pytest.raises(AttributeError):
        left.name = "renamed"
    with pytest.raises(AttributeError):
        electrical.name = "renamed"
    with pytest.raises(TypeError):
        type(net)()

    equal_but_foreign = eqiora.PhysicalDomain(
        "electrical",
        across_dimension=electrical.across_dimension,
        through_dimension=electrical.through_dimension,
    )
    foreign = eqiora.ConservingPort("foreign", domain=equal_but_foreign)
    invalid = eqiora.connect(left, foreign)
    with pytest.raises(eqiora.EqioraError, match="exact same"):
        eqiora.Model.define(
            "nominal",
            electrical,
            equal_but_foreign,
            left,
            foreign,
            eqiora.Relation("left_owner", residual=eqiora.across(left)),
            eqiora.Relation("foreign_owner", residual=eqiora.across(foreign)),
            invalid,
        )


def test_native_physical_category_errors_do_not_reach_semantics() -> None:
    electrical, left, _, _, _ = physical_pair()
    field = eqiora.Field("x")

    with pytest.raises(TypeError):
        eqiora.ConservingPort("invalid", domain=field)
    with pytest.raises(TypeError):
        eqiora.across(field)
    with pytest.raises(TypeError):
        eqiora.through(electrical)
    with pytest.raises(TypeError, match="ConservingPort"):
        eqiora.connect(left, field)
    with pytest.raises(TypeError, match="exactly one"):
        eqiora.Relation("missing")
    with pytest.raises(TypeError, match="exactly one"):
        eqiora.Relation("ambiguous", residual=field, residuals=[field])
    with pytest.raises(TypeError, match="iterable"):
        eqiora.Relation("invalid", residuals=1.0)

    empty = eqiora.Relation("empty", residuals=[])
    with pytest.raises(eqiora.EqioraError, match="at least one residual"):
        eqiora.Model.define("empty", empty)
