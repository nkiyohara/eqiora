"""Independent installed-wheel oracle for the bounded native Mesh display hook."""

from __future__ import annotations

import gc
import hashlib
import importlib.util
import inspect
import sys
import weakref
from collections.abc import Collection, Mapping
from dataclasses import dataclass
from typing import Any

import numpy as np
import pytest

import eqiora


PLAIN_MIME = "text/plain"
WIDGET_MIME = "application/vnd.jupyter.widget-view+json"
SUPPORTED_MIMES = frozenset((PLAIN_MIME, WIDGET_MIME))

SOURCE_DIGEST = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
CANONICAL_BYTES = 42_388
CANONICAL_RAW_SHA256 = (
    "9d3c6211e6832aa5a5f7e99fa210058ff1b76eab7f1e99aaa7033c282d6e2dd2"
)
MESH_DIGEST = "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b"
PROFILE = "circular-hole-gmsh-4.15.2/v1"
VERTEX_COUNT = 662
TRIANGLE_COUNT = 1_210
COORDINATE_BYTES = 10_592
COORDINATE_BYTES_SHA256 = (
    "42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d"
)
TRIANGLE_BYTES = 14_520
TRIANGLE_BYTES_SHA256 = (
    "05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642"
)

UNSUPPORTED_DIAGNOSTIC = (
    "Notebook view unavailable: this N1 viewer supports only the exact accepted "
    "Gmsh 4.15.2 circular-hole Mesh (662 vertices, 1210 triangles)."
)
CORRUPT_DIAGNOSTIC = (
    "Notebook view unavailable: the installed Eqiora Notebook presentation "
    "runtime or assets are incomplete. Reinstall eqiora[notebook]."
)

STANDARD_ARGUMENTS: dict[str, Any] = {
    "classification_tolerance": 1e-12,
    "x_lower": "inlet",
    "x_upper": "outlet",
}

IDENTITY_TRAIT = "_eqiora_n1_model_id"
IDENTITY_REJECTION = "Eqiora Mesh model identity is immutable"
RETIRED_ORACLE_CLOSE_KIND = "eqiora:n1-oracle-close"


def _geometry(**overrides: object) -> object:
    arguments = STANDARD_ARGUMENTS | overrides
    graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(0.0, 2.2),
        y_bounds=(0.0, 0.41),
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=1e-10,
    ).circular_through_cut(
        center=(0.2, 0.2),
        radius=0.05,
        boolean_tolerance=1e-10,
    )
    return graph.planar_circular_section(
        classification_tolerance=arguments["classification_tolerance"],
        region="fluid",
        x_lower=arguments["x_lower"],
        x_upper=arguments["x_upper"],
        y_lower="walls",
        y_upper="walls",
        hole="cylinder",
    )


def _mesh(authored: object | None = None) -> object:
    source = _geometry() if authored is None else authored
    request = eqiora.meshing.MeshRequest(
        maximum_boundary_error=1e-4,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=50,
    )
    plan = eqiora.meshing.resolve(source, request)
    return eqiora.meshing.generate(source, plan=plan)


def _bundle(
    mesh: object,
    *,
    include: Collection[str] | None = None,
    exclude: Collection[str] | None = None,
) -> dict[str, object]:
    result = mesh._repr_mimebundle_(include=include, exclude=exclude)
    assert type(result) is dict
    assert set(result) <= SUPPORTED_MIMES
    return result


def _model_id(bundle: Mapping[str, object]) -> str:
    value = bundle[WIDGET_MIME]
    assert type(value) is dict
    assert value.keys() == {"version_major", "version_minor", "model_id"}
    assert value["version_major"] == 2
    assert value["version_minor"] == 0
    model_id = value["model_id"]
    assert isinstance(model_id, str) and model_id
    return model_id


def _widget_registry() -> Mapping[str, object]:
    from ipywidgets import Widget

    registry = Widget.widgets
    assert isinstance(registry, Mapping)
    return registry


