//! Ordinary packaged controls, checked against independently solved recurrences.
use eqiora::api::ModelDocument;
use eqiora::kernel::{ClockDomainDef, FieldRole, KernelNode, RationalTime};
use eqiora::package::{
    BundleEntryV1, BundleRoleV1, ExactVersion, InMemoryPackageStore, NormalizedRelativePath,
    PackageDependencyV1, PackageManifestV1, PackageSourcesV1, PackagedModelDocument, QualifiedName,
    ResolutionRecordV1, SourceFileV1, prepare_package_release_v1,
};
use eqiora::sem::{ExecutionSession, Interpreter, ReferenceConfig};
use eqiora::{Id, RawId, ValueLiteral};

#[path = "support/embedded_package.rs"]
mod embedded_package;

const CONTROLS: &str = include_str!("../../../packages/Eqiora.Controls.Sampled/src/sampled.eqi");
const PHYSICAL: &str = include_str!("../../../examples/standard-sampled-components/src/main.eqi");
const IMPORT: &str = "import Eqiora.Controls.Sampled.sampled as controls;";
const ROOT: &str = r#"
import Eqiora.Controls.Sampled.sampled as controls;
model Main(input u: 1 at tick, input rate: 1/s at tick,
           output delayed: 1 at tick, output before: 1 at tick, output after: 1 at tick) {
  clock tick = periodic(0.25[s]);
  instance delay: controls.UnitDelay(tick = tick, initial_value = 5);
  instance integral: controls.DiscreteIntegrator(tick = tick, initial_value = 3);
  connect u -> delay.u;
  connect rate -> integral.rate;
  connect delay.y -> delayed;
  connect integral.before -> before;
  connect integral.after -> after;
}
"#;

fn replace_exact(source: &str, needle: &str, replacement: &str, count: usize) -> String {
    assert_eq!(
        source.matches(needle).count(),
        count,
        "fixture needle: {needle}"
    );
    let result = source.replace(needle, replacement);
    assert_ne!(result, source, "mutation must alter the specimen");
    result
}

fn clock(denominator: u64) -> ClockDomainDef {
    ClockDomainDef::periodic(
        Id::new(),
        RationalTime::new(1, denominator).unwrap(),
        RationalTime::ZERO,
    )
    .unwrap()
}

fn source_document(
    controls: &str,
    root: &str,
    denominator: u64,
) -> Result<ModelDocument, Vec<eqiora::Diagnostic>> {
    let source = format!(
        "{controls}\n{}",
        replace_exact(&period_source(root, denominator), IMPORT, "", 1).replace("controls.", "")
    );
    ModelDocument::compile_selected("sampled.eqi", &source, "Main", &[])
}

fn period_source(root: &str, denominator: u64) -> String {
    assert!(matches!(denominator, 2 | 4));
    let needle = "clock tick = periodic(0.25[s]);";
    assert_eq!(root.matches(needle).count(), 1);
    if denominator == 4 {
        root.to_owned()
    } else {
        replace_exact(root, needle, "clock tick = periodic(0.5[s]);", 1)
    }
}

fn locked_document(controls: &str, root: &str, denominator: u64) -> PackagedModelDocument {
    let sources = embedded_package::sources(
        include_bytes!("../../../packages/Eqiora.Controls.Sampled/package.json"),
        &[
            (
                "README.md",
                BundleRoleV1::Documentation,
                include_bytes!("../../../packages/Eqiora.Controls.Sampled/README.md"),
            ),
            (
                "src/sampled.eqi",
                BundleRoleV1::ModelSource,
                controls.as_bytes(),
            ),
        ],
    );
    let controls = prepare_package_release_v1(sources, &[]).unwrap();
    let path = NormalizedRelativePath::parse("src/main.eqi").unwrap();
    let manifest = PackageManifestV1::new(
        "main",
        QualifiedName::parse("org.example.SampledControls").unwrap(),
        ExactVersion::parse("0.1.0").unwrap(),
        vec![PackageDependencyV1::new(
            controls.package_identity().unwrap(),
        )],
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .unwrap();
    let sources = PackageSourcesV1::new(
        manifest,
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            period_source(root, denominator).into_bytes(),
        )],
    )
    .unwrap();
    let root = prepare_package_release_v1(sources, std::slice::from_ref(&controls)).unwrap();
    let resolution =
        ResolutionRecordV1::from_exact_releases(&root, std::slice::from_ref(&controls)).unwrap();
    let mut store = InMemoryPackageStore::default();
    store.insert(&controls).unwrap();
    store.insert(&root).unwrap();
    PackagedModelDocument::compile_selected(&store, &resolution, "Main", &[]).unwrap()
}

