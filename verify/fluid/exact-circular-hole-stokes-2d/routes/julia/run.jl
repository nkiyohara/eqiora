#!/usr/bin/env julia
#
# Julia numerical-oracle route for the Eqiora exact circular-hole steady Stokes 2D slice.
#
#     julia --project=@stdlib run.jl        # or simply: julia run.jl
#
# Deterministic, rerunnable, Julia standard library only. Writes
# `expected/julia-route-frozen.json` and `expected/run-log.txt`, and exits
# nonzero if any internal check fails.
#
# THIS IS ONE INDEPENDENTLY FROZEN ROUTE. No route-to-route comparison is
# performed here and none may be inferred from this output.

using LinearAlgebra, Printf, SHA

const HERE = @__DIR__
include(joinpath(HERE, "src", "oracle.jl"))
using .Oracle
using .Oracle.Geometry, .Oracle.Mini, .Oracle.Witness, .Oracle.Audit, .Oracle.Falsify

BLAS.set_num_threads(1)          # advisory f64 numbers must not depend on threads

const LOG = IOBuffer()
say(s = "") = (println(s); println(LOG, s))

const CHECKS = Tuple{String,Bool,String}[]
check!(name, ok, detail = "") = (push!(CHECKS, (name, ok, string(detail)));
                                 say(@sprintf("  [%s] %-48s %s", ok ? "ok" : "FAIL",
                                              name, string(detail))))
absorb!(rows) = for r in rows
    check!(r[1], r[2], r[3])
end

# ----------------------------------------------------------------- json emit

struct Raw
    s::String
end
hp(x) = @sprintf("%.24e", BigFloat(x))
jv(x::Raw) = x.s
jv(x::AbstractString) = "\"" * escape_string(String(x)) * "\""
jv(x::Bool) = x ? "true" : "false"
jv(x::Integer) = string(x)
jv(x::Float64) = isfinite(x) ? repr(x) : "\"" * string(x) * "\""
jv(x::BigFloat) = jv([("f64", Float64(x)), ("hp", hp(x))])
jv(x::Tuple) = jv(collect(x))
jv(v::AbstractVector) = "[" * join(jv.(v), ",") * "]"
jv(v::AbstractVector{<:Pair}) = jv([(p.first, p.second) for p in v])
jv(v::AbstractVector{<:Tuple{AbstractString,Any}}) =
    "{" * join([jv(k) * ":" * jv(val) for (k, val) in v], ",") * "}"
# Raw keeps a nested object verbatim instead of re-quoting it as a JSON string.
obj(pairs...) = Raw(jv(Tuple{String,Any}[(String(k), v) for (k, v) in pairs]))

# ------------------------------------------------------------------- prelude

setprecision(BigFloat, 256)
const T = BigFloat
const EXACT = T(2)^-200          # BigFloat "exact" threshold for audit identities

say("=" ^ 78)
say("Eqiora exact circular-hole steady Stokes 2D -- Julia numerical-oracle route (independent, non-implementing)")
say("=" ^ 78)
say("julia            : $(VERSION)")
say("BigFloat working precision : $(precision(BigFloat)) bits")
say()
say("ONE INDEPENDENTLY FROZEN ROUTE. Route-to-route comparison NOT performed here.")
say("Implementation of this slice may not begin until the integrator compares this")
say("output with the separately frozen first (Python) route.")
say()

# --------------------------------------------------- 1. method audit (internal)

say("-- 1. internal method audit (not a capability or convergence claim) --------")
absorb!(audit_quadrature(T, EXACT).checks)
absorb!(audit_bubble(T, EXACT).checks)
absorb!(audit_patch(T, EXACT).checks)
say()

# ------------------------------------------------------ 2. mesh reconstruction