def _delegate(model_id: str) -> object:
    delegate = _widget_registry().get(model_id)
    assert delegate is not None
    return delegate


def _bytes(value: object) -> bytes:
    if isinstance(value, bytes):
        return value
    if isinstance(value, bytearray):
        return bytes(value)
    if isinstance(value, memoryview):
        return value.tobytes()
    tobytes = getattr(value, "tobytes", None)
    assert callable(tobytes)
    encoded = tobytes()
    assert isinstance(encoded, bytes)
    return encoded


@dataclass(frozen=True)
class _MeshSnapshot:
    text: str
    source_digest: str
    realized_geometry_digest: str
    mesh_digest: str
    correspondence_digest: str
    realization_digest: str
    canonical: bytes
    coordinate_object: object
    coordinate_bytes: bytes
    coordinate_shape: tuple[int, ...]
    coordinate_dtype: str
    coordinate_writeable: bool
    cell_object: object
    cell_bytes: bytes
    cell_shape: tuple[int, ...]
    cell_dtype: str
    cell_writeable: bool


def _snapshot(mesh: object) -> _MeshSnapshot:
    coordinates = mesh.coordinates
    cells = mesh.cells
    return _MeshSnapshot(
        text=repr(mesh),
        source_digest=mesh.source_digest,
        realized_geometry_digest=mesh.realized_geometry_digest,
        mesh_digest=mesh.digest,
        correspondence_digest=mesh.correspondence_digest,
        realization_digest=mesh.realization_digest,
        canonical=mesh.canonical_bytes,
        coordinate_object=coordinates,
        coordinate_bytes=coordinates.tobytes(order="C"),
        coordinate_shape=coordinates.shape,
        coordinate_dtype=coordinates.dtype.str,
        coordinate_writeable=coordinates.flags.writeable,
        cell_object=cells,
        cell_bytes=cells.tobytes(order="C"),
        cell_shape=cells.shape,
        cell_dtype=cells.dtype.str,
        cell_writeable=cells.flags.writeable,
    )


def _assert_unchanged(mesh: object, expected: _MeshSnapshot) -> None:
    assert repr(mesh) == expected.text
    assert mesh.source_digest == expected.source_digest
    assert mesh.realized_geometry_digest == expected.realized_geometry_digest
    assert mesh.digest == expected.mesh_digest
    assert mesh.correspondence_digest == expected.correspondence_digest
    assert mesh.realization_digest == expected.realization_digest
    assert mesh.canonical_bytes == expected.canonical
    assert mesh.coordinates is expected.coordinate_object
    assert mesh.coordinates.tobytes(order="C") == expected.coordinate_bytes
    assert mesh.coordinates.shape == expected.coordinate_shape
    assert mesh.coordinates.dtype.str == expected.coordinate_dtype
    assert mesh.coordinates.flags.writeable is expected.coordinate_writeable
    assert mesh.cells is expected.cell_object
    assert mesh.cells.tobytes(order="C") == expected.cell_bytes
    assert mesh.cells.shape == expected.cell_shape
    assert mesh.cells.dtype.str == expected.cell_dtype
    assert mesh.cells.flags.writeable is expected.cell_writeable


def _assert_exact_reference(mesh: object) -> None:
    assert mesh.source_digest == SOURCE_DIGEST
    assert mesh.digest == MESH_DIGEST
    assert len(mesh.canonical_bytes) == CANONICAL_BYTES
    assert hashlib.sha256(mesh.canonical_bytes).hexdigest() == CANONICAL_RAW_SHA256
    assert mesh.dimension == 2
    assert mesh.vertex_count == VERTEX_COUNT
    assert mesh.cell_count == TRIANGLE_COUNT
    assert mesh.coordinates.shape == (VERTEX_COUNT, 2)
    assert mesh.coordinates.dtype == np.dtype(np.float64)
    assert not mesh.coordinates.flags.writeable
    assert hashlib.sha256(mesh.coordinates.tobytes(order="C")).hexdigest() == (
        COORDINATE_BYTES_SHA256
    )
    assert mesh.cells.shape == (TRIANGLE_COUNT, 3)
    assert mesh.cells.dtype == np.dtype(np.uint32)
    assert not mesh.cells.flags.writeable
    assert hashlib.sha256(mesh.cells.tobytes(order="C")).hexdigest() == (
        TRIANGLE_BYTES_SHA256
    )


