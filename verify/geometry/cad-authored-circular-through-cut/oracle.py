"""Independent oracle for the bounded rectangle-extrusion circular through-cut.

Standard library only, with no Eqiora implementation import. Expected values
were frozen before implementation. Edge and vertex counts remain unclaimed
because seam splitting is provider-specific.
"""

import hashlib
import json
from decimal import ROUND_HALF_EVEN, Context, Decimal, getcontext

getcontext().prec = 96
C30 = Context(prec=30, rounding=ROUND_HALF_EVEN)

WIRE = (
    b'{"schema":"eqiora.cad-authored-operation-graph-envelope/v2",'
    b'"encoding":"eqiora.canonical-json/v1","length_unit":"metre",'
    b'"requested_modeling_tolerance_m":1e-10,'
    b'"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.0},'
    b'"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle",'
    b'"sketch_plane":"sketch-plane","constraint":"closed-by-construction",'
    b'"x_bounds_m":[-0.04,0.04],"y_bounds_m":[-0.025,0.025]},'
    b'"face":{"id":"profile-face","kind":"one-closed-loop-face",'
    b'"profile":"rectangle-profile","region_count":1},'
    b'"extrusion":{"id":"positive-z-extrusion","kind":"positive-z",'
    b'"face":"profile-face","depth_m":0.02,"repair":"none"},'
    b'"cut_sketch_plane":{"id":"cut-sketch-plane","kind":"on-face","face":"end-cap"},'
    b'"cut_profile":{"id":"circle-profile","kind":"circle",'
    b'"sketch_plane":"cut-sketch-plane","constraint":"closed-by-construction",'
    b'"center_m":[0.02,0.0],"radius_m":0.008},'
    b'"cut_face":{"id":"cut-profile-face","kind":"one-closed-loop-face",'
    b'"profile":"circle-profile","region_count":1},'
    b'"cut":{"id":"circular-through-cut",'
    b'"kind":"difference-through-all-negative-z","target":"positive-z-extrusion",'
    b'"tool_face":"cut-profile-face","requested_tolerance_m":1e-9,"repair":"none"},'
    b'"selections":["start-cap","end-cap","profile-x-lower","profile-x-upper",'
    b'"profile-y-lower","profile-y-upper","cut-wall"]}'
)
SCHEMA = "eqiora.cad-authored-operation-graph-envelope/v2"
WIRE_SHA256 = "00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47"
GRAPH = json.loads(WIRE.decode("ascii"))
SELECTIONS = GRAPH["selections"]

W, H, DEPTH, R = Decimal("0.08"), Decimal("0.05"), Decimal("0.02"), Decimal("0.008")
X0, X1 = Decimal("-0.04"), Decimal("0.04")
Y0, Y1 = Decimal("-0.025"), Decimal("0.025")
CX, CY = Decimal("0.02"), Decimal("0")
MODEL_TOL, CUT_TOL, REL = Decimal("1e-10"), Decimal("1e-9"), Decimal("1e-29")

REFS = {
    "volume_m3": Decimal("7.59787614034050646547678164694E-5"),
    "cap_area_m2": Decimal("3.79893807017025323273839082347E-3"),
    "x_side_area_m2": Decimal("1.00000000000000000000000000000E-3"),
    "y_side_area_m2": Decimal("1.60000000000000000000000000000E-3"),
    "cut_wall_area_m2": Decimal("1.00530964914873383630804588265E-3"),
    "seven_face_area_m2": Decimal("1.38031857894892403017848275296E-2"),
}
TOPOLOGY = {"bodies": 1, "shells": 1, "faces": 7, "genus": 1}
CYCLES = {
    "start-cap": 2,
    "end-cap": 2,
    "profile-x-lower": 1,
    "profile-x-upper": 1,
    "profile-y-lower": 1,
    "profile-y-upper": 1,
    "cut-wall": 2,
}
LINEAGE = {
    "created": ["cut-wall"],
    "deleted": [],
    "merged": [],
    "retained_modified": ["end-cap", "start-cap"],
    "retained_unchanged": [
        "profile-x-lower",
        "profile-x-upper",
        "profile-y-lower",
        "profile-y-upper",
    ],
    "split": [],
}

TOL_CASES = (
    ("main", Decimal("1e-10"), Decimal("1e-9"), Decimal("1e-9")),
    ("discriminator", Decimal("1e-10"), Decimal("1e-11"), Decimal("1e-11")),
)
TOL_MUTANTS = {"min_clamp": min, "max_clamp": max, "base_clamp": lambda b, r: b}

RECT = (X0, X1, Y0, Y1)
TINY = (Decimal("0"), Decimal("4e-9"), Decimal("0"), Decimal("4e-9"))
ADMISSION_CASES = (
    ("main-through-cut", RECT, "0.02", "0", "0.008", "1e-9", "0.012", True),
    ("centre-outside-profile", RECT, "0.10", "0", "0.008", "1e-9", "-0.068", False),
    ("asymmetric-overhang", RECT, "0.0335", "0", "0.008", "1e-9", "-0.0015", False),
    (
        "clearance-equals-tolerance",
        TINY,
        "2e-9",
        "2e-9",
        "1e-9",
        "1e-9",
        "1e-9",
        False,
    ),
    (
        "clearance-above-tolerance",
        TINY,
        "2e-9",
        "2e-9",
        "0.5e-9",
        "1e-9",
        "1.5e-9",
        True,
    ),
)


