use super::*;

pub(crate) fn validate_resources(
    capability: NativeCapability,
    spatial: NativeSpatialPolicy,
    resources: &NativeMeshResources,
) -> Result<(), Diagnostic> {
    match (capability, spatial, resources) {
        (
            NativeCapability::ScalarElliptic,
            NativeSpatialPolicy::ScalarQ1 | NativeSpatialPolicy::ScalarTpfa,
            resources @ NativeMeshResources::Cartesian { .. },
        ) => validate_cartesian_resources(resources),
        (
            NativeCapability::IsotropicElasticity,
            NativeSpatialPolicy::ElasticityQ1,
            resources @ NativeMeshResources::Cartesian { .. },
        ) => validate_cartesian_resources(resources),
        (
            NativeCapability::SteadyIncompressibleStokes,
            NativeSpatialPolicy::StokesMiniP1(_),
            resources @ (NativeMeshResources::ReferenceSimplicial { .. }
            | NativeMeshResources::GmshSimplicial { .. }),
        ) => validate_simplicial_resources(resources),
        (
            NativeCapability::TransientIncompressibleFlow,
            NativeSpatialPolicy::TransientMiniP1(_),
            resources @ NativeMeshResources::AffineTriangleSimplicial { .. },
        ) => validate_simplicial_resources(resources),
        (
            NativeCapability::TransientIncompressibleFlow,
            NativeSpatialPolicy::TransientCellCentered(_),
            resources @ NativeMeshResources::Cartesian { .. },
        ) => validate_cartesian_resources(resources),
        _ => Err(invalid(
            "Model capability, spatial policy, and common Mesh kind are cross-wired",
        )),
    }
}

pub(crate) fn validate_cartesian_resources(
    resources: &NativeMeshResources,
) -> Result<(), Diagnostic> {
    let NativeMeshResources::Cartesian {
        geometry,
        mesh,
        correspondence,
        production,
    } = resources
    else {
        return Err(invalid("authenticated owner is not Cartesian"));
    };
    let policy = production
        .cartesian_cells()
        .ok_or_else(|| invalid("Cartesian resource has a non-Cartesian policy"))?;
    correspondence.validate_against_planar_rectangle_v2_cartesian(
        geometry,
        mesh,
        policy.cells(),
    )?;
    production.validate_against_structured_cartesian_v1_resources(
        policy,
        geometry,
        mesh,
        correspondence,
    )?;
    let [nx, ny] = policy.cells();
    if mesh.dimension() != 2
        || mesh.mesh().axis_cell_count(0) != Some(nx)
        || mesh.mesh().axis_cell_count(1) != Some(ny)
    {
        return Err(invalid(
            "Cartesian Mesh topology differs from its exact production policy",
        ));
    }
    Ok(())
}