say("-- 2. independent mesh reconstruction and source-fact recheck -------------")
const G = Geometry.DFG_SOURCE
const M = build_mesh(G, 50)
absorb!(mesh_checks(G, M))
const MM = measured_metrics(G, M)
const ID50 = ideal_metrics(BigFloat, 1 // 20, 50)
say(@sprintf("  measured Hausdorff bound        = %.17e m", MM.hausdorff))
say(@sprintf("  ideal sagitta(50)               = %.17e m", Float64(ID50.sagitta)))
say(@sprintf("  measured polygon area           = %.17e m^2", MM.polygon_area))
say(@sprintf("  measured polygon perimeter      = %.17e m", MM.polygon_perimeter))
const MESH_DIGEST = bytes2hex(sha256(geometric_digest(M)))
say("  index-free geometric mesh digest = $MESH_DIGEST")
say()

# ------------------------------------------------------------ 3. lower & solve

say("-- 3. lower the frozen witness and solve in elevated precision ------------")
const PH = Witness.DFG_PHYSICAL
check!("scale.P_equals_mu_U_over_L", PH.P == PH.mu * PH.U / PH.L, repr(PH.P))
check!("scale.Theta_one_ulp_below_9e-5",
       PH.P * PH.U * PH.L == prevfloat(9.0e-5), repr(PH.P * PH.U * PH.L))
check!("scale.G_equals_U_over_L", PH.U / PH.L == 0.7317073170731707, repr(PH.U / PH.L))

const PR = build_problem(T, M, PH)
const A, B = assemble(PR)
absorb!(structural_checks(M, PR, A, B))
say(@sprintf("  mu_hat = mu U / (P L)           = %s", hp(PR.sc.muhat)))

const SOL = solve_reduced(PR, A, B)
const RES = apply_full(PR, SOL.xfull) - B      # independent cell-loop reapplication
const RES_DENSE = A * SOL.xfull - B

check!("solve.independent_reapplication_agrees",
       maximum(abs, RES - RES_DENSE) <= EXACT * max(1, maximum(abs, RES_DENSE)),
       Float64(maximum(abs, RES - RES_DENSE)))

const RTRUE = sqrt(sum(z -> z * z, RES[PR.free]))
const WEAK = [RES[pdof(PR, v)] for v in 1:PR.nv]
const WEAK2 = sqrt(sum(z -> z * z, WEAK))
# Relative tolerance amended from 1e-11 by the case contract owner; see
# ../../amendment/. The superseded value's verdict on the f64-rounded elevated
# solution flipped with the reapplication precision. atol, the iteration cap and
# every other element of the frozen tuple are unchanged.
const RTOL = 1e-06
const TARGET = max(T(1e-13), T(RTOL) * SOL.bred_norm)
const ANORM = maximum(sum(abs, A[i, PR.free]) for i in PR.free)
const XNORM = maximum(abs, SOL.xfull[PR.free])
const BNORM = maximum(abs, B[PR.free] - A[PR.free, PR.essential] * PR.uess)
const ALLOW = 4096 * eps(Float64) * (1 + Float64(ANORM) * Float64(XNORM) + Float64(BNORM))

check!("residual.true_reduced_within_target", RTRUE <= TARGET + T(ALLOW),
       @sprintf("%.6e <= %.6e + %.6e", Float64(RTRUE), Float64(TARGET), ALLOW))
check!("residual.weak_pressure_rows_within_target", WEAK2 <= TARGET + T(ALLOW),
       @sprintf("%.6e", Float64(WEAK2)))
check!("residual.finite", isfinite(RTRUE) && isfinite(WEAK2))

const PS = physical_solution(PR, PH, SOL.xfull, RES)
const O = observe(M, PR, PH, PS)
say()

say("-- 4. frozen observations -------------------------------------------------")
for v in O.velocity
    say(@sprintf("  u @ bary of cell for target (%s, %s)  bary=(%s, %s)",
                 repr(v[1][1]), repr(v[1][2]), repr(v[4][1]), repr(v[4][2])))
    say(@sprintf("       ux = %s m/s", hp(v[5][1])))
    say(@sprintf("       uy = %s m/s", hp(v[5][2])))
    check!("probe.velocity_unique_cell_$(v[1])", v[3] == 1, "tied cells = $(v[3])")
end
for p in O.pressure
    say(@sprintf("  p %-22s vertex (%s, %s)  = %s Pa",
                 p[1], repr(p[4][1]), repr(p[4][2]), hp(p[5])))
    if length(p[3]) > 1
        for (vv, xy, val) in p[3]
            say(@sprintf("       exact tie candidate (%s, %s) -> %s Pa",
                         repr(xy[1]), repr(xy[2]), hp(val)))
        end
    end
end
say(@sprintf("  signed inlet  flux = %s m^2/s", hp(O.flux[:inlet])))
say(@sprintf("  signed outlet flux = %s m^2/s", hp(O.flux[:outlet])))
say(@sprintf("  wall flux = %s, cylinder flux = %s m^2/s",
             hp(O.flux[:walls]), hp(O.flux[:cylinder])))
say(@sprintf("  cylinder constraint force ON THE FLUID   = (%s, %s) N/m",
             hp(O.reaction_cylinder_on_fluid[1]), hp(O.reaction_cylinder_on_fluid[2])))
say(@sprintf("  fluid force ON THE CYLINDER (negation)   = (%s, %s) N/m",
             hp(O.reaction_fluid_on_cylinder[1]), hp(O.reaction_fluid_on_cylinder[2])))
say(@sprintf("  all-essential constrained reaction       = (%s, %s) N/m",
             hp(O.reaction_all_essential[1]), hp(O.reaction_all_essential[2])))
say(@sprintf("  integrated body force                    = (%s, %s) N/m",
             hp(O.body_force[1]), hp(O.body_force[2])))
say(@sprintf("  integrated applied traction              = (%s, %s) N/m",
             hp(O.applied_traction[1]), hp(O.applied_traction[2])))
say(@sprintf("  componentwise sum                        = (%s, %s) N/m",
             hp(O.balance[1]), hp(O.balance[2])))
say(@sprintf("  pressure integral (supplementary)        = %s Pa m^2", hp(O.pressure_integral)))

check!("balance.momentum_le_1e-10",
       max(abs(O.balance[1]), abs(O.balance[2])) <= 1e-10,
       Float64(max(abs(O.balance[1]), abs(O.balance[2]))))
check!("balance.flux_sum_le_1e-8", abs(O.flux[:inlet] + O.flux[:outlet]) <= 1e-8,
       Float64(abs(O.flux[:inlet] + O.flux[:outlet])))
check!("balance.wall_and_cylinder_flux_zero",
       O.flux[:walls] == 0 && O.flux[:cylinder] == 0)
check!("orientation.cylinder_negation_exact",
       O.reaction_fluid_on_cylinder == (-O.reaction_cylinder_on_fluid[1],
                                        -O.reaction_cylinder_on_fluid[2]))
check!("orientation.cylinder_force_is_not_zero",
       abs(O.reaction_cylinder_on_fluid[1]) > Falsify.TOL_REACTION)
say()

# --------------------------------------------- 5. stability of the frozen values

say("-- 5. stability of the frozen values --------------------------------------")

"""
Lower, assemble, solve and observe one configuration. `build` are the witness
lowering switches; `asm` are the local-operator switches, applied identically to
`assemble` and to the independent `apply_full` reapplication so a falsified run
recovers its reaction with its own operator.
"""
function solve_observe(mesh, ::Type{S}; build = (;), asm = (;)) where {S}
    pr = build_problem(S, mesh, PH; build...)
    a, b = assemble(pr; asm...)
    s = solve_reduced(pr, a, b)
    r = apply_full(pr, s.xfull; asm...) - b
    pr, observe(mesh, pr, PH, physical_solution(pr, PH, s.xfull, r))
end

setprecision(BigFloat, 384)
let (_, o2) = solve_observe(M, BigFloat)
    d = deviate(O, o2)
    check!("stability.precision_256_vs_384", !detected(d),
           @sprintf("vel %.2e pre %.2e flux %.2e reac %.2e", d.velocity, d.pressure,
                    d.flux, d.reaction))
end
setprecision(BigFloat, 256)

let (_, o2) = solve_observe(permute(M), T)
    d = deviate(O, o2)
    check!("stability.reindexing_invariant", !detected(d),
           @sprintf("vel %.2e pre %.2e flux %.2e reac %.2e selectors_moved=%s",
                    d.velocity, d.pressure, d.flux, d.reaction, d.selectors_moved))
end

const ULP_MESH = build_mesh(G, 50; trig_ulp = 1)
check!("stability.one_ulp_trig_actually_moved_the_mesh",
       geometric_digest(ULP_MESH) != geometric_digest(M))
const ULP_DEV = let (_, o2) = solve_observe(ULP_MESH, T)
    deviate(O, o2)
end
say(@sprintf("  one-ulp cos/sin perturbation moves: vel %.3e  pre %.3e  flux %.3e  reac %.3e",
             ULP_DEV.velocity, ULP_DEV.pressure, ULP_DEV.flux, ULP_DEV.reaction))
say(@sprintf("      route-to-route tolerances:      vel %.3e  pre %.3e  flux %.3e  reac %.3e",
             Falsify.TOL_VELOCITY, Falsify.TOL_PRESSURE, Falsify.TOL_FLUX,
             Falsify.TOL_REACTION))
check!("stability.one_ulp_trig_selectors_unmoved", !ULP_DEV.selectors_moved)
check!("stability.one_ulp_trig_within_route_tolerance", !detected(ULP_DEV))

const MUHAT_DEV = let (_, o2) = solve_observe(M, T; build = (muhat_one = true,))
    deviate(O, o2)
end
say(@sprintf("  mu_hat := 1 exactly moves:          vel %.3e  pre %.3e  flux %.3e  reac %.3e",
             MUHAT_DEV.velocity, MUHAT_DEV.pressure, MUHAT_DEV.flux, MUHAT_DEV.reaction))
say()

# ------------------------------------------------------------- 6. falsifiers

say("-- 6. falsifiers ----------------------------------------------------------")
const FALSIFIERS = Tuple{String,Deviation,String}[]

function falsify!(name, dev, note = "")
    push!(FALSIFIERS, (name, dev, note))
    check!("falsifier.$name", detected(dev),
           @sprintf("vel %.3e pre %.3e flux %.3e reac %.3e %s",
                    dev.velocity, dev.pressure, dev.flux, dev.reaction, note))
end

# F1 wrong quad diagonal O_j--I_i in place of the frozen O_i--I_j
let (_, o2) = solve_observe(build_mesh(G, 50; diagonal = :OjIi), T)
    falsify!("wrong_OiIj_diagonal", deviate(O, o2))
end

# F2/F3/F4/F5 local-operator defects
for (name, kw, note) in (
    ("vector_laplacian", (viscous = :laplacian,), "mu grad:grad instead of 2 mu sym:sym"),
    ("unnormalized_bubble", (bubble = :raw,),
     "assembled with l0*l1*l2 while the barycentre evaluation keeps the 27x convention"),
    ("coupling_sign_both_blocks", (coupling = -1,), "p -> -p"),
    ("coupling_sign_momentum_only", (momentum_coupling = -1,), "also destroys symmetry"))
    if name == "coupling_sign_momentum_only"
        pr = build_problem(T, M, PH)
        a, _ = assemble(pr; kw...)
        check!("falsifier.momentum_only_sign_flip_loses_exact_symmetry",
               any(a[i, j] != a[j, i] for i in 1:pr.ndof for j in 1:i),
               "reduced/full CSR symmetry assertion would reject before the solve")
    end
    (_, o2) = solve_observe(M, T; asm = kw)
    falsify!(name, deviate(O, o2), note)
end

# F6 dropped bubble unknowns. Removing the bubbles removes the MINI enrichment
# that makes the P1/P1 pair inf-sup stable, so the reduced system is expected to
# become exactly singular rather than to give merely wrong numbers.
try
    (_, o2) = solve_observe(M, T; build = (pin_bubbles = true,))
    falsify!("dropped_bubble_unknowns", deviate(O, o2))
catch err
    check!("falsifier.dropped_bubble_unknowns",
           err isa LinearAlgebra.SingularException,
           "reduced system is singular without the bubble enrichment: $(typeof(err))")
end

# F7 swapped inlet/outlet membership
let (_, o2) = solve_observe(M, T; build = (swap_inlet_outlet = true,))
    falsify!("swapped_inlet_outlet_membership", deviate(O, o2))
end

# F8 reversed inlet normal in the prescribed essential data
let (_, o2) = solve_observe(M, T; build = (reverse_inlet_normal = true,))
    falsify!("reversed_inlet_normal_in_boundary_data", deviate(O, o2),
             "flux-sum identity still holds: it is the probes that catch this")
end

# F9 reversed normal used in the flux observation itself
const REV_FLUX = -O.flux[:inlet] + O.flux[:outlet]
check!("falsifier.reversed_normal_in_flux_breaks_balance", abs(REV_FLUX) > 1e-8,
       @sprintf("|sum| = %.6e > 1e-8", Float64(abs(REV_FLUX))))

# F10 omitted cylinder facets: incomplete velocity/traction partition
let names = Dict(k => copy(v) for (k, v) in M.names)
    delete!(names, :cylinder)
    covered = sort!(vcat(values(names)...))
    check!("falsifier.omitted_cylinder_breaks_partition",
           covered != collect(1:length(M.bfacets)),
           "$(length(covered)) of $(length(M.bfacets)) facets covered")
end
let (_, o2) = solve_observe(M, T; build = (omit_cylinder = true,))
    falsify!("omitted_cylinder_changes_solution", deviate(O, o2))
end

# F11 zero traction substituted for cylinder no-slip: named reaction inadmissible
let pr = build_problem(T, M, PH; cylinder_traction = true)
    ess = Set((d + 1) ÷ 2 for d in pr.essential if d <= 2 * pr.nv)
    cyl = Set(vcat([[M.bfacets[f][1], M.bfacets[f][2]] for f in M.names[:cylinder]]...))
    check!("falsifier.cylinder_traction_leaves_no_constrained_cylinder_vertex",
           isempty(intersect(ess, cyl)),
           "constrained cylinder vertices = $(length(intersect(ess, cyl)))")
end

# F12 stale / renumbered correspondence replayed onto a refined mesh: the frozen
# n=50 facet index lists no longer name the same entities.
let refined = build_mesh(G, 52)
    stale_sides = [refined.facet_side[f] for f in M.names[:cylinder]]
    check!("falsifier.stale_correspondence_from_refined_mesh_rejects",
           length(refined.bfacets) != length(M.bfacets) ||
           any(!=(:circle), stale_sides),
           "n=52 gives $(length(refined.bfacets)) facets vs $(length(M.bfacets)); " *
           "$(count(!=(:circle), stale_sides)) of the frozen cylinder indices are not chords")
end

# The RFC 0082 corner-coincidence branch is a rejection here rather than an
# implemented corner-reuse path; n = 64 puts a radial hit exactly on the (0, 0)
# corner direction and must reject. The n = 50 witness provably never does.
check!("guard.radial_hit_on_corner_rejects",
       try
           build_mesh(G, 64)
           false
       catch err
           err isa ErrorException
       end, "n=64 casts a ray along the (0,0) corner direction")

# F13 inappropriate gauge alongside the nonempty traction partition
const GAUGE = gauge_falsifier(M, PR, A, B, PH, O)
check!("falsifier.gauge_with_traction_partition_is_wrong",
       abs(GAUGE.gamma) > 0 && GAUGE.probe_shift > Falsify.TOL_PRESSURE,
       @sprintf("gamma_hat = %.6e, pressure probes shift by %.6e Pa",
                Float64(GAUGE.gamma), Float64(GAUGE.probe_shift)))
say()

# ---------------------------------- 7. advisory: frozen solve selection in f64

say("-- 7. ADVISORY: frozen f64 solve selection on this witness -----------------")
say("  Julia analogue of the frozen tuple (MINRES, Identity, f64, rtol 1e-6,")
say("  atol 1e-13, <=10000 iterations). NOT the registered eqiora.reference")
say("  backend and NOT a hosted measurement; a feasibility indicator only.")

include(joinpath(HERE, "src", "minres.jl"))

const ARED64 = Float64.(A[PR.free, PR.free])
const BRED64 = Float64.(B[PR.free] - A[PR.free, PR.essential] * PR.uess)
const EV = eigvals(Symmetric(ARED64))
const KAPPA = cond(ARED64)
say(@sprintf("  cond_2(A_hat_reduced)  = %.6e", KAPPA))
say(@sprintf("  |lambda| in [%.4e, %.4e], %d negative / %d positive",
             minimum(abs, EV), maximum(abs, EV), count(<(0), EV), count(>(0), EV)))
say(@sprintf("  ||x_hat||_inf          = %.6e   (pressure block dominates)",
             Float64(XNORM)))

function observe64(xred)
    prf = build_problem(Float64, M, PH)
    af, bf = assemble(prf)
    x = zeros(Float64, prf.ndof)
    for (i, d) in enumerate(prf.essential)
        x[d] = prf.uess[i]
    end
    for (i, d) in enumerate(prf.free)
        x[d] = xred[i]
    end
    r = apply_full(prf, x) - bf
    observe(M, prf, PH, physical_solution(prf, PH, x, r))
end
prodtol = (2e-12 + 5e-7 * 0.3, 2e-14 + 5e-7 * PH.P, 2e-13 + 5e-7 * 0.123,
           2e-14 + 5e-7 * 0.0003)
function advisory(tag, o2, extra = "")
    d = deviate(O, o2)
    say(@sprintf("  %-14s vel %.3e/%.3e %s | pre %.3e/%.3e %s | flux %.3e/%.3e %s | reac %.3e/%.3e %s %s",
                 tag, d.velocity, prodtol[1], d.velocity <= prodtol[1] ? "OK" : "OVER",
                 d.pressure, prodtol[2], d.pressure <= prodtol[2] ? "OK" : "OVER",
                 d.flux, prodtol[3], d.flux <= prodtol[3] ? "OK" : "OVER",
                 d.reaction, prodtol[4], d.reaction <= prodtol[4] ? "OK" : "OVER", extra))
    d
end
const LU_DEV = advisory("f64 dense LU", observe64(ARED64 \ BRED64),
                        "(not the frozen selection)")
const XM, ITM, ESTM = minres(ARED64, BRED64; rtol = RTOL)
const FLOOR_TRUE, FLOOR_AT = minres(ARED64, BRED64; stop = false, maxiter = 20000,
                                    probe = 100)[4]
const TRUE_M = norm(ARED64 * XM - BRED64)
say(@sprintf("  MINRES: iterations = %d / 10000, recurred residual = %.6e, target = %.6e",
             ITM, ESTM, Float64(TARGET)))
say(@sprintf("  MINRES: independently reapplied TRUE residual = %.6e (allowance %.6e)",
             TRUE_M, ALLOW))
say(@sprintf("  MINRES: best TRUE residual over 20000 unstopped iterations = %.6e at %d",
             FLOOR_TRUE, FLOOR_AT))
const MINRES_DEV = advisory("f64 MINRES", observe64(XM), "(the frozen selection)")
const MINRES_OK = MINRES_DEV.velocity <= prodtol[1] && MINRES_DEV.pressure <= prodtol[2] &&
                  MINRES_DEV.flux <= prodtol[3] && MINRES_DEV.reaction <= prodtol[4]
say()
say(MINRES_OK ?
    "  ADVISORY RESULT: the frozen selection met every pointwise production tolerance." :
    "  ADVISORY RESULT: the frozen selection did NOT meet the pointwise production")
MINRES_OK || say("  tolerances for pressure and reaction. Reported, not relaxed. See README.")
say()

# ------------------------------------------------------------------- 8. freeze

say("-- 8. freeze --------------------------------------------------------------")
frozen = obj(
    "route" => "julia",
    "statement" => "One independently frozen Julia numerical-oracle route for the Eqiora exact circular-hole steady Stokes 2D slice. " *
                   "Route-to-route comparison has NOT been performed here. Implementation must not " *
                   "begin until the integrator compares this with the separately frozen first route.",
    "coarse_mesh_facts" => [
        "All 104 mesh vertices lie on the boundary.",
        "Trace closure fixes 103 velocity vertices; only the outlet midpoint (2.2, 0.2) m is free.",
        "MINI bubble velocities remain cell-interior unknowns on every cell.",
        "The reported cylinder vector is the algebraic constrained-vertex force on this mesh. " *
        "It is not drag, not a physically scaled force, and not mesh-independent.",
    ],
    "julia_version" => string(VERSION),
    "bigfloat_precision_bits" => 256,
    "source" => obj("bounds_m" => [[G.xlo, G.xhi], [G.ylo, G.yhi]],
                    "circle_centre_m" => [G.cx, G.cy], "radius_m" => G.r,
                    "classification_tolerance_m" => G.tol),
    "mesh" => obj("segments" => M.nseg, "vertices" => length(M.xy),
                  "cells" => length(M.cells), "boundary_facets" => length(M.bfacets),
                  "outer_loop_vertices" => length(M.outer_loop),
                  "interior_edges" => 104, "euler_characteristic" => 0,
                  "inlet_facets" => length(M.names[:inlet]),
                  "outlet_facets" => length(M.names[:outlet]),
                  "wall_facets" => length(M.names[:walls]),
                  "cylinder_facets" => length(M.names[:cylinder]),
                  "quad_diagonal" => "O_i--I_j",
                  "cells_per_ray_pair" => ["(O_i,O_j,I_j)", "(O_i,I_j,I_i)"],
                  "measured_hausdorff_bound_m" => MM.hausdorff,
                  "measured_polygon_area_m2" => MM.polygon_area,
                  "measured_polygon_perimeter_m" => MM.polygon_perimeter,
                  "geometric_digest_sha256" => MESH_DIGEST),
    "scales" => obj("L_m" => PH.L, "U_m_per_s" => PH.U, "P_Pa" => PH.P,
                    "G_per_s" => PH.U / PH.L, "Theta_W_per_m" => PH.P * PH.U * PH.L,
                    "Theta_mathematical_W_per_m" => 9.0e-5,
                    "mu_Pa_s" => PH.mu, "H_m" => PH.H, "Umax_m_per_s" => PH.Umax,
                    "mu_hat" => PR.sc.muhat),
    "dofs" => obj("p1_velocity" => 2 * PR.nv, "bubble_velocity" => 2 * PR.nc,
                  "pressure" => PR.nv, "full" => PR.ndof,
                  "essential_velocity" => length(PR.essential),
                  "reduced" => length(PR.free), "essential_vertices" => 103,
                  "free_vertices" => 1),
    "pressure_reference" => obj("kind" => "BoundaryTraction", "gauge_rows" => 0,
                               "gauge_columns" => 0, "gauge_multipliers" => 0,
                               "zero_integral_constraints" => 0,
                               "traction_partition_facets" => length(PR.traction)),
    "velocity_probes" => [obj("target_m" => [v[1][1], v[1][2]],
                              "tied_cells" => v[3],
                              "barycentre_m" => [v[4][1], v[4][2]],
                              "u_x_m_per_s" => v[5][1], "u_y_m_per_s" => v[5][2])
                          for v in O.velocity],
    "pressure_probes" => [obj("name" => p[1], "vertex_m" => [p[4][1], p[4][2]],
                              "p_Pa" => p[5], "exact_tie_count" => length(p[3]),
                              "tie_candidates" =>
                                  [obj("vertex_m" => [c[2][1], c[2][2]], "p_Pa" => c[3])
                                   for c in p[3]])
                          for p in O.pressure],
    "fluxes" => obj("inlet_m2_per_s" => O.flux[:inlet],
                    "outlet_m2_per_s" => O.flux[:outlet],
                    "walls_m2_per_s" => O.flux[:walls],
                    "cylinder_m2_per_s" => O.flux[:cylinder],
                    "sum_m2_per_s" => O.flux[:inlet] + O.flux[:outlet]),
    "reactions" => obj("cylinder_constraint_force_on_fluid_N_per_m" =>
                           [O.reaction_cylinder_on_fluid[1], O.reaction_cylinder_on_fluid[2]],
                       "fluid_force_on_cylinder_N_per_m" =>
                           [O.reaction_fluid_on_cylinder[1], O.reaction_fluid_on_cylinder[2]],
                       "all_essential_constrained_reaction_N_per_m" =>
                           [O.reaction_all_essential[1], O.reaction_all_essential[2]],
                       "integrated_body_force_N_per_m" => [O.body_force[1], O.body_force[2]],
                       "integrated_applied_traction_N_per_m" =>
                           [O.applied_traction[1], O.applied_traction[2]],
                       "componentwise_sum_N_per_m" => [O.balance[1], O.balance[2]]),
    "residuals" => obj("selected_target" => Float64(TARGET),
                       "roundoff_allowance" => ALLOW,
                       "true_reduced_2norm" => RTRUE,
                       "weak_pressure_row_2norm" => WEAK2,
                       "weak_pressure_row_infnorm" => maximum(abs, WEAK),
                       "A_hat_reduced_inf_norm" => Float64(ANORM),
                       "x_hat_inf_norm" => Float64(XNORM),
                       "b_hat_reduced_inf_norm" => Float64(BNORM),
                       "b_hat_reduced_2norm" => SOL.bred_norm),
    "supplementary" => obj("pressure_integral_Pa_m2" => O.pressure_integral),
    "stability" => obj("one_ulp_trig_velocity" => ULP_DEV.velocity,
                       "one_ulp_trig_pressure" => ULP_DEV.pressure,
                       "one_ulp_trig_flux" => ULP_DEV.flux,
                       "one_ulp_trig_reaction" => ULP_DEV.reaction,
                       "one_ulp_trig_selectors_moved" => ULP_DEV.selectors_moved,
                       "muhat_one_velocity" => MUHAT_DEV.velocity,
                       "muhat_one_pressure" => MUHAT_DEV.pressure,
                       "muhat_one_reaction" => MUHAT_DEV.reaction),
    "advisory_f64_solve" => obj(
        "disclaimer" => "Julia analogue of the frozen solve tuple at the amended rtol 1e-6, " *
                        "run locally. NOT the registered eqiora.reference backend and NOT a " *
                        "hosted measurement.",
        "cond2_A_hat_reduced" => KAPPA,
        "abs_eigenvalue_min" => minimum(abs, EV), "abs_eigenvalue_max" => maximum(abs, EV),
        "negative_eigenvalues" => count(<(0), EV), "positive_eigenvalues" => count(>(0), EV),
        "minres_iterations" => ITM, "minres_iteration_cap" => 10000,
        "minres_recurred_residual" => ESTM, "minres_true_residual" => TRUE_M,
        "minres_best_true_residual_20000_unstopped" => FLOOR_TRUE,
        "minres_best_true_residual_at_iteration" => FLOOR_AT,
        "minres_max_probe_error_pressure_Pa" => MINRES_DEV.pressure,
        "minres_max_probe_error_reaction_N_per_m" => MINRES_DEV.reaction,
        "minres_max_probe_error_velocity_m_per_s" => MINRES_DEV.velocity,
        "minres_max_probe_error_flux_m2_per_s" => MINRES_DEV.flux,
        "minres_meets_pointwise_production_tolerances" => MINRES_OK,
        "dense_lu_max_probe_error_pressure_Pa" => LU_DEV.pressure,
        "dense_lu_max_probe_error_reaction_N_per_m" => LU_DEV.reaction),
    "checks" => obj("total" => length(CHECKS), "passed" => count(c -> c[2], CHECKS),
                    "failed" => count(c -> !c[2], CHECKS)),
    "check_names" => [c[1] for c in CHECKS],
)

mkpath(joinpath(HERE, "expected"))
write(joinpath(HERE, "expected", "julia-route-frozen.json"), frozen.s * "\n")

const NFAIL = count(c -> !c[2], CHECKS)
say(@sprintf("  checks: %d total, %d passed, %d failed", length(CHECKS),
             count(c -> c[2], CHECKS), NFAIL))
for c in CHECKS
    c[2] || say("  FAILED: $(c[1])  $(c[3])")
end
say("  frozen file sha256 = " *
    bytes2hex(sha256(read(joinpath(HERE, "expected", "julia-route-frozen.json")))))
for f in ["run.jl", "src/oracle.jl", "src/geometry.jl", "src/mini.jl", "src/witness.jl",
          "src/audit.jl", "src/falsify.jl", "src/minres.jl"]
    say(@sprintf("  sha256 %-20s %s", f, bytes2hex(sha256(read(joinpath(HERE, f))))))
end
say()
say(NFAIL == 0 ? "ROUTE STATUS: all internal checks passed." :
                 "ROUTE STATUS: FAILED.")

write(joinpath(HERE, "expected", "run-log.txt"), String(take!(LOG)))
exit(NFAIL == 0 ? 0 : 1)
