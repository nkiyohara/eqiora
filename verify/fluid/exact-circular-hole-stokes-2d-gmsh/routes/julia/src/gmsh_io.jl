module GmshIO

using SHA

export BoundaryReceipt, Mesh, read_boundary_receipt, run_gmsh, read_msh
export signed_area2, boundary_groups, topology_summary, reindex_mesh
export boundary_mapping_sha256, curve_facet_multiplicities

const ACCEPTED_GEO_SHA256 =
    "81c96068891d6b506827339cd6fecf07eafcb867c76f01747c35d134167d367e"
const ACCEPTED_MSH_SHA256 =
    "ab7340cec1976f713b5c5deab76fc7d554593126f1c1cd68cc021749911a206a"
const EXACT_SOURCE_SHA256 =
    "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
export ACCEPTED_GEO_SHA256, ACCEPTED_MSH_SHA256, EXACT_SOURCE_SHA256

struct BoundaryReceipt
    points::Vector{NTuple{2,Float64}}
    geo_sha256::String
end

struct Mesh
    points::Vector{NTuple{2,Float64}}
    node_tags::Vector{Int}
    triangles::Vector{NTuple{3,Int}}
    triangle_tags::Vector{Int}
    boundary_edges::Vector{NTuple{2,Int}}
    boundary_element_tags::Vector{Int}
    boundary_curve_tags::Vector{Int}
end

signed_area2(p, q, r) =
    (q[1] - p[1]) * (r[2] - p[2]) - (q[2] - p[2]) * (r[1] - p[1])

function loop_area(points)
    sum(points[i][1] * points[mod1(i + 1, length(points))][2] -
        points[mod1(i + 1, length(points))][1] * points[i][2]
        for i in eachindex(points)) / 2
end

"""Read the sealed GEO without interpreting or reconstructing its coordinates."""
function read_boundary_receipt(path::AbstractString)
    bytes = read(path)
    digest = bytes2hex(sha256(bytes))
    digest == ACCEPTED_GEO_SHA256 || error("accepted GEO digest mismatch: $digest")
    text = String(bytes)
    number = raw"[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?"
    point = Regex("^Point\\((\\d+)\\) = \\{($number), ($number), ($number)\\};\$")
    points = NTuple{2,Float64}[]
    tags = Int[]
    for line in eachline(IOBuffer(text))
        m = match(point, line)
        isnothing(m) && continue
        push!(tags, parse(Int, m.captures[1]))
        parse(Float64, m.captures[4]) == 0.0 || error("nonplanar GEO point")
        push!(points, (parse(Float64, m.captures[2]), parse(Float64, m.captures[3])))
    end
    tags == collect(1:104) || error("GEO point tags are not exactly 1:104")
    length(points) == 104 || error("expected 104 GEO points, got $(length(points))")
    loop_area(points[1:50]) < 0 || error("hole GEO traversal is not clockwise")
    loop_area(points[51:104]) > 0 || error("outer receipt is not counterclockwise")
    count(line -> startswith(line, "Point("), split(text, '\n')) == 104 ||
        error("a Point has a mesh-size field or malformed spelling")
    BoundaryReceipt(points, digest)
end

curve_facet_multiplicities(mesh::Mesh) =
    Dict(tag => count(==(tag), mesh.boundary_curve_tags)
         for tag in sort!(unique(mesh.boundary_curve_tags)))

function boundary_mapping_sha256(mesh::Mesh)
    records = String[]
    for i in eachindex(mesh.boundary_edges)
        a, b = mesh.boundary_edges[i]
        pa, pb = mesh.points[a], mesh.points[b]
        endpoints = sort((pa, pb))
        push!(records, join((mesh.boundary_curve_tags[i], mesh.boundary_element_tags[i],
                             repr(endpoints[1][1]), repr(endpoints[1][2]),
                             repr(endpoints[2][1]), repr(endpoints[2][2])), " "))
    end
    bytes2hex(sha256(join(sort(records), "\n") * "\n"))
