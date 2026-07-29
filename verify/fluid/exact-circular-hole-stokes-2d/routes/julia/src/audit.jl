# Internal method audit for the assembly, solve and reaction machinery.
#
# NOT a capability claim and NOT a PDE-convergence claim. It only asks whether
# this file's own algebra reproduces a closed-form answer that is exactly
# representable in the MINI/P1 space, so that a defect in the operator, the
# essential lifting, the traction partition or the reaction recovery is caught
# without appeal to any other route.
#
# Fixture: Omega = (0, 2) x (0, 1), viscosity mu, body force zero,
#
#     u = (x, -y),      p = 2 mu.
#
# `div u = 0`, `sym(grad u)` is constant so `div sigma = 0`, and on x = 2 the
# parent-outward traction is `sigma n = (2 mu - p, 0) = 0`. Velocity is affine
# and pressure is constant, so both lie exactly in the discrete space: the
# Galerkin solution must reproduce them with zero bubble coefficients.
#
# A plane-Poiseuille profile is deliberately NOT used here: it is quadratic in
# y, is not in the MINI velocity space, and could therefore only be checked
# asymptotically, which is exactly the PDE-convergence claim the frozen contract forbids.

module Audit

using ..Mini

export audit_quadrature, audit_bubble, audit_patch, AuditReport

struct AuditReport
    checks::Vector{Tuple{String,Bool,String}}
end

npass(r::AuditReport) = count(c -> c[2], r.checks)
nfail(r::AuditReport) = count(c -> !c[2], r.checks)
export npass, nfail

record!(r, name, ok, detail = "") = push!(r.checks, (name, ok, detail))

# --------------------------------------------------------------- quadrature

"Positivity, mass and exactness through total degree four of the Duffy rule."
function audit_quadrature(::Type{T}, tol) where {T}
    r = AuditReport(Tuple{String,Bool,String}[])
    q = duffy_gauss3(T)
    record!(r, "quadrature.points", length(q.w) == 9, string(length(q.w)))
    record!(r, "quadrature.positive", all(>(0), q.w) && all(>=(0), q.xi) && all(>=(0), q.eta))
    record!(r, "quadrature.interior", all(q.xi .+ q.eta .<= 1))
    record!(r, "quadrature.mass", abs(sum(q.w) - T(1) / 2) <= tol,
            string(Float64(sum(q.w) - T(1) / 2)))
    fact(n) = prod(T(1):T(max(n, 1)))
    worst = zero(T)
    for a in 0:4, b in 0:(4-a)
        num = sum(q.w[i] * q.xi[i]^a * q.eta[i]^b for i in eachindex(q.w))
        exact = fact(a) * fact(b) / fact(a + b + 2)
        worst = max(worst, abs(num - exact))
    end
    record!(r, "quadrature.exact_degree4", worst <= tol, string(Float64(worst)))
    # Degree five must NOT be exact, or the stated degree bound is not tight.
    d5 = abs(sum(q.w[i] * q.xi[i]^3 * q.eta[i]^2 for i in eachindex(q.w)) -
             fact(3) * fact(2) / fact(7))
    record!(r, "quadrature.degree5_inexact_as_declared", d5 > tol, string(Float64(d5)))
    r
end