def test_hook_signature_invalid_arguments_and_plain_filtering_are_exact() -> None:
    mesh = _mesh()
    _assert_exact_reference(mesh)
    text = repr(mesh)
    signature = inspect.signature(mesh._repr_mimebundle_)
    assert tuple(signature.parameters) == ("include", "exclude")
    assert all(parameter.default is None for parameter in signature.parameters.values())

    presentation_before = {
        name
        for name in sys.modules
        if name == "anywidget"
        or name.startswith("anywidget.")
        or name == "eqiora._presentation"
        or name.startswith("eqiora._presentation.")
    }
    cases = (
        (None, {WIDGET_MIME}, {PLAIN_MIME: text}),
        ({PLAIN_MIME}, None, {PLAIN_MIME: text}),
        ({PLAIN_MIME}, {PLAIN_MIME}, {}),
        (set(), None, {}),
        ({"application/x-foreign"}, None, {}),
        ({PLAIN_MIME, "application/x-foreign"}, None, {PLAIN_MIME: text}),
        (None, SUPPORTED_MIMES, {}),
    )
    for include, exclude, expected in cases:
        assert _bundle(mesh, include=include, exclude=exclude) == expected

    presentation_after = {
        name
        for name in sys.modules
        if name == "anywidget"
        or name.startswith("anywidget.")
        or name == "eqiora._presentation"
        or name.startswith("eqiora._presentation.")
    }
    assert presentation_after == presentation_before

    for keyword, value in (
        ("include", 7),
        ("exclude", object()),
        ("include", [PLAIN_MIME, 1]),
        ("exclude", [WIDGET_MIME, None]),
    ):
        with pytest.raises(TypeError):
            mesh._repr_mimebundle_(**{keyword: value})
    presentation_after_invalid = {
        name
        for name in sys.modules
        if name == "anywidget"
        or name.startswith("anywidget.")
        or name == "eqiora._presentation"
        or name.startswith("eqiora._presentation.")
    }
    assert presentation_after_invalid == presentation_before


def test_absent_optional_runtime_is_plain_and_zero_comm() -> None:
    if importlib.util.find_spec("anywidget") is not None:
        pytest.skip("the exact base-only candidate profile owns this observation")

    mesh = _mesh()
    text = repr(mesh)
    assert _bundle(mesh) == {PLAIN_MIME: text}
    assert _bundle(mesh, include={WIDGET_MIME}) == {}
    assert not any(
        name == "anywidget" or name.startswith("anywidget.") for name in sys.modules
    )


