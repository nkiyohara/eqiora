# Independent reconstruction of the RFC 0081 exact circular-hole source and the
# RFC 0082 source-bound chordal reference mesh (50 chords, 104 vertices,
# 104 positively oriented affine triangles).
#
# Nothing here reads a mesh authored elsewhere: every coordinate is generated
# from the public construction and every count is rechecked, not assumed.

module Geometry

export SourceGeometry, ChordalMesh, DFG_SOURCE
export build_mesh, ideal_metrics, measured_metrics, geometric_digest
export signed_area, facet_normal, boundary_angle

# ---------------------------------------------------------------- exact source

"RFC 0081 exact geometry: axis-aligned rectangle with one interior circular hole."
struct SourceGeometry
    xlo::Float64
    xhi::Float64
    ylo::Float64
    yhi::Float64
    cx::Float64
    cy::Float64
    r::Float64
    tol::Float64
end

const DFG_SOURCE = SourceGeometry(0.0, 2.2, 0.0, 0.41, 0.2, 0.2, 0.05, 1e-12)

"""
Chordal reference mesh.

`bfacets[k] = (a, b, cell)` is a directed boundary edge `a -> b` that occurs in
that cyclic order inside the positively oriented `cell`; the parent-outward
normal is therefore `rot(-90 deg)` of `b - a`.
"""
struct ChordalMesh
    xy::Vector{NTuple{2,Float64}}
    cells::Vector{NTuple{3,Int}}
    bfacets::Vector{NTuple{3,Int}}
    facet_side::Vector{Symbol}          # :xlo :xhi :ylo :yhi :circle
    names::Dict{Symbol,Vector{Int}}     # named set -> indices into bfacets
    inner::Vector{Int}                  # ray index (1-based) -> circle vertex
    outer::Vector{Int}                  # ray index (1-based) -> rectangle hit
    corners::Vector{Int}                # 4 corner vertices, lexicographic order
    outer_loop::Vector{Int}             # 54 outer-boundary vertices, CCW
    nseg::Int
end

# ------------------------------------------------------------------ primitives

signed_area(p, q, r) =
    ((q[1] - p[1]) * (r[2] - p[2]) - (q[2] - p[2]) * (r[1] - p[1])) / 2

"Boundary angle of `p` about the circle centre, normalized to `[0, 2pi)`."
function boundary_angle(g::SourceGeometry, p)
    a = atan(p[2] - g.cy, p[1] - g.cx)
    a < 0 ? a + 2 * pi : a
end

"""
Cast one circular direction from the centre to the rectangle.

The cast-axis coordinate is assigned the exact rectangle bound; only the
transverse coordinate is reconstructed. The algebraically equal but
rounding-sensitive `c + ((bound - c)/d)*d` spelling is forbidden by RFC 0082.
"""
function ray_hit(g::SourceGeometry, ct::Float64, st::Float64)
    tx = ct > 0 ? (g.xhi - g.cx) / ct : (ct < 0 ? (g.xlo - g.cx) / ct : Inf)
    ty = st > 0 ? (g.yhi - g.cy) / st : (st < 0 ? (g.ylo - g.cy) / st : Inf)
    if tx <= ty
        xb = ct > 0 ? g.xhi : g.xlo
        return (xb, g.cy + tx * st), (ct > 0 ? :xhi : :xlo)
    else
        yb = st > 0 ? g.yhi : g.ylo
        return (g.cx + ty * ct, yb), (st > 0 ? :yhi : :ylo)
    end
end

"Unit parent-outward normal of the directed boundary edge `a -> b`."
function facet_normal(xy, a::Int, b::Int)
    dx = xy[b][1] - xy[a][1]
    dy = xy[b][2] - xy[a][2]
    len = hypot(dx, dy)
    (dy / len, -dx / len), len
end

# ---------------------------------------------------------------- construction

