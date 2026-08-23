#!/usr/bin/env julia

using LinearAlgebra
using Printf
using SHA
using SparseArrays

const HERE = @__DIR__
include(joinpath(HERE, "src", "gmsh_io.jl"))
using .GmshIO
include(joinpath(HERE, "src", "stokes.jl"))
using .Stokes
include(joinpath(HERE, "src", "observations.jl"))
using .Observations
include(joinpath(HERE, "src", "format.jl"))
using .Format

const BASE_COMMIT = "934493bcb487c1753fb4b3ddffaab88d7150aa7d"
const MESH_EVIDENCE_COMMIT = "05254257d98caee8cac924759d01d92c25801169"
const GMSH_ARCHIVE_SHA256 =
    "6c62116e072db29fd1f701fdb9d3d34b46ed5373545063e177b965a008274745"
const GMSH_EXECUTABLE_SHA256 =
    "9dccade5dd1374b28c18af9085d7ce63216cf7ac39d3cefbc0adbfabafba2c7f"
const EQIORA_MESH_DIGEST =
    "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b"
const EQIORA_CANONICAL_RAW_SHA256 =
    "9d3c6211e6832aa5a5f7e99fa210058ff1b76eab7f1e99aaa7033c282d6e2dd2"
const COORDINATE_BUFFER_SHA256 =
    "42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d"
const TRIANGLE_BUFFER_SHA256 =
    "05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642"
const BOUNDARY_MAPPING_SHA256 =
    "8eafb74f5727d720ac6b8e67d4413c07687636ada7cb72af78194634475b1d83"

const CHECKS = Tuple{String,Bool,String}[]
const LOG = IOBuffer()
say(value = "") = println(LOG, value)
function check!(name, condition, detail = "")
    passed = Bool(condition)
    push!(CHECKS, (name, passed, string(detail)))
    say(@sprintf("  %-74s %s%s", name, passed ? "PASS" : "FAIL",
                 isempty(string(detail)) ? "" : "  " * string(detail)))
    passed
end

function required_env(name)
    haskey(ENV, name) || error("set $name to the frozen external input")
    ENV[name]
end

const GMSH = required_env("GMSH")
const GMSH_ARCHIVE = required_env("GMSH_ARCHIVE")
const GMSH_GEO = required_env("GMSH_GEO")
const GMSH_MSH = required_env("GMSH_MSH")
const SCRATCH_ROOT = joinpath(HERE, ".scratch")
mkpath(SCRATCH_ROOT)

say("Julia Route B: exact-source Gmsh 4.15.2 MINI/P1 Stokes oracle")
say("base=$BASE_COMMIT mesh_evidence=$MESH_EVIDENCE_COMMIT")
say()

const ARCHIVE_DIGEST = bytes2hex(sha256(read(GMSH_ARCHIVE)))
const EXECUTABLE_DIGEST = bytes2hex(sha256(read(GMSH)))
const GEO_DIGEST = bytes2hex(sha256(read(GMSH_GEO)))
const SEALED_MSH_DIGEST = bytes2hex(sha256(read(GMSH_MSH)))

say("-- ordinary positive path: exact inputs and Gmsh replay ------------------")
check!("input.official_linux64_archive_sha256", ARCHIVE_DIGEST == GMSH_ARCHIVE_SHA256,
       ARCHIVE_DIGEST)
check!("input.official_linux64_executable_sha256",
       EXECUTABLE_DIGEST == GMSH_EXECUTABLE_SHA256, EXECUTABLE_DIGEST)
check!("input.accepted_geo_sha256", GEO_DIGEST == ACCEPTED_GEO_SHA256, GEO_DIGEST)
check!("input.sealed_msh_sha256", SEALED_MSH_DIGEST == ACCEPTED_MSH_SHA256,
       SEALED_MSH_DIGEST)

const RECEIPT = read_boundary_receipt(GMSH_GEO)
const CURVE_GROUPS = boundary_groups(RECEIPT)

const POSITIVE = mktempdir(SCRATCH_ROOT) do directory
    msh_path = joinpath(directory, "accepted-owner-region.msh")
    version = run_gmsh(GMSH, GMSH_GEO, msh_path)
    bytes = read(msh_path)
    digest = bytes2hex(sha256(bytes))
    check!("gmsh.version_exact", version == "4.15.2", version)
    check!("gmsh.replay_msh_sha256", digest == ACCEPTED_MSH_SHA256, digest)
    check!("gmsh.replay_byte_identical_to_sealed_input", bytes == read(GMSH_MSH))
    (version = version, mesh = read_msh(msh_path), msh_sha256 = digest)