def test_same_shape_foreign_source_is_unsupported_before_optional_import() -> None:
    accepted = _mesh()
    swapped = _mesh(_geometry(x_lower="outlet", x_upper="inlet"))
    assert swapped.source_digest != accepted.source_digest
    assert swapped.canonical_bytes == accepted.canonical_bytes
    assert swapped.digest == accepted.digest
    assert swapped.coordinates.shape == accepted.coordinates.shape == (
        VERTEX_COUNT,
        2,
    )
    assert swapped.cells.shape == accepted.cells.shape == (TRIANGLE_COUNT, 3)

    before = set(_widget_registry()) if importlib.util.find_spec("anywidget") else set()
    expected = f"{repr(swapped)}\n{UNSUPPORTED_DIAGNOSTIC}"
    assert _bundle(swapped) == {PLAIN_MIME: expected}
    assert _bundle(swapped, include={WIDGET_MIME}) == {}
    if importlib.util.find_spec("anywidget"):
        assert set(_widget_registry()) == before


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns rich protocol evidence",
)
def test_rich_hook_filtering_reuses_open_delegate_and_refreshes_after_close() -> None:
    import anywidget

    assert anywidget.__version__ == "0.11.0"
    mesh = _mesh()
    expected = _snapshot(mesh)
    registry = _widget_registry()
    before = set(registry)

    # Filtering precedes delegate construction even when the extra is installed.
    assert _bundle(mesh, include={PLAIN_MIME}) == {PLAIN_MIME: repr(mesh)}
    assert _bundle(mesh, include={WIDGET_MIME}, exclude={WIDGET_MIME}) == {}
    assert set(registry) == before

    first = _bundle(mesh)
    first_id = _model_id(first)
    assert first[PLAIN_MIME] == repr(mesh)
    assert first_id not in before
    assert _model_id(_bundle(mesh, include={WIDGET_MIME})) == first_id
    assert _model_id(_bundle(mesh)) == first_id
    assert set(registry) == before | {first_id}

    delegate = _delegate(first_id)
    delegate.close()
    refreshed = _bundle(mesh)
    refreshed_id = _model_id(refreshed)
    assert refreshed_id != first_id
    assert refreshed[PLAIN_MIME] == repr(mesh)
    assert refreshed_id in registry
    _assert_unchanged(mesh, expected)
    _delegate(refreshed_id).close()


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns rich protocol evidence",
)
def test_delegates_are_per_mesh_and_outlive_collected_mesh_wrappers() -> None:
    first = _mesh()
    second = _mesh()
    first_id = _model_id(_bundle(first))
    second_id = _model_id(_bundle(second))
    assert first_id != second_id

    delegate = _delegate(first_id)
    first_reference = weakref.ref(first)
    del first
    gc.collect()
    assert first_reference() is None
    assert _delegate(first_id) is delegate
    assert _model_id(delegate._repr_mimebundle_()[0]) == first_id

    delegate.close()
    _delegate(second_id).close()


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns private payload evidence",
)
def test_private_payload_is_exact_little_endian_immutable_and_mesh_preserving() -> None:
    from traitlets import TraitError

    mesh = _mesh()
    expected = _snapshot(mesh)
    model_id = _model_id(_bundle(mesh))
    delegate = _delegate(model_id)
    state = delegate.get_state()
    payload_keys = {
        "profile",
        "mesh_digest",
        "vertex_count",
        "triangle_count",
        "coordinates_f64_le",
        "triangles_u32_le",
        "correspondence_digest",
        "selection_membership",
        "selection_membership_sha256",
    }
    assert payload_keys <= state.keys()
    assert state["profile"] == PROFILE
    assert state["mesh_digest"] == MESH_DIGEST
    assert state["vertex_count"] == VERTEX_COUNT
    assert state["triangle_count"] == TRIANGLE_COUNT

    coordinate_bytes = _bytes(state["coordinates_f64_le"])
    triangle_bytes = _bytes(state["triangles_u32_le"])
    selection_membership = _bytes(state["selection_membership"])
    assert len(coordinate_bytes) == COORDINATE_BYTES
    assert len(triangle_bytes) == TRIANGLE_BYTES
    assert coordinate_bytes == np.asarray(mesh.coordinates, dtype="<f8").tobytes(
        order="C"
    )
    assert triangle_bytes == np.asarray(mesh.cells, dtype="<u4").tobytes(order="C")
    assert state["correspondence_digest"] == mesh.correspondence_digest
    assert selection_membership.startswith(b"eqiora.mesh-selection-membership/v1\0")
    assert hashlib.sha256(selection_membership).hexdigest() == state[
        "selection_membership_sha256"
    ]

    mutations: dict[str, object] = {
        "profile": "foreign/v1",
        "mesh_digest": "f" * 64,
        "vertex_count": VERTEX_COUNT - 1,
        "triangle_count": TRIANGLE_COUNT - 1,
        "coordinates_f64_le": bytes(COORDINATE_BYTES),
        "triangles_u32_le": bytes(TRIANGLE_BYTES),
        "correspondence_digest": "f" * 64,
        "selection_membership": b"foreign",
        "selection_membership_sha256": "f" * 64,
    }
    for name, mutation in mutations.items():
        original = delegate.get_state()[name]
        with pytest.raises((TraitError, TypeError, ValueError)):
            delegate.set_state({name: mutation})
        current = delegate.get_state()[name]
        if name.endswith("_le"):
            assert _bytes(current) == _bytes(original)
        else:
            assert current == original

    callbacks = getattr(getattr(delegate, "_msg_callbacks", None), "callbacks", ())
    assert tuple(callbacks) == ()
    _assert_unchanged(mesh, expected)
    delegate.close()


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns corrupt-runtime evidence",
)
def test_unexpected_delegate_shape_closes_comm_and_returns_exact_corrupt_fallback(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    mesh = _mesh()
    first_id = _model_id(_bundle(mesh))
    delegate = _delegate(first_id)
    delegate_type = type(delegate)

    monkeypatch.setattr(
        delegate_type,
        "_repr_mimebundle_",
        lambda self, *args, **kwargs: {WIDGET_MIME: {"model_id": first_id}},
    )
    expected = f"{repr(mesh)}\n{CORRUPT_DIAGNOSTIC}"
    assert _bundle(mesh) == {PLAIN_MIME: expected}
    assert first_id not in _widget_registry()


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns private payload evidence",
)
def test_model_identity_trait_is_exact_and_rejects_incoming_mutation() -> None:
    """Evidence O12: client-facing identity immutability.

    The stable owner is ``interfaces.python-rich-mesh-display``.

    Precommitted by the non-writer evidence owner from the accepted decision
    (`issue312-h2-returned-judgements-decision-v1.md` §3.3); the implementer
    did not author these expectations.
    """
    from traitlets import TraitError

    mesh = _mesh()
    expected = _snapshot(mesh)
    model_id = _model_id(_bundle(mesh))
    delegate = _delegate(model_id)

    # Ordinary positive path: the identity trait exists, syncs, and equals both
    # the advertised widget-view model id and ipywidgets' own model identity.
    state = delegate.get_state()
    assert IDENTITY_TRAIT in state
    identity = state[IDENTITY_TRAIT]
    assert isinstance(identity, str)
    assert identity == model_id
    assert delegate.model_id == identity
    assert len(identity) == 32
    assert set(identity) <= set("0123456789abcdef")

    # Incoming (client-to-kernel) mutation is rejected with the identity's own
    # distinct diagnostic, and rejection leaves the trait and Mesh unchanged.
    for mutation in ("f" * 32, "", identity.upper()):
        with pytest.raises(TraitError, match=IDENTITY_REJECTION):
            delegate.set_state({IDENTITY_TRAIT: mutation})
        assert delegate.get_state()[IDENTITY_TRAIT] == identity
    _assert_unchanged(mesh, expected)
    delegate.close()


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns private payload evidence",
)
def test_model_identity_kernel_side_assignment_is_rejected_symmetrically() -> None:
    """Evidence O12: the accepted symmetric identity guard.

    The stable owner is ``interfaces.python-rich-mesh-display``.

    This precommitted evidence is an ordinary RED on the predecessor, where a
    kernel-side assignment silently diverges the synced identity from the comm
    identity. It becomes GREEN only when production extends the accepted
    ``@traitlets.validate`` guard to the identity trait.
    """
    from traitlets import TraitError

    mesh = _mesh()
    model_id = _model_id(_bundle(mesh))
    delegate = _delegate(model_id)
    try:
        with pytest.raises(TraitError, match=IDENTITY_REJECTION):
            delegate._eqiora_n1_model_id = "f" * 32
        assert delegate.get_state()[IDENTITY_TRAIT] == model_id
    finally:
        delegate.close()


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns rich protocol evidence",
)
def test_shipped_view_module_text_declares_no_write_or_custom_message_bridge() -> None:
    """Evidence O9, the static half over the client artifact.

    The stable owner is ``interfaces.python-rich-mesh-display``.

    The synced ``_esm`` state is the exact module text every host runs, so
    literal membership here measures the shipped artifact rather than a source
    tree. Bounds: these are literal-membership probes only; a dynamically
    constructed method or event name would evade them. The runtime half —
    observing every client-to-server payload for a mesh model id — lives in
    ``notebook-hosts.spec.ts``.

    The final assertion is the precommitted falsifier for the O6 write-half
    deletion: expected RED while the shipped module still carries the retired
    kernel-close custom-message kind, GREEN once it is deleted.
    """
    mesh = _mesh()
    delegate = _delegate(_model_id(_bundle(mesh)))
    esm = delegate.get_state()["_esm"]
    delegate.close()
    assert isinstance(esm, str)

    # Ordinary positive path: this is the real shipped module, carrying the
    # observation seam the host evidence drives.
    assert "__eqioraN1Oracle" in esm
    assert "snapshot" in esm

    # Negative probes: no custom-message handler registration and no model
    # save/write helper is even named by the shipped module text.
    assert "msg:custom" not in esm
    assert "save_changes" not in esm

    # Precommitted O6 falsifier: the kernel-close message kind must vanish.
    assert RETIRED_ORACLE_CLOSE_KIND not in esm


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns rich protocol evidence",
)
def test_incoming_custom_messages_are_inert_and_leave_delegate_open() -> None:
    """Evidence O9 resolves the custom-message half.

    The stable owner is ``interfaces.python-rich-mesh-display``. It defines how
    ``incoming_payload_writes_and_custom_messages_reject`` is discharged.

    Recorded reading (per the accepted review's finding 4): the Jupyter comm
    protocol has no rejection frame for a custom message, so refusal to act —
    the delegate stays open, its state and the Mesh stay unchanged, and no
    registered consumer exists afterwards — is the strongest observable
    rejection, and this evidence adopts proven inertness as the discharge.
    The payload-write half remains discharged by ``set_state`` raising (the
    payload and identity tests above).

    A capture callback is registered only to prove the ordinary positive path:
    each probe traverses the real incoming dispatch into the custom-message
    branch before its inertness is asserted. Expected RED before the O6
    kernel-side deletion, whose override still intercepts the retired close
    kind and closes the delegate; GREEN once incoming custom messages fall
    through to the base handler.
    """
    mesh = _mesh()
    expected = _snapshot(mesh)
    model_id = _model_id(_bundle(mesh))
    delegate = _delegate(model_id)

    received: list[tuple[object, tuple[object, ...]]] = []

    def _capture(_widget: object, content: object, buffers: object) -> None:
        received.append((content, tuple(buffers)))

    delegate.on_msg(_capture)
    probes: tuple[tuple[object, list[bytes]], ...] = (
        ({"kind": "probe"}, []),
        ({"kind": "probe-with-buffer"}, [b"\x00\x01"]),
        ({"kind": RETIRED_ORACLE_CLOSE_KIND}, []),
        ({"kind": RETIRED_ORACLE_CLOSE_KIND}, [b"\x00"]),
        (RETIRED_ORACLE_CLOSE_KIND, []),
        ({}, []),
    )
    try:
        for index, (content, buffers) in enumerate(probes):
            delegate._handle_msg(
                {
                    "content": {"data": {"method": "custom", "content": content}},
                    "buffers": buffers,
                }
            )
            # Ordinary positive path: the probe reached the custom branch.
            assert len(received) == index + 1, (
                "an incoming custom message was consumed before the "
                "base dispatch instead of falling through untouched"
            )
            assert received[index] == (content, tuple(buffers))
            # ...and was refused: the delegate stays open and unchanged.
            assert _delegate(model_id) is delegate
            assert delegate.get_state()[IDENTITY_TRAIT] == model_id
    finally:
        delegate.on_msg(_capture, remove=True)

    callbacks = getattr(getattr(delegate, "_msg_callbacks", None), "callbacks", ())
    assert tuple(callbacks) == ()
    _assert_unchanged(mesh, expected)
    delegate.close()
