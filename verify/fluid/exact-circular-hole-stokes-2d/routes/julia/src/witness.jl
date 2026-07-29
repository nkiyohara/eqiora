# The frozen physical witness of the contract, lowered onto the reconstructed
# chordal mesh, plus every geometric observation selector.
#
# Selectors are geometric only. No observation is allowed to depend on vertex,
# cell or facet numbering; `permute` below exists to prove that.

module Witness

using ..Geometry
using ..Mini

export Physical, DFG_PHYSICAL, inlet_speed
export build_problem, physical_solution, Observations, observe, permute

# ------------------------------------------------------------------- witness

struct Physical
    mu::Float64
    H::Float64
    Umax::Float64
    L::Float64
    U::Float64
    P::Float64
end

const DFG_PHYSICAL = Physical(0.001, 0.41, 0.3, 0.41, 0.3, 0.0007317073170731707)

"Prescribed inlet speed `g(y) = 4 Umax y (H - y) / H^2`, evaluated in `T`."
inlet_speed(::Type{T}, ph::Physical, y) where {T} =
    4 * T(ph.Umax) * T(y) * (T(ph.H) - T(y)) / T(ph.H)^2

# ------------------------------------------------------------ problem lowering

"""
    build_problem(T, mesh, ph; kwargs...)

Lower the frozen witness onto `mesh`.

Keyword falsifier switches: `swap_inlet_outlet` exchanges the inlet and outlet
named sets, `omit_cylinder` drops the cylinder from the velocity partition,
`cylinder_traction` replaces cylinder no-slip by zero traction,
`reverse_inlet_normal` prescribes `u = [-g, 0]`, `pin_bubbles` removes the
bubble unknowns, `muhat_one` forces the dimensionless viscosity to exactly one.
"""
function build_problem(::Type{T}, m::ChordalMesh, ph::Physical = DFG_PHYSICAL;
                       swap_inlet_outlet::Bool = false,
                       omit_cylinder::Bool = false,
                       cylinder_traction::Bool = false,
                       reverse_inlet_normal::Bool = false,
                       pin_bubbles::Bool = false,
                       muhat_one::Bool = false) where {T}
    nv = length(m.xy)
    nc = length(m.cells)
    ndof = 2 * nv + 2 * nc + nv

    sc = Mini.Scales(T, ph.L, ph.U, ph.P, ph.mu)
    muhat_one && (sc = Mini.Scales{T}(sc.L, sc.U, sc.P, sc.G, sc.Theta, one(T)))

    xh = [(T(p[1]) / T(ph.L), T(p[2]) / T(ph.L)) for p in m.xy]

    inlet_set = swap_inlet_outlet ? :outlet : :inlet
    outlet_set = swap_inlet_outlet ? :inlet : :outlet
    velocity_sets = Symbol[inlet_set, :walls]
    (omit_cylinder || cylinder_traction) || push!(velocity_sets, :cylinder)

    # Essential vertices are the closure of the velocity facets: a vertex on one
    # velocity facet stays fixed even when its other facet carries traction.
    inlet_vertices = Set{Int}()
    for f in m.names[inlet_set]
        push!(inlet_vertices, m.bfacets[f][1], m.bfacets[f][2])
    end
    ess_vertices = Set{Int}()
    for s in velocity_sets, f in m.names[s]
        push!(ess_vertices, m.bfacets[f][1], m.bfacets[f][2])
    end

    essential = Int[]
    uess = T[]
    for v in sort!(collect(ess_vertices)), c in 1:2
        val = zero(T)
        if v in inlet_vertices && c == 1
            val = inlet_speed(T, ph, m.xy[v][2]) / T(ph.U)
            reverse_inlet_normal && (val = -val)
        end
        push!(essential, 2 * (v - 1) + c)
        push!(uess, val)
    end
    if pin_bubbles
        for k in 1:nc, c in 1:2
            push!(essential, 2 * nv + 2 * (k - 1) + c)
            push!(uess, zero(T))
        end
    end
    order = sortperm(essential)
    essential = essential[order]
    uess = uess[order]
    free = setdiff(1:ndof, essential)

    traction = Tuple{Int,Int,NTuple{2,T}}[]
    tsets = Symbol[outlet_set]
    cylinder_traction && push!(tsets, :cylinder)
    for s in tsets, f in m.names[s]
        push!(traction, (m.bfacets[f][1], m.bfacets[f][2], (zero(T), zero(T))))
    end

    Problem{T}(xh, m.cells, sc, ndof, nv, nc, essential, free, uess, traction,
               Mini.duffy_gauss3(T))
end

# ------------------------------------------------------- physical observations

"Physical (coherent-SI) reconstruction of a dimensionless solution."
struct PhysicalSolution{T}
    uvert::Vector{NTuple{2,T}}      # m/s at P1 vertices
    ubub::Vector{NTuple{2,T}}       # m/s bubble coefficients (barycentre value)
    p::Vector{T}                    # Pa at P1 vertices
    reaction::Vector{NTuple{2,T}}   # N/m constraint force on the fluid
