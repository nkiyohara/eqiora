"""Installed renderer projections share native identities and admitted structure."""

import xml.etree.ElementTree as ET

import eqiora
import pytest


PROFILES = ("latex", "mathml", "unicode", "plain", "speech")
SOURCE = r"""model M() {
  parameter a @{a}: 1 = 2;
  parameter b @{b}: 1 = 3;
  variable x @{\mathbf{x_i}_j}: 1;
  relation law { x = (a-b)/(a+b); }
}"""


def references(rendering):
    return tuple((ref.graph_id, ref.role, ref.declarations, ref.operator)
                 for ref in rendering.references)


def test_installed_profiles_keep_native_structure_and_identity_links():
    model = eqiora.compile(source=SOURCE)
    digest = model.digest
    expected_refs = None
    for profile in PROFILES:
        (rendering,) = model.render_equations("law", profile)
        assert isinstance(rendering, eqiora.MathRendering)
        assert repr(rendering).startswith("MathRendering(profile=")
        assert len(repr(rendering)) < 200
        assert rendering.profile == profile
        assert not rendering.used_fallback
        assert rendering.plain == "((x_{i})_{j}) = (((a) - (b)) / ((a) + (b)))"
        assert "divided by" in rendering.speech
        assert "minus" in rendering.speech
        assert all(isinstance(ref, eqiora.MathReference) for ref in rendering.references)
        assert all(repr(ref).startswith("MathReference(graph_id=") and len(repr(ref)) < 300 for ref in rendering.references)
        if expected_refs is None:
            expected_refs = references(rendering)
        assert references(rendering) == expected_refs
        if profile == "mathml":
            root = ET.fromstring(rendering.text)
            assert root.tag == "{http://www.w3.org/1998/Math/MathML}math"
            assert root.find(".//{*}mfrac") is not None
            assert root.find(".//{*}msub/{*}mstyle/{*}msub") is not None
            assert all("href" not in key and not key.startswith("on")
                       for element in root.iter() for key in element.attrib)
        with pytest.raises(AttributeError):
            rendering.text = "changed"
        with pytest.raises(AttributeError):
            rendering.references[0].graph_id = "changed"
    assert model.digest == digest
    assert model.render_formulations() == ()
    with pytest.raises(ValueError, match="exact Relation"):
        model.render_equations("a")
    with pytest.raises(ValueError, match="profile"):
        model.render_equations("law", "rich")
    with pytest.raises(ValueError, match="profile"):
        model.notation_labels("rich")


def test_installed_types_keep_channels_and_spatial_axes_distinct():
    scalar = eqiora.ValueType.complex()
    channel = eqiora.ValueType.array(scalar, 2)
    vector = eqiora.ValueType.vector(scalar, 2)
    for profile in PROFILES:
        first, second = channel.render(profile), vector.render(profile)
        assert first.text != second.text
        assert "channel axis 2" in first.speech
        assert "spatial axis 2" in second.speech
        assert "complex" in first.plain
        if profile == "mathml":
            ET.fromstring(first.text)
            ET.fromstring(second.text)


def test_installed_rendering_after_artifact_reopen_uses_exact_identity_fallback():
    model = eqiora.compile(source=SOURCE)
    reopened = eqiora.Model.from_bytes(model.to_bytes())
    assert reopened.digest == model.digest
    assert {ref.graph_id for ref in model.render_equations("law")[0].references} == {
        label.graph_id for label in reopened.notation_labels()
    }
    # Source selectors are not recreated from presentation text on bare replay.
    with pytest.raises(ValueError, match="exact Relation"):
        reopened.render_equations("law")


def test_installed_unit_bearing_scalar_constants_do_not_lose_dimensions():
    meters = eqiora.compile(source="model M(){variable x @{x}:m;relation law{x=2[m];}}")
    seconds = eqiora.compile(source="model M(){variable x @{x}:s;relation law{x=2[s];}}")
    for profile in PROFILES:
        first = meters.render_equations("law", profile)[0]
        second = seconds.render_equations("law", profile)[0]
        assert first.text != second.text
        assert "dimensions m" in first.speech
        assert "dimensions s" in second.speech
        if profile == "mathml":
            ET.fromstring(first.text)
            ET.fromstring(second.text)
