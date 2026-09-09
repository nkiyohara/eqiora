//! Independently specified conductivity law through exact package source and Model replay.
use eqiora::artifact::ModelEnvelope;
use eqiora::package::*;
use eqiora::sem::{Interpreter, ReferenceConfig};
const SOURCE: &str = r#"
public property contract Conductivity(input temperature: K): kg * m / s^3 / K {
 derivatives first_open_intervals;
}
public property release Samples: Conductivity {
 table {
  data conductivity_samples;
  axis temperature: K;
  value conductivity: kg * m / s^3 / K;
  interpolation piecewise_affine;
  preprocessing identity;
  missing reject;
  knot_derivative reject;
  endpoint_derivative reject;
 }
 validity temperature in [300[K],360[K]];
 outside reject;
 branch single;
 citation synthetic_definition;
 license repository_license;
}
component FourierFlux(parameter T: K, property conductivity: Conductivity) {
 variable flux: kg / s^3;
 relation law { flux = -conductivity(temperature = T) * 2[K / m]; }
}
component SlabConductance(parameter T: K, property conductivity: Conductivity) {
 variable conductance: kg * m^2 / s^3 / K;
 relation law { conductance = conductivity(temperature = T) * 0.1[m]; }
}
public model Main() {
 instance cool_flux: FourierFlux(T = 310[K], conductivity = Samples);
 instance warm_flux: FourierFlux(T = 340[K], conductivity = Samples);
 instance cool_slab: SlabConductance(T = 310[K], conductivity = Samples);
 instance warm_slab: SlabConductance(T = 340[K], conductivity = Samples);
}
"#;
fn package(source: &str) -> Result<PackageReleaseV1, PackagePreparationError> {
    package_assets(
        source,
        vec![300., 10., 320., 14., 360., 18.],
        b"Synthetic conductivity: (300,10), (320,14), (360,18).",
    )
}

fn package_assets(
    source: &str,
    values: Vec<f64>,
    citation: &[u8],
) -> Result<PackageReleaseV1, PackagePreparationError> {
    let array = eqiora_schema::resolved_array::ResolvedF64Array::new(vec![3, 2], values).unwrap();
    let payloads = [
        (
            "src/main.eqi",
            BundleRoleV1::ModelSource,
            source.as_bytes().to_vec(),
        ),
        (
            "data/conductivity_samples.json",
            BundleRoleV1::ResolvedArray,
            array.canonical_json().unwrap(),
        ),
        (
            "docs/synthetic_definition.md",
            BundleRoleV1::Documentation,
            citation.to_vec(),
        ),
        (
            "docs/repository_license.md",
            BundleRoleV1::Documentation,
            b"CC0-1.0 synthetic specimen.".to_vec(),
        ),
    ];
    let entries = payloads
        .iter()
        .map(|(path, role, _)| {
            BundleEntryV1::new(NormalizedRelativePath::parse(*path).unwrap(), *role)
        })
        .collect();
    let manifest = PackageManifestV1::new(
        "main",
        QualifiedName::parse("org.example.Table").unwrap(),
        ExactVersion::parse("1.0.0").unwrap(),
        vec![],
        entries,
    )
    .unwrap();
    let files = payloads
        .into_iter()
        .map(|(path, role, bytes)| {
            SourceFileV1::new(NormalizedRelativePath::parse(path).unwrap(), role, bytes)
        })
        .collect();
    prepare_package_release_v1(PackageSourcesV1::new(manifest, files).unwrap(), &[])
}
#[test]
fn exact_source_table_drives_two_distinct_consumers_and_replays() {
    let release = package(SOURCE).unwrap();
    let resolution = ResolutionRecordV1::from_exact_releases(&release, &[]).unwrap();
    let mut store = InMemoryPackageStore::default();
    store.insert(&release).unwrap();
    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main").unwrap();
    let document = packaged.model();
    let bytes = ModelEnvelope::from_program(document.program())
        .unwrap()
        .canonical_json()
        .unwrap();
    let reopened = ModelEnvelope::from_json(&bytes, Default::default())
        .unwrap()
        .to_program()
        .unwrap();
    assert_eq!(document.program(), &reopened);
    for program in [document.program(), &reopened] {
        let result = Interpreter::new()
            .run(program, ReferenceConfig::new(0., 1.).unwrap())
            .unwrap();
        // Linear interpolation gives k(310)=12 and k(340)=16 independently.
        for (name, expected) in [
            ("cool_flux.flux", -24.),
            ("warm_flux.flux", -32.),
            ("cool_slab.conductance", 1.2),
            ("warm_slab.conductance", 1.6),
        ] {
            let field = document.field_ref(name).unwrap().id().erase();
            assert!((result.last_value(field).unwrap().value() - expected).abs() < 1e-10);
        }
    }
}
#[test]
fn table_attribution_requires_the_actual_package_document_closure() {
    assert!(
        package(&SOURCE.replace(
            "citation synthetic_definition",
            "citation foreign.synthetic_definition"
        ))
        .is_err()
    );
    assert!(package(&SOURCE.replace("license repository_license", "license absent")).is_err());
}

