# Falsifier battery and exact structural assertions.
#
# Each falsifier is a deliberate defect. It is recorded as DETECTED only when a
# frozen observation moves by more than the contract route-to-route tolerance,
# or when a structural precondition rejects. Where a defect is provably NOT
# detectable by a given observation, that is recorded too: an honest falsifier
# battery has to state what it does not catch.

module Falsify

using Printf
using ..Geometry
using ..Mini
using ..Witness

export Deviation, deviate, structural_checks, mesh_checks, gauge_falsifier

# contract route-to-route tolerances: absolute_floor + 2e-10 * physical_scale.
const TOL_VELOCITY = 2e-12 + 2e-10 * 0.3
const TOL_PRESSURE = 2e-14 + 2e-10 * 0.0007317073170731707
const TOL_FLUX = 2e-13 + 2e-10 * 0.123
const TOL_REACTION = 2e-14 + 2e-10 * 0.0003

struct Deviation
    velocity::Float64
    pressure::Float64
    flux::Float64
    reaction::Float64
    balance::Float64
    flux_balance::Float64
    selectors_moved::Bool
end

detected(d::Deviation) =
    d.selectors_moved || d.velocity > TOL_VELOCITY || d.pressure > TOL_PRESSURE ||
    d.flux > TOL_FLUX || d.reaction > TOL_REACTION
export detected

"Largest tolerance-relative deviation between two observation sets."
function deviate(a::Observations{S}, b::Observations{U}) where {S,U}
    moved = false
    v = 0.0
    for i in eachindex(a.velocity)
        # Compare the selected cell geometrically, never by index: renumbering
        # must be invisible here.
        a.velocity[i][4] == b.velocity[i][4] || (moved = true)
        for c in 1:2
            v = max(v, abs(Float64(a.velocity[i][5][c]) - Float64(b.velocity[i][5][c])))
        end
    end
    p = 0.0
    for i in eachindex(a.pressure)
        a.pressure[i][4] == b.pressure[i][4] || (moved = true)
        p = max(p, abs(Float64(a.pressure[i][5]) - Float64(b.pressure[i][5])))
    end
    f = maximum(abs(Float64(a.flux[s]) - Float64(b.flux[s])) for s in (:inlet, :outlet))
    r = max(abs(Float64(a.reaction_cylinder_on_fluid[1]) - Float64(b.reaction_cylinder_on_fluid[1])),
            abs(Float64(a.reaction_cylinder_on_fluid[2]) - Float64(b.reaction_cylinder_on_fluid[2])))
    bal = max(abs(Float64(b.balance[1])), abs(Float64(b.balance[2])))
    fb = abs(Float64(b.flux[:inlet]) + Float64(b.flux[:outlet]))
    Deviation(v, p, f, r, bal, fb, moved)
end

# --------------------------------------------------------------- mesh checks