pub(crate) fn validate_simplicial_resources(
    resources: &NativeMeshResources,
) -> Result<(), Diagnostic> {
    match resources {
        NativeMeshResources::ReferenceSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        } => {
            let policy = production.planar_mesh_quality().ok_or_else(|| {
                invalid("reference simplicial resource has a non-planar production policy")
            })?;
            correspondence.validate_against_planar_circular_hole_v2_reference(
                geometry,
                mesh,
                policy.maximum_boundary_error_m(),
                policy.maximum_boundary_facets(),
            )?;
            production.validate_against_planar_circular_hole_reference_v1_resources(
                policy,
                geometry,
                mesh,
                correspondence,
            )?;
        }
        NativeMeshResources::AffineTriangleSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        } => {
            let policy = production.affine_triangle_cells().ok_or_else(|| {
                invalid("affine-triangle resource has a non-affine-triangle production policy")
            })?;
            correspondence.validate_against_planar_rectangle_v2_affine_triangles(
                geometry,
                mesh,
                policy.cells(),
            )?;
            production.validate_against_affine_triangle_rectangle_v1_resources(
                policy,
                geometry,
                mesh,
                correspondence,
            )?;
        }
        NativeMeshResources::AdjacentPartitionSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        } => {
            let policy = production.affine_triangle_cells().ok_or_else(|| {
                invalid("adjacent-partition resource has a non-affine-triangle production policy")
            })?;
            correspondence.validate_against_adjacent_rectangle_partition_affine_triangles(
                geometry,
                mesh,
                policy.cells(),
            )?;
            production.validate_against_affine_triangle_rectangle_v1_resources(
                policy,
                geometry,
                mesh,
                correspondence,
            )?;
        }
        NativeMeshResources::GmshSimplicial {
            geometry,
            policy,
            provider_output,
            ..
        } => {
            let replayed =
                derive_gmsh_resources(geometry.clone(), *policy, provider_output.to_vec())?;
            if &replayed != resources {
                return Err(invalid(
                    "Gmsh common Mesh resources differ from exact provider-output replay",
                ));
            }
        }
        NativeMeshResources::Cartesian { .. } => {
            return Err(invalid("authenticated owner is not simplicial"));
        }
    }
    let mesh = match resources {
        NativeMeshResources::ReferenceSimplicial { mesh, .. }
        | NativeMeshResources::AffineTriangleSimplicial { mesh, .. }
        | NativeMeshResources::AdjacentPartitionSimplicial { mesh, .. }
        | NativeMeshResources::GmshSimplicial { mesh, .. } => mesh,
        NativeMeshResources::Cartesian { .. } => unreachable!("rejected above"),
    };
    if mesh.dimension() != 2 {
        return Err(invalid(
            "steady Stokes requires a two-dimensional common Mesh",
        ));
    }
    Ok(())
}

pub(crate) fn derive_gmsh_resources(
    geometry: CanonicalGeometryV1,
    policy: eqiora_artifact::PlanarMeshQualityV1,
    provider_output: Vec<u8>,
) -> Result<NativeMeshResources, Diagnostic> {
    CanonicalGeometryV1::decode_planar_circular_hole_v2_canonical(
        geometry.canonical_bytes(),
        eqiora_geometry::CanonicalGeometryLimits::default(),
    )
    .map_err(|_| invalid("Gmsh provider observation requires exact planar circular-hole v2"))?;
    let quality = eqiora_meshing::MeshQualityGate::new(policy.minimum_mean_ratio())?;
    let importer = GmshSimplexImporter::new(2, quality, GmshImportLimits::default())?;
    let imported = importer.import_ascii_bytes_with_entities(&provider_output)?;
    let (tagged_facets, tagged_cells) = derive_entity_assignments(&imported)?;
    let expected_tags = [1_u32, 5, 6, 7, 8]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    if tagged_facets
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        != expected_tags
    {
        return Err(invalid(
            "Gmsh provider observation has a foreign boundary entity-tag inventory",
        ));
    }
    if tagged_cells.keys().copied().collect::<Vec<_>>() != [1] {
        return Err(invalid(
            "Gmsh provider observation has a foreign source-face entity-tag inventory",
        ));
    }
    let mut source_edge_facets: [Vec<usize>; 5] = std::array::from_fn(|_| Vec::new());
    for (tag, source_edge) in [(1_u32, 4_usize), (5, 2), (6, 1), (7, 3), (8, 0)] {
        source_edge_facets[source_edge] = tagged_facets
            .get(&tag)
            .expect("exact tag inventory checked")
            .clone();
    }
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(imported.mesh())?;
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_mesh_assignments(
            &geometry,
            &mesh,
            source_edge_facets,
        )?;
    let production = MeshProductionLineageEnvelopeV1::from_gmsh_4152_resources(
        policy,
        &geometry,
        &mesh,
        &correspondence,
    )?;
    Ok(NativeMeshResources::GmshSimplicial {
        geometry,
        policy,
        provider_output: provider_output.into_boxed_slice(),
        mesh,
        correspondence,
        production,
    })
}

