module Stokes

using LinearAlgebra
using SparseArrays
using ..GmshIO

export Physics, DiscreteProblem, DiscreteSolution, build_problem, assemble, solve_system
export apply_cells, vdof, bdof, pdof, local_matrix

struct Physics
    length_m::Float64
    velocity_m_per_s::Float64
    pressure_pa::Float64
    viscosity_pa_s::Float64
    channel_height_m::Float64
    inlet_umax_m_per_s::Float64
end

const PHYSICS = Physics(0.41, 0.3, 0.001 * 0.3 / 0.41, 0.001, 0.41, 0.3)
export PHYSICS

struct DiscreteProblem
    mesh::Mesh
    physics::Physics
    normalized_points::Vector{NTuple{2,Float64}}
    essential_dofs::Vector{Int}
    free_dofs::Vector{Int}
    essential_values::Vector{Float64}
    essential_vertices::Vector{Int}
    traction_edges::Vector{Int}
    groups::Dict{Symbol,Vector{Int}}
end

struct DiscreteSolution
    coefficients::Vector{Float64}
    rhs::Vector{Float64}
    residual::Vector{Float64}
    assembled_reapplication_gap_2norm::Float64
    reduced_rhs_2norm::Float64
    reduced_residual_2norm::Float64
    pressure_row_residual_2norm::Float64
    matrix_inf_norm::Float64
    solution_inf_norm::Float64
    rhs_inf_norm::Float64
    residual_target::Float64
    roundoff_allowance::Float64
end

vdof(v::Int, component::Int) = 2 * (v - 1) + component
bdof(nv::Int, cell::Int, component::Int) = 2 * nv + 2 * (cell - 1) + component
pdof(nv::Int, nc::Int, v::Int) = 2 * nv + 2 * nc + v

function build_problem(mesh::Mesh, receipt::BoundaryReceipt; physics = PHYSICS,
                       reverse_inlet = false, swap_inlet_outlet = false)
    curve_groups = boundary_groups(receipt)
    swap_inlet_outlet && ((curve_groups[:inlet], curve_groups[:outlet]) =
                          (curve_groups[:outlet], curve_groups[:inlet]))
    groups = Dict{Symbol,Vector{Int}}()
    for name in keys(curve_groups)
        tags = Set(curve_groups[name])
        groups[name] = findall(tag -> tag in tags, mesh.boundary_curve_tags)
    end
    covered = sort(vcat(values(groups)...))
    covered == collect(eachindex(mesh.boundary_edges)) ||
        error("boundary groups do not form a disjoint complete facet partition")

    velocity_edges = vcat(groups[:inlet], groups[:walls], groups[:cylinder])
    essential_vertices = sort!(unique(vcat([[mesh.boundary_edges[e][1],
                                             mesh.boundary_edges[e][2]]
                                            for e in velocity_edges]...)))
    essential_dofs = sort!(vcat([[vdof(v, 1), vdof(v, 2)] for v in essential_vertices]...))
    value_for_dof = Dict{Int,Float64}()
    inlet_vertices = Set(vcat([[mesh.boundary_edges[e][1], mesh.boundary_edges[e][2]]
                               for e in groups[:inlet]]...))
    for v in essential_vertices
        u = if v in inlet_vertices
            y = mesh.points[v][2]
            ux = 4 * physics.inlet_umax_m_per_s * y * (physics.channel_height_m - y) /
                 physics.channel_height_m^2
            (reverse_inlet ? -ux : ux, 0.0)
        else
            (0.0, 0.0)
        end
        value_for_dof[vdof(v, 1)] = u[1] / physics.velocity_m_per_s
        value_for_dof[vdof(v, 2)] = u[2] / physics.velocity_m_per_s
    end
    essential_values = [value_for_dof[d] for d in essential_dofs]
    nv, nc = length(mesh.points), length(mesh.triangles)
    ndof = 3 * nv + 2 * nc
    free_dofs = setdiff(1:ndof, essential_dofs)
    normalized = [(p[1] / physics.length_m, p[2] / physics.length_m) for p in mesh.points]
    DiscreteProblem(mesh, physics, normalized, essential_dofs, free_dofs,
                    essential_values, essential_vertices, groups[:outlet], groups)
end