"""
Recheck every source-policy and topology fact of the reconstructed mesh
independently of the numbers quoted in the frozen contract and RFC 0082.
"""
function mesh_checks(g::SourceGeometry, m::ChordalMesh)
    out = Tuple{String,Bool,String}[]
    add(n, ok, d = "") = push!(out, (n, ok, string(d)))

    add("mesh.vertices_104", length(m.xy) == 104, length(m.xy))
    add("mesh.cells_104", length(m.cells) == 104, length(m.cells))
    add("mesh.boundary_facets_104", length(m.bfacets) == 104, length(m.bfacets))
    add("mesh.outer_loop_54", length(m.outer_loop) == 54, length(m.outer_loop))
    add("mesh.segments_50", m.nseg == 50, m.nseg)
    add("mesh.inlet_facets_14", length(m.names[:inlet]) == 14, length(m.names[:inlet]))
    add("mesh.outlet_facets_2", length(m.names[:outlet]) == 2, length(m.names[:outlet]))
    add("mesh.wall_facets_38", length(m.names[:walls]) == 38, length(m.names[:walls]))
    add("mesh.cylinder_facets_50", length(m.names[:cylinder]) == 50, length(m.names[:cylinder]))

    covered = sort!(vcat(m.names[:inlet], m.names[:outlet], m.names[:walls], m.names[:cylinder]))
    add("mesh.partition_complete_once", covered == collect(1:length(m.bfacets)),
        "$(length(covered)) of $(length(m.bfacets))")

    areas = [signed_area(m.xy[c[1]], m.xy[c[2]], m.xy[c[3]]) for c in m.cells]
    add("mesh.all_cells_positive", all(>(0), areas), minimum(areas))
    add("mesh.area_matches_source",
        abs(sum(areas) - (2.2 * 0.41 - measured_metrics(g, m).polygon_area)) < 1e-15,
        sum(areas))

    # Every mesh edge is used once (boundary) or twice (interior), never more.
    used = Dict{Tuple{Int,Int},Int}()
    for c in m.cells, t in 1:3
        e = minmax(c[t], c[mod1(t + 1, 3)])
        used[e] = get(used, e, 0) + 1
    end
    nb = count(==(1), values(used))
    ni = count(==(2), values(used))
    add("mesh.manifold_edges", nb + ni == length(used), "$(length(used)) edges")
    add("mesh.boundary_edges_104", nb == 104, nb)
    add("mesh.interior_edges_104", ni == 104, ni)
    add("mesh.euler_characteristic_0",
        length(m.xy) - length(used) + length(m.cells) == 0,
        length(m.xy) - length(used) + length(m.cells))

    # Every declared boundary facet really is an unshared cell edge, traversed in
    # its owning cell's cyclic order.
    ok_dir = true
    for (a, b, k) in m.bfacets
        tri = m.cells[k]
        any(t -> tri[t] == a && tri[mod1(t + 1, 3)] == b, 1:3) || (ok_dir = false)
        get(used, minmax(a, b), 0) == 1 || (ok_dir = false)
    end
    add("mesh.facets_are_oriented_boundary_edges", ok_dir)

    # Named-set normals: inlet -1x, outlet +1x, cylinder inward to the centre.
    nrm(f) = facet_normal(m.xy, m.bfacets[f][1], m.bfacets[f][2])[1]
    add("mesh.inlet_normal_is_minus_x",
        all(f -> nrm(f) == (-1.0, 0.0), m.names[:inlet]))
    add("mesh.outlet_normal_is_plus_x",
        all(f -> nrm(f) == (1.0, 0.0), m.names[:outlet]))
    add("mesh.cylinder_normal_points_into_hole",
        all(m.names[:cylinder]) do f
            a, b, _ = m.bfacets[f]
            n = nrm(f)
            mx = (m.xy[a][1] + m.xy[b][1]) / 2 - g.cx
            my = (m.xy[a][2] + m.xy[b][2]) / 2 - g.cy
            n[1] * mx + n[2] * my < 0
        end)
    add("mesh.wall_normals_are_pm_y",
        all(f -> nrm(f) in ((0.0, 1.0), (0.0, -1.0)), m.names[:walls]))

    # RFC 0082 approximation contract, measured not assumed.
    mm = measured_metrics(g, m)
    allowance = 128 * eps(Float64) * 2.2
    accepted = mm.hausdorff + allowance
    add("mesh.boundary_error_within_1e-4", accepted <= 1e-4, accepted)
    id49 = ideal_metrics(BigFloat, 1 // 20, 49)
    add("mesh.49_segments_would_exceed", Float64(id49.sagitta) > 1e-4 - allowance,
        Float64(id49.sagitta))
    add("mesh.evaluation_allowance", allowance == 6.252776074688882e-14, allowance)

    # Frozen RFC 0082 ideal digits, reproduced from the exact decimal radius.
    id50 = ideal_metrics(BigFloat, 1 // 20, 50)
    frozen = (sagitta49 = big"1.0273036248318289955797595210037224856637053318839e-4",
              sagitta50 = big"9.8663578586421902383159656827472333154739014922844e-5",
              dA50 = big"2.0654536205467760336685969666957589060533063430286e-5",
              dP50 = big"2.0666771241244346537321549979462280729278040417922e-4")
    # RFC 0082 quotes ~50 significant digits, so compare relatively at 1e-45:
    # far tighter than any transcription slip, far looser than the quoted width.
    rel(a, b) = Float64(abs(a - b) / abs(b))
    for (nm, got, want) in (("sagitta49", id49.sagitta, frozen.sagitta49),
                            ("sagitta50", id50.sagitta, frozen.sagitta50),
                            ("area_deficit50", id50.area_deficit, frozen.dA50),
                            ("perimeter_deficit50", id50.perimeter_deficit, frozen.dP50))
        add("rfc0082.$nm", rel(got, want) < 1e-45, @sprintf("rel diff %.3e", rel(got, want)))
    end
    out
end

# ---------------------------------------------------------- structural checks

"""
Exact structural assertions. These are equality/absence facts, never
floating-point comparisons: the pressure reference is `BoundaryTraction` and no
gauge row, column, multiplier or scale exists.
"""
function structural_checks(m::ChordalMesh, pr::Problem{T}, A::Matrix{T},
                           b::Vector{T}) where {T}
    out = Tuple{String,Bool,String}[]
    add(n, ok, d = "") = push!(out, (n, ok, string(d)))

    nv, nc = pr.nv, pr.nc
    add("dof.layout_exact", pr.ndof == 2 * nv + 2 * nc + nv, pr.ndof)
    add("dof.no_gauge_row", size(A, 1) == 2 * nv + 2 * nc + nv, size(A, 1))
    add("dof.no_gauge_column", size(A, 2) == pr.ndof, size(A, 2))
    add("dof.p1_velocity_208", 2 * nv == 208, 2 * nv)
    add("dof.bubble_208", 2 * nc == 208, 2 * nc)
    add("dof.pressure_104", nv == 104, nv)
    add("dof.essential_206", length(pr.essential) == 206, length(pr.essential))
    add("dof.reduced_314", length(pr.free) == 314, length(pr.free))

    ess_vertices = unique([(d + 1) ÷ 2 for d in pr.essential if d <= 2 * nv])
    add("trace.essential_vertices_103", length(ess_vertices) == 103, length(ess_vertices))
    free_v = setdiff(1:nv, ess_vertices)
    add("trace.single_free_vertex", length(free_v) == 1, length(free_v))
    add("trace.free_vertex_is_outlet_midpoint",
        length(free_v) == 1 && m.xy[free_v[1]] == (2.2, 0.2),
        length(free_v) == 1 ? string(m.xy[free_v[1]]) : "n/a")
    add("trace.all_vertices_on_boundary",
        length(unique(vcat([[f[1], f[2]] for f in m.bfacets]...))) == nv)
    add("trace.bubbles_are_free",
        all(d -> d in pr.free, [2 * nv + i for i in 1:(2*nc)]))

    # A vertex shared by an essential and an outlet facet stays fixed while the
    # outlet facet keeps its full-system traction action.
    outlet_v = unique(vcat([[m.bfacets[f][1], m.bfacets[f][2]] for f in m.names[:outlet]]...))
    shared = intersect(outlet_v, ess_vertices)
    add("trace.outlet_corners_still_fixed", length(shared) == 2, length(shared))
    add("trace.outlet_facets_still_in_traction_partition",
        length(pr.traction) == 2, length(pr.traction))

    add("assembly.exact_symmetry",
        all(A[i, j] == A[j, i] for i in 1:pr.ndof for j in 1:i), "")
    add("load.zero_rhs_body_and_traction", all(iszero, b), maximum(abs, b))

    # Facet loads must never reach bubble, pressure or interior rows: probe with
    # a nonzero traction and check where it lands.
    probe = Problem{T}(pr.xh, pr.cells, pr.sc, pr.ndof, nv, nc, pr.essential,
                       pr.free, pr.uess,
                       [(a, bb, (one(T), one(T))) for (a, bb, _) in pr.traction],
                       pr.quad)
    _, bp = assemble(probe)
    touched = findall(!iszero, bp)
    add("load.facet_load_only_on_facet_p1_rows",
        all(d -> d <= 2 * nv, touched) &&
        sort(unique([(d + 1) ÷ 2 for d in touched])) == sort(outlet_v),
        length(touched))
    out
end

# ------------------------------------------------------------ gauge falsifier

"""
Add the forbidden `ZeroIntegral(pressure)` gauge alongside the nonempty outlet
traction partition and measure the damage.

RFC 0047 makes the gauge absent for this boundary family because the traction
boundary already fixes the constant pressure mode. Returns the multiplier and
the induced pressure shift.
"""
function gauge_falsifier(m::ChordalMesh, pr::Problem{T}, A::Matrix{T}, b::Vector{T},
                         ph::Physical, base::Observations{T}) where {T}
    n = pr.ndof
    mvec = zeros(T, n)
    q = pr.quad
    for c in pr.cells
        x0, x1, x2 = pr.xh[c[1]], pr.xh[c[2]], pr.xh[c[3]]
        det = (x1[1] - x0[1]) * (x2[2] - x0[2]) - (x2[1] - x0[1]) * (x1[2] - x0[2])
        for i in eachindex(q.w)
            phi, _ = Mini.mini_basis(q.xi[i], q.eta[i])
            for k in 1:3
                mvec[pdof(pr, c[k])] += q.w[i] * det * phi[k]
            end
        end
    end
    Ag = zeros(T, n + 1, n + 1)
    Ag[1:n, 1:n] = A
    Ag[1:n, n+1] = mvec
    Ag[n+1, 1:n] = mvec
    bg = vcat(b, zero(T))
    free = vcat(pr.free, n + 1)
    xg = zeros(T, n + 1)
    for (i, d) in enumerate(pr.essential)
        xg[d] = pr.uess[i]
    end
    xg[free] = Ag[free, free] \ (bg[free] - Ag[free, pr.essential] * pr.uess)
    (gamma = xg[n+1],
     probe_shift = maximum(abs(T(ph.P) * xg[pdof(pr, p[2])] - p[5]) for p in base.pressure))
end

end # module