end

const MESH = POSITIVE.mesh
const TOPOLOGY = topology_summary(MESH)
const MULTIPLICITIES = curve_facet_multiplicities(MESH)
const MAPPING_DIGEST = boundary_mapping_sha256(MESH)
const GROUP_FACETS = Dict(name => count(tag -> tag in Set(CURVE_GROUPS[name]),
                                        MESH.boundary_curve_tags)
                          for name in (:inlet, :outlet, :walls, :cylinder))

say()
say("-- mesh topology and correspondence --------------------------------------")
check!("mesh.vertices_662", TOPOLOGY.vertices == 662, TOPOLOGY.vertices)
check!("mesh.triangles_1210", TOPOLOGY.triangles == 1210, TOPOLOGY.triangles)
check!("mesh.boundary_facets_114", TOPOLOGY.boundary_facets == 114,
       TOPOLOGY.boundary_facets)
check!("mesh.interior_edges_1758", TOPOLOGY.interior_edges == 1758,
       TOPOLOGY.interior_edges)
check!("mesh.euler_characteristic_zero", TOPOLOGY.euler_characteristic == 0,
       TOPOLOGY.euler_characteristic)
check!("mesh.all_triangles_positive", TOPOLOGY.minimum_area2 > 0,
       repr(TOPOLOGY.minimum_area2))
check!("mesh.minimum_measure_matches_owner",
       TOPOLOGY.minimum_area2 == 2.6093038450074273e-5,
       repr(TOPOLOGY.minimum_area2))
check!("mesh.minimum_quality_matches_owner",
       TOPOLOGY.minimum_mean_ratio == 0.5236522686855336,
       repr(TOPOLOGY.minimum_mean_ratio))
check!("mapping.curve_counts", map(name -> length(CURVE_GROUPS[name]),
                                    (:inlet, :outlet, :walls, :cylinder)) == (14, 2, 38, 50))
check!("mapping.facet_counts", map(name -> GROUP_FACETS[name],
                                    (:inlet, :outlet, :walls, :cylinder)) == (14, 2, 48, 50))
check!("mapping.partition_complete_once",
       sum(values(GROUP_FACETS)) == length(MESH.boundary_edges))
check!("mapping.canonical_sha256", MAPPING_DIGEST == BOUNDARY_MAPPING_SHA256,
       MAPPING_DIGEST)

const PROBLEM = build_problem(MESH, RECEIPT)
const MATRIX, RHS = assemble(PROBLEM)
const PRIMARY = solve_system(PROBLEM, MATRIX, RHS; algorithm = :lu, refinement_steps = 2)
const OBSERVED = observe(PROBLEM, PRIMARY)
const NV, NC = length(MESH.points), length(MESH.triangles)
const BOUNDARY_VERTICES = sort!(unique(vcat([[e[1], e[2]] for e in MESH.boundary_edges]...)))
const FREE_BOUNDARY_VERTICES = setdiff(BOUNDARY_VERTICES, PROBLEM.essential_vertices)
const REDUCED_RESIDUAL_INF = norm(PRIMARY.residual[PROBLEM.free_dofs], Inf)
const PRESSURE_ROWS = (2 * NV + 2 * NC + 1):(3 * NV + 2 * NC)
const WEAK_INF = norm(PRIMARY.residual[PRESSURE_ROWS], Inf)
const WEAK_ALLOWANCE = 4096 * eps(Float64) *
                       (1 + PRIMARY.pressure_row_residual_2norm + PRIMARY.residual_target)
const BACKWARD_ERROR_INF = REDUCED_RESIDUAL_INF /
    (PRIMARY.matrix_inf_norm * PRIMARY.solution_inf_norm + PRIMARY.rhs_inf_norm)

say()
say("-- accepted MINI/P1 solve ------------------------------------------------")
check!("dof.full_4406", length(PRIMARY.coefficients) == 4406,
       length(PRIMARY.coefficients))
check!("dof.reduced_4180", length(PROBLEM.free_dofs) == 4180,
       length(PROBLEM.free_dofs))
check!("dof.essential_velocity_226", length(PROBLEM.essential_dofs) == 226,
       length(PROBLEM.essential_dofs))