fn inputs(
    document: &ModelDocument,
    names: &[(&str, [f64; 3])],
    tick: RawId,
) -> Vec<(RawId, RawId, Vec<ValueLiteral>)> {
    names
        .iter()
        .map(|(name, samples)| {
            let id = document.aliases()[*name];
            let Some(KernelNode::Port(port)) = document.program().node(id) else {
                panic!("input port")
            };
            let (_, ty) = port.signal_contract().unwrap();
            let values = samples
                .map(|value| ValueLiteral::new(ty.clone(), [(value, 0.)]).unwrap())
                .to_vec();
            (id, tick, values)
        })
        .collect()
}

fn config(denominator: u64) -> ReferenceConfig {
    let period = 1. / denominator as f64;
    ReferenceConfig::new(2. * period, period)
        .unwrap()
        .with_nonlinear_tolerances(1e-12, 0.)
        .unwrap()
}

fn check_outputs(
    session: &ExecutionSession,
    document: &ModelDocument,
    denominator: u64,
    names: [&str; 3],
    scale: f64,
    position: bool,
) {
    // Independent closed recurrence, not a second execution of the implementation.
    let expected = match (denominator, position) {
        (4, false) => [[5., 2., -1.], [3., 3.5, 3.25], [3.5, 3.25, 4.]],
        (2, false) => [[5., 2., -1.], [3., 4., 3.5], [4., 3.5, 5.]],
        (4, true) => [[5., -1., 4.], [3., 2.75, 3.75], [2.75, 3.75, 4.25]],
        (2, true) => [[5., -1., 4.], [3., 2.5, 4.5], [2.5, 4.5, 5.5]],
        _ => unreachable!(),
    };
    for (name, expected) in names.into_iter().zip(expected) {
        for (index, value) in expected.into_iter().enumerate() {
            let (time, actual) = session
                .output(document.aliases()[name], index as u64)
                .unwrap();
            assert_eq!(time, RationalTime::new(index as u64, denominator).unwrap());
            // At most three ticks, <32 linear equation dependencies and scale
            // factors <=2: 256 absolute residual errors bound this tiny system.
            assert!((actual.real_scalar_value().unwrap().value() - value * scale).abs() <= 256e-12);
        }
        assert!(session.output(document.aliases()[name], 3).is_none());
    }
}

fn reversed_controls() -> String {
    let delay = replace_exact(
        CONTROLS,
        "    y = pre(memory);\n    next(memory) = u;",
        "    next(memory) = u;\n    y = pre(memory);",
        1,
    );
    replace_exact(
        &delay,
        "    next(memory) = pre(memory) + period(tick) * rate;\n    before = pre(memory);\n    after = next(memory);",
        "    after = next(memory);\n    before = pre(memory);\n    next(memory) = pre(memory) + period(tick) * rate;",
        1,
    )
}

#[test]
fn source_and_locked_blocks_execute_both_periods_and_both_equation_orders() {
    for denominator in [4, 2] {
        for controls in [CONTROLS.to_owned(), reversed_controls()] {
            let direct = source_document(&controls, ROOT, denominator).unwrap();
            let locked = locked_document(&controls, ROOT, denominator);
            for document in [&direct, locked.model()] {
                assert_eq!(document.program().nodes().filter(|node| matches!(node, KernelNode::Field(field) if field.role() == FieldRole::State)).count(), 2);
                let mut session = Interpreter::new()
                    .execution_session(
                        document.program(),
                        config(denominator),
                        inputs(
                            document,
                            &[("u", [2., -1., 3.]), ("rate", [2., -1., 3.])],
                            document.aliases()["tick"],
                        ),
                    )
                    .unwrap();
                for name in ["delayed", "before", "after"] {
                    assert!(session.output(document.aliases()[name], 0).is_none());
                }
                assert_eq!(session.advance_ticks(1).unwrap(), 1);
                let mut resumed = Interpreter::new()
                    .resume_execution(document.program(), &session.checkpoint())
                    .unwrap();
                assert_eq!(session.advance_ticks(2).unwrap(), 2);
                assert_eq!(resumed.advance_ticks(2).unwrap(), 2);
                for result in [&session, &resumed] {
                    check_outputs(
                        result,
                        document,
                        denominator,
                        ["delayed", "before", "after"],
                        1.,
                        false,
                    );
                }
            }
        }
    }
}

