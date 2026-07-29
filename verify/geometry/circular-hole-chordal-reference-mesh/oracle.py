#!/usr/bin/env python3
r"""Independent pre-implementation oracle for the chordal circular-hole reference mesh.

Owner lane: non-implementing Opus 5 (A2). This file is the frozen evidence
reference for `geometry.circular-hole-chordal-reference-mesh`. It is authored
without reading any implementation source and must not be tuned by the
implementing lane: an implementer that believes a value here is wrong returns
the proof rather than adjusting the value.

It consolidates three mutually independent routes and requires them to agree:

  R1  coordinate route  -- build the inscribed regular n-gon from vertex
      coordinates only and *measure* every quantity (both directed Hausdorff
      distances by search over the boundaries, perimeter by chord summation,
      area by the shoelace sum). No closed form is used as an input.
  R2  closed-form route -- the frozen sagitta / area-deficit / perimeter-deficit
      expressions of the issue, evaluated at 80 decimal digits with a
      stdlib-only ``decimal`` transcendental kernel.
  R3  identity route    -- the algebraic identities the frozen rule depends on
      (half-angle sagitta form, the ``acos(1-x) = 2 asin(sqrt(x/2))`` half-angle
      inverse, chord length, shoelace triangle term), each verified as a
      residual against the coordinate construction.

Everything else is an *active* check against the frozen DFG witness: the
n=49/n=50 high-precision values, the derived binary64 evaluation allowance, the
n=50 selection, the 104/104 reference topology (derived from ray casting plus
the Euler characteristic of the annulus, not asserted), the n=8/16/32/64
second-order convergence, and the stable-asin versus naive-acos deep
cancellation falsifier.

Stdlib only. Deterministic. Prints ``key=value`` lines for a future Rust test
and exits nonzero on any mismatch.

Run:  python3 verify/geometry/circular-hole-chordal-reference-mesh/oracle.py
"""

from __future__ import annotations

import math
import sys
from decimal import Decimal, getcontext

# --------------------------------------------------------------------------
# high-precision kernel (stdlib ``decimal`` only)
# --------------------------------------------------------------------------

PREC = 80
getcontext().prec = PREC

D = Decimal


def _pi() -> Decimal:
    """Gauss/Machin-free series for pi (CPython ``decimal`` documentation recipe)."""
    getcontext().prec += 10
    three = D(3)
    lasts, t, s, n, na, d, da = 0, three, 3, 1, 0, 0, 24
    while s != lasts:
        lasts = s
        n, na = n + na, na + 8
        d, da = d + da, da + 32
        t = (t * n) / d
        s += t
    getcontext().prec -= 10
    return +s


def dcos(x: Decimal) -> Decimal:
    getcontext().prec += 10
    i, lasts, s, fact, num, sign = 0, 0, D(1), 1, D(1), 1
    while s != lasts:
        lasts = s
        i += 2
        fact *= i * (i - 1)
        num *= x * x
        sign *= -1
        s += num / fact * sign
    getcontext().prec -= 10
    return +s


def dsin(x: Decimal) -> Decimal:
    getcontext().prec += 10
    i, lasts, s, fact, num, sign = 1, 0, D(x), 1, D(x), 1
    while s != lasts:
        lasts = s
        i += 2
        fact *= i * (i - 1)
        num *= x * x
        sign *= -1
        s += num / fact * sign
    getcontext().prec -= 10
    return +s


def dasin(x: Decimal) -> Decimal:
    """Newton on ``sin(t) - x``; used only for |x| well inside the unit interval."""
    t = D(repr(math.asin(float(x))))
    for _ in range(6):
        t = t - (dsin(t) - x) / dcos(t)
    return +t


PI = _pi()
TWO_PI = 2 * PI


def sig(x: Decimal, digits: int = 50) -> str:
    return f"{+x:.{digits - 1}e}"


# --------------------------------------------------------------------------
# R2 -- closed forms (issue "Frozen circular-boundary rule")
# --------------------------------------------------------------------------


def cf_sagitta(r: Decimal, n: int) -> Decimal:
    s = dsin(PI / (2 * n))
    return 2 * r * s * s


def cf_area_deficit(r: Decimal, n: int) -> Decimal:
    return PI * r * r - D(n) / 2 * r * r * dsin(TWO_PI / n)


def cf_perimeter_deficit(r: Decimal, n: int) -> Decimal:
    return 2 * PI * r - 2 * n * r * dsin(PI / n)


# --------------------------------------------------------------------------
# R1 -- coordinate route (measurement only)
# --------------------------------------------------------------------------


def vertices(r: Decimal, n: int) -> list[tuple[Decimal, Decimal]]:
    return [(r * dcos(TWO_PI * k / n), r * dsin(TWO_PI * k / n)) for k in range(n)]


def edges(v):
    return [(v[k], v[(k + 1) % len(v)]) for k in range(len(v))]