end

function physical_solution(pr::Problem{T}, ph::Physical, x::Vector{T},
                           res::Vector{T}) where {T}
    U, P, L = T(ph.U), T(ph.P), T(ph.L)
    uvert = [(U * x[vdof(pr, v, 1)], U * x[vdof(pr, v, 2)]) for v in 1:pr.nv]
    ubub = [(U * x[bdof(pr, k, 1)], U * x[bdof(pr, k, 2)]) for k in 1:pr.nc]
    pp = [P * x[pdof(pr, v)] for v in 1:pr.nv]
    rr = [(P * L * res[vdof(pr, v, 1)], P * L * res[vdof(pr, v, 2)]) for v in 1:pr.nv]
    PhysicalSolution{T}(uvert, ubub, pp, rr)
end

# -------------------------------------------------------------- selectors

"Sorted vertex-coordinate triple of a cell, the contract exact-tie key."
cell_key(m::ChordalMesh, k::Int) =
    sort([m.xy[m.cells[k][1]], m.xy[m.cells[k][2]], m.xy[m.cells[k][3]]])

"""
Cell whose physical barycentre is closest to `target`, with an exact tie broken
by the lexicographically sorted triple of vertex coordinates. Returns the cell
index and the number of cells attaining the minimum.
"""
function select_cell(m::ChordalMesh, target::NTuple{2,Float64})
    best = Inf
    for k in eachindex(m.cells)
        bx, by = barycentre(m, k)
        d = (bx - target[1])^2 + (by - target[2])^2
        d < best && (best = d)
    end
    tied = [k for k in eachindex(m.cells)
            if (barycentre(m, k)[1] - target[1])^2 +
               (barycentre(m, k)[2] - target[2])^2 == best]
    length(tied) == 1 && return tied[1], 1
    sort!(tied; by = k -> cell_key(m, k))
    tied[1], length(tied)
end

function barycentre(m::ChordalMesh, k::Int)
    a, b, c = m.cells[k]
    ((m.xy[a][1] + m.xy[b][1] + m.xy[c][1]) / 3,
     (m.xy[a][2] + m.xy[b][2] + m.xy[c][2]) / 3)
end

"""
Vertex minimizing the scalar `score`, with an exact tie broken by lexicographic
coordinate order. Returns the vertex and how many candidates attained the
minimum, so a tie is reported rather than silently resolved.
"""
function select_vertex(m::ChordalMesh, candidates::Vector{Int}, score)
    vals = [score(m.xy[v]) for v in candidates]
    best = minimum(vals)
    tied = [candidates[i] for i in eachindex(candidates) if vals[i] == best]
    sort!(tied; by = v -> m.xy[v])
    tied[1], tied
end

cylinder_vertices(m::ChordalMesh) =
    sort!(unique!(vcat([[m.bfacets[f][1], m.bfacets[f][2]] for f in m.names[:cylinder]]...)))

# ------------------------------------------------------------------ observing

"""
Frozen observations.

`velocity[i] = (target, cell, tie_multiplicity, barycentre, u)`.
`pressure[i] = (name, vertex, tied_candidates, coordinates, p)`, where
`tied_candidates` lists every vertex attaining the exact extremum together with
its coordinates and pressure, so a tie is auditable rather than hidden behind
the lexicographic tie-break.
"""
struct Observations{T}
    velocity::Vector{Tuple{NTuple{2,Float64},Int,Int,NTuple{2,Float64},NTuple{2,T}}}
    pressure::Vector{Tuple{String,Int,Vector{Tuple{Int,NTuple{2,Float64},T}},
                           NTuple{2,Float64},T}}
    flux::Dict{Symbol,T}
    reaction_cylinder_on_fluid::NTuple{2,T}
    reaction_fluid_on_cylinder::NTuple{2,T}
    reaction_all_essential::NTuple{2,T}
    body_force::NTuple{2,T}
    applied_traction::NTuple{2,T}
    balance::NTuple{2,T}
    pressure_integral::T
end

const VELOCITY_TARGETS = [(0.10, 0.20), (0.20, 0.30), (0.30, 0.20),
                          (1.00, 0.20), (2.00, 0.20)]

