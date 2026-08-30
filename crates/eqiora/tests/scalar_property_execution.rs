use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use eqiora::api::ModelDocument;
use eqiora::artifact::{
    CartesianMeshCellsV1, GeometryMeshCorrespondenceEnvelopeV1, MeshProductionLineageEnvelopeV1,
    ModelEnvelope,
};
use eqiora::geometry::{CanonicalGeometryV1, GeometryGraph, PlanarTopologyHandle};
use eqiora::package::{
    AuthorManifestV1, AuthorPackageSourcesV1, BundleEntryV1, BundleRoleV1, DependencyRequirementV1,
    ExactVersion, InMemoryPackageStore, NormalizedRelativePath, PackageReleaseV1,
    PackagedModelDocument, QualifiedName, ResolutionRecordV1, SourceFileV1,
    prepare_package_release_v1,
};
use eqiora::solver::REFERENCE_LINEAR_SOLVER;
use eqiora_numerics::{
    AuthenticatedCommonMesh, CommonResult, CommonScalarPlan, CommonSolvePolicy,
    CommonSpatialPolicy, resolve_common_plan,
};

const VERSION: &str = "1.0.0";
const SOURCE_PATH: &str = "src/main.eqi";
const PROPERTY_PACKAGE: &str = "org.example.ScalarProperties";
const NORMALIZED_DIFFUSIVITY: f64 = 0.025;
const PARAMETERS: &[(&str, f64)] = &[
    ("wave_number", std::f64::consts::PI),
    ("source_scale", 1.0),
    ("boundary_offset", 0.0),
];

#[test]
fn one_exact_release_runs_through_two_independent_common_scalar_consumers() {
    let geometry = rectangle_geometry();
    let properties = property_release(25, "org.example.measurement", "Diffusivity");

    for consumer in [Consumer::Potential, Consumer::Temperature] {
        let property = compile_property_consumer(&properties, consumer, &geometry)
            .expect("exact property-bound Component compiles against caller Geometry");
        let direct = ModelDocument::compile_with_geometry(
            "direct-scalar-consumer.eqi",
            &consumer.source(false),
            &geometry,
            Some(consumer.wrapper()),
            PARAMETERS,
        )
        .expect("direct Parameter Component compiles against the same Geometry");

        assert_eq!(property.property_bindings().len(), 1);
        let binding = property.property_bindings().next().unwrap();
        assert_eq!(binding.3, consumer.requirement());
        assert_eq!(binding.4, NORMALIZED_DIFFUSIVITY);
        assert_eq!(binding.5, "unconditional");
        assert_eq!(binding.6, "org.example.measurement");
        assert_eq!(binding.7, "spdx.CC0_1_0");
        assert!(property.model().structurally_equivalent(&direct).unwrap());

        let property_plan = resolve_scalar(property.model(), &geometry);
        let direct_plan = resolve_scalar(&direct, &geometry);
        assert_eq!(
            property.compilation().model_digest().to_hex(),
            property_plan.model_digest(),
            "the common Plan must retain the exact package compilation's Model identity"
        );
        assert_ne!(property_plan.identity(), direct_plan.identity());
        let property_result = property_plan.run_result().expect("property Plan runs");
        let direct_result = direct_plan.run_result().expect("direct Plan runs");
        assert_eq!(
            property_result.plan().model_digest(),
            property.compilation().model_digest().to_hex(),
            "the Result must retain the package compilation's Model lineage"
        );
        assert_eq!(
            property.property_bindings().next().unwrap().6,
            "org.example.measurement",
            "execution must not replace exact provenance with the normalized scalar"
        );
        assert_same_scalar_result(&property_result, &direct_result);
    }
}