def pt_seg_dist(p, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    l2 = dx * dx + dy * dy
    t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2
    t = D(0) if t < 0 else (D(1) if t > 1 else t)
    qx, qy = a[0] + t * dx, a[1] + t * dy
    return ((p[0] - qx) ** 2 + (p[1] - qy) ** 2).sqrt()


def maximize(f, lo, hi, grid=48, refines=5, tern=80):
    """Grid scan, bracket, then ternary refine. Global unimodality not assumed."""
    lo, hi = D(lo), D(hi)
    for _ in range(refines):
        xs = [lo + (hi - lo) * i / grid for i in range(grid + 1)]
        vals = [f(x) for x in xs]
        i = max(range(len(xs)), key=lambda j: vals[j])
        lo, hi = xs[max(i - 1, 0)], xs[min(i + 1, grid)]
    for _ in range(tern):
        m1 = lo + (hi - lo) / 3
        m2 = hi - (hi - lo) / 3
        if f(m1) < f(m2):
            lo = m1
        else:
            hi = m2
    x = (lo + hi) / 2
    return x, f(x)


def measured_circle_to_poly(r: Decimal, n: int, e) -> Decimal:
    def f(th):
        p = (r * dcos(th), r * dsin(th))
        return min(pt_seg_dist(p, a, b) for a, b in e)

    best = D(0)
    for s in (0, 1):
        _, v = maximize(f, TWO_PI * s / n, TWO_PI * (s + 1) / n)
        best = max(best, v)
    return best


def measured_poly_to_circle(r: Decimal, n: int, e) -> Decimal:
    best = D(0)
    for a, b in e[:2]:

        def g(t, a=a, b=b):
            x = a[0] + t * (b[0] - a[0])
            y = a[1] + t * (b[1] - a[1])
            return abs(r - (x * x + y * y).sqrt())

        _, v = maximize(g, 0, 1)
        best = max(best, v)
    return best


def measured_perimeter(e) -> Decimal:
    return sum((((b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2).sqrt() for a, b in e), D(0))


def measured_area(v) -> Decimal:
    n = len(v)
    s = D(0)
    for k in range(n):
        x1, y1 = v[k]
        x2, y2 = v[(k + 1) % n]
        s += x1 * y2 - x2 * y1
    return s / 2


# --------------------------------------------------------------------------
# selection rule (frozen): stable half-angle inverse + sagitta correction
# --------------------------------------------------------------------------

MIN_SEGMENTS = 8
HARD_LIMIT = 100_000


def analytic_candidate_hp(r: Decimal, eps: Decimal) -> int:
    """n0 = ceil(pi / (2 asin(sqrt(eps_eff / (2 r))))), clamped to the minimum count.

    The `eps >= 2r` branch is taken *before* evaluating the inverse, exactly as
    the frozen rule requires.
    """
    if eps >= 2 * r:
        return MIN_SEGMENTS
    a = dasin((eps / (2 * r)).sqrt())
    q = PI / (2 * a)
    n0 = int(q.to_integral_value(rounding="ROUND_CEILING"))
    return max(n0, MIN_SEGMENTS)


def required_count_hp(r: Decimal, eps: Decimal) -> int:
    """Analytic candidate then direct stable-sagitta predicate correction."""
    n = analytic_candidate_hp(r, eps)
    while n > MIN_SEGMENTS and cf_sagitta(r, n - 1) <= eps:
        n -= 1
    while cf_sagitta(r, n) > eps:
        n += 1
    return n


def f64_stable_candidate(r: float, eps: float) -> int:
    if eps >= 2.0 * r:
        return MIN_SEGMENTS
    return max(
        MIN_SEGMENTS, math.ceil(math.pi / (2.0 * math.asin(math.sqrt(eps / (2.0 * r)))))
    )


def f64_naive_candidate(r: float, eps: float):
    """The forbidden route: acos(1 - eps/r). Returns None where it breaks down."""
    if eps >= 2.0 * r:
        return MIN_SEGMENTS
    x = 1.0 - eps / r
    if not -1.0 <= x < 1.0:
        return None  # 1 - eps/r rounded to 1.0: acos == 0, count is undefined
    return max(MIN_SEGMENTS, math.ceil(math.pi / math.acos(x)))


# --------------------------------------------------------------------------
# frozen DFG witness
# --------------------------------------------------------------------------

BOUNDS = ((0.0, 2.2), (0.0, 0.41))
CENTRE = (0.2, 0.2)
RADIUS = 0.05
TOLERANCE = 1e-12
MAX_ERROR = 1e-4
MIN_QUALITY = 1e-5
MAX_SEGMENTS = 50

R = D("0.05")

FROZEN = {
    "sagitta_n49": D("0.00010273036248318289955797595210037224856637053318839"),
    "sagitta_n50": D("0.000098663578586421902383159656827472333154739014922844"),
    "area_deficit_n50": D("0.000020654536205467760336685969666957589060533063430286"),
    "perimeter_deficit_n50": D(
        "0.00020666771241244346537321549979462280729278040417922"
    ),
}
FROZEN_ALLOWANCE_M = 6.252776074688882e-14
FROZEN_AREA_ALLOWANCE_M2 = 1.9643675380784617e-14

# --------------------------------------------------------------------------
# harness
# --------------------------------------------------------------------------

OUT: list[str] = []
FAILURES: list[str] = []


def emit(key: str, value) -> None:
    OUT.append(f"{key}={value}")


def check(key: str, ok: bool, detail: str = "") -> bool:
    emit(f"check.{key}", "pass" if ok else "FAIL")
    if not ok:
        FAILURES.append(f"{key}: {detail}" if detail else key)
    return ok


def rel(a: Decimal, b: Decimal) -> Decimal:
    return abs(a - b) / abs(b)


# --------------------------------------------------------------------------
# 0. inputs
# --------------------------------------------------------------------------

emit("oracle.slice", "geometry.circular-hole-chordal-reference-mesh")
emit("oracle.route", "coordinate+closed-form+identity")
emit("oracle.decimal_digits", PREC)
emit("dfg.bounds_x", f"{BOUNDS[0][0]},{BOUNDS[0][1]}")
emit("dfg.bounds_y", f"{BOUNDS[1][0]},{BOUNDS[1][1]}")
emit("dfg.centre", f"{CENTRE[0]},{CENTRE[1]}")
emit("dfg.radius_m", repr(RADIUS))
emit("dfg.tolerance_m", repr(TOLERANCE))
emit("dfg.max_error_m", repr(MAX_ERROR))
emit("dfg.min_quality", repr(MIN_QUALITY))
emit("dfg.max_segments", MAX_SEGMENTS)

# --------------------------------------------------------------------------
# 1. derived binary64 evaluation allowance
# --------------------------------------------------------------------------

F64_EPS = sys.float_info.epsilon
scale_m = max(
    abs(BOUNDS[0][0]),
    abs(BOUNDS[0][1]),
    abs(BOUNDS[1][0]),
    abs(BOUNDS[1][1]),
    abs(CENTRE[0]),
    abs(CENTRE[1]),
    RADIUS,
    sys.float_info.min,
)
allowance_m = 128.0 * F64_EPS * scale_m
area_allowance_m2 = 2.0 * math.pi * RADIUS * allowance_m
eps_eff = MAX_ERROR - allowance_m

emit("allowance.scale_m", repr(scale_m))
emit("allowance.f64_epsilon", repr(F64_EPS))
emit("allowance.evaluation_m", repr(allowance_m))
emit("allowance.area_m2", repr(area_allowance_m2))
emit("allowance.epsilon_effective_m", repr(eps_eff))

check(
    "allowance_matches_frozen",
    allowance_m == FROZEN_ALLOWANCE_M,
    f"{allowance_m!r} != {FROZEN_ALLOWANCE_M!r}",
)
check(
    "area_allowance_matches_frozen",
    area_allowance_m2 == FROZEN_AREA_ALLOWANCE_M2,
    f"{area_allowance_m2!r} != {FROZEN_AREA_ALLOWANCE_M2!r}",
)
check("allowance_scale_is_max_bound", scale_m == 2.2)
check(
    "requested_error_exceeds_allowance",
    MAX_ERROR > allowance_m and math.isfinite(eps_eff) and eps_eff > 0.0,
)

EPS_EFF = D(repr(eps_eff))

# --------------------------------------------------------------------------
# 2. frozen high-precision ideal values (R2 vs the issue text)
# --------------------------------------------------------------------------

computed = {
    "sagitta_n49": cf_sagitta(R, 49),
    "sagitta_n50": cf_sagitta(R, 50),
    "area_deficit_n50": cf_area_deficit(R, 50),
    "perimeter_deficit_n50": cf_perimeter_deficit(R, 50),
}

for name, value in computed.items():
    emit(f"ideal.{name}", sig(value, 50))
    emit(f"ideal.{name}.f64", repr(float(value)))
    r_err = rel(value, FROZEN[name])
    emit(f"ideal.{name}.rel_residual", sig(r_err, 3))
    check(f"frozen_{name}", r_err < D("1e-46"), f"rel residual {sig(r_err, 6)}")

check(
    "deficits_positive",
    all(v > 0 for v in computed.values()),
)
check("sagitta_monotone_n49_gt_n50", computed["sagitta_n49"] > computed["sagitta_n50"])

# --------------------------------------------------------------------------
# 3. n = 50 selection, and the 49-segment falsifier
# --------------------------------------------------------------------------

n0_hp = analytic_candidate_hp(R, EPS_EFF)
n_req_hp = required_count_hp(R, EPS_EFF)
n0_f64 = f64_stable_candidate(RADIUS, eps_eff)

emit("select.analytic_candidate_n0", n0_hp)
emit("select.required_n", n_req_hp)
emit("select.analytic_candidate_n0_f64_stable", n0_f64)
emit("select.accepted_n", n_req_hp)

check("selection_n0_is_50", n0_hp == 50, f"n0={n0_hp}")
check("selection_required_is_50", n_req_hp == 50, f"n={n_req_hp}")
check("selection_f64_stable_agrees", n0_f64 == n0_hp, f"{n0_f64} != {n0_hp}")
check("selection_never_below_analytic", n_req_hp >= n0_hp)
check("selection_within_max_segments", n_req_hp <= MAX_SEGMENTS)
check("selection_at_least_min_topology", n_req_hp >= MIN_SEGMENTS)

check(
    "falsifier_n49_insufficient",
    computed["sagitta_n49"] > EPS_EFF and float(computed["sagitta_n49"]) > MAX_ERROR,
    "sagitta(49) must exceed both eps_eff and the raw 1e-4 request",
)
check("selection_n50_sufficient", computed["sagitta_n50"] <= EPS_EFF)
check(
    "selection_is_minimal",
    required_count_hp(R, EPS_EFF) == 50 and cf_sagitta(R, 49) > EPS_EFF,
)

# accepted bound = measured boundary maximum + allowance, must not exceed request
accepted_bound = float(computed["sagitta_n50"]) + allowance_m
emit("select.ideal_accepted_bound_m", repr(accepted_bound))
check("accepted_bound_within_request", accepted_bound <= MAX_ERROR)

# --------------------------------------------------------------------------
# 3b. admission predicate: the reference path either yields a count or rejects
# --------------------------------------------------------------------------


def admit(requested_m: float, max_segments: int = MAX_SEGMENTS):
    """Return (accepted_count, None) or (None, rejection reason).

    Encodes exactly the frozen approximation policy: finite positive request,
    strictly above the derived evaluation allowance, the `eps >= 2r` branch taken
    before the inverse, the minimum topology count, the caller's `max_segments`,
    and the private hard work limit -- all before any topology is allocated.
    """
    if not math.isfinite(requested_m) or requested_m <= 0.0:
        return None, "request not finite and positive"
    if max_segments < MIN_SEGMENTS:
        return None, "max_segments below minimum topology count"
    if max_segments > HARD_LIMIT:
        return None, "max_segments above hard work limit"
    if not requested_m > allowance_m:
        return None, "request not strictly above evaluation allowance"
    eff = requested_m - allowance_m
    if not math.isfinite(eff) or eff <= 0.0:
        return None, "effective error not finite and positive"
    candidate = analytic_candidate_hp(R, D(repr(eff)))
    if candidate > HARD_LIMIT:
        return None, "analytic count above hard work limit"
    n = required_count_hp(R, D(repr(eff)))
    if n > HARD_LIMIT:
        return None, "corrected count above hard work limit"
    if n > max_segments:
        return None, "corrected count above max_segments"
    return n, None


dfg_n, dfg_why = admit(MAX_ERROR, MAX_SEGMENTS)
emit("admit.dfg", dfg_n if dfg_n is not None else f"reject:{dfg_why}")
check("admit_dfg_accepts_50", dfg_n == 50, f"{dfg_n} / {dfg_why}")

REJECTIONS = (
    ("max_segments_49", MAX_ERROR, 49),
    ("max_segments_7", MAX_ERROR, 7),
    ("max_segments_above_hard_limit", MAX_ERROR, HARD_LIMIT + 1),
    ("error_nan", float("nan"), MAX_SEGMENTS),
    ("error_inf", float("inf"), MAX_SEGMENTS),
    ("error_zero", 0.0, MAX_SEGMENTS),
    ("error_negative", -1e-4, MAX_SEGMENTS),
    ("error_equal_to_allowance", allowance_m, MAX_SEGMENTS),
    ("error_below_allowance", math.nextafter(allowance_m, 0.0), MAX_SEGMENTS),
    (
        "error_above_allowance_by_one_ulp",
        math.nextafter(allowance_m, math.inf),
        HARD_LIMIT,
    ),
)
for label, req, cap in REJECTIONS:
    n_r, why = admit(req, cap)
    emit(f"admit.{label}", n_r if n_r is not None else f"reject:{why}")
    check(f"falsifier_{label}_rejects", n_r is None, f"accepted {n_r}")

# the one-ulp-above-allowance case must reject *because of work*, not input shape
one_ulp_eff = math.nextafter(allowance_m, math.inf) - allowance_m
one_ulp_n0 = f64_stable_candidate(RADIUS, one_ulp_eff)
emit("admit.one_ulp_effective_error_m", repr(one_ulp_eff))
emit("admit.one_ulp_analytic_candidate", one_ulp_n0)
emit("admit.hard_work_limit", HARD_LIMIT)
check(
    "falsifier_above_hard_limit_is_work_bound",
    one_ulp_eff > 0.0 and one_ulp_n0 > HARD_LIMIT,
    f"n0={one_ulp_n0}",
)

# epsilon_effective >= 2r branch: n0 is defined as the minimum topology count,
# taken *before* the out-of-domain half-angle inverse would be evaluated
big_request = 0.2
big_eff = big_request - allowance_m
big_n, big_why = admit(big_request, MAX_SEGMENTS)
emit("branch.big_request_m", repr(big_request))
emit("branch.big_epsilon_effective_m", repr(big_eff))
emit("branch.big_two_r_m", repr(2.0 * RADIUS))
emit("branch.big_analytic_candidate", analytic_candidate_hp(R, D(repr(big_eff))))
emit("branch.big_accepted_n", big_n if big_n is not None else f"reject:{big_why}")
check(
    "branch_eps_ge_2r_selects_min_topology",
    big_eff >= 2.0 * RADIUS
    and analytic_candidate_hp(R, D(repr(big_eff))) == MIN_SEGMENTS
    and big_n == MIN_SEGMENTS,
    f"n={big_n}",
)
check(
    "branch_eps_ge_2r_argument_would_be_out_of_domain",
    D(repr(big_eff)) / (2 * R) > 1,
    "sqrt argument exceeds 1, so asin must not be reached",
)

# --------------------------------------------------------------------------
# 4. R1 vs R2 vs R3 agreement, and second-order convergence
# --------------------------------------------------------------------------

CONV_N = (8, 16, 32, 64)
meas: dict[int, tuple[Decimal, Decimal, Decimal]] = {}

for n in (*CONV_N, 49, 50):
    v = vertices(R, n)
    e = edges(v)
    d_cp = measured_circle_to_poly(R, n, e)
    d_pc = measured_poly_to_circle(R, n, e)
    haus = max(d_cp, d_pc)
    ad = PI * R * R - measured_area(v)
    pd = 2 * PI * R - measured_perimeter(e)
    meas[n] = (haus, ad, pd)

    emit(f"measured.n{n}.hausdorff_m", sig(haus, 40))
    emit(f"measured.n{n}.area_deficit_m2", sig(ad, 40))
    emit(f"measured.n{n}.perimeter_deficit_m", sig(pd, 40))
    emit(f"measured.n{n}.directed_gap", sig(abs(d_cp - d_pc), 3))

    check(f"route_agree_hausdorff_n{n}", rel(haus, cf_sagitta(R, n)) < D("1e-30"))
    check(f"route_agree_area_n{n}", rel(ad, cf_area_deficit(R, n)) < D("1e-40"))
    check(
        f"route_agree_perimeter_n{n}", rel(pd, cf_perimeter_deficit(R, n)) < D("1e-40")
    )
    # the two directed Hausdorff distances coincide for the inscribed n-gon
    check(f"directed_hausdorff_symmetric_n{n}", abs(d_cp - d_pc) < D("1e-30") * haus)

check(
    "measured_matches_frozen_sagitta_n49",
    rel(meas[49][0], FROZEN["sagitta_n49"]) < D("1e-30"),
)
check(
    "measured_matches_frozen_sagitta_n50",
    rel(meas[50][0], FROZEN["sagitta_n50"]) < D("1e-30"),
)
check(
    "measured_matches_frozen_area_n50",
    rel(meas[50][1], FROZEN["area_deficit_n50"]) < D("1e-40"),
)
check(
    "measured_matches_frozen_perimeter_n50",
    rel(meas[50][2], FROZEN["perimeter_deficit_n50"]) < D("1e-40"),
)

orders: dict[str, list[float]] = {"boundary": [], "area": [], "perimeter": []}
for a, b in zip(CONV_N, CONV_N[1:]):
    for idx, key in enumerate(("boundary", "area", "perimeter")):
        o = math.log2(float(meas[a][idx] / meas[b][idx]))
        orders[key].append(o)
        emit(f"convergence.{key}.order_{a}_{b}", f"{o:.12f}")

for key, seq in orders.items():
    ok_band = all(1.90 < o < 2.00 for o in seq)
    ok_mono = all(x < y for x, y in zip(seq, seq[1:]))
    check(f"convergence_second_order_{key}", ok_band, f"{seq}")
    check(f"convergence_approaches_two_{key}", ok_mono, f"{seq}")

# leading constants confirm the O(n^-2) constant, not merely the exponent
lead = {
    "boundary": PI * PI * R / 2,
    "area": 2 * PI**3 * R * R / 3,
    "perimeter": PI**3 * R / 3,
}
for idx, key in enumerate(("boundary", "area", "perimeter")):
    ratio = meas[64][idx] * 64 * 64 / lead[key]
    emit(f"convergence.{key}.leading_constant", sig(lead[key], 20))
    emit(f"convergence.{key}.n64_scaled_ratio", sig(ratio, 12))
    check(
        f"convergence_leading_constant_{key}",
        abs(ratio - 1) < D("0.005"),
        sig(ratio, 8),
    )

# --------------------------------------------------------------------------
# 5. R3 -- identities the frozen rule depends on
# --------------------------------------------------------------------------

ident_max = D(0)
for n in (8, 16, 32, 49, 50, 64):
    v = vertices(R, n)
    # sagitta half-angle form vs the direct 1 - cos form
    ident_max = max(ident_max, abs(cf_sagitta(R, n) - R * (1 - dcos(PI / n))))
    # chord length from coordinates vs 2 r sin(pi/n)
    chord = ((v[1][0] - v[0][0]) ** 2 + (v[1][1] - v[0][1]) ** 2).sqrt()
    ident_max = max(ident_max, abs(chord - 2 * R * dsin(PI / n)))
    # shoelace triangle term vs r^2 sin(2 pi / n) / 2
    tri = (v[0][0] * v[1][1] - v[1][0] * v[0][1]) / 2
    ident_max = max(ident_max, abs(tri - R * R * dsin(TWO_PI / n) / 2))

emit("identity.max_residual", sig(ident_max, 3))
check("identity_closed_forms", ident_max < D("1e-60"), sig(ident_max, 6))

# half-angle inverse: t = 2 asin(sqrt(x/2)) satisfies 1 - cos(t) = x exactly
halfangle_max = D(0)
for xs in ("0.5", "1e-3", "1e-9", "1e-18", "2e-12"):
    x = D(xs)
    t = 2 * dasin((x / 2).sqrt())
    halfangle_max = max(halfangle_max, abs((1 - dcos(t)) - x) / x)
emit("identity.half_angle_inverse_max_rel_residual", sig(halfangle_max, 3))
check("identity_half_angle_inverse", halfangle_max < D("1e-60"), sig(halfangle_max, 6))

# --------------------------------------------------------------------------
# 6. stable-asin vs naive-acos deep cancellation falsifier (no mesh allocated)
# --------------------------------------------------------------------------

# Primary falsifier: eps/r underflows below one ulp of 1.0, so fl(1 - eps/r) == 1.0
# and acos returns exactly 0. The stable half-angle inverse stays well defined.
FALSIFIER_EPS = 1e-18
x_naive = 1.0 - FALSIFIER_EPS / RADIUS
naive_primary = f64_naive_candidate(RADIUS, FALSIFIER_EPS)
stable_primary = f64_stable_candidate(RADIUS, FALSIFIER_EPS)
exact_primary = analytic_candidate_hp(R, D(repr(FALSIFIER_EPS)))

emit("falsifier.eps_m", repr(FALSIFIER_EPS))
emit("falsifier.naive_acos_argument", repr(x_naive))
emit("falsifier.naive_acos_value", repr(math.acos(x_naive)))
emit(
    "falsifier.naive_candidate", "undefined" if naive_primary is None else naive_primary
)
emit("falsifier.stable_candidate", stable_primary)
emit("falsifier.exact_candidate", exact_primary)

check(
    "falsifier_naive_acos_cancels_to_unity",
    x_naive == 1.0 and math.acos(x_naive) == 0.0,
    f"1 - eps/r = {x_naive!r}",
)
check("falsifier_naive_acos_undefined", naive_primary is None)
check(
    "falsifier_stable_asin_survives",
    stable_primary == exact_primary and exact_primary > HARD_LIMIT,
    f"stable={stable_primary} exact={exact_primary}",
)

# Supporting scan: even where the naive route returns a number it is wrong.
worst_naive = 0
worst_naive_eps = None
worst_stable = 0
for k in range(1, 61):
    e_scan = 1e-8 * 10.0 ** (-k / 6.0)
    ex = analytic_candidate_hp(R, D(repr(e_scan)))
    st = f64_stable_candidate(RADIUS, e_scan)
    nv = f64_naive_candidate(RADIUS, e_scan)
    worst_stable = max(worst_stable, abs(st - ex))
    if nv is None:
        continue
    if abs(nv - ex) > worst_naive:
        worst_naive, worst_naive_eps = abs(nv - ex), e_scan

emit("falsifier.scan_points", 60)
emit("falsifier.scan_worst_naive_delta", worst_naive)
emit("falsifier.scan_worst_naive_eps_m", repr(worst_naive_eps))
emit("falsifier.scan_worst_stable_delta", worst_stable)
check("falsifier_scan_naive_disagrees", worst_naive >= 1, f"delta={worst_naive}")
check("falsifier_scan_stable_exact", worst_stable == 0, f"delta={worst_stable}")

# --------------------------------------------------------------------------
# 7. binary64 loop: measured boundary quantity, deficits, simplicity
# --------------------------------------------------------------------------

N = 50
cx, cy = CENTRE
fv = [
    (
        cx + RADIUS * math.cos(2.0 * math.pi * i / N),
        cy + RADIUS * math.sin(2.0 * math.pi * i / N),
    )
    for i in range(N)
]
loc = [(x - cx, y - cy) for x, y in fv]


def f64_pt_seg_dist(a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    l2 = dx * dx + dy * dy
    t = -(a[0] * dx + a[1] * dy) / l2
    t = 0.0 if t < 0.0 else (1.0 if t > 1.0 else t)
    return math.hypot(a[0] + t * dx, a[1] + t * dy)


d_min = min(f64_pt_seg_dist(loc[i], loc[(i + 1) % N]) for i in range(N))
r_max = max(math.hypot(x, y) for x, y in loc)
boundary_measured = max(RADIUS - d_min, r_max - RADIUS)

per_f64 = sum(
    math.hypot(loc[(i + 1) % N][0] - loc[i][0], loc[(i + 1) % N][1] - loc[i][1])
    for i in range(N)
)
area_f64 = 0.5 * sum(
    loc[i][0] * loc[(i + 1) % N][1] - loc[(i + 1) % N][0] * loc[i][1] for i in range(N)
)
pd_f64 = 2.0 * math.pi * RADIUS - per_f64
ad_f64 = math.pi * RADIUS * RADIUS - area_f64

crosses = [
    (loc[(i + 1) % N][0] - loc[i][0]) * (loc[(i + 2) % N][1] - loc[(i + 1) % N][1])
    - (loc[(i + 1) % N][1] - loc[i][1]) * (loc[(i + 2) % N][0] - loc[(i + 1) % N][0])
    for i in range(N)
]

emit("f64.d_min_m", repr(d_min))
emit("f64.r_max_m", repr(r_max))
emit("f64.boundary_measured_m", repr(boundary_measured))
emit("f64.perimeter_deficit_m", repr(pd_f64))
emit("f64.area_deficit_m2", repr(ad_f64))
emit("f64.accepted_bound_m", repr(boundary_measured + allowance_m))
emit(
    "f64.boundary_residual_m",
    repr(abs(boundary_measured - float(computed["sagitta_n50"]))),
)
emit(
    "f64.perimeter_residual_m",
    repr(abs(pd_f64 - float(computed["perimeter_deficit_n50"]))),
)
emit("f64.area_residual_m2", repr(abs(ad_f64 - float(computed["area_deficit_n50"]))))

check(
    "f64_boundary_within_allowance",
    abs(boundary_measured - float(computed["sagitta_n50"])) <= allowance_m,
)
check(
    "f64_perimeter_within_allowance",
    abs(pd_f64 - float(computed["perimeter_deficit_n50"])) <= allowance_m,
)
check(
    "f64_area_within_area_allowance",
    abs(ad_f64 - float(computed["area_deficit_n50"])) <= area_allowance_m2,
)
check("f64_accepted_bound_within_request", boundary_measured + allowance_m <= MAX_ERROR)
check("f64_loop_convex_simple", all(c > 0.0 for c in crosses))
check("f64_centre_strictly_interior", 0.0 < d_min <= r_max)
# vertices sit on the circle, so the outward excursion is pure rounding while the
# inward one is the sagitta: max(r - d_min, R_max - r) is dominated by r - d_min
check(
    "f64_vertices_on_circle_within_allowance",
    abs(r_max - RADIUS) <= allowance_m,
    repr(r_max - RADIUS),
)
check(
    "f64_boundary_maximum_is_inward",
    RADIUS - d_min > r_max - RADIUS and boundary_measured == RADIUS - d_min,
)

# --------------------------------------------------------------------------
# 8. frozen reference topology, derived (not asserted)
# --------------------------------------------------------------------------

(x_lo, x_hi), (y_lo, y_hi) = BOUNDS
CORNERS = ((x_lo, y_lo), (x_hi, y_lo), (x_hi, y_hi), (x_lo, y_hi))
SIDES = ("x_low", "x_high", "y_low", "y_high")
SIDE_PLANE = (x_lo, x_hi, y_lo, y_hi)


def cast_ray(theta: float):
    """Centre ray to the rectangle; returns (hit point, side index).

    The hit lies on the plane of the side it hits *by definition*, so the
    cast-axis coordinate is the frozen bound itself and must not be recovered
    from the parametric round trip ``c + ((plane - c) / d) * d``. That round
    trip rounds twice and lands up to one ulp of the centre off the plane,
    which would defeat the exact incidence test in ``sides_of``. Only the
    transverse coordinate is evaluated parametrically.
    """
    dx, dy = math.cos(theta), math.sin(theta)
    best_t, best_side = math.inf, -1
    for side, (num, den) in enumerate(
        ((x_lo - cx, dx), (x_hi - cx, dx), (y_lo - cy, dy), (y_hi - cy, dy))
    ):
        if den == 0.0:
            continue
        t = num / den
        if t > 0.0 and t < best_t:
            best_t, best_side = t, side
    if best_side < 0:
        raise AssertionError(f"centre ray at {theta!r} escaped the rectangle")
    plane = SIDE_PLANE[best_side]
    if best_side < 2:
        return (plane, cy + best_t * dy), best_side
    return (cx + best_t * dx, plane), best_side


def ang(p) -> float:
    return math.atan2(p[1] - cy, p[0] - cx) % (2.0 * math.pi)


ray_angles = [2.0 * math.pi * i / N for i in range(N)]
hits = [cast_ray(t) for t in ray_angles]
outer_pts = [h[0] for h in hits]
side_of = [h[1] for h in hits]

corner_gap = min(
    math.hypot(p[0] - q[0], p[1] - q[1]) for p in outer_pts for q in CORNERS
)
corner_angles = [ang(q) for q in CORNERS]
angle_gap = min(
    abs((a - b + math.pi) % (2.0 * math.pi) - math.pi)
    for a in ray_angles
    for b in corner_angles
)

emit("topology.n_circle", N)
emit("topology.min_ray_to_corner_m", repr(corner_gap))
emit("topology.min_ray_to_corner_angle_rad", repr(angle_gap))
check(
    "topology_no_corner_reuse_for_dfg",
    corner_gap > TOLERANCE,
    "no DFG ray falls within the source tolerance of a corner",
)

# each corner sits strictly inside a distinct adjacent-ray sector
sector_of_corner = [int(a // (2.0 * math.pi / N)) for a in corner_angles]
check("topology_corners_in_distinct_sectors", len(set(sector_of_corner)) == 4)

outer_ring = sorted(
    [(ang(p), p, side_of[i]) for i, p in enumerate(outer_pts)]
    + [(ang(q), q, None) for q in CORNERS]
)
n_outer = len(outer_ring)


def sides_of(entry):
    """Exact coordinate-side membership of an outer-ring vertex.

    Bit equality is the right predicate only because every ring vertex is
    *constructed* on its side plane: a ray hit carries the frozen bound on its
    cast axis, and a corner is a pair of frozen bounds. No tolerance is used,
    so a vertex that is merely near a side is not counted as on it.
    """
    p = entry[1]
    s = set()
    if p[0] == x_lo:
        s.add(0)
    if p[0] == x_hi:
        s.add(1)
    if p[1] == y_lo:
        s.add(2)
    if p[1] == y_hi:
        s.add(3)
    return s


# Totality of the classification: every ray hit lies on exactly one side and
# every corner on exactly two. This is what makes the edge partition below
# exhaustive, and it is asserted rather than assumed -- a vertex that misses
# its own side plane silently withholds *both* of its incident ring edges.
membership = [len(sides_of(e)) for e in outer_ring]
membership_expected = [2 if e[2] is None else 1 for e in outer_ring]

# edges per rectangle side: a boundary edge belongs to the side both endpoints share
side_edges = [0, 0, 0, 0]
for i in range(n_outer):
    shared = sides_of(outer_ring[i]) & sides_of(outer_ring[(i + 1) % n_outer])
    if len(shared) == 1:
        side_edges[shared.pop()] += 1

# Independent route to the same partition: on each side the ring visits the
# start corner, the k hits on that side in angular order, then the end corner,
# so the side carries exactly k + 1 edges, and the four sides carry
# sum(k) + 4 = N + 4 = n_outer edges in total.
side_hits = [side_of.count(s) for s in range(4)]
side_edges_rule = [k + 1 for k in side_hits]

n_vertices = N + n_outer
n_boundary_edges = N + n_outer
# The chordal region is an annulus (chi = 0) triangulated with no interior
# vertices, so 3F = 2*E_interior + E_boundary, i.e. E = (3F + E_boundary) / 2.
# Substituting into V - E + F = 0 gives F = 2V - E_boundary. The face count is
# therefore derived from the ray cast, not asserted.
n_faces = 2 * n_vertices - n_boundary_edges
n_interior_edges = (3 * n_faces - n_boundary_edges) // 2
n_edges = n_interior_edges + n_boundary_edges
chi = n_vertices - n_edges + n_faces

# independent count from the construction rule: 2 triangles per adjacent ray pair
# plus one fan triangle per crossed rectangle corner
n_faces_rule = 2 * N + len(CORNERS)

emit("topology.outer_loop_vertices", n_outer)
emit("topology.vertices", n_vertices)
emit("topology.boundary_edges", n_boundary_edges)
emit("topology.interior_edges", n_interior_edges)
emit("topology.edges", n_edges)
emit("topology.triangles", n_faces)
emit("topology.triangles_from_construction_rule", n_faces_rule)
emit("topology.euler_characteristic", chi)
emit("topology.set_dim1_0_x_low_edges", side_edges[0])
emit("topology.set_dim1_1_x_high_edges", side_edges[1])
emit("topology.set_dim1_2_y_low_edges", side_edges[2])
emit("topology.set_dim1_3_y_high_edges", side_edges[3])
emit("topology.set_dim1_4_circular_edges", N)
emit("topology.set_dim0_corner_vertices", len(CORNERS))
emit("topology.set_dim2_face_triangles", n_faces)
emit("topology.side_ray_hits", ",".join(str(k) for k in side_hits))
emit("topology.side_edges_from_hit_rule", ",".join(str(k) for k in side_edges_rule))
emit(
    "topology.side_membership_anomalies",
    sum(1 for m, x in zip(membership, membership_expected) if m != x),
)

check("topology_outer_loop_54", n_outer == 54, f"{n_outer}")
check("topology_vertices_104", n_vertices == 104, f"{n_vertices}")
check("topology_triangles_104", n_faces == 104, f"{n_faces}")
check("topology_routes_agree", n_faces == n_faces_rule, f"{n_faces} != {n_faces_rule}")
check("topology_annulus_euler_zero", chi == 0, f"chi={chi}")
check(
    "topology_side_membership_exact",
    membership == membership_expected,
    f"anomalous ring vertices: "
    f"{[(i, m) for i, (m, x) in enumerate(zip(membership, membership_expected)) if m != x]}",
)
check(
    "topology_side_edges_partition_outer", sum(side_edges) == n_outer, f"{side_edges}"
)
check(
    "topology_side_edges_match_hit_rule",
    side_edges == side_edges_rule,
    f"{side_edges} != {side_edges_rule}",
)
check(
    "topology_every_side_carries_edges", all(s > 0 for s in side_edges), f"{side_edges}"
)
check("topology_circular_edges_equal_n", N == n_req_hp)
check(
    "topology_all_four_sides_hit",
    len(set(side_of)) == 4,
    f"sides hit: {sorted(set(SIDES[s] for s in side_of))}",
)

# --------------------------------------------------------------------------
# report
# --------------------------------------------------------------------------

emit("oracle.checks_total", sum(1 for line in OUT if line.startswith("check.")))
emit("oracle.checks_failed", len(FAILURES))
emit("oracle.result", "pass" if not FAILURES else "fail")

print("\n".join(OUT))
if FAILURES:
    print("\n".join(f"# FAILURE {f}" for f in FAILURES), file=sys.stderr)
    sys.exit(1)