#[test]
fn voltage_and_position_adapters_execute_without_another_memory_owner() {
    for denominator in [4, 2] {
        let direct = source_document(CONTROLS, PHYSICAL, denominator).unwrap();
        let locked = locked_document(CONTROLS, PHYSICAL, denominator);
        for document in [&direct, locked.model()] {
            assert_eq!(document.program().nodes().filter(|node| matches!(node, KernelNode::Field(field) if field.role() == FieldRole::State)).count(), 4);
            let mut session = Interpreter::new()
                .execution_session(
                    document.program(),
                    config(denominator),
                    inputs(
                        document,
                        &[
                            ("voltage", [4., -2., 6.]),
                            ("slew", [4., -2., 6.]),
                            ("position", [-0.5, 2., 1.]),
                            ("velocity", [-0.5, 2., 1.]),
                        ],
                        document.aliases()["tick"],
                    ),
                )
                .unwrap();
            session.advance_ticks(3).unwrap();
            check_outputs(
                &session,
                document,
                denominator,
                ["delayed_voltage", "voltage_before", "voltage_after"],
                2.,
                false,
            );
            check_outputs(
                &session,
                document,
                denominator,
                ["delayed_position", "position_before", "position_after"],
                0.5,
                true,
            );
        }
    }
}

#[test]
fn wrong_units_missing_initialization_and_foreign_clocks_fail_closed() {
    for (source, fragment) in [
        (
            replace_exact(ROOT, "input rate: 1/s", "input rate: V/s", 1),
            "signal Connection requires dimension-matched inputs",
        ),
        (
            replace_exact(ROOT, ", initial_value = 5", "", 1),
            "required Parameter `initial_value`",
        ),
        (
            replace_exact(
                &replace_exact(
                    ROOT,
                    "instance integral:",
                    "clock foreign = periodic(0.25[s]); instance integral:",
                    1,
                ),
                "controls.DiscreteIntegrator(tick = tick",
                "controls.DiscreteIntegrator(tick = foreign",
                1,
            ),
            "activation",
        ),
        (
            replace_exact(PHYSICAL, "input velocity: m / s", "input velocity: m", 2),
            "dimension",
        ),
    ] {
        let errors = source_document(CONTROLS, &source, 4).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.code().0 == "EQ0603" && error.message().contains(fragment)),
            "{fragment}: {errors:?}"
        );
    }
    let uninitialized = source_document(
        &replace_exact(
            CONTROLS,
            "  initial { pre(memory) = initial_value; }\n",
            "",
            2,
        ),
        ROOT,
        4,
    )
    .unwrap();
    let errors = Interpreter::new()
        .execution_session(
            uninitialized.program(),
            config(4),
            inputs(
                &uninitialized,
                &[("u", [2., -1., 3.]), ("rate", [2., -1., 3.])],
                uninitialized.aliases()["tick"],
            ),
        )
        .unwrap_err();
    assert!(
        errors.iter().any(|error| error.code().0 == "EQ0503"
            && error.message().contains("found 0 equations and 2 unknowns")
            && error.graph_path().is_some_and(|path| path
                .segments()
                .iter()
                .any(|segment| segment == "initialization"))),
        "missing initial equations: {errors:?}"
    );
    let valid = source_document(CONTROLS, ROOT, 4).unwrap();
    let errors = Interpreter::new()
        .execution_session(
            valid.program(),
            config(4),
            inputs(
                &valid,
                &[("u", [2., -1., 3.]), ("rate", [2., -1., 3.])],
                clock(4).id().erase(),
            ),
        )
        .unwrap_err();
    assert!(
        errors.iter().any(|error| error.code().0 == "EQ0501"
            && error.message().contains("exact declared ClockDomain")),
        "{errors:?}"
    );
}

#[test]
fn cross_wired_normalized_occurrences_fail_the_independent_voltage_oracle() {
    let wrong = replace_exact(
        PHYSICAL,
        "connect volts.rate -> voltage_integral.rate;",
        "connect metres.rate -> voltage_integral.rate;",
        1,
    );
    let document = source_document(CONTROLS, &wrong, 4).unwrap();
    let mut session = Interpreter::new()
        .execution_session(
            document.program(),
            config(4),
            inputs(
                &document,
                &[
                    ("voltage", [4., -2., 6.]),
                    ("slew", [4., -2., 6.]),
                    ("position", [-0.5, 2., 1.]),
                    ("velocity", [-0.5, 2., 1.]),
                ],
                document.aliases()["tick"],
            ),
        )
        .unwrap();
    session.advance_ticks(1).unwrap();
    let actual = session
        .output(document.aliases()["voltage_after"], 0)
        .unwrap()
        .1
        .real_scalar_value()
        .unwrap()
        .value();
    // Correct voltage is 2*(3+(1/4)*2)=7 V. Wrong-rate -1 produces
    // 2*(3-(1/4))=5.5 V: a 1.5 V gap, independent of execution output.
    assert!((actual - 5.5).abs() <= 256e-12);
    assert!((actual - 7.).abs() > 1.);
}