pub(crate) fn derive_entity_assignments(
    imported: &GmshSimplicialImport,
) -> Result<TaggedMeshAssignments, Diagnostic> {
    let mesh = imported.mesh();
    let dimension = mesh.topological_dimension();
    let facet_dimension = dimension
        .checked_sub(1)
        .ok_or_else(|| invalid("Gmsh simplex Mesh has no boundary stratum"))?;
    let facet_count = mesh
        .entity_count(facet_dimension)
        .ok_or_else(|| invalid("Gmsh simplex Mesh omitted its facet stratum"))?;
    let mut facet_by_vertices = BTreeMap::new();
    let mut boundary_facets = BTreeSet::new();
    for facet_index in 0..facet_count {
        let facet = MeshEntity::new(facet_dimension, facet_index);
        let mut vertices = mesh
            .entity_vertices(facet)
            .ok_or_else(|| invalid("Gmsh Mesh facet omitted its vertex closure"))?
            .into_iter()
            .map(MeshEntity::index)
            .collect::<Vec<_>>();
        vertices.sort_unstable();
        if facet_by_vertices.insert(vertices, facet_index).is_some() {
            return Err(invalid(
                "Gmsh Mesh has duplicate canonical facet connectivity",
            ));
        }
        let parents = mesh
            .incidence(facet, dimension)
            .ok_or_else(|| invalid("Gmsh Mesh facet omitted parent incidence"))?;
        if parents.len() == 1 {
            boundary_facets.insert(facet_index);
        }
    }
    let mut tagged_facets: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut assigned_facets = BTreeSet::new();
    for block in imported
        .element_blocks()
        .iter()
        .filter(|block| block.dimension() == facet_dimension)
    {
        let facets = tagged_facets.entry(block.entity_tag()).or_default();
        for element in block.elements() {
            let mut vertices = element.clone();
            vertices.sort_unstable();
            let facet = *facet_by_vertices
                .get(&vertices)
                .ok_or_else(|| invalid("Gmsh boundary element is absent from Mesh topology"))?;
            if !boundary_facets.contains(&facet) || !assigned_facets.insert(facet) {
                return Err(invalid(
                    "Gmsh boundary assignment is interior or duplicated",
                ));
            }
            facets.push(facet);
        }
    }
    for facets in tagged_facets.values_mut() {
        facets.sort_unstable();
    }
    if assigned_facets != boundary_facets {
        return Err(invalid(
            "Gmsh entity blocks do not assign every Mesh boundary facet",
        ));
    }

    let mut cell_by_vertices = BTreeMap::new();
    for (cell_index, cell) in mesh.cells().iter().enumerate() {
        let mut vertices = cell.clone();
        vertices.sort_unstable();
        if cell_by_vertices.insert(vertices, cell_index).is_some() {
            return Err(invalid("Gmsh Mesh has duplicate canonical cells"));
        }
    }
    let mut tagged_cells: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut assigned_cells = BTreeSet::new();
    for block in imported
        .element_blocks()
        .iter()
        .filter(|block| block.dimension() == dimension)
    {
        let cells = tagged_cells.entry(block.entity_tag()).or_default();
        for element in block.elements() {
            let mut vertices = element.clone();
            vertices.sort_unstable();
            let cell = *cell_by_vertices
                .get(&vertices)
                .ok_or_else(|| invalid("Gmsh top element is absent from Mesh topology"))?;
            if !assigned_cells.insert(cell) {
                return Err(invalid("Gmsh top cell assignment is duplicated"));
            }
            cells.push(cell);
        }
    }
    for cells in tagged_cells.values_mut() {
        cells.sort_unstable();
    }
    if assigned_cells != (0..mesh.cells().len()).collect() {
        return Err(invalid(
            "Gmsh entity blocks do not assign every Mesh top cell",
        ));
    }
    Ok((tagged_facets, tagged_cells))
}
