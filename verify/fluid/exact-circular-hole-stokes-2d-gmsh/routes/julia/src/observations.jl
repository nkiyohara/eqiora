module Observations

using LinearAlgebra
using ..GmshIO
using ..Stokes

export observe, ObservationSet, selector_report, deviation

struct ObservationSet
    velocity_probes::Vector{NamedTuple}
    pressure_probes::Vector{NamedTuple}
    pressure_extrema::NamedTuple
    fluxes::Dict{Symbol,Float64}
    cylinder_force_on_fluid::NTuple{2,Float64}
    all_essential_force_on_fluid::NTuple{2,Float64}
    body_force::NTuple{2,Float64}
    applied_traction::NTuple{2,Float64}
    momentum_closure::NTuple{2,Float64}
    pressure_integral::Float64
end

function triangle_barycentre(mesh, cell)
    p = (mesh.points[cell[1]], mesh.points[cell[2]], mesh.points[cell[3]])
    ((p[1][1] + p[2][1] + p[3][1]) / 3,
     (p[1][2] + p[2][2] + p[3][2]) / 3)
end

cell_key(mesh, cell) = Tuple(sort([mesh.points[cell[1]], mesh.points[cell[2]], mesh.points[cell[3]]]))

function velocity_probe(problem, solution, target)
    mesh = problem.mesh
    ranked = sort([(sum((triangle_barycentre(mesh, c)[d] - target[d])^2 for d in 1:2),
                    cell_key(mesh, c), k)
                   for (k, c) in enumerate(mesh.triangles)])
    k = ranked[1][3]
    tied = [entry for entry in ranked if entry[1] == ranked[1][1]]
    next_untied = findfirst(entry -> entry[1] != ranked[1][1], ranked)
    cell = mesh.triangles[k]
    nv = length(mesh.points)
    p1x = sum(solution.coefficients[vdof(v, 1)] for v in cell) / 3
    p1y = sum(solution.coefficients[vdof(v, 2)] for v in cell) / 3
    ux = problem.physics.velocity_m_per_s * (p1x + solution.coefficients[bdof(nv, k, 1)])
    uy = problem.physics.velocity_m_per_s * (p1y + solution.coefficients[bdof(nv, k, 2)])
    (target_m = target, barycentre_m = triangle_barycentre(mesh, cell),
     selection_distance2_m2 = ranked[1][1], selection_gap_m2 = ranked[2][1] - ranked[1][1],
     exact_tie_count = length(tied),
     selection_gap_to_untied_m2 = isnothing(next_untied) ? Inf :
                                  ranked[next_untied][1] - ranked[1][1],
     cell_vertex_coordinates_m = cell_key(mesh, cell),
     velocity_m_per_s = (ux, uy))
end

function pressure_value(problem, solution, vertex)
    nv, nc = length(problem.mesh.points), length(problem.mesh.triangles)
    problem.physics.pressure_pa * solution.coefficients[pdof(nv, nc, vertex)]
end

function extreme_probe(problem, solution, name, candidates, score, direction)
    ranked = sort([(direction * score(problem.mesh.points[v]), problem.mesh.points[v], v)
                   for v in candidates])
    chosen = ranked[1]
    tied = [entry for entry in ranked if entry[1] == chosen[1]]
    next_untied = findfirst(entry -> entry[1] != chosen[1], ranked)
    (name = name, vertex_m = chosen[2], pressure_pa = pressure_value(problem, solution, chosen[3]),
     exact_tie_count = length(tied),
     tie_candidates = [(vertex_m = entry[2], pressure_pa = pressure_value(problem, solution, entry[3]))
                       for entry in tied],
     selection_gap_to_untied = isnothing(next_untied) ? Inf :
                               ranked[next_untied][1] - chosen[1])
end

function nearest_probe(problem, solution, name, candidates, target)
    ranked = sort([(sum((problem.mesh.points[v][d] - target[d])^2 for d in 1:2),
                    problem.mesh.points[v], v) for v in candidates])
    chosen = ranked[1]
    (name = name, target_m = target, vertex_m = chosen[2],
     pressure_pa = pressure_value(problem, solution, chosen[3]),
     selection_distance2_m2 = chosen[1], selection_gap_m2 = ranked[2][1] - chosen[1])
end

function adjacent_oriented_edge(mesh, edge_index)
    a, b = mesh.boundary_edges[edge_index]
    for cell in mesh.triangles, i in 1:3
        u, v = cell[i], cell[mod1(i + 1, 3)]
        if minmax(u, v) == minmax(a, b)
            return u, v
        end
    end
    error("boundary edge has no adjacent triangle")
end

function signed_flux(problem, solution, edge_indices)
    total = 0.0
    for edge in edge_indices
        a, b = adjacent_oriented_edge(problem.mesh, edge)
        pa, pb = problem.mesh.points[a], problem.mesh.points[b]
        dx, dy = pb[1] - pa[1], pb[2] - pa[2]
        ux = problem.physics.velocity_m_per_s *
             (solution.coefficients[vdof(a, 1)] + solution.coefficients[vdof(b, 1)]) / 2
        uy = problem.physics.velocity_m_per_s *
             (solution.coefficients[vdof(a, 2)] + solution.coefficients[vdof(b, 2)]) / 2
        total += ux * dy - uy * dx # length times right-hand parent-outward normal
    end
    total
end