function observe(m::ChordalMesh, pr::Problem{T}, ph::Physical,
                 ps::PhysicalSolution{T}) where {T}
    vel = Tuple{NTuple{2,Float64},Int,Int,NTuple{2,Float64},NTuple{2,T}}[]
    for t in VELOCITY_TARGETS
        k, mult = select_cell(m, t)
        a, b, c = m.cells[k]
        # At the barycentre every barycentric coordinate is 1/3 and the
        # normalized bubble is exactly one.
        u1 = (ps.uvert[a][1] + ps.uvert[b][1] + ps.uvert[c][1]) / 3 + ps.ubub[k][1]
        u2 = (ps.uvert[a][2] + ps.uvert[b][2] + ps.uvert[c][2]) / 3 + ps.ubub[k][2]
        push!(vel, (t, k, mult, barycentre(m, k), (u1, u2)))
    end

    cyl = cylinder_vertices(m)
    outer = m.outer_loop
    probes = [("cylinder_min_x", cyl, p -> p[1]),
              ("cylinder_max_x", cyl, p -> -p[1]),
              ("cylinder_min_y", cyl, p -> p[2]),
              ("cylinder_max_y", cyl, p -> -p[2]),
              ("outer_near_inlet_mid", outer, p -> (p[1] - 0.0)^2 + (p[2] - 0.20)^2),
              ("outer_near_outlet_mid", outer, p -> (p[1] - 2.2)^2 + (p[2] - 0.20)^2)]
    pres = Tuple{String,Int,Vector{Tuple{Int,NTuple{2,Float64},T}},NTuple{2,Float64},T}[]
    for (name, cand, score) in probes
        v, tied = select_vertex(m, collect(cand), score)
        push!(pres, (name, v, [(t, m.xy[t], ps.p[t]) for t in tied], m.xy[v], ps.p[v]))
    end

    flux = Dict{Symbol,T}()
    for s in (:inlet, :outlet, :walls, :cylinder)
        flux[s] = signed_flux(m, ps, m.names[s])
    end

    rc = sum_reaction(ps, cyl)
    ess_vertices = sort!(unique!([(d + 1) ÷ 2 for d in pr.essential if d <= 2 * pr.nv]))
    ress = sum_reaction(ps, ess_vertices)
    body = (zero(T), zero(T))
    trac = (zero(T), zero(T))
    for (a, b, t) in pr.traction
        len = hypot(m.xy[b][1] - m.xy[a][1], m.xy[b][2] - m.xy[a][2])
        trac = (trac[1] + T(len) * T(ph.P) * t[1], trac[2] + T(len) * T(ph.P) * t[2])
    end
    bal = (ress[1] + body[1] + trac[1], ress[2] + body[2] + trac[2])

    Observations{T}(vel, pres, flux, rc, (-rc[1], -rc[2]), ress, body, trac, bal,
                    pressure_integral(m, pr, ps))
end

function signed_flux(m::ChordalMesh, ps::PhysicalSolution{T}, facets) where {T}
    tot = zero(T)
    for f in facets
        a, b, _ = m.bfacets[f]
        n, len = facet_normal(m.xy, a, b)
        um = ((ps.uvert[a][1] + ps.uvert[b][1]) / 2, (ps.uvert[a][2] + ps.uvert[b][2]) / 2)
        tot += T(len) * (um[1] * T(n[1]) + um[2] * T(n[2]))
    end
    tot
end

sum_reaction(ps::PhysicalSolution{T}, vs) where {T} =
    (sum(T[ps.reaction[v][1] for v in vs]), sum(T[ps.reaction[v][2] for v in vs]))

"Integral of the P1 pressure over the mesh, `sum_K |K| * mean(p)`."
function pressure_integral(m::ChordalMesh, pr::Problem{T}, ps::PhysicalSolution{T}) where {T}
    tot = zero(T)
    for (k, c) in enumerate(m.cells)
        area = T(signed_area(m.xy[c[1]], m.xy[c[2]], m.xy[c[3]]))
        tot += area * (ps.p[c[1]] + ps.p[c[2]] + ps.p[c[3]]) / 3
    end
    tot
end

# ------------------------------------------------------- reindexing invariance

"""
Renumber vertices and cells and rotate every cell's stored vertex order by one
position. Geometry, connectivity and orientation are unchanged, so every
geometric observation must be unchanged too.
"""
function permute(m::ChordalMesh)
    nv = length(m.xy)
    nc = length(m.cells)
    pv = [mod(7 * (v - 1) + 3, nv) + 1 for v in 1:nv]        # bijection, gcd(7,104)=1
    pc = [nc + 1 - k for k in 1:nc]
    inv_v = zeros(Int, nv)
    for v in 1:nv
        inv_v[pv[v]] = v
    end
    xy = Vector{NTuple{2,Float64}}(undef, nv)
    for v in 1:nv
        xy[pv[v]] = m.xy[v]
    end
    cells = Vector{NTuple{3,Int}}(undef, nc)
    for k in 1:nc
        c = m.cells[k]
        cells[pc[k]] = (pv[c[2]], pv[c[3]], pv[c[1]])        # cyclic: orientation kept
    end
    bfacets = [(pv[f[1]], pv[f[2]], pc[f[3]]) for f in m.bfacets]
    names = Dict(k => copy(v) for (k, v) in m.names)
    ChordalMesh(xy, cells, bfacets, copy(m.facet_side), names,
                [pv[v] for v in m.inner], [pv[v] for v in m.outer],
                [pv[v] for v in m.corners], [pv[v] for v in m.outer_loop], m.nseg)
end

end # module