check!("trace.essential_vertices_113", length(PROBLEM.essential_vertices) == 113,
       length(PROBLEM.essential_vertices))
check!("trace.only_free_boundary_vertex_is_outlet_midpoint",
       length(FREE_BOUNDARY_VERTICES) == 1 &&
       MESH.points[only(FREE_BOUNDARY_VERTICES)] == (2.2, 0.2),
       [MESH.points[v] for v in FREE_BOUNDARY_VERTICES])
check!("pressure_reference.boundary_traction_no_gauge",
       length(PRIMARY.coefficients) == 3 * NV + 2 * NC &&
       length(PROBLEM.traction_edges) == 2)
check!("assembly.exact_symmetry", MATRIX == transpose(MATRIX))
check!("assembly.nnz_83268", nnz(MATRIX) == 83268, nnz(MATRIX))
check!("residual.true_reduced_within_existing_limit",
       PRIMARY.reduced_residual_2norm <= PRIMARY.residual_target + PRIMARY.roundoff_allowance,
       "$(PRIMARY.reduced_residual_2norm) <= $(PRIMARY.residual_target + PRIMARY.roundoff_allowance)")
check!("residual.weak_pressure_rows_within_existing_limit",
       PRIMARY.pressure_row_residual_2norm <= PRIMARY.residual_target + WEAK_ALLOWANCE,
       "$(PRIMARY.pressure_row_residual_2norm) <= $(PRIMARY.residual_target + WEAK_ALLOWANCE)")
check!("residual.independent_cell_reapplication_finite",
       all(isfinite, PRIMARY.residual) && isfinite(PRIMARY.assembled_reapplication_gap_2norm))
check!("balance.signed_flux", abs(OBSERVED.fluxes[:inlet] + OBSERVED.fluxes[:outlet]) <= 1e-8,
       repr(OBSERVED.fluxes[:inlet] + OBSERVED.fluxes[:outlet]))
check!("balance.no_slip_flux_exact_zero",
       OBSERVED.fluxes[:walls] == 0.0 && OBSERVED.fluxes[:cylinder] == 0.0)
check!("balance.momentum_componentwise", maximum(abs, OBSERVED.momentum_closure) <= 1e-10,
       OBSERVED.momentum_closure)
check!("reaction.cylinder_force_nonzero",
       maximum(abs, OBSERVED.cylinder_force_on_fluid) > 8e-14,
       OBSERVED.cylinder_force_on_fluid)
check!("selector.velocity_unique_and_separated",
       all(x.exact_tie_count == 1 && x.selection_gap_to_untied_m2 > 0
           for x in OBSERVED.velocity_probes))
check!("selector.pressure_expected_ties",
       [x.exact_tie_count for x in OBSERVED.pressure_probes[1:4]] == [1, 1, 2, 2])
check!("selector.pressure_ties_separated_from_untied",
       all(x.selection_gap_to_untied > 0 for x in OBSERVED.pressure_probes[1:4]))
check!("selector.pressure_extrema_stable",
       min(OBSERVED.pressure_extrema.minimum.gap_pa,
           OBSERVED.pressure_extrema.maximum.gap_pa) > 2e-14 + 2e-10 * PHYSICS.pressure_pa)

const TOLERANCES = (
    velocity_m_per_s = 2e-12 + 2e-10 * PHYSICS.velocity_m_per_s,
    pressure_pa = 2e-14 + 2e-10 * PHYSICS.pressure_pa,
    flux_m2_per_s = 2e-13 + 2e-10 * (PHYSICS.velocity_m_per_s * PHYSICS.length_m),
    reaction_n_per_m = 2e-14 + 2e-10 * (PHYSICS.pressure_pa * PHYSICS.length_m),
)

function within_tolerances(delta)
    !delta.selectors_moved && delta.velocity <= TOLERANCES.velocity_m_per_s &&
    delta.pressure <= TOLERANCES.pressure_pa &&
    delta.pressure_extrema <= TOLERANCES.pressure_pa &&
    delta.flux <= TOLERANCES.flux_m2_per_s &&
    delta.reaction <= TOLERANCES.reaction_n_per_m
end

say()
say("-- independent solver and indexing checks --------------------------------")
const QR_SOLUTION = solve_system(PROBLEM, MATRIX, RHS; algorithm = :qr, refinement_steps = 2)
const QR_OBSERVED = observe(PROBLEM, QR_SOLUTION)
const LU_QR_DEVIATION = deviation(OBSERVED, QR_OBSERVED)
check!("stability.refined_sparse_lu_vs_refined_sparse_qr",
       within_tolerances(LU_QR_DEVIATION), LU_QR_DEVIATION)

