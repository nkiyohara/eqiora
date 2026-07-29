#!/usr/bin/env julia
#
# Route B: the same reduced-system reapplication measurement as
# `measure_reapplication_floor.py`, on route B's independently constructed
# operator -- explicit Duffy quadrature, bubbles never condensed, BigFloat 256.
#
#     julia amendment/measure_reapplication_floor.jl
#     julia amendment/measure_reapplication_floor.jl --check
#
# Writes `expected/route-b-reapplication.json`. No production or candidate
# implementation is read or executed; the only solve is route B's own.
#
# The two routes are measured separately and never averaged. Their agreement on
# which side of a target `fl(x*)` falls is the point; their exact residual norms
# differ because their elevated solutions differ in the far digits, and one
# ulp of difference in any component moves the rounded vector.

using LinearAlgebra, Printf

const HERE = @__DIR__
const CASE = dirname(HERE)
include(joinpath(CASE, "routes", "julia", "src", "oracle.jl"))
using .Oracle
using .Oracle.Geometry, .Oracle.Mini, .Oracle.Witness

BLAS.set_num_threads(1)
setprecision(BigFloat, 256)

const SUPERSEDED_RTOL = 1e-11
const AMENDED_RTOL = 1e-06
const ATOL = 1e-13
const EPS64 = eps(Float64)

const G = Geometry.DFG_SOURCE
const M = build_mesh(G, 50)
const PH = Witness.DFG_PHYSICAL
const PR = build_problem(BigFloat, M, PH)
const A, B = assemble(PR)
const SOL = solve_reduced(PR, A, B)

const AH = A[PR.free, PR.free]
const BH = B[PR.free] - A[PR.free, PR.essential] * PR.uess
const XH = SOL.xfull[PR.free]
const N = length(PR.free)
const B2 = norm(BH)

const SUPERSEDED_TARGET = max(ATOL, SUPERSEDED_RTOL * Float64(B2))
const AMENDED_TARGET = max(ATOL, AMENDED_RTOL * Float64(B2))

# fl(x*) and the certification that the rounding is decided
const X64 = Float64.(XH)
const XB = BigFloat.(X64)
const TIE = minimum(abs(abs(XH[k] - XB[k]) / (BigFloat(2)^(exponent(XH[k]) - 52)) - 0.5)
                    for k in 1:N if XH[k] != 0)

const RHO_ELEVATED = norm(AH * XB - BH)

# f64 reapplication, several orderings on route B's own operator
const AH64 = Float64.(AH)
const BH64 = Float64.(BH)
blas_matvec() = norm(AH64 * X64 - BH64)
function row_loop(rev::Bool)
    acc = 0.0
    for k in 1:N
        t = -BH64[k]
        rng = rev ? (N:-1:1) : (1:N)
        for j in rng
            t += AH64[k, j] * X64[j]
        end
        acc += t * t
    end
    sqrt(acc)
end
function full_system()
    xf = zeros(Float64, PR.ndof)
    for (i, d) in enumerate(PR.essential)
        xf[d] = Float64(PR.uess[i])
    end
    for (i, d) in enumerate(PR.free)
        xf[d] = X64[i]
    end
    r = Float64.(A) * xf - Float64.(B)
    norm(r[PR.free])
end
const ORDERINGS = Dict("blas_dense_matvec" => blas_matvec(),
                       "row_loop_natural" => row_loop(false),
                       "row_loop_reversed" => row_loop(true),
                       "full_system_path" => full_system())
const F64_MIN = minimum(values(ORDERINGS))
const F64_MAX = maximum(values(ORDERINGS))

# gamma_m bound over route B's own sparsity
const NNZ = [count(!=(0), AH[k, :]) for k in 1:N]
const CANC = [sum(abs(AH[k, j] * XB[j]) for j in 1:N) for k in 1:N]
const GAMMA = sqrt(sum(k -> begin
                           g = (NNZ[k] + 1) * (EPS64 / 2) / (1 - (NNZ[k] + 1) * (EPS64 / 2))
                           (g * Float64(CANC[k] + abs(BH[k])))^2
                       end, 1:N))

jf(x::Float64) = isfinite(x) ? repr(x) : error("non-finite leaf")
jf(x::Int) = string(x)
jf(x::String) = "\"" * escape_string(x) * "\""
kv(pairs) = "{" * join(["\n    " * jf(String(k)) * ": " * (v isa String ? jf(v) : jf(v))
                        for (k, v) in pairs], ",") * "\n  }"

const REPORT = """
{
  "schema": "eqiora.verify/exact-circular-hole-stokes-2d/amendment/route-b/v1",
  "route": "julia",
  "statement": "Route B measurement of the reduced-system residual carried by the f64-rounded elevated-precision oracle solution. Independently assembled by explicit Duffy quadrature with the bubbles never condensed. No production or candidate implementation was read or executed.",
  "environment": $(kv(["julia" => string(VERSION), "bigfloat_precision_bits" => 256,
                       "binary64_eps" => EPS64])),
  "system": $(kv(["reduced_dimension" => N, "b_hat_2norm" => Float64(B2),
                  "x_hat_2norm" => Float64(norm(XH)),
                  "x_hat_inf_norm" => Float64(maximum(abs, XH)),
                  "A_hat_inf_norm" => Float64(maximum(sum(abs, AH[k, :]) for k in 1:N)),
                  "nonzeros_per_row_min" => minimum(NNZ),
                  "nonzeros_per_row_max" => maximum(NNZ),
                  "max_row_cancellation_sum_abs_A_x" => Float64(maximum(CANC))])),
  "targets": $(kv(["superseded_relative_tolerance" => SUPERSEDED_RTOL,
                   "superseded_target" => SUPERSEDED_TARGET,
                   "amended_relative_tolerance" => AMENDED_RTOL,
                   "amended_target" => AMENDED_TARGET,
                   "absolute_tolerance" => ATOL])),
  "representation": $(kv(["elevated_residual_at_exact_solution" => Float64(norm(AH * XH - BH)),
                          "min_ulp_distance_from_a_rounding_tie" => Float64(TIE),
                          "x_minus_rounded_2norm" => Float64(norm(XH - XB)),
                          "rho_elevated" => Float64(RHO_ELEVATED)])),
  "evaluation": $(kv(vcat([k => v for (k, v) in sort(collect(ORDERINGS), by = first)],
                          ["f64_min" => F64_MIN, "f64_max" => F64_MAX,
                           "gamma_m_bound_2norm" => GAMMA,
                           "decidable_bound_rho_plus_gamma" => Float64(RHO_ELEVATED) + GAMMA])))
}
"""

const OUT = joinpath(HERE, "expected", "route-b-reapplication.json")
mkpath(dirname(OUT))
if "--check" in ARGS
    if !isfile(OUT) || read(OUT, String) != REPORT
        println(stderr, "FAIL: route-b-reapplication.json would change")
        exit(1)
    end
    println("route-b-reapplication.json reproduced byte for byte")
else
    write(OUT, REPORT)
    println("wrote amendment/expected/route-b-reapplication.json")
end
@printf("  rho_elevated = %.17g   f64 in [%.17g, %.17g]\n", Float64(RHO_ELEVATED), F64_MIN, F64_MAX)
@printf("  superseded target %.6e : elevated %s, every f64 ordering %s\n",
        SUPERSEDED_TARGET,
        Float64(RHO_ELEVATED) <= SUPERSEDED_TARGET ? "ACCEPTS" : "rejects",
        F64_MIN > SUPERSEDED_TARGET ? "REJECTS" : "does not all reject")