fn evaluate(
    definition: &eqiora::kernel::pure_operator::PureOperatorDefinition,
    point: f64,
) -> Result<f64, ()> {
    use eqiora::kernel::{ExprDagBuilder, ExprNode};
    let mut builder = ExprDagBuilder::new();
    let kind = eqiora::ValueType::scalar(
        eqiora::ScalarDomain::Real,
        definition.formals()[0].dimension().unwrap(),
    )
    .unwrap();
    let argument = builder
        .push(ExprNode::Constant(
            eqiora::ValueLiteral::from_real(kind, point).unwrap(),
        ))
        .unwrap();
    let root = builder.pure_operator(definition, [argument]).unwrap();
    let dag = builder.finish([root]).unwrap();
    let result = eqiora::ir::ScalarOperatorIr::lower(&dag)
        .unwrap()
        .evaluate_typed(&[root], &mut |_| None)
        .map_err(|_| ())?;
    Ok(result[0].real_scalar_value().unwrap().value())
}
#[test]
fn declared_open_derivatives_use_independent_slopes_and_reject_boundaries() {
    let analytic = SOURCE;
    let start = analytic.find(" table {").unwrap();
    let end = analytic[start..].find("\n validity").unwrap() + start;
    let analytic = format!(
        "{} analytic {{ value = 10[kg * m / s^3 / K] + 0.2[kg * m / s^3 / K^2] * (temperature - 300[K]); }}{}",
        &analytic[..start],
        &analytic[end..]
    );
    for (source, expected) in [(SOURCE, [0.2, 0.1]), (analytic.as_str(), [0.2, 0.2])] {
        let release = package(source).unwrap();
        let resolution = ResolutionRecordV1::from_exact_releases(&release, &[]).unwrap();
        let mut store = InMemoryPackageStore::default();
        store.insert(&release).unwrap();
        let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main").unwrap();
        let release = packaged
            .model()
            .program()
            .nodes()
            .find_map(|node| match node {
                eqiora::kernel::KernelNode::Relation(value) => {
                    value.expression().properties().values().next()
                }
                _ => None,
            })
            .unwrap();
        let derivative = release.partial("temperature").unwrap();
        for (point, expected) in [310., 340.].into_iter().zip(expected) {
            assert!((evaluate(&derivative, point).unwrap() - expected).abs() < 1e-12);
        }
        for endpoint in [299., 300., 360., 361.] {
            assert!(evaluate(&derivative, endpoint).is_err());
        }
        if matches!(release.meaning(), eqiora::kernel::PropertyMeaning::Table(_)) {
            assert!(evaluate(&derivative, 320.).is_err());
        }
        let value = release.meaning().definition().unwrap();
        for point in [300., 320., 360.] {
            assert!(evaluate(value, point).is_ok());
        }
        for point in [299., 361.] {
            assert!(evaluate(value, point).is_err());
        }
    }
}

#[test]
fn referenced_document_identity_is_exact_but_not_mathematical_structure() {
    let original = package(SOURCE).unwrap();
    let documentation = package_assets(
        SOURCE,
        vec![300., 10., 320., 14., 360., 18.],
        b"Same synthetic data, revised attribution document.",
    )
    .unwrap();
    let different_data = package_assets(
        SOURCE,
        vec![300., 11., 320., 14., 360., 18.],
        b"Synthetic conductivity: (300,10), (320,14), (360,18).",
    )
    .unwrap();
    let subset = package(&SOURCE.replace("[300[K],360[K]]", "[310[K],350[K]]")).unwrap();
    let compile = |release: &PackageReleaseV1| {
        let resolution = ResolutionRecordV1::from_exact_releases(release, &[]).unwrap();
        let mut store = InMemoryPackageStore::default();
        store.insert(release).unwrap();
        PackagedModelDocument::compile_locked(&store, &resolution, "Main")
            .unwrap()
            .model()
            .clone()
    };
    assert_ne!(
        original.package_identity().unwrap(),
        documentation.package_identity().unwrap()
    );
    assert_ne!(
        original.package_identity().unwrap(),
        different_data.package_identity().unwrap()
    );
    let first = compile(&original);
    let attribution = compile(&documentation);
    assert_ne!(
        first.canonical_json().unwrap(),
        attribution.canonical_json().unwrap()
    );
    assert!(first.structurally_equivalent(&attribution).unwrap());
    assert!(
        !first
            .structurally_equivalent(&compile(&different_data))
            .unwrap()
    );
    assert!(!first.structurally_equivalent(&compile(&subset)).unwrap());
}

