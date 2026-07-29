# MINI / P1 steady Stokes assembly built from first principles.
#
# Everything is formed by an explicit positive 3x3 Gauss-Legendre Duffy
# quadrature loop over every affine triangle: no closed-form cell block, no
# static condensation, no reuse of any other route's algebra. The assembled
# system is the dimensionless congruent system of RFC 0045 (`A_hat = D A D /
# Theta`), obtained by assembling directly in normalized coordinates.

module Mini

using LinearAlgebra

export Scales, Problem, Quadrature, duffy_gauss3
export build_problem, assemble, apply_full, solve_reduced, Solution

# --------------------------------------------------------------- quadrature

struct Quadrature{T}
    xi::Vector{T}
    eta::Vector{T}
    w::Vector{T}
end

"""
Positive 3x3 Gauss-Legendre Duffy rule on the reference triangle
`{(xi, eta) : xi >= 0, eta >= 0, xi + eta <= 1}`.

Three-point Gauss-Legendre on each unit-square axis is exact through degree
five per axis; the Duffy image of a total-degree-`d` triangle monomial has
degree `d + 1` in the collapsed axis, so the rule is exact through total degree
four, which is the MINI bubble-gradient product degree required by RFC 0043.
"""
function duffy_gauss3(::Type{T}) where {T}
    g = sqrt(T(3) / T(5))
    nodes = (-g, zero(T), g)
    wts = (T(5) / 9, T(8) / 9, T(5) / 9)
    xi = T[]; eta = T[]; w = T[]
    for a in 1:3, b in 1:3
        s = (1 + nodes[a]) / 2
        t = (1 + nodes[b]) / 2
        push!(xi, s)
        push!(eta, t * (1 - s))
        push!(w, (wts[a] / 2) * (wts[b] / 2) * (1 - s))
    end
    Quadrature{T}(xi, eta, w)
end

# ------------------------------------------------------------------- scaling

"Coherent-SI Realization scale profile of RFC 0045."
struct Scales{T}
    L::T
    U::T
    P::T
    G::T          # U / L, gauge-block scale (unused when no gauge exists)
    Theta::T      # P U L
    muhat::T      # mu U / (P L)
end

function Scales(::Type{T}, L, U, P, mu) where {T}
    Lt, Ut, Pt, mut = T(L), T(U), T(P), T(mu)
    Scales{T}(Lt, Ut, Pt, Ut / Lt, Pt * Ut * Lt, mut * Ut / (Pt * Lt))
end

# ------------------------------------------------------------------- problem

"""
Assembled discrete problem in dimensionless (hat) variables.

DOF layout, chosen once and never inferred from names:

    1 .. 2NV                    P1 vertex velocity, `2(v-1)+c`
    2NV+1 .. 2NV+2NC            cell bubble velocity, `2NV+2(k-1)+c`
    2NV+2NC+1 .. 2NV+2NC+NV     P1 pressure, `2NV+2NC+v`
"""
struct Problem{T}
    xh::Vector{NTuple{2,T}}             # normalized vertex coordinates
    cells::Vector{NTuple{3,Int}}
    sc::Scales{T}
    ndof::Int
    nv::Int
    nc::Int
    essential::Vector{Int}              # essential velocity dofs, ascending
    free::Vector{Int}                   # remaining dofs, ascending
    uess::Vector{T}                     # dimensionless essential values
    traction::Vector{Tuple{Int,Int,NTuple{2,T}}}   # (a, b, t_hat) facet loads
    quad::Quadrature{T}
end

vdof(p::Problem, v::Int, c::Int) = 2 * (v - 1) + c
bdof(p::Problem, k::Int, c::Int) = 2 * p.nv + 2 * (k - 1) + c
pdof(p::Problem, v::Int) = 2 * p.nv + 2 * p.nc + v

export vdof, bdof, pdof

# ----------------------------------------------------------- basis at a point