"""
    build_mesh(g, n; diagonal = :OiIj, trig_ulp = 0)

Reconstruct the chordal mesh with `n` circular segments.

`diagonal` selects the shared quad diagonal for adjacent rays `i`, `j`:
`:OiIj` is the frozen RFC 0082 choice `O_i--I_j` with cells `(O_i,O_j,I_j)` and
`(O_i,I_j,I_i)`; `:OjIi` is the falsifier alternative `O_j--I_i`.

`trig_ulp != 0` nudges every `cos`/`sin` value by one unit in the last place in
a fixed alternating pattern. Two independent routes may disagree by up to one
ulp on `cos`/`sin` because libm results are implementation-defined; this switch
lets the driver measure how far that can move a frozen observation instead of
assuming it cannot.
"""
function build_mesh(g::SourceGeometry = DFG_SOURCE, n::Int = 50;
                    diagonal::Symbol = :OiIj, trig_ulp::Int = 0)
    n >= 8 || error("RFC 0082 requires at least eight circular segments")
    diagonal in (:OiIj, :OjIi) || error("unknown diagonal $diagonal")

    nudge(v, k) = trig_ulp == 0 ? v :
                  (iseven(k) ? nextfloat(v, trig_ulp) : prevfloat(v, trig_ulp))
    theta = [2 * pi * (i - 1) / n for i in 1:n]
    ct = [nudge(cos(theta[i]), i) for i in 1:n]
    st = [nudge(sin(theta[i]), i + 1) for i in 1:n]

    xy = NTuple{2,Float64}[]
    inner = Int[]
    for i in 1:n
        push!(xy, (g.cx + g.r * ct[i], g.cy + g.r * st[i]))
        push!(inner, length(xy))
    end

    outer = Int[]
    hit_side = Symbol[]
    for i in 1:n
        p, side = ray_hit(g, ct[i], st[i])
        push!(xy, p)
        push!(outer, length(xy))
        push!(hit_side, side)
    end

    corner_xy = [(g.xlo, g.ylo), (g.xlo, g.yhi), (g.xhi, g.ylo), (g.xhi, g.yhi)]
    corners = Int[]
    for c in corner_xy
        # A radial hit within the source classification tolerance would have to
        # reuse the exact corner; independently verify that never happens here.
        for i in 1:n
            hypot(xy[outer[i]][1] - c[1], xy[outer[i]][2] - c[2]) > g.tol ||
                error("ray hit coincides with a rectangle corner within tolerance")
        end
        push!(xy, c)
        push!(corners, length(xy))
    end

    # Assign each corner to the ray gap that strictly contains its boundary angle.
    gap_corners = [Int[] for _ in 1:n]
    for c in corners
        a = boundary_angle(g, xy[c])
        placed = false
        for i in 1:n
            lo = theta[i]
            hi = i < n ? theta[i+1] : 2 * pi
            if lo < a < hi
                push!(gap_corners[i], c)
                placed = true
                break
            end
        end
        placed || error("rectangle corner is not strictly inside any ray gap")
    end
    for i in 1:n
        sort!(gap_corners[i]; by = c -> boundary_angle(g, xy[c]))
    end

    cells = NTuple{3,Int}[]
    bfacets = NTuple{3,Int}[]
    facet_side = Symbol[]

    for i in 1:n
        j = i == n ? 1 : i + 1
        Oi, Oj, Ii, Ij = outer[i], outer[j], inner[i], inner[j]

        if diagonal === :OiIj
            c1 = (Oi, Oj, Ij)      # outer edge O_i -> O_j, chord edge nowhere
            c2 = (Oi, Ij, Ii)      # chord edge I_j -> I_i
        else
            c1 = (Oi, Oj, Ii)      # falsifier: diagonal O_j--I_i
            c2 = (Oj, Ij, Ii)
        end
        push!(cells, c1)
        push!(cells, c2)
        outer_cell = length(cells) - 1
        chord_cell = length(cells)

        # Outer side of the quad: boundary when no corner falls in this gap,
        # otherwise the base of a deterministic fan anchored at O_i.
        if isempty(gap_corners[i])
            hit_side[i] === hit_side[j] ||
                error("outer chord spans two rectangle sides without a corner")
            push!(bfacets, (Oi, Oj, outer_cell))
            push!(facet_side, hit_side[i])
        else
            seq = vcat(Oi, gap_corners[i], Oj)      # length k + 2
            k = length(gap_corners[i])
            fan_base = length(cells)                # cells fan_base+1 .. fan_base+k
            for m in 2:(k+1)
                push!(cells, (seq[1], seq[m], seq[m+1]))
            end
            for m in 1:(k+1)
                a, b = seq[m], seq[m+1]
                cid = fan_base + (m == 1 ? 1 : m - 1)
                tri = cells[cid]
                any(t -> tri[t] == a && tri[mod1(t + 1, 3)] == b, 1:3) ||
                    error("fan boundary edge is not carried by its own cell")
                push!(bfacets, (a, b, cid))
                push!(facet_side, side_of(g, xy[a], xy[b]))
            end
        end

        # Circle chord, traversed clockwise so its normal points into the hole.
        push!(bfacets, (Ij, Ii, chord_cell))
        push!(facet_side, :circle)
    end

    outer_loop = Int[]
    for i in 1:n
        push!(outer_loop, outer[i])
        append!(outer_loop, gap_corners[i])
    end

    names = Dict{Symbol,Vector{Int}}(
        :inlet => findall(==(:xlo), facet_side),
        :outlet => findall(==(:xhi), facet_side),
        :walls => findall(s -> s === :ylo || s === :yhi, facet_side),
        :cylinder => findall(==(:circle), facet_side),
    )

    ChordalMesh(xy, cells, bfacets, facet_side, names, inner, outer, corners,
                outer_loop, n)