def machin_pi() -> Decimal:
    """Derive pi from Machin's identity and an alternating arctan series."""

    def arctan_inv(x: int) -> Decimal:
        xsq = Decimal(x) * Decimal(x)
        term = total = Decimal(1) / Decimal(x)
        k = 1
        while True:
            term = -term / xsq
            delta = term / (2 * k + 1)
            if total + delta == total:
                return total
            total += delta
            k += 1

    return 4 * (4 * arctan_inv(5) - arctan_inv(239))


PI = machin_pi()


def effective_boolean_tolerance(modeling_m: Decimal, requested_m: Decimal) -> Decimal:
    """The identity-only modeling tolerance never clamps the Boolean request."""
    del modeling_m
    return requested_m


def inward_clearance(cx: Decimal, cy: Decimal, radius: Decimal, rect: tuple) -> Decimal:
    """Signed inward clearance of the authored circle in its rectangle."""
    x0, x1, y0, y1 = rect
    return min(cx - x0, x1 - cx, cy - y0, y1 - cy) - radius


def derived() -> dict[str, Decimal]:
    cap = W * H - PI * R * R
    x_side, y_side = H * DEPTH, W * DEPTH
    wall = 2 * PI * R * DEPTH
    return {
        "volume_m3": cap * DEPTH,
        "cap_area_m2": cap,
        "x_side_area_m2": x_side,
        "y_side_area_m2": y_side,
        "cut_wall_area_m2": wall,
        "seven_face_area_m2": 2 * cap + 2 * x_side + 2 * y_side + wall,
    }


def fmt(value: Decimal) -> str:
    rounded = C30.plus(value)
    return str(rounded.quantize(Decimal(1).scaleb(rounded.adjusted() - 29)))


def dec(value: Decimal) -> str:
    return str(value.normalize())


def check() -> None:
    assert len(WIRE) == 1292
    digest = hashlib.sha256(SCHEMA.encode("utf-8") + b"\x00" + WIRE).hexdigest()
    assert digest == WIRE_SHA256
    assert GRAPH["schema"] == SCHEMA and GRAPH["length_unit"] == "metre"
    assert [Decimal(str(v)) for v in GRAPH["profile"]["x_bounds_m"]] == [X0, X1]
    assert [Decimal(str(v)) for v in GRAPH["profile"]["y_bounds_m"]] == [Y0, Y1]
    assert [Decimal(str(v)) for v in GRAPH["cut_profile"]["center_m"]] == [CX, CY]
    assert Decimal(str(GRAPH["extrusion"]["depth_m"])) == DEPTH
    assert Decimal(str(GRAPH["cut_profile"]["radius_m"])) == R
    assert Decimal(str(GRAPH["requested_modeling_tolerance_m"])) == MODEL_TOL
    assert Decimal(str(GRAPH["cut"]["requested_tolerance_m"])) == CUT_TOL
    assert str(PI).startswith("3.14159265358979323846264338327950288419716939937510")

    got = derived()
    for key, value in got.items():
        assert abs(value - REFS[key]) <= REL * abs(REFS[key]), key
    assert got["seven_face_area_m2"] == (
        2 * got["cap_area_m2"]
        + 2 * got["x_side_area_m2"]
        + 2 * got["y_side_area_m2"]
        + got["cut_wall_area_m2"]
    )

    for name, base, requested, expected in TOL_CASES:
        assert effective_boolean_tolerance(base, requested) == expected, name
    for name, mutant in TOL_MUTANTS.items():
        assert any(mutant(base, requested) != expected for _, base, requested, expected in TOL_CASES), name

    for name, rect, cx, cy, radius, tolerance, clearance, admitted in ADMISSION_CASES:
        actual = inward_clearance(Decimal(cx), Decimal(cy), Decimal(radius), rect)
        assert actual == Decimal(clearance), name
        assert (actual > Decimal(tolerance)) is admitted, name

    assert TOPOLOGY == {"bodies": 1, "shells": 1, "faces": 7, "genus": 1}
    assert [CYCLES[selection] for selection in SELECTIONS] == [2, 2, 1, 1, 1, 1, 2]
    assert sorted(name for names in LINEAGE.values() for name in names) == sorted(SELECTIONS)
    assert not (LINEAGE["deleted"] or LINEAGE["split"] or LINEAGE["merged"])


def report() -> dict:
    values = derived()
    return {
        "case": "issue-271-rectangle-extrusion-circular-through-cut",
        "schema": SCHEMA,
        "wire_bytes": len(WIRE),
        "wire_sha256": WIRE_SHA256,
        "f64_relative_tolerance": "4e-15",
        "reference_relative_tolerance": dec(REL),
        "derived": {key: fmt(value) for key, value in values.items()},
        "tolerances": [
            {
                "case": name,
                "modeling": dec(base),
                "requested": dec(requested),
                "effective": dec(effective_boolean_tolerance(base, requested)),
            }
            for name, base, requested, _ in TOL_CASES
        ],
        "circle_admission": [
            {
                "case": name,
                "clearance_m": dec(
                    inward_clearance(Decimal(cx), Decimal(cy), Decimal(radius), rect)
                ),
                "tolerance_m": dec(Decimal(tolerance)),
                "admitted": admitted,
            }
            for name, rect, cx, cy, radius, tolerance, _, admitted in ADMISSION_CASES
        ],
        "topology": TOPOLOGY,
        "face_boundary_cycles": CYCLES,
        "lineage": LINEAGE,
    }


def main() -> None:
    check()
    print(json.dumps(report(), sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