"""
Return `(phi, gradref)` for the four MINI scalar bases at reference point
`(xi, eta)`: three P1 barycentric functions plus the normalized cell bubble
`27 * l0 * l1 * l2`, which is one at the barycentre and zero on every edge.
"""
function mini_basis(xi::T, eta::T) where {T}
    l0 = 1 - xi - eta
    l1 = xi
    l2 = eta
    g0 = (-one(T), -one(T))
    g1 = (one(T), zero(T))
    g2 = (zero(T), one(T))
    b = 27 * l0 * l1 * l2
    gb = (27 * (l1 * l2 * g0[1] + l0 * l2 * g1[1] + l0 * l1 * g2[1]),
          27 * (l1 * l2 * g0[2] + l0 * l2 * g1[2] + l0 * l1 * g2[2]))
    ((l0, l1, l2, b), (g0, g1, g2, gb))
end

"Unnormalized-bubble variant used only by the falsifier battery."
function mini_basis_raw(xi::T, eta::T) where {T}
    (phi, gref) = mini_basis(xi, eta)
    ((phi[1], phi[2], phi[3], phi[4] / 27),
     (gref[1], gref[2], gref[3], (gref[4][1] / 27, gref[4][2] / 27)))
end

# ------------------------------------------------------------------ assembly

"""
Assemble the full dimensionless system.

`viscous = :symmetric` uses `2 mu sym(grad u) : sym(grad v)`;
`viscous = :laplacian` is the falsifier `mu grad(u) : grad(v)`.
`coupling` scales the pressure/velocity block; `-1` reverses its sign.
`momentum_coupling` scales only the momentum-row copy, so `-1` also destroys
exact symmetry.
"""
function assemble(p::Problem{T};
                  viscous::Symbol = :symmetric,
                  coupling::Int = 1,
                  momentum_coupling::Int = 1,
                  bubble::Symbol = :normalized,
                  drop_bubble::Bool = false) where {T}
    n = p.ndof
    A = zeros(T, n, n)
    b = zeros(T, n)
    basis = bubble === :normalized ? mini_basis : mini_basis_raw
    nb = drop_bubble ? 3 : 4

    for (k, cell) in enumerate(p.cells)
        x0, x1, x2 = p.xh[cell[1]], p.xh[cell[2]], p.xh[cell[3]]
        J11 = x1[1] - x0[1]; J12 = x2[1] - x0[1]
        J21 = x1[2] - x0[2]; J22 = x2[2] - x0[2]
        det = J11 * J22 - J12 * J21
        det > 0 || error("cell $k is not positively oriented")

        gdof = ntuple(a -> a <= 3 ? cell[a] : k, 4)
        for q in eachindex(p.quad.w)
            phi, gref = basis(p.quad.xi[q], p.quad.eta[q])
            w = p.quad.w[q] * det
            gx = ntuple(a -> ((J22 * gref[a][1] - J21 * gref[a][2]) / det,
                              (-J12 * gref[a][1] + J11 * gref[a][2]) / det), 4)

            for a in 1:nb, c in 1:2
                ia = a <= 3 ? vdof(p, gdof[a], c) : bdof(p, k, c)
                for e in 1:nb, d in 1:2
                    ie = e <= 3 ? vdof(p, gdof[e], d) : bdof(p, k, d)
                    dot_g = gx[a][1] * gx[e][1] + gx[a][2] * gx[e][2]
                    val = if viscous === :symmetric
                        (c == d ? dot_g : zero(T)) + gx[a][d] * gx[e][c]
                    elseif viscous === :laplacian
                        c == d ? dot_g : zero(T)
                    else
                        error("unknown viscous form $viscous")
                    end
                    A[ia, ie] += w * p.sc.muhat * val
                end
                # Mixed block: c(v, p) = -int p div(v).
                for m in 1:3
                    ip = pdof(p, cell[m])
                    val = -w * phi[m] * gx[a][c]
                    A[ia, ip] += momentum_coupling * coupling * val
                    A[ip, ia] += coupling * val
                end
            end
        end
    end

    # Constant-traction P1 facet load: length * traction / 2 at each endpoint.
    for (a, bb, t) in p.traction
        dx = p.xh[bb][1] - p.xh[a][1]
        dy = p.xh[bb][2] - p.xh[a][2]
        len = sqrt(dx * dx + dy * dy)
        for c in 1:2
            b[vdof(p, a, c)] += len * t[c] / 2
            b[vdof(p, bb, c)] += len * t[c] / 2
        end
    end
    A, b