#[test]
fn provenance_and_value_changes_keep_identity_and_execution_roles_distinct() {
    let geometry = rectangle_geometry();
    let baseline_release = property_release(25, "org.example.measurement", "Diffusivity");
    let provenance_release = property_release(25, "org.example.remeasurement", "Diffusivity");
    let value_release = property_release(50, "org.example.measurement", "Diffusivity");
    let reordered_release = reordered_property_release();

    let baseline = compile_property_consumer(&baseline_release, Consumer::Potential, &geometry)
        .expect("baseline property package compiles");
    let provenance = compile_property_consumer(&provenance_release, Consumer::Potential, &geometry)
        .expect("provenance variant compiles");
    let value = compile_property_consumer(&value_release, Consumer::Potential, &geometry)
        .expect("value variant compiles");
    let reordered = compile_property_consumer(&reordered_release, Consumer::Potential, &geometry)
        .expect("declaration-order variant compiles");
    assert_ne!(
        baseline.model().digest().unwrap(),
        provenance.model().digest().unwrap()
    );
    assert_ne!(
        baseline.model().digest().unwrap(),
        value.model().digest().unwrap()
    );

    let baseline_plan = resolve_scalar(baseline.model(), &geometry);
    let provenance_plan = resolve_scalar(provenance.model(), &geometry);
    let value_plan = resolve_scalar(value.model(), &geometry);
    let reordered_plan = resolve_scalar(reordered.model(), &geometry);
    assert_ne!(baseline_plan.identity(), provenance_plan.identity());
    assert_ne!(baseline_plan.identity(), value_plan.identity());

    let baseline_result = baseline_plan.run_result().expect("baseline runs");
    let provenance_result = provenance_plan
        .run_result()
        .expect("provenance variant runs");
    let value_result = value_plan.run_result().expect("value variant runs");
    let reordered_result = reordered_plan
        .run_result()
        .expect("declaration-order variant runs");
    assert_same_scalar_result(&baseline_result, &provenance_result);
    assert!(
        baseline
            .model()
            .structurally_equivalent(reordered.model())
            .unwrap(),
        "property declaration order must not alter the effective Law"
    );
    assert_same_scalar_result(&baseline_result, &reordered_result);
    assert_ne!(
        baseline_result.field_block(0, 0).unwrap().1,
        value_result.field_block(0, 0).unwrap().1,
        "a changed normalized coefficient must reach numerical execution"
    );

    let wrong_contract = property_release(25, "org.example.measurement", "OtherDiffusivity");
    assert!(
        compile_property_consumer(&wrong_contract, Consumer::Potential, &geometry).is_err(),
        "a nominally foreign release must fail before a Model or Plan is exposed"
    );
}

#[derive(Clone, Copy)]
enum Consumer {
    Potential,
    Temperature,
}

impl Consumer {
    const fn package(self) -> &'static str {
        match self {
            Self::Potential => "org.example.PotentialDiffusion",
            Self::Temperature => "org.example.TemperatureDiffusion",
        }
    }

    const fn core(self) -> &'static str {
        match self {
            Self::Potential => "PotentialDiffusion",
            Self::Temperature => "TemperatureDiffusion",
        }
    }

    const fn wrapper(self) -> &'static str {
        match self {
            Self::Potential => "ExecutablePotentialDiffusion",
            Self::Temperature => "ExecutableTemperatureDiffusion",
        }
    }

    const fn field(self) -> &'static str {
        match self {
            Self::Potential => "potential",
            Self::Temperature => "temperature",
        }
    }

    const fn requirement(self) -> &'static str {
        match self {
            Self::Potential => "diffusivity",
            Self::Temperature => "conductivity",
        }
    }

    fn source(self, property: bool) -> String {
        let coefficient_declaration = if property {
            format!(
                "  public property {}: props.Diffusivity;",
                self.requirement()
            )
        } else {
            format!("  public parameter {}: 1;", self.requirement())
        };
        let coefficient_binding = if property {
            format!(
                "property {} = props.ReferenceDiffusivity",
                self.requirement()
            )
        } else {
            format!("{} = {NORMALIZED_DIFFUSIVITY}", self.requirement())
        };
        format!(
            r#"
public component {core} {{
  public support square: volume(ambient_dimension = 2);
  public support x_lower: boundary(parent = square);
  public support x_upper: boundary(parent = square);
  public support y_lower: boundary(parent = square);
  public support y_upper: boundary(parent = square);
  representation scalar_space = continuum;
  field {field} on square as scalar_space: 1 = 0;
{coefficient_declaration}
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  public parameter boundary_offset: 1;
  relation balance continuous on square {{
    -div({coefficient} * grad({field}))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }}
  relation x_lower_value continuous on x_lower {{ trace({field}) - boundary_offset = 0; }}
  relation x_upper_value continuous on x_upper {{ trace({field}) - boundary_offset = 0; }}
  relation y_lower_value continuous on y_lower {{ trace({field}) - boundary_offset = 0; }}
  relation y_upper_value continuous on y_upper {{ trace({field}) - boundary_offset = 0; }}
}}

public component {wrapper} {{
  public support square: volume(ambient_dimension = 2);
  public support x_lower: boundary(parent = square);
  public support x_upper: boundary(parent = square);
  public support y_lower: boundary(parent = square);
  public support y_upper: boundary(parent = square);
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  public parameter boundary_offset: 1;
  instance equation: {core}(
    support square = square,
    support x_lower = x_lower,
    support x_upper = x_upper,
    support y_lower = y_lower,
    support y_upper = y_upper,
    {coefficient_binding},
    wave_number = wave_number,
    source_scale = source_scale,
    boundary_offset = boundary_offset
  );
}}
"#,
            core = self.core(),
            wrapper = self.wrapper(),
            field = self.field(),
            coefficient = self.requirement(),
        )
    }
}

fn property_release(value: u32, citation: &str, contract: &str) -> PackageReleaseV1 {
    let source = format!(
        r#"
public property contract Diffusivity {{ scalar value: 1; }}
public property contract OtherDiffusivity {{ scalar value: 1; }}
public property release ReferenceDiffusivity implements {contract} {{
  value = {value};
  source_unit: 1 = 1 / 1000;
  validity = unconditional;
  citation = {citation};
  license = spdx.CC0_1_0;
}}
"#
    );
    release(PROPERTY_PACKAGE, &source, &[])
}

