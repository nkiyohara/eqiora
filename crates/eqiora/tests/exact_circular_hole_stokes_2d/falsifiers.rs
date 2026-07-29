use super::*;

#[test]
fn sparse_lu_fails_closed_at_the_superseded_residual_target() {
    let Err(error) = execute_witness(1.0e-11) else {
        panic!("the superseded residual target unexpectedly accepted");
    };
    assert_eq!(
        error.code(),
        eqiora::diagnostic::codes::NUMERICAL_SOLVE_FAILED
    );
    assert!(error.message().contains("true residual"));
    assert!(error.message().contains("exceeds target"));
}

#[test]
fn geometry_binding_rejects_a_foreign_exact_source_revision() {
    let source = exact_source();
    let owner = frozen_owner(&source);
    let geometry = GeometryDefinitionV1::from_region(owner.region());
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(owner.mesh()).expect("mesh artifact");
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)
        .expect("correspondence");
    let foreign = circular_source([0.21, 0.2], [0, 1], vec![2, 3], 4);
    let program = geometry_program_from_text(&foreign, SOURCE);
    let error = SteadyStokesGeometryBinding2d::new(
        &program,
        foreign,
        owner,
        geometry,
        mesh,
        correspondence,
    )
    .expect_err("foreign source revision must reject before assembly");
    assert!(error.message().contains("another exact source revision"));
}

#[test]
fn geometry_binding_rejects_a_stale_mesh_correspondence() {
    let source = exact_source();
    let fine = frozen_owner(&source);
    let fine_geometry = GeometryDefinitionV1::from_region(fine.region());
    let fine_mesh = SimplicialMeshEnvelopeV1::from_mesh(fine.mesh()).expect("fine mesh artifact");
    let stale = GeometryMeshCorrespondenceEnvelopeV1::from_region(&fine_geometry, &fine_mesh)
        .expect("fine correspondence");

    let coarse = CircularHoleChordalMeshV1::from_exact(
        &source,
        0.2,
        8,
        MeshQualityGate::new(1.0e-8).expect("coarse quality gate"),
    )
    .expect("coarse owner");
    let coarse_geometry = GeometryDefinitionV1::from_region(coarse.region());
    let coarse_mesh =
        SimplicialMeshEnvelopeV1::from_mesh(coarse.mesh()).expect("coarse mesh artifact");
    let program = geometry_program_from_text(&source, SOURCE);
    let error = SteadyStokesGeometryBinding2d::new(
        &program,
        source,
        coarse,
        coarse_geometry,
        coarse_mesh,
        stale,
    )
    .expect_err("stale correspondence must reject before assembly");
    assert!(
        error.message().contains("correspondence")
            || error.message().contains("artifact reference")
    );
}

#[test]
fn incomplete_circle_partition_rejects_before_stokes_assembly() {
    let source = circular_source([0.2, 0.2], [0, 1], vec![2], 3);
    let error = match execute_source_witness(source, SOURCE, 1.0e-6) {
        Ok(_) => panic!("an omitted circular boundary unexpectedly executed"),
        Err(error) => error,
    };
    assert!(
        error
            .message()
            .contains("must cover every boundary facet exactly once")
    );
}

#[test]
fn swapped_inlet_and_outlet_membership_misses_the_frozen_oracle() {
    let swapped = circular_source([0.2, 0.2], [1, 0], vec![2, 3], 4);
    let witness =
        execute_source_witness(swapped, SOURCE, 1.0e-6).expect("swapped witness still solves");
    let mesh = witness.solution.velocity().mesh();
    let inlet_flux = signed_flux(
        mesh,
        witness.solution.velocity().vertex_values(),
        &witness.inlet_facets,
    );
    let reaction = witness
        .solution
        .named_boundary_reaction("cylinder")
        .expect("swapped witness retains the named surface");
    let oracle = frozen_oracle();
    let expected_reaction = oracle
        .observations
        .cylinder_reaction_n_m
        .constraint_force_on_fluid;
    assert!(
        (inlet_flux - oracle.observations.signed_flux_m2_s.inlet).abs() > flux_tolerance()
            || (reaction[0] - expected_reaction[0]).abs() > reaction_tolerance()
            || (reaction[1] - expected_reaction[1]).abs() > reaction_tolerance(),
        "swapping inlet and outlet unexpectedly preserved every frozen observation"
    );
}