end

"""
Apply the full operator to `x` by looping cells again, without touching any
assembled matrix. This is the independent reapplication used for every reported
residual and balance.

The keyword switches mirror `assemble` so that a falsified run recovers its
reaction with its own defective operator rather than with the frozen one.
"""
function apply_full(p::Problem{T}, x::Vector{T};
                    viscous::Symbol = :symmetric,
                    coupling::Int = 1,
                    momentum_coupling::Int = 1,
                    bubble::Symbol = :normalized,
                    drop_bubble::Bool = false) where {T}
    y = zeros(T, p.ndof)
    basis = bubble === :normalized ? mini_basis : mini_basis_raw
    nb = drop_bubble ? 3 : 4
    for (k, cell) in enumerate(p.cells)
        x0, x1, x2 = p.xh[cell[1]], p.xh[cell[2]], p.xh[cell[3]]
        J11 = x1[1] - x0[1]; J12 = x2[1] - x0[1]
        J21 = x1[2] - x0[2]; J22 = x2[2] - x0[2]
        det = J11 * J22 - J12 * J21
        gdof = ntuple(a -> a <= 3 ? cell[a] : k, 4)
        idx = Matrix{Int}(undef, 4, 2)
        for a in 1:4, c in 1:2
            idx[a, c] = a <= 3 ? vdof(p, gdof[a], c) : bdof(p, k, c)
        end
        pidx = ntuple(m -> pdof(p, cell[m]), 3)
        for q in eachindex(p.quad.w)
            phi, gref = basis(p.quad.xi[q], p.quad.eta[q])
            w = p.quad.w[q] * det
            gx = ntuple(a -> ((J22 * gref[a][1] - J21 * gref[a][2]) / det,
                              (-J12 * gref[a][1] + J11 * gref[a][2]) / det), 4)
            # div(u_h) and grad-contractions at this point
            divu = zero(T)
            for a in 1:nb, c in 1:2
                divu += gx[a][c] * x[idx[a, c]]
            end
            ph = zero(T)
            for m in 1:3
                ph += phi[m] * x[pidx[m]]
            end
            gu = zeros(T, 2, 2)     # gu[c, d] = d_d u_c
            for a in 1:nb, c in 1:2
                xa = x[idx[a, c]]
                gu[c, 1] += gx[a][1] * xa
                gu[c, 2] += gx[a][2] * xa
            end
            for a in 1:nb, c in 1:2
                acc = p.sc.muhat * (gx[a][1] * gu[c, 1] + gx[a][2] * gu[c, 2])
                viscous === :symmetric &&
                    (acc += p.sc.muhat * (gx[a][1] * gu[1, c] + gx[a][2] * gu[2, c]))
                acc -= momentum_coupling * coupling * ph * gx[a][c]
                y[idx[a, c]] += w * acc
            end
            for m in 1:3
                y[pidx[m]] -= w * coupling * phi[m] * divu
            end
        end
    end
    y
end

# ---------------------------------------------------------------------- solve

struct Solution{T}
    xfull::Vector{T}
    reduced_residual::Vector{T}
    bred_norm::T
end

function solve_reduced(p::Problem{T}, A::Matrix{T}, b::Vector{T}) where {T}
    xfull = zeros(T, p.ndof)
    for (i, d) in enumerate(p.essential)
        xfull[d] = p.uess[i]
    end
    bred = b[p.free] - A[p.free, p.essential] * p.uess
    Ared = A[p.free, p.free]
    xred = Ared \ bred
    for (i, d) in enumerate(p.free)
        xfull[d] = xred[i]
    end
    Solution{T}(xfull, Ared * xred - bred, sqrt(sum(z -> z * z, bred)))
end

end # module