end

function boundary_groups(receipt::BoundaryReceipt)
    outer = receipt.points[51:104]
    groups = Dict(:inlet => Int[], :outlet => Int[], :walls => Int[], :cylinder => collect(1:50))
    for i in eachindex(outer)
        a, b = outer[i], outer[mod1(i + 1, length(outer))]
        tag = 50 + i
        if a[1] == 0.0 && b[1] == 0.0
            push!(groups[:inlet], tag)
        elseif a[1] == 2.2 && b[1] == 2.2
            push!(groups[:outlet], tag)
        elseif (a[2] == 0.0 && b[2] == 0.0) || (a[2] == 0.41 && b[2] == 0.41)
            push!(groups[:walls], tag)
        else
            error("outer receipt edge $i does not lie on one rectangle side")
        end
    end
    groups
end

function run_gmsh(gmsh::AbstractString, geo::AbstractString, msh::AbstractString)
    version = strip(read(`$gmsh --version`, String))
    version == "4.15.2" || error("expected exact Gmsh 4.15.2, got $version")
    run(`$gmsh $geo -2 -o $msh -v 0`)
    version
end

function section_lines(lines, name)
    firstline = findfirst(==(string('$', name)), lines)
    lastline = findfirst(==(string("\$End", name)), lines)
    isnothing(firstline) && error("missing \$$name")
    isnothing(lastline) && error("missing \$End$name")
    lines[firstline+1:lastline-1]
end

function read_msh(path::AbstractString)
    lines = readlines(path)
    meshformat = section_lines(lines, "MeshFormat")
    split(meshformat[1]) == ["4.1", "0", "8"] || error("expected ASCII MSH 4.1")

    node_tokens = split(join(section_lines(lines, "Nodes"), ' '))
    at = 1
    takeint() = (v = parse(Int, node_tokens[at]); at += 1; v)
    takefloat() = (v = parse(Float64, node_tokens[at]); at += 1; v)
    nblocks, nnodes, min_tag, max_tag = takeint(), takeint(), takeint(), takeint()
    tags = Int[]
    coords = Dict{Int,NTuple{2,Float64}}()
    for _ in 1:nblocks
        dim, _, parametric, count = takeint(), takeint(), takeint(), takeint()
        blocktags = [takeint() for _ in 1:count]
        append!(tags, blocktags)
        for tag in blocktags
            x, y, z = takefloat(), takefloat(), takefloat()
            z == 0.0 || error("nonplanar node $tag")
            for _ in 1:(parametric == 0 ? 0 : dim)
                takefloat()
            end
            coords[tag] = (x, y)
        end
    end
    at == length(node_tokens) + 1 || error("unconsumed node tokens")
    length(coords) == nnodes || error("node count mismatch")
    minimum(tags) == min_tag && maximum(tags) == max_tag || error("node tag range mismatch")
    sort!(tags)
    tag_to_index = Dict(tag => i for (i, tag) in enumerate(tags))
    points = [coords[tag] for tag in tags]

    element_tokens = split(join(section_lines(lines, "Elements"), ' '))
    at = 1
    takeelemint() = (v = parse(Int, element_tokens[at]); at += 1; v)
    eblocks, nelements, min_etag, max_etag =
        takeelemint(), takeelemint(), takeelemint(), takeelemint()
    seen_element_tags = Int[]
    triangles = NTuple{3,Int}[]
    triangle_tags = Int[]
    edges = NTuple{2,Int}[]
    edge_tags = Int[]
    curve_tags = Int[]
    nodes_per_type = Dict(15 => 1, 1 => 2, 2 => 3)
    for _ in 1:eblocks
        dim, entity, etype, count = takeelemint(), takeelemint(), takeelemint(), takeelemint()
        haskey(nodes_per_type, etype) || error("unsupported element type $etype")
        nlocal = nodes_per_type[etype]
        for _ in 1:count
            etag = takeelemint()
            push!(seen_element_tags, etag)
            localtags = [takeelemint() for _ in 1:nlocal]
            indices = [tag_to_index[tag] for tag in localtags]
            if dim == 1 && etype == 1
                push!(edges, (indices[1], indices[2]))
                push!(edge_tags, etag)
                push!(curve_tags, entity)
            elseif dim == 2 && etype == 2
                push!(triangles, (indices[1], indices[2], indices[3]))
                push!(triangle_tags, etag)
            elseif !((dim == 0 && etype == 15))
                error("unexpected element dimension/type $dim/$etype")
            end
        end
    end
    at == length(element_tokens) + 1 || error("unconsumed element tokens")
    length(seen_element_tags) == nelements || error("element count mismatch")
    minimum(seen_element_tags) == min_etag && maximum(seen_element_tags) == max_etag ||
        error("element tag range mismatch")

    Mesh(points, tags, triangles, triangle_tags, edges, edge_tags, curve_tags)