end

"Name the exact rectangle side that carries a collinear outer edge."
function side_of(g::SourceGeometry, a, b)
    a[1] == g.xlo && b[1] == g.xlo && return :xlo
    a[1] == g.xhi && b[1] == g.xhi && return :xhi
    a[2] == g.ylo && b[2] == g.ylo && return :ylo
    a[2] == g.yhi && b[2] == g.yhi && return :yhi
    error("outer edge is not exactly collinear with a rectangle side")
end

# --------------------------------------------------------------------- metrics

"""
RFC 0082 ideal closed forms, evaluated in the requested precision.

The radius is taken as an exact rational so the frozen RFC digits are
reproduced; passing the binary64 radius instead shifts `area_deficit` by about
`2.3e-21 m^2`, which is exactly twice the binary64 representation error of the
radius and is reported separately by the driver.
"""
function ideal_metrics(::Type{T}, r::Rational, n::Int) where {T}
    R = T(numerator(r)) / T(denominator(r))
    N = T(n)
    pi_ = T(pi)
    (sagitta = 2 * R * sin(pi_ / (2 * N))^2,
     area_deficit = pi_ * R^2 - (N / 2) * R^2 * sin(2 * pi_ / N),
     perimeter_deficit = 2 * pi_ * R - 2 * N * R * sin(pi_ / N))
end

"Measure the generated binary64 loop rather than trusting the closed form."
function measured_metrics(g::SourceGeometry, m::ChordalMesh)
    n = m.nseg
    dmin = Inf
    rmax = 0.0
    per = 0.0
    area2 = 0.0
    for i in 1:n
        j = i == n ? 1 : i + 1
        a = m.xy[m.inner[i]]
        b = m.xy[m.inner[j]]
        rmax = max(rmax, hypot(a[1] - g.cx, a[2] - g.cy))
        ex, ey = b[1] - a[1], b[2] - a[2]
        L2 = ex^2 + ey^2
        per += sqrt(L2)
        t = clamp(((g.cx - a[1]) * ex + (g.cy - a[2]) * ey) / L2, 0.0, 1.0)
        px, py = a[1] + t * ex, a[2] + t * ey
        dmin = min(dmin, hypot(px - g.cx, py - g.cy))
        area2 += a[1] * b[2] - b[1] * a[2]
    end
    (dmin = dmin, rmax = rmax, hausdorff = max(g.r - dmin, rmax - g.r),
     polygon_area = area2 / 2, polygon_perimeter = per)
end

# ---------------------------------------------------------- index-free digest

"""
Canonical, index-order-independent description of the mesh.

Vertices are emitted in lexicographic coordinate order; each cell is emitted as
its lexicographically sorted vertex-coordinate triple, and the cell list is
sorted. Two meshes that differ only by renumbering therefore produce identical
text, which is what makes route-to-route mesh comparison index-free.
"""
function geometric_digest(m::ChordalMesh)
    io = IOBuffer()
    for p in sort(m.xy)
        print(io, "V ", repr(p[1]), " ", repr(p[2]), "\n")
    end
    tri = String[]
    for c in m.cells
        t = sort([m.xy[c[1]], m.xy[c[2]], m.xy[c[3]]])
        push!(tri, string("C ", repr(t[1][1]), " ", repr(t[1][2]), " ",
                          repr(t[2][1]), " ", repr(t[2][2]), " ",
                          repr(t[3][1]), " ", repr(t[3][2])))
    end
    for s in sort(tri)
        print(io, s, "\n")
    end
    fac = String[]
    for (k, f) in enumerate(m.bfacets)
        p, q = m.xy[f[1]], m.xy[f[2]]
        push!(fac, string("F ", m.facet_side[k], " ", repr(p[1]), " ", repr(p[2]),
                          " ", repr(q[1]), " ", repr(q[2])))
    end
    for s in sort(fac)
        print(io, s, "\n")
    end
    String(take!(io))
end

end # module