"Normalized MINI bubble: one at the barycentre, zero on every edge."
function audit_bubble(::Type{T}, tol) where {T}
    r = AuditReport(Tuple{String,Bool,String}[])
    third = T(1) / 3
    phi, _ = Mini.mini_basis(third, third)
    record!(r, "bubble.barycentre_is_one", abs(phi[4] - 1) <= tol, string(Float64(phi[4])))
    onedge = maximum(abs(Mini.mini_basis(T(s), zero(T))[1][4]) for s in (0, 1//4, 1//2, 1))
    onedge = max(onedge, maximum(abs(Mini.mini_basis(zero(T), T(s))[1][4]) for s in (0, 1//4, 1//2, 1)))
    onedge = max(onedge, maximum(abs(Mini.mini_basis(T(s), 1 - T(s))[1][4]) for s in (0, 1//4, 1//2, 1)))
    record!(r, "bubble.vanishes_on_edges", onedge <= tol, string(Float64(onedge)))
    q = duffy_gauss3(T)
    ib = sum(q.w[i] * Mini.mini_basis(q.xi[i], q.eta[i])[1][4] for i in eachindex(q.w))
    # int_ref b = 27 * 2*(1/2) * 1/5! = 27/120 = 0.225 on the unit reference triangle.
    record!(r, "bubble.reference_integral", abs(ib - T(27) / 120) <= tol, string(Float64(ib)))
    p1 = sum(Mini.mini_basis(T(1) / 5, T(1) / 3)[1][1:3])
    record!(r, "p1.partition_of_unity", abs(p1 - 1) <= tol, string(Float64(p1)))
    r
end

# ------------------------------------------------------------ exact patch test

function patch_problem(::Type{T}, mu, nx::Int, ny::Int, Lx, Ly) where {T}
    vid(i, j) = j * (nx + 1) + i + 1
    xy = NTuple{2,T}[]
    for j in 0:ny, i in 0:nx
        push!(xy, (T(i) * T(Lx) / nx, T(j) * T(Ly) / ny))
    end
    cells = NTuple{3,Int}[]
    for j in 0:(ny-1), i in 0:(nx-1)
        cells = push!(cells, (vid(i, j), vid(i + 1, j), vid(i + 1, j + 1)))
        cells = push!(cells, (vid(i, j), vid(i + 1, j + 1), vid(i, j + 1)))
    end
    nv, nc = length(xy), length(cells)
    ndof = 2 * nv + 2 * nc + nv
    sc = Mini.Scales{T}(one(T), one(T), one(T), one(T), one(T), T(mu))

    exact_u(p) = (p[1], -p[2])
    ess = Int[]; uess = T[]
    for v in 1:nv
        p = xy[v]
        if p[1] == 0 || p[2] == 0 || p[2] == T(Ly)
            u = exact_u(p)
            push!(ess, 2 * (v - 1) + 1); push!(uess, u[1])
            push!(ess, 2 * (v - 1) + 2); push!(uess, u[2])
        end
    end
    trac = Tuple{Int,Int,NTuple{2,T}}[]
    for j in 0:(ny-1)
        trac = push!(trac, (vid(nx, j), vid(nx, j + 1), (zero(T), zero(T))))
    end
    free = setdiff(1:ndof, ess)
    pr = Problem{T}(xy, cells, sc, ndof, nv, nc, ess, free, uess, trac, duffy_gauss3(T))
    pr, exact_u
end

"""
Solve the exact patch fixture and check field, bubbles, pressure, symmetry,
fluxes and reaction against the closed-form answer derived above.
"""
function audit_patch(::Type{T}, tol; mu = 3 // 2, nx = 3, ny = 2, Lx = 2, Ly = 1) where {T}
    r = AuditReport(Tuple{String,Bool,String}[])
    pr, exact_u = patch_problem(T, T(numerator(mu)) / T(denominator(mu)), nx, ny, Lx, Ly)
    A, b = assemble(pr)

    asym = maximum(abs(A[i, j] - A[j, i]) for i in 1:pr.ndof, j in 1:pr.ndof)
    record!(r, "patch.matrix_symmetric", asym == 0, string(Float64(asym)))

    sol = solve_reduced(pr, A, b)
    x = sol.xfull
    muT = T(numerator(mu)) / T(denominator(mu))

    worst_u = zero(T)
    for v in 1:pr.nv
        ue = exact_u(pr.xh[v])
        worst_u = max(worst_u, abs(x[vdof(pr, v, 1)] - ue[1]), abs(x[vdof(pr, v, 2)] - ue[2]))
    end
    record!(r, "patch.velocity_exact", worst_u <= tol, string(Float64(worst_u)))

    worst_b = maximum(abs(x[bdof(pr, k, c)]) for k in 1:pr.nc, c in 1:2)
    record!(r, "patch.bubbles_vanish", worst_b <= tol, string(Float64(worst_b)))

    worst_p = maximum(abs(x[pdof(pr, v)] - 2 * muT) for v in 1:pr.nv)
    record!(r, "patch.pressure_equals_2mu", worst_p <= tol, string(Float64(worst_p)))

    res = apply_full(pr, x) - b
    total = (sum(res[vdof(pr, v, 1)] for v in 1:pr.nv), sum(res[vdof(pr, v, 2)] for v in 1:pr.nv))
    record!(r, "patch.reaction_balance", max(abs(total[1]), abs(total[2])) <= tol,
            string(Float64(total[1])) * "," * string(Float64(total[2])))

    # Analytic per-side reaction: sigma = diag(2 mu - p, -2 mu - p) = diag(0, -4 mu).
    ry_bottom = sum(res[vdof(pr, v, 2)] for v in 1:pr.nv if pr.xh[v][2] == 0)
    ry_top = sum(res[vdof(pr, v, 2)] for v in 1:pr.nv if pr.xh[v][2] == T(Ly))
    record!(r, "patch.reaction_bottom_y", abs(ry_bottom - 4 * muT * T(Lx)) <= tol,
            string(Float64(ry_bottom)))
    record!(r, "patch.reaction_top_y", abs(ry_top + 4 * muT * T(Lx)) <= tol,
            string(Float64(ry_top)))

    # Trapezoidal P1 fluxes: left 0, bottom 0, right +Ly, top -Lx.
    flux(a, bb, n) = begin
        len = sqrt(sum((pr.xh[bb][k] - pr.xh[a][k])^2 for k in 1:2))
        len * sum(n[k] * (x[vdof(pr, a, k)] + x[vdof(pr, bb, k)]) / 2 for k in 1:2)
    end
    fr = sum(flux(a, bb, (one(T), zero(T))) for (a, bb, _) in pr.traction)
    record!(r, "patch.outflow_flux", abs(fr - T(Ly) * T(Lx)) <= tol, string(Float64(fr)))

    resid = maximum(abs, res[pr.free])
    record!(r, "patch.free_rows_solved", resid <= tol, string(Float64(resid)))
    r
end

end # module