#[test]
fn ordinary_residual_derivatives_respect_selected_inputs_at_table_boundaries() {
    use eqiora::ir::{DifferentiationRole, LinearizedRelation, RelationTangent, ScalarOperatorIr};
    use eqiora::kernel::{KernelNode, SymbolRef};
    let declarations = SOURCE.split_once("component FourierFlux").unwrap().0;
    let source = format!(
        r#"{declarations}
component Sensitivity(property conductivity: Conductivity) {{
 variable temperature: K;
 variable phi: K / m;
 variable flux: kg / s^3;
 relation law {{ flux = conductivity(temperature = temperature) * phi; }}
}}
public model Main() {{ instance sample: Sensitivity(conductivity = Samples); }}
"#
    );
    let release = package(&source).unwrap();
    let resolution = ResolutionRecordV1::from_exact_releases(&release, &[]).unwrap();
    let mut store = InMemoryPackageStore::default();
    store.insert(&release).unwrap();
    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main").unwrap();
    let document = packaged.model();
    let temperature = document.field_ref("sample.temperature").unwrap();
    let phi = document.field_ref("sample.phi").unwrap();
    let flux = document.field_ref("sample.flux").unwrap();
    let relation = document
        .program()
        .nodes()
        .find_map(|node| match node {
            KernelNode::Relation(relation) => Some(relation),
            _ => None,
        })
        .unwrap();
    let residual = document
        .program()
        .numerical_residuals(relation.id().erase())
        .unwrap();
    let ir = ScalarOperatorIr::lower(&residual).unwrap();
    for (point, conductivity, slope) in [
        (300., 10., None),
        (310., 12., Some(0.2)),
        (320., 14., None),
        (340., 16., Some(0.1)),
        (360., 18., None),
    ] {
        let inputs = ir
            .symbols()
            .iter()
            .map(|symbol| {
                let field = match symbol {
                    SymbolRef::Field(id) if *id == temperature.id() => {
                        ([0, 0, 0, 0, 1, 0, 0], point)
                    }
                    SymbolRef::Field(id) if *id == phi.id() => ([0, -1, 0, 0, 1, 0, 0], 2.),
                    SymbolRef::Field(id) if *id == flux.id() => ([1, 0, -3, 0, 0, 0, 0], 0.),
                    _ => panic!("unexpected residual input"),
                };
                eqiora::ValueLiteral::from_real(
                    eqiora::ValueType::scalar(
                        eqiora::ScalarDomain::Real,
                        eqiora::DimExponents::from_integers(field.0).unwrap(),
                    )
                    .unwrap(),
                    field.1,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let roles = ir
            .symbols()
            .iter()
            .map(|symbol| {
                if *symbol == SymbolRef::Field(temperature.id()) {
                    DifferentiationRole::Unknown
                } else {
                    DifferentiationRole::Frozen
                }
            })
            .collect::<Vec<_>>();
        if let Some(slope) = slope {
            let linear = ir.linearize_typed(&inputs, &roles).unwrap();
            let mut tangent = [0.];
            linear
                .jvp(RelationTangent::Unknown(&[1.]), &mut tangent)
                .unwrap();
            assert!((tangent[0] + 2. * slope).abs() < 1e-12);
        } else {
            assert!(
                ir.linearize_typed(&inputs, &roles).is_err(),
                "temperature derivative at {point} must reject"
            );
            let full_roles = ir
                .symbols()
                .iter()
                .map(|symbol| {
                    if *symbol == SymbolRef::Field(flux.id()) {
                        DifferentiationRole::Frozen
                    } else {
                        DifferentiationRole::Unknown
                    }
                })
                .collect::<Vec<_>>();
            assert!(
                ir.linearize_typed(&inputs, &full_roles).is_err(),
                "full temperature/phi map has an undefined temperature column"
            );
        }
        let roles = ir
            .symbols()
            .iter()
            .map(|symbol| {
                if *symbol == SymbolRef::Field(phi.id()) {
                    DifferentiationRole::Unknown
                } else {
                    DifferentiationRole::Frozen
                }
            })
            .collect::<Vec<_>>();
        let linear = ir
            .linearize_typed(&inputs, &roles)
            .expect("fixed temperature permits the phi-only derivative map");
        let mut primal = [0.];
        linear.primal(&mut primal).unwrap();
        assert!((primal[0] + 2. * conductivity).abs() < 1e-12);
        let mut tangent = [0.];
        linear
            .jvp(RelationTangent::Unknown(&[1.]), &mut tangent)
            .unwrap();
        assert!((tangent[0] + conductivity).abs() < 1e-12);
    }
}