function force_on_fluid(problem, solution, vertices)
    scale = problem.physics.pressure_pa * problem.physics.length_m
    (scale * sum(solution.residual[vdof(v, 1)] for v in vertices),
     scale * sum(solution.residual[vdof(v, 2)] for v in vertices))
end

function observe(problem::DiscreteProblem, solution::DiscreteSolution)
    mesh = problem.mesh
    velocity_targets = [(0.10, 0.20), (0.20, 0.30), (0.30, 0.20),
                        (1.00, 0.20), (2.00, 0.20)]
    velocity = [velocity_probe(problem, solution, target) for target in velocity_targets]
    cylinder_vertices = sort!(unique(vcat([[mesh.boundary_edges[e][1], mesh.boundary_edges[e][2]]
                                            for e in problem.groups[:cylinder]]...)))
    outer_vertices = sort!(unique(vcat([[mesh.boundary_edges[e][1], mesh.boundary_edges[e][2]]
                                        for name in (:inlet, :outlet, :walls)
                                        for e in problem.groups[name]]...)))
    pressure = [
        extreme_probe(problem, solution, "cylinder_min_x", cylinder_vertices, p -> p[1], 1),
        extreme_probe(problem, solution, "cylinder_max_x", cylinder_vertices, p -> p[1], -1),
        extreme_probe(problem, solution, "cylinder_min_y", cylinder_vertices, p -> p[2], 1),
        extreme_probe(problem, solution, "cylinder_max_y", cylinder_vertices, p -> p[2], -1),
        nearest_probe(problem, solution, "outer_near_inlet_mid", outer_vertices, (0.0, 0.2)),
        nearest_probe(problem, solution, "outer_near_outlet_mid", outer_vertices, (2.2, 0.2)),
    ]
    values = [(pressure_value(problem, solution, v), mesh.points[v], v)
              for v in eachindex(mesh.points)]
    ascending = sort(values)
    descending = sort(values; rev = true)
    extrema = (minimum = (pressure_pa = ascending[1][1], vertex_m = ascending[1][2],
                          gap_pa = ascending[2][1] - ascending[1][1]),
               maximum = (pressure_pa = descending[1][1], vertex_m = descending[1][2],
                          gap_pa = descending[1][1] - descending[2][1]))
    fluxes = Dict(name => signed_flux(problem, solution, problem.groups[name])
                  for name in (:inlet, :outlet, :walls, :cylinder))
    cylinder_force = force_on_fluid(problem, solution, cylinder_vertices)
    all_force = force_on_fluid(problem, solution, problem.essential_vertices)
    body = (0.0, 0.0)
    traction = (0.0, 0.0)
    closure = (all_force[1] + body[1] + traction[1],
               all_force[2] + body[2] + traction[2])
    nv, nc = length(mesh.points), length(mesh.triangles)
    integral = 0.0
    for cell in mesh.triangles
        area = signed_area2(mesh.points[cell[1]], mesh.points[cell[2]], mesh.points[cell[3]]) / 2
        integral += area * sum(pressure_value(problem, solution, v) for v in cell) / 3
    end
    ObservationSet(velocity, pressure, extrema, fluxes, cylinder_force, all_force,
                   body, traction, closure, integral)
end

selector_report(observations) =
    (velocity_min_gap_m2 = minimum(x.selection_gap_to_untied_m2 for x in observations.velocity_probes),
     pressure_min_gap = minimum(x.selection_gap_to_untied for x in observations.pressure_probes[1:4]),
     nearest_pressure_min_gap_m2 = minimum(x.selection_gap_m2 for x in observations.pressure_probes[5:6]),
     pressure_extrema_min_gap_pa = min(observations.pressure_extrema.minimum.gap_pa,
                                       observations.pressure_extrema.maximum.gap_pa))

function deviation(a::ObservationSet, b::ObservationSet)
    velocity = maximum(abs(a.velocity_probes[i].velocity_m_per_s[c] -
                           b.velocity_probes[i].velocity_m_per_s[c])
                       for i in eachindex(a.velocity_probes), c in 1:2)
    pressure = maximum(abs(a.pressure_probes[i].pressure_pa - b.pressure_probes[i].pressure_pa)
                       for i in eachindex(a.pressure_probes))
    extrema = maximum((abs(a.pressure_extrema.minimum.pressure_pa -
                           b.pressure_extrema.minimum.pressure_pa),
                       abs(a.pressure_extrema.maximum.pressure_pa -
                           b.pressure_extrema.maximum.pressure_pa)))
    flux = maximum(abs(a.fluxes[name] - b.fluxes[name])
                   for name in (:inlet, :outlet, :walls, :cylinder))
    reaction = maximum(abs(a.cylinder_force_on_fluid[c] - b.cylinder_force_on_fluid[c])
                       for c in 1:2)
    closure = maximum(abs, b.momentum_closure)
    selectors_moved = any(a.velocity_probes[i].cell_vertex_coordinates_m !=
                          b.velocity_probes[i].cell_vertex_coordinates_m
                          for i in eachindex(a.velocity_probes)) ||
                      any(a.pressure_probes[i].vertex_m != b.pressure_probes[i].vertex_m
                          for i in eachindex(a.pressure_probes)) ||
                      a.pressure_extrema.minimum.vertex_m != b.pressure_extrema.minimum.vertex_m ||
                      a.pressure_extrema.maximum.vertex_m != b.pressure_extrema.maximum.vertex_m
    (; velocity, pressure, pressure_extrema = extrema, flux, reaction, closure, selectors_moved)
end

end