fn reordered_property_release() -> PackageReleaseV1 {
    release(
        PROPERTY_PACKAGE,
        r#"
public property contract OtherDiffusivity { scalar value: 1; }
public property contract Diffusivity { scalar value: 1; }
public property release ReferenceDiffusivity implements Diffusivity {
  value = 25;
  source_unit: 1 = 1 / 1000;
  validity = unconditional;
  citation = org.example.measurement;
  license = spdx.CC0_1_0;
}
"#,
        &[],
    )
}

fn compile_property_consumer(
    properties: &PackageReleaseV1,
    consumer: Consumer,
    geometry: &CanonicalGeometryV1,
) -> Result<PackagedModelDocument, String> {
    let root = try_release(
        consumer.package(),
        &consumer.source(true),
        &[("props", properties)],
    )?;
    let resolution =
        ResolutionRecordV1::from_exact_releases(&root, std::slice::from_ref(properties))
            .map_err(|error| error.to_string())?;
    let mut store = InMemoryPackageStore::default();
    store
        .insert(properties)
        .map_err(|error| error.to_string())?;
    store.insert(&root).map_err(|error| error.to_string())?;
    PackagedModelDocument::compile_locked_with_geometry(
        &store,
        &resolution,
        consumer.wrapper(),
        geometry,
        PARAMETERS,
    )
    .map_err(|error| error.to_string())
}

fn release(
    name: &str,
    source: &str,
    dependencies: &[(&str, &PackageReleaseV1)],
) -> PackageReleaseV1 {
    try_release(name, source, dependencies).expect("prepare exact package release")
}

fn try_release(
    name: &str,
    source: &str,
    dependencies: &[(&str, &PackageReleaseV1)],
) -> Result<PackageReleaseV1, String> {
    let path = NormalizedRelativePath::parse(SOURCE_PATH).unwrap();
    let requirements = dependencies
        .iter()
        .map(|(alias, release)| {
            DependencyRequirementV1::new(
                QualifiedName::parse(*alias).unwrap(),
                release.package_identity().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse(name).unwrap(),
        ExactVersion::parse(VERSION).unwrap(),
        requirements,
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .unwrap();
    let sources = AuthorPackageSourcesV1::new(
        manifest,
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            source.as_bytes().to_vec(),
        )],
    )
    .unwrap();
    let exact_dependencies = dependencies
        .iter()
        .map(|(_, release)| (*release).clone())
        .collect::<Vec<_>>();
    prepare_package_release_v1(sources, &exact_dependencies).map_err(|error| error.to_string())
}

fn rectangle_geometry() -> CanonicalGeometryV1 {
    let graph = GeometryGraph::new();
    let rectangle = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
    let edges = rectangle.boundaries();
    graph
        .build(
            &rectangle,
            &BTreeMap::from([
                ("square".to_owned(), vec![rectangle.region().into()]),
                (
                    "x_lower".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[0])],
                ),
                (
                    "x_upper".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[1])],
                ),
                (
                    "y_lower".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[2])],
                ),
                (
                    "y_upper".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[3])],
                ),
            ]),
        )
        .unwrap()
}

fn resolve_scalar(document: &ModelDocument, geometry: &CanonicalGeometryV1) -> CommonScalarPlan {
    let cells = CartesianMeshCellsV1::new([6, 6]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
            geometry,
            cells.cells(),
        )
        .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
        cells,
        geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    let owner = AuthenticatedCommonMesh::structured_cartesian(
        geometry.clone(),
        mesh,
        correspondence,
        production,
    )
    .unwrap();
    let solver =
        CommonSolvePolicy::linear(1.0e-10, 1.0e-12, NonZeroUsize::new(10_000).unwrap()).unwrap();
    let model = ModelEnvelope::from_program(document.program()).unwrap();
    resolve_common_plan(
        &model,
        owner,
        CommonSpatialPolicy::Q1,
        solver,
        None,
        None,
        &REFERENCE_LINEAR_SOLVER,
        None,
    )
    .unwrap()
    .project(
        |_| panic!("scalar consumer resolved as ODE"),
        |plan| plan,
        |_| panic!("scalar consumer resolved as elasticity"),
        |_| panic!("scalar consumer resolved as Stokes"),
        |_| panic!("scalar consumer resolved as transient flow"),
        |_| panic!("scalar consumer resolved as FSI"),
    )
}

fn assert_same_scalar_result(left: &CommonResult, right: &CommonResult) {
    assert_eq!(left.family_name(), "scalar");
    assert_eq!(right.family_name(), "scalar");
    assert_eq!(left.field_count(), 1);
    assert_eq!(right.field_count(), 1);
    let left = left.field_block(0, 0).unwrap();
    let right = right.field_block(0, 0).unwrap();
    assert_eq!(left.0, right.0);
    assert_eq!(left.1, right.1);
    assert_eq!(left.2, right.2);
}
