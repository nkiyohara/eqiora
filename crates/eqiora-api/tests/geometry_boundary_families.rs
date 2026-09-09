use eqiora_api::ModelDocument;
use eqiora_artifact::AcceptedModelArtifact;
use eqiora_compiler::StaticBindingValue;
use eqiora_geometry::{CanonicalGeometryV1, GeometryGraph};
use eqiora_graph::GraphStore;
use eqiora_schema::kernel::KernelNode;

const SOURCE: &str = r#"public connector MechanicalBoundary {
  trace displacement: m;
  flux traction: kg * m ^ -1 * s ^ -2;
  shape spatial_vector;
  frame spatial;
  pairing euclidean_boundary_duality;
  orientation parent_outward;
}

public component ExteriorLaw(
  support body: volume(ambient_dimension = 2),
  support exterior: complete_exterior(parent = body),
  port mechanical[boundary in exterior]: MechanicalBoundary over boundary,
) {
  relation law[boundary in exterior] on boundary {
    mechanical[boundary = boundary].displacement - mechanical[boundary = boundary].displacement = 0;
    mechanical[boundary = boundary].traction - mechanical[boundary = boundary].traction = 0;
  }
}

public component ExteriorWrapper(
  support body: volume(ambient_dimension = 2),
  support exterior: complete_exterior(parent = body),
  port mechanical[boundary in exterior]: MechanicalBoundary over boundary,
) {
  connect [boundary in exterior] child.mechanical[boundary = boundary], mechanical[boundary = boundary];
  instance child: ExteriorLaw(body = body, exterior = exterior);
}

public component BoundaryTerminal(
  support body: volume(ambient_dimension = 2),
  support face: boundary(parent = body),
  port mechanical: MechanicalBoundary over face,
) {
  relation law on face {
    mechanical.displacement - mechanical.displacement = 0;
    mechanical.traction - mechanical.traction = 0;
  }
}

public model Main(
  support body: volume(ambient_dimension = 2),
  support left: boundary(parent = body),
  support right: boundary(parent = body),
  support bottom: boundary(parent = body),
  support top: boundary(parent = body),
) {
  connect wrapped.mechanical[boundary = left], left_terminal.mechanical;
  connect wrapped.mechanical[boundary = right], right_terminal.mechanical;
  connect wrapped.mechanical[boundary = bottom], bottom_terminal.mechanical;
  connect wrapped.mechanical[boundary = top], top_terminal.mechanical;
  instance wrapped: ExteriorWrapper(body = body, exterior = boundaries(left, right, bottom, top));
  instance left_terminal: BoundaryTerminal(body = body, face = left);
  instance right_terminal: BoundaryTerminal(body = body, face = right);
  instance bottom_terminal: BoundaryTerminal(body = body, face = bottom);
  instance top_terminal: BoundaryTerminal(body = body, face = top);
}
"#;

fn geometry() -> CanonicalGeometryV1 {
    let graph = GeometryGraph::new();
    let rectangle = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
    let boundaries = rectangle.boundaries();
    graph
        .build(
            &rectangle,
            &std::collections::BTreeMap::from([
                ("body".to_owned(), vec![rectangle.region().into()]),
                ("left".to_owned(), vec![boundaries[0].into()]),
                ("right".to_owned(), vec![boundaries[1].into()]),
                ("bottom".to_owned(), vec![boundaries[2].into()]),
                ("top".to_owned(), vec![boundaries[3].into()]),
            ]),
        )
        .unwrap()
}

fn compile(
    source: &str,
    geometry: &CanonicalGeometryV1,
) -> Result<ModelDocument, Vec<eqiora_core::Diagnostic>> {
    let bindings = ["body", "left", "right", "bottom", "top"].map(|name| {
        (
            name,
            StaticBindingValue::GeometrySupport {
                geometry,
                selection: geometry.entity_set(name).unwrap(),
                parent: (name != "body").then(|| geometry.entity_set("body").unwrap()),
            },
        )
    });
    let document =
        ModelDocument::compile_selected("geometry-exterior.eqi", source, "Main", &bindings)?;
    let module = eqiora_lang::Module::parse("geometry-exterior.eqi", source)?;
    let native = ModelDocument::compile_module(&module, Some("Main"), &bindings)?;
    assert_eq!(
        document.canonical_json().unwrap(),
        native.canonical_json().unwrap()
    );
    Ok(document)
}

#[test]
fn geometry_spatial_vector_family_admits_and_replays_exact_same_support_junctions() {
    let geometry = geometry();
    let document = compile(SOURCE, &geometry).unwrap();
    let bytes = document.canonical_json().unwrap();
    assert!(ModelDocument::replay(&bytes).is_err());
    let artifact = AcceptedModelArtifact::from_json(&bytes, Default::default()).unwrap();
    let (transaction, model) = artifact.to_transaction().unwrap();
    let store = eqiora_graph::InMemoryGraphStore::restore_snapshot(
        transaction,
        eqiora_graph::Revision(document.program().revision().0),
    )
    .unwrap();
    let replay = eqiora_sem::KernelProgram::from_snapshot_with_geometry(
        &store.snapshot(),
        model,
        &[&geometry],
    )
    .unwrap();
    assert_eq!(&replay, document.program());
    let module = eqiora_lang::Module::from_document(
        eqiora_lang::parse("geometry-exterior.eqi", SOURCE)
            .into_document()
            .unwrap(),
    );
    let source_replay = compile(&eqiora_lang::format(module.document()), &geometry).unwrap();
    assert_eq!(source_replay.canonical_json().unwrap(), bytes);
    for program in [document.program(), &replay] {
        let connections = program
            .nodes()
            .filter_map(|node| match node {
                KernelNode::Connection(connection) => Some(connection.id()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(connections.len(), 4);
        for connection in connections {
            let residual = program
                .compose_boundary_physical_junction(connection)
                .unwrap();
            assert!(matches!(
                residual.geometry(),
                eqiora_sem::BoundaryJunctionGeometry::Coincident
            ));
            assert_eq!(residual.dag().roots().len(), 2);
        }
    }
}

#[test]
fn geometry_family_rejects_different_exact_support_even_with_same_connector() {
    let geometry = geometry();
    let wrong_face = SOURCE.replace("face = left);", "face = right);");
    let errors = compile(&wrong_face, &geometry).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("NoncoincidentBoundaries")),
        "{errors:?}"
    );
}