end

function topology_summary(mesh::Mesh)
    usage = Dict{Tuple{Int,Int},Int}()
    for c in mesh.triangles, i in 1:3
        edge = minmax(c[i], c[mod1(i + 1, 3)])
        usage[edge] = get(usage, edge, 0) + 1
    end
    boundary = Set(minmax(e...) for e in mesh.boundary_edges)
    all(Set(k for (k, v) in usage if v == 1) == boundary) ||
        error("line elements do not equal the triangle boundary")
    all(v in (1, 2) for v in values(usage)) || error("nonmanifold triangle edge")
    (vertices = length(mesh.points), triangles = length(mesh.triangles),
     boundary_facets = length(mesh.boundary_edges), edges = length(usage),
     interior_edges = count(==(2), values(usage)),
     euler_characteristic = length(mesh.points) - length(usage) + length(mesh.triangles),
     minimum_area2 = minimum(signed_area2(mesh.points[c[1]], mesh.points[c[2]], mesh.points[c[3]])
                            for c in mesh.triangles),
     minimum_mean_ratio = minimum(begin
         p, q, r = mesh.points[c[1]], mesh.points[c[2]], mesh.points[c[3]]
         det = signed_area2(p, q, r)
         frobenius2 = (q[1] - p[1])^2 + (q[2] - p[2])^2 +
                      (r[1] - p[1])^2 + (r[2] - p[2])^2
         2 * abs(det) / frobenius2
     end for c in mesh.triangles))
end

"""Reverse both inventories and cyclically rotate cells without changing the mesh."""
function reindex_mesh(mesh::Mesh)
    nv, nc, nb = length(mesh.points), length(mesh.triangles), length(mesh.boundary_edges)
    point_order = reverse(1:nv)
    old_to_new = zeros(Int, nv)
    for (new, old) in enumerate(point_order)
        old_to_new[old] = new
    end
    points = mesh.points[point_order]
    node_tags = mesh.node_tags[point_order]
    triangles = NTuple{3,Int}[]
    triangle_tags = Int[]
    for old_cell in reverse(1:nc)
        c = mesh.triangles[old_cell]
        push!(triangles, (old_to_new[c[2]], old_to_new[c[3]], old_to_new[c[1]]))
        push!(triangle_tags, mesh.triangle_tags[old_cell])
    end
    boundary_edges = NTuple{2,Int}[]
    boundary_element_tags = Int[]
    boundary_curve_tags = Int[]
    for old_edge in reverse(1:nb)
        e = mesh.boundary_edges[old_edge]
        push!(boundary_edges, (old_to_new[e[1]], old_to_new[e[2]]))
        push!(boundary_element_tags, mesh.boundary_element_tags[old_edge])
        push!(boundary_curve_tags, mesh.boundary_curve_tags[old_edge])
    end
    Mesh(points, node_tags, triangles, triangle_tags, boundary_edges,
         boundary_element_tags, boundary_curve_tags)
end

end