const REINDEXED_PROBLEM = build_problem(reindex_mesh(MESH), RECEIPT)
const REINDEXED_MATRIX, REINDEXED_RHS = assemble(REINDEXED_PROBLEM)
const REINDEXED_SOLUTION = solve_system(REINDEXED_PROBLEM, REINDEXED_MATRIX, REINDEXED_RHS;
                                        algorithm = :lu, refinement_steps = 2)
const REINDEXED_OBSERVED = observe(REINDEXED_PROBLEM, REINDEXED_SOLUTION)
const REINDEX_DEVIATION = deviation(OBSERVED, REINDEXED_OBSERVED)
check!("stability.vertex_cell_facet_reindexing", within_tolerances(REINDEX_DEVIATION),
       REINDEX_DEVIATION)
check!("stability.reindexed_boundary_mapping_digest",
       boundary_mapping_sha256(REINDEXED_PROBLEM.mesh) == BOUNDARY_MAPPING_SHA256)

say()
say("-- falsifiers (after the ordinary positive path) --------------------------")
const VECTOR_MATRIX, VECTOR_RHS = assemble(PROBLEM; vector_laplacian = true)
const VECTOR_SOLUTION = solve_system(PROBLEM, VECTOR_MATRIX, VECTOR_RHS;
                                     refinement_steps = 2, vector_laplacian = true)
const VECTOR_DEVIATION = deviation(OBSERVED, observe(PROBLEM, VECTOR_SOLUTION))
check!("falsifier.vector_laplacian_detected", !within_tolerances(VECTOR_DEVIATION),
       VECTOR_DEVIATION)

const SIGN_MATRIX, SIGN_RHS = assemble(PROBLEM; pressure_sign = -1.0)
const SIGN_SOLUTION = solve_system(PROBLEM, SIGN_MATRIX, SIGN_RHS;
                                   refinement_steps = 2, pressure_sign = -1.0)
const SIGN_DEVIATION = deviation(OBSERVED, observe(PROBLEM, SIGN_SOLUTION))
check!("falsifier.pressure_coupling_sign_detected", !within_tolerances(SIGN_DEVIATION),
       SIGN_DEVIATION)

const SWAPPED_PROBLEM = build_problem(MESH, RECEIPT; swap_inlet_outlet = true)
const SWAPPED_MATRIX, SWAPPED_RHS = assemble(SWAPPED_PROBLEM)
const SWAPPED_SOLUTION = solve_system(SWAPPED_PROBLEM, SWAPPED_MATRIX, SWAPPED_RHS;
                                      refinement_steps = 2)
const SWAPPED_DEVIATION = deviation(OBSERVED, observe(SWAPPED_PROBLEM, SWAPPED_SOLUTION))
check!("falsifier.swapped_inlet_outlet_detected", !within_tolerances(SWAPPED_DEVIATION),
       SWAPPED_DEVIATION)
check!("falsifier.reversed_flux_normal_detected",
       abs(-OBSERVED.fluxes[:inlet] + OBSERVED.fluxes[:outlet]) > 1e-8)
const SUFFIXED_VERSION_REJECTED = mktempdir(SCRATCH_ROOT) do directory
    fake = joinpath(directory, "gmsh")
    write(fake, "#!/bin/sh\nprintf '4.15.2-nox\\n'\n")
    chmod(fake, 0o755)
    try
        run_gmsh(fake, GMSH_GEO, joinpath(directory, "must-not-exist.msh"))
        false
    catch error
        occursin("expected exact Gmsh 4.15.2", sprint(showerror, error)) &&
        !isfile(joinpath(directory, "must-not-exist.msh"))
    end
end
check!("falsifier.suffixed_gmsh_version_rejected", SUFFIXED_VERSION_REJECTED)

const ALGORITHM5 = mktempdir(SCRATCH_ROOT) do directory
    geo = joinpath(directory, "algorithm5.geo")
    msh = joinpath(directory, "algorithm5.msh")
    write(geo, replace(read(GMSH_GEO, String),
                       "Mesh.Algorithm = 6;" => "Mesh.Algorithm = 5;"))
    run_gmsh(GMSH, geo, msh)
    wrong_mesh = read_msh(msh)
    (geo_sha256 = bytes2hex(sha256(read(geo))), msh_sha256 = bytes2hex(sha256(read(msh))),
     topology = topology_summary(wrong_mesh))