function quadrature()
    g = sqrt(3 / 5)
    nodes = ((1 - g) / 2, 0.5, (1 + g) / 2)
    weights = (5 / 18, 4 / 9, 5 / 18)
    [(s, t * (1 - s), ws * wt * (1 - s))
     for (s, ws) in zip(nodes, weights) for (t, wt) in zip(nodes, weights)]
end

function basis(xi, eta)
    l = (1 - xi - eta, xi, eta)
    gl = ((-1.0, -1.0), (1.0, 0.0), (0.0, 1.0))
    bubble = 27 * l[1] * l[2] * l[3]
    gb = (27 * (-l[2] * l[3] + l[1] * l[3]),
          27 * (-l[2] * l[3] + l[1] * l[2]))
    ((l[1], l[2], l[3], bubble), (gl[1], gl[2], gl[3], gb))
end

function cell_dofs(nv, nc, cell_index, vertices)
    (vdof(vertices[1], 1), vdof(vertices[1], 2),
     vdof(vertices[2], 1), vdof(vertices[2], 2),
     vdof(vertices[3], 1), vdof(vertices[3], 2),
     bdof(nv, cell_index, 1), bdof(nv, cell_index, 2),
     pdof(nv, nc, vertices[1]), pdof(nv, nc, vertices[2]), pdof(nv, nc, vertices[3]))
end

"""Fresh numerical cell construction for the accepted symmetric-gradient MINI/P1 form."""
function local_matrix(problem::DiscreteProblem, cell_index::Int;
                      vector_laplacian = false, pressure_sign = 1.0)
    vertices = problem.mesh.triangles[cell_index]
    x0, x1, x2 = (problem.normalized_points[v] for v in vertices)
    j11, j12 = x1[1] - x0[1], x2[1] - x0[1]
    j21, j22 = x1[2] - x0[2], x2[2] - x0[2]
    determinant = j11 * j22 - j12 * j21
    determinant > 0 || error("nonpositive Gmsh triangle $cell_index")
    inverse_transpose(g) = ((j22 * g[1] - j21 * g[2]) / determinant,
                            (-j12 * g[1] + j11 * g[2]) / determinant)
    muhat = problem.physics.viscosity_pa_s * problem.physics.velocity_m_per_s /
            (problem.physics.pressure_pa * problem.physics.length_m)
    block = zeros(11, 11)
    for (xi, eta, weight) in quadrature()
        phi, gradref = basis(xi, eta)
        grad = map(inverse_transpose, gradref)
        w = weight * determinant
        for a in 1:4, c in 1:2, e in 1:4, d in 1:2
            row = 2 * (a - 1) + c
            col = 2 * (e - 1) + d
            dotgrad = grad[a][1] * grad[e][1] + grad[a][2] * grad[e][2]
            block[row, col] += w * muhat *
                ((c == d ? dotgrad : 0.0) +
                 (vector_laplacian ? 0.0 : grad[a][d] * grad[e][c]))
        end
        for a in 1:4, c in 1:2, m in 1:3
            velocity = 2 * (a - 1) + c
            pressure = 8 + m
            coupling = -pressure_sign * w * phi[m] * grad[a][c]
            block[velocity, pressure] += coupling
            block[pressure, velocity] += coupling
        end
    end
    block
end

function assemble(problem::DiscreteProblem; vector_laplacian = false, pressure_sign = 1.0)
    nv, nc = length(problem.mesh.points), length(problem.mesh.triangles)
    ndof = 3 * nv + 2 * nc
    rows = Int[]
    columns = Int[]
    values = Float64[]
    for (k, cell) in enumerate(problem.mesh.triangles)
        dofs = cell_dofs(nv, nc, k, cell)
        block = local_matrix(problem, k; vector_laplacian, pressure_sign)
        for j in 1:11, i in 1:11
            if block[i, j] != 0.0
                push!(rows, dofs[i])
                push!(columns, dofs[j])
                push!(values, block[i, j])
            end
        end
    end
    matrix = sparse(rows, columns, values, ndof, ndof)
    rhs = zeros(ndof) # accepted body and outlet traction are both identically zero
    matrix, rhs
end