#[test]
fn reversing_the_inlet_normal_term_flips_the_signed_flux_oracle() {
    let reversed_normal = SOURCE.replace(
        "trace(velocity) + normal(isotropic_lift(inlet_profile)) = 0;",
        "trace(velocity) - normal(isotropic_lift(inlet_profile)) = 0;",
    );
    assert_ne!(reversed_normal, SOURCE);
    let witness = execute_source_witness(exact_source(), &reversed_normal, 1.0e-6)
        .expect("reversed inlet-normal witness still solves");
    let inlet_flux = signed_flux(
        witness.solution.velocity().mesh(),
        witness.solution.velocity().vertex_values(),
        &witness.inlet_facets,
    );
    let expected = frozen_oracle().observations.signed_flux_m2_s.inlet;
    assert!(
        (inlet_flux - expected).abs() > flux_tolerance(),
        "reversing the inlet normal term unexpectedly preserved signed flux"
    );
}

#[test]
fn a_traction_cylinder_cannot_be_named_as_a_constrained_reaction_surface() {
    let cylinder_traction = SOURCE.replace(
        "relation upper_wall continuous on y_upper { trace(velocity) = 0; }",
        r#"relation upper_wall continuous on y_upper {
    normal(
      2 * dynamic_viscosity * symmetric_part(grad(velocity))
      - isotropic_lift(pressure)
    ) = 0;
  }"#,
    );
    assert_ne!(cylinder_traction, SOURCE);
    let error = match execute_source_witness(exact_source(), &cylinder_traction, 1.0e-6) {
        Ok(_) => panic!("a traction cylinder unexpectedly reported a constrained reaction"),
        Err(error) => error,
    };
    assert!(error.message().contains("unconstrained vertex"));
}

#[test]
fn geometric_observation_selectors_ignore_vertex_and_cell_indices() {
    let source = exact_source();
    let owner = frozen_owner(&source);
    let mesh = owner.mesh();
    let geometry = GeometryDefinitionV1::from_region(owner.region());
    let envelope = SimplicialMeshEnvelopeV1::from_mesh(mesh).expect("mesh artifact");
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &envelope)
        .expect("correspondence");
    let cylinder_facets = correspondence
        .region_entity_set_entities(&geometry, "cylinder")
        .expect("cylinder facets");
    let exterior_facets = ["inlet", "outlet", "walls"]
        .into_iter()
        .flat_map(|name| {
            correspondence
                .region_entity_set_entities(&geometry, name)
                .expect("exterior facets")
        })
        .collect::<Vec<_>>();
    let cylinder_vertices = facet_vertices(mesh, &cylinder_facets);
    let exterior_vertices = facet_vertices(mesh, &exterior_facets);

    let old_to_new = (0..mesh.vertices().len())
        .map(|old| mesh.vertices().len() - 1 - old)
        .collect::<Vec<_>>();
    let vertices = mesh.vertices().iter().rev().cloned().collect::<Vec<_>>();
    let cells = mesh
        .cells()
        .iter()
        .rev()
        .map(|cell| cell.iter().map(|old| old_to_new[*old]).collect())
        .collect();
    let reindexed = SimplicialMesh::new(2, vertices, cells, mesh.quality_gate())
        .expect("index-only mesh permutation");
    let remapped_cylinder = cylinder_vertices
        .iter()
        .map(|old| old_to_new[*old])
        .collect::<BTreeSet<_>>();
    let remapped_exterior = exterior_vertices
        .iter()
        .map(|old| old_to_new[*old])
        .collect::<BTreeSet<_>>();

    for target in [
        [0.10, 0.20],
        [0.20, 0.30],
        [0.30, 0.20],
        [1.00, 0.20],
        [2.00, 0.20],
    ] {
        let original = select_cell_by_barycentre(mesh, target);
        let permuted = select_cell_by_barycentre(&reindexed, target);
        assert_eq!(
            cell_coordinate_key(mesh, original),
            cell_coordinate_key(&reindexed, permuted)
        );
    }
    for (axis, maximum) in [(0, false), (0, true), (1, false), (1, true)] {
        let original = select_extreme_vertex(mesh, &cylinder_vertices, axis, maximum);
        let permuted = select_extreme_vertex(&reindexed, &remapped_cylinder, axis, maximum);
        assert_eq!(
            vertex_coordinate(mesh, original),
            vertex_coordinate(&reindexed, permuted)
        );
    }
    for target in [[0.0, 0.20], [2.2, 0.20]] {
        let original = select_nearest_vertex(mesh, &exterior_vertices, target);
        let permuted = select_nearest_vertex(&reindexed, &remapped_exterior, target);
        assert_eq!(
            vertex_coordinate(mesh, original),
            vertex_coordinate(&reindexed, permuted)
        );
    }
}