end
check!("falsifier.algorithm5_changes_exact_mesh",
       ALGORITHM5.msh_sha256 != ACCEPTED_MSH_SHA256 &&
       (ALGORITHM5.topology.vertices, ALGORITHM5.topology.triangles) != (662, 1210),
       (ALGORITHM5.msh_sha256, ALGORITHM5.topology.vertices, ALGORITHM5.topology.triangles))

const SUBDIVIDED_CURVES = [(curve = tag, facets = MULTIPLICITIES[tag])
                           for tag in sort!(collect(keys(MULTIPLICITIES)))
                           if MULTIPLICITIES[tag] != 1]
const RECORD = (
    schema = "eqiora.verify/exact-circular-hole-stokes-2d-gmsh/julia-route/v1",
    route = "julia",
    statement = "Fresh Julia numerical derivation on the sealed Gmsh 4.15.2 mesh; no Eqiora implementation or other Stokes route was read or run.",
    provenance = (base_commit = BASE_COMMIT, mesh_evidence_commit = MESH_EVIDENCE_COMMIT,
                  julia_version = string(VERSION), kernel = string(Sys.KERNEL),
                  architecture = string(Sys.ARCH), julia_threads = Threads.nthreads()),
    inputs = (exact_source_sha256 = EXACT_SOURCE_SHA256,
              official_linux64_url = "https://gmsh.info/bin/Linux/gmsh-4.15.2-Linux64.tgz",
              official_linux64_archive_sha256 = ARCHIVE_DIGEST,
              official_linux64_executable_sha256 = EXECUTABLE_DIGEST,
              geo_sha256 = GEO_DIGEST, msh_sha256 = POSITIVE.msh_sha256,
              sealed_msh_byte_equal = true,
              eqiora_mesh_digest_cited_not_recomputed = EQIORA_MESH_DIGEST,
              eqiora_canonical_raw_sha256_cited_not_recomputed = EQIORA_CANONICAL_RAW_SHA256,
              coordinate_buffer_sha256_cited_not_recomputed = COORDINATE_BUFFER_SHA256,
              triangle_buffer_sha256_cited_not_recomputed = TRIANGLE_BUFFER_SHA256),
    gmsh = (version = POSITIVE.version, factory = "Built-in", num_threads = 1,
            algorithm = 6, element_order = 1, save_all = 1, msh_file_version = 4.1,
            binary = 0, random_factor = 0, point_characteristic_length = "absent",
            traversal = "hole points/lines first; outer points/lines second; Plane Surface outer then hole",
            elements = "linear 2D triangles"),
    mesh = (vertices = TOPOLOGY.vertices, triangles = TOPOLOGY.triangles,
            boundary_facets = TOPOLOGY.boundary_facets, edges = TOPOLOGY.edges,
            interior_edges = TOPOLOGY.interior_edges,
            euler_characteristic = TOPOLOGY.euler_characteristic,
            minimum_signed_measure_scale = TOPOLOGY.minimum_area2,
            minimum_mean_ratio = TOPOLOGY.minimum_mean_ratio,
            boundary_vertices = length(BOUNDARY_VERTICES),
            interior_vertices = NV - length(BOUNDARY_VERTICES)),
    mapping = (boundary_mapping_sha256 = MAPPING_DIGEST,
               inlet_curve_tags = CURVE_GROUPS[:inlet],
               outlet_curve_tags = CURVE_GROUPS[:outlet],
               wall_curve_tags = CURVE_GROUPS[:walls],
               cylinder_curve_tags = CURVE_GROUPS[:cylinder],
               inlet_facets = GROUP_FACETS[:inlet], outlet_facets = GROUP_FACETS[:outlet],
               wall_facets = GROUP_FACETS[:walls], cylinder_facets = GROUP_FACETS[:cylinder],
               subdivided_curves = SUBDIVIDED_CURVES),
    formulation = (equations = "-div(2 mu sym(grad(u)) - p I) = 0; div(u) = 0",
                   velocity_space = "continuous vector MINI: P1 plus normalized 27*l0*l1*l2 cell bubble",
                   pressure_space = "continuous scalar P1",
                   quadrature = "positive degree-four 3x3 Gauss-Legendre Duffy",
                   pressure_reference = "BoundaryTraction; no gauge row, column, multiplier, or ZeroIntegral constraint",
                   body_force = [0.0, 0.0], outlet_traction_pa = [0.0, 0.0],
                   walls = "no slip", cylinder = "no slip",
                   inlet = "u=(4 Umax y (H-y)/H^2, 0)"),
    scales = (length_m = PHYSICS.length_m, velocity_m_per_s = PHYSICS.velocity_m_per_s,
              pressure_pa = PHYSICS.pressure_pa,
              gradient_per_s = PHYSICS.velocity_m_per_s / PHYSICS.length_m,
              theta_w_per_m = PHYSICS.pressure_pa * PHYSICS.velocity_m_per_s * PHYSICS.length_m,
              dynamic_viscosity_pa_s = PHYSICS.viscosity_pa_s,
              channel_height_m = PHYSICS.channel_height_m,
              inlet_umax_m_per_s = PHYSICS.inlet_umax_m_per_s),
    dofs = (p1_velocity = 2 * NV, bubble_velocity = 2 * NC, pressure = NV,
            full = length(PRIMARY.coefficients), essential_velocity = length(PROBLEM.essential_dofs),
            reduced = length(PROBLEM.free_dofs), essential_vertices = length(PROBLEM.essential_vertices),
            free_velocity_vertices = NV - length(PROBLEM.essential_vertices)),
    residuals = (selected_target = PRIMARY.residual_target,
                 roundoff_allowance = PRIMARY.roundoff_allowance,
                 true_reduced_2norm = PRIMARY.reduced_residual_2norm,
                 true_reduced_infnorm = REDUCED_RESIDUAL_INF,
                 weak_pressure_row_2norm = PRIMARY.pressure_row_residual_2norm,
                 weak_pressure_row_infnorm = WEAK_INF,
                 weak_roundoff_allowance = WEAK_ALLOWANCE,
                 assembled_vs_cell_reapplication_2norm = PRIMARY.assembled_reapplication_gap_2norm,
                 matrix_inf_norm = PRIMARY.matrix_inf_norm,
                 solution_inf_norm = PRIMARY.solution_inf_norm,
                 reduced_rhs_inf_norm = PRIMARY.rhs_inf_norm,
                 reduced_rhs_2norm = PRIMARY.reduced_rhs_2norm,
                 normwise_backward_error_inf = BACKWARD_ERROR_INF),
    tolerances = (formula = "absolute_floor + 2e-10 * existing physical scale",
                  velocity_m_per_s = TOLERANCES.velocity_m_per_s,
                  pressure_pa = TOLERANCES.pressure_pa,
                  flux_m2_per_s = TOLERANCES.flux_m2_per_s,
                  reaction_n_per_m = TOLERANCES.reaction_n_per_m,
                  signed_flux_balance_m2_per_s = 1e-8,
                  momentum_closure_n_per_m = 1e-10,
                  selectors = "exact coordinates/ties; no floating tolerance"),
    velocity_probes = OBSERVED.velocity_probes,
    pressure_probes = OBSERVED.pressure_probes,
    pressure_extrema = OBSERVED.pressure_extrema,
    fluxes_m2_per_s = (inlet = OBSERVED.fluxes[:inlet], outlet = OBSERVED.fluxes[:outlet],
                       walls = OBSERVED.fluxes[:walls], cylinder = OBSERVED.fluxes[:cylinder],
                       net = OBSERVED.fluxes[:inlet] + OBSERVED.fluxes[:outlet]),
    forces_n_per_m = (cylinder_constraint_force_on_fluid = OBSERVED.cylinder_force_on_fluid,
                      fluid_force_on_cylinder = (-OBSERVED.cylinder_force_on_fluid[1],
                                                  -OBSERVED.cylinder_force_on_fluid[2]),
                      all_essential_constraint_force_on_fluid = OBSERVED.all_essential_force_on_fluid,
                      integrated_body_force = OBSERVED.body_force,
                      integrated_applied_traction = OBSERVED.applied_traction,
                      momentum_closure = OBSERVED.momentum_closure),
    supplementary = (pressure_integral_pa_m2 = OBSERVED.pressure_integral,
                     selector_separation = selector_report(OBSERVED)),
    stability = (primary = "two-step refined sparse LU",
                 independent = "two-step refined sparse QR",
                 lu_vs_qr = LU_QR_DEVIATION, reindexing = REINDEX_DEVIATION,
                 smallest_tolerance_margin = minimum((
                     TOLERANCES.velocity_m_per_s / max(LU_QR_DEVIATION.velocity, REINDEX_DEVIATION.velocity),
                     TOLERANCES.pressure_pa / max(LU_QR_DEVIATION.pressure, REINDEX_DEVIATION.pressure),
                     TOLERANCES.flux_m2_per_s / max(LU_QR_DEVIATION.flux, REINDEX_DEVIATION.flux),
                     TOLERANCES.reaction_n_per_m / max(LU_QR_DEVIATION.reaction, REINDEX_DEVIATION.reaction)))),
    falsifiers = (algorithm5 = ALGORITHM5, vector_laplacian = VECTOR_DEVIATION,
                  pressure_coupling_sign = SIGN_DEVIATION,
                  swapped_inlet_outlet = SWAPPED_DEVIATION,
                  reversed_flux_observation_net_m2_per_s =
                      -OBSERVED.fluxes[:inlet] + OBSERVED.fluxes[:outlet],
                  suffixed_version = "4.15.2-nox rejects before meshing"),
    limitations = [
        "The cited Eqiora canonical mesh and buffer digests are owned by the accepted mesh evidence seam and are not recomputed by this Julia route.",
        "This is one Linux x86-64, Julia 1.12.6, official Gmsh 4.15.2 numerical derivation; it is not a cross-platform byte-identity claim.",
        "The cylinder vector is the algebraic constrained-vertex force on this mesh, not drag, lift, a coefficient, or a mesh-independent value.",
        "No PDE convergence, Navier-Stokes, transient, curved-element, performance, hosted, or Eqiora production execution claim is made.",
    ],
    checks = (total = length(CHECKS), passed = count(c -> c[2], CHECKS),
              failed = count(c -> !c[2], CHECKS), names = [c[1] for c in CHECKS]),
)