function apply_cells(problem::DiscreteProblem, coefficients::Vector{Float64};
                     vector_laplacian = false, pressure_sign = 1.0)
    nv, nc = length(problem.mesh.points), length(problem.mesh.triangles)
    result = zeros(length(coefficients))
    for (k, cell) in enumerate(problem.mesh.triangles)
        dofs = cell_dofs(nv, nc, k, cell)
        x0, x1, x2 = (problem.normalized_points[v] for v in cell)
        j11, j12 = x1[1] - x0[1], x2[1] - x0[1]
        j21, j22 = x1[2] - x0[2], x2[2] - x0[2]
        determinant = j11 * j22 - j12 * j21
        inverse_transpose(g) = ((j22 * g[1] - j21 * g[2]) / determinant,
                                (-j12 * g[1] + j11 * g[2]) / determinant)
        muhat = problem.physics.viscosity_pa_s * problem.physics.velocity_m_per_s /
                (problem.physics.pressure_pa * problem.physics.length_m)
        local_coefficients = coefficients[collect(dofs)]
        local_result = zeros(11)
        for (xi, eta, weight) in quadrature()
            phi, gradref = basis(xi, eta)
            grad = map(inverse_transpose, gradref)
            w = weight * determinant
            velocity_gradient = zeros(2, 2)
            for a in 1:4, c in 1:2, d in 1:2
                velocity_gradient[c, d] += grad[a][d] * local_coefficients[2 * (a - 1) + c]
            end
            pressure = sum(phi[m] * local_coefficients[8 + m] for m in 1:3)
            divergence = velocity_gradient[1, 1] + velocity_gradient[2, 2]
            for a in 1:4, c in 1:2
                action = muhat * sum(grad[a][d] * velocity_gradient[c, d] for d in 1:2)
                if !vector_laplacian
                    action += muhat * sum(grad[a][d] * velocity_gradient[d, c] for d in 1:2)
                end
                action -= pressure_sign * pressure * grad[a][c]
                local_result[2 * (a - 1) + c] += w * action
            end
            for m in 1:3
                local_result[8 + m] -= w * pressure_sign * phi[m] * divergence
            end
        end
        for i in eachindex(dofs)
            result[dofs[i]] += local_result[i]
        end
    end
    result
end

function sparse_inf_norm(matrix::SparseMatrixCSC)
    rowsums = zeros(size(matrix, 1))
    for column in 1:size(matrix, 2)
        for at in nzrange(matrix, column)
            rowsums[rowvals(matrix)[at]] += abs(nonzeros(matrix)[at])
        end
    end
    maximum(rowsums)
end

function solve_system(problem::DiscreteProblem, matrix::SparseMatrixCSC,
                      rhs::Vector{Float64}; algorithm = :lu, refinement_steps = 0,
                      vector_laplacian = false, pressure_sign = 1.0)
    free, essential = problem.free_dofs, problem.essential_dofs
    reduced = matrix[free, free]
    reduced_rhs = rhs[free] - matrix[free, essential] * problem.essential_values
    factor = algorithm == :lu ? lu(reduced) : algorithm == :qr ? qr(reduced) :
             error("unknown factorization $algorithm")
    coefficients = zeros(length(rhs))
    coefficients[essential] = problem.essential_values
    coefficients[free] = factor \ reduced_rhs
    for _ in 1:refinement_steps
        correction_rhs = -(matrix * coefficients - rhs)[free]
        coefficients[free] += factor \ correction_rhs
    end
    reapplied = apply_cells(problem, coefficients; vector_laplacian, pressure_sign) - rhs
    assembled = matrix * coefficients - rhs
    gap = norm(reapplied - assembled)
    reduced_residual = norm(reapplied[free])
    nv, nc = length(problem.mesh.points), length(problem.mesh.triangles)
    pressure_rows = (2 * nv + 2 * nc + 1):(3 * nv + 2 * nc)
    weak = norm(reapplied[pressure_rows])
    anorm = sparse_inf_norm(reduced)
    xnorm = norm(coefficients[free], Inf)
    bnorm = norm(reduced_rhs, Inf)
    target = max(1e-13, 1e-6 * norm(reduced_rhs))
    allowance = 4096 * eps(Float64) * (1 + anorm * xnorm + bnorm)
    DiscreteSolution(coefficients, rhs, reapplied, gap, norm(reduced_rhs),
                     reduced_residual, weak, anorm, xnorm, bnorm, target, allowance)
end

end