const RECORD_BYTES = canonical_json(RECORD)
const RECORD_SHA256 = bytes2hex(sha256(RECORD_BYTES))
say()
say("-- frozen observations ----------------------------------------------------")
for probe in OBSERVED.velocity_probes
    say("  velocity target=$(probe.target_m) barycentre=$(probe.barycentre_m) u=$(probe.velocity_m_per_s)")
end
for probe in OBSERVED.pressure_probes
    ties = hasproperty(probe, :exact_tie_count) ? probe.exact_tie_count : 1
    say("  pressure $(probe.name) vertex=$(probe.vertex_m) p=$(probe.pressure_pa) Pa ties=$ties")
end
say("  pressure extrema=$(OBSERVED.pressure_extrema)")
say("  fluxes=$(OBSERVED.fluxes)")
say("  cylinder force on fluid=$(OBSERVED.cylinder_force_on_fluid) N/m")
say("  momentum closure=$(OBSERVED.momentum_closure) N/m")
say("  refined LU vs QR=$(LU_QR_DEVIATION)")
say("  reindexing=$(REINDEX_DEVIATION)")
say()
say("record_sha256=$RECORD_SHA256")
say(@sprintf("checks=%d passed=%d failed=%d", length(CHECKS), count(c -> c[2], CHECKS),
             count(c -> !c[2], CHECKS)))

const EXPECTED_DIR = joinpath(HERE, "expected")
const EXPECTED_RECORD = joinpath(EXPECTED_DIR, "julia-route-frozen.json")
const EXPECTED_LOG = joinpath(EXPECTED_DIR, "run-log.txt")
const LOG_BYTES = String(take!(LOG))
const FREEZE = "--freeze" in ARGS
if FREEZE
    any(c -> !c[2], CHECKS) && error("refusing to freeze a failing route")
    mkpath(EXPECTED_DIR)
    write(EXPECTED_RECORD, RECORD_BYTES)
    write(EXPECTED_LOG, LOG_BYTES)
else
    isfile(EXPECTED_RECORD) || error("missing frozen record; run once with --freeze")
    read(EXPECTED_RECORD, String) == RECORD_BYTES || error("frozen record mismatch")
    isfile(EXPECTED_LOG) || error("missing frozen run log")
    read(EXPECTED_LOG, String) == LOG_BYTES || error("frozen run log mismatch")
end

print(LOG_BYTES)
exit(any(c -> !c[2], CHECKS) ? 1 : 0)
