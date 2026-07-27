use std::num::NonZeroUsize;

use eqiora::EntityKind;
use eqiora::api::{ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod};
use eqiora::compatibility::ExactModelCodec;
use eqiora::kernel::KernelNode;
use eqiora::realization::RealizationRevision;
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods, PyModule};

#[test]
fn python_native_modeling_crosses_only_shared_rust_contracts() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let module = native.bind(py);
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;

        py.run(
            c_str!(
                r#"
x = eqiora.Field("x", initial=1.0)
rate = eqiora.Parameter(
    "rate",
    dimension=eqiora.Dimension(time=-1),
    value=1.0,
)
flow = eqiora.Relation(
    "flow",
    residual=eqiora.derivative(x) + rate * x,
)
scalar_model = eqiora.Model.define("decay", x, rate, flow)

voltage = eqiora.Dimension(mass=1, length=2, time=-3, current=-1)
current = eqiora.Dimension(current=1)
electrical = eqiora.PhysicalDomain(
    "electrical",
    across_dimension=voltage,
    through_dimension=current,
)
left = eqiora.ConservingPort("left", domain=electrical)
right = eqiora.ConservingPort("right", domain=electrical)
tap = eqiora.ConservingPort("tap", domain=electrical)
component = eqiora.Relation(
    "component",
    residuals=[
        eqiora.across(left) - eqiora.across(tap),
        eqiora.through(right) + eqiora.through(tap),
    ],
)
connection = eqiora.connect(left, right, tap)
physical_model = eqiora.Model.define(
    "physical_pair",
    electrical,
    left,
    right,
    tap,
    component,
    connection,
)

interval = eqiora.Domain.box("interval", (0.0, 1.0))
lower_end = interval.boundary(
    "lower_end",
    axis=0,
    side=eqiora.BoundarySide.Lower,
)
upper_end = interval.boundary(
    "upper_end",
    axis=0,
    side=eqiora.BoundarySide.Upper,
)
scalar_space = eqiora.Representation.continuum("scalar_space")
potential = eqiora.Field(
    "potential",
    domain=interval,
    representation=scalar_space,
)
source_scale = eqiora.Parameter(
    "source_scale",
    dimension=eqiora.Dimension(length=-2),
    value=1.0,
)
spatial_model = eqiora.Model.define(
    "native_poisson",
    source_scale,
    upper_end,
    interval,
    potential,
    scalar_space,
    lower_end,
    eqiora.Relation(
        "upper_value",
        domain=upper_end,
        residual=eqiora.trace(potential),
    ),
    eqiora.Relation(
        "balance",
        domain=interval,
        residual=-eqiora.div(eqiora.grad(potential)) - source_scale,
    ),
    eqiora.Relation(
        "lower_value",
        domain=lower_end,
        residual=eqiora.trace(potential),
    ),
)
"#
            ),
            None,
            Some(&locals),
        )?;

        let scalar = replay_python_model(&locals, "scalar_model");
        let source_scalar = eqiora::api::ModelDocument::compile(
            "source-decay.eqi",
            r#"
model source_decay {
  parameter coefficient: 1 / s = 1;
  field state: 1 = 1;
  relation balance continuous { derivative(state) + coefficient * state = 0; }
}
"#,
        )
        .unwrap();
        assert_ne!(scalar.digest().unwrap(), source_scalar.digest().unwrap());
        assert_eq!(
            scalar.structural_fingerprint().unwrap(),
            source_scalar.structural_fingerprint().unwrap()
        );
        assert!(scalar.structurally_equivalent(&source_scalar).unwrap());
        let result = scalar.run_reference(0.2, 0.1).unwrap();
        let series = result.series();
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].time(), &[0.0, 0.1, 0.2]);
        assert_eq!(series[0].values()[0], 1.0);
        assert!((series[0].values()[1] - 1.0 / 1.1).abs() < 1.0e-14);
        assert!((series[0].values()[2] - 1.0 / 1.1_f64.powi(2)).abs() < 1.0e-14);

        let physical = replay_python_model(&locals, "physical_model");
        let source_physical = eqiora::api::ModelDocument::compile(
            "source-physical.eqi",
            r#"
model source_physical {
  domain pin = scalar_physical(
    across = kg * m ^ 2 / (s ^ 3 * A),
    through = A
  );
  port a: conserving on pin;
  port b: conserving on pin;
  port c: conserving on pin;
  relation law continuous {
    across(a) - across(c) = 0;
    through(b) + through(c) = 0;
  }
  connect conserving a, b, c;
}
"#,
        )
        .unwrap();
        assert_ne!(
            physical.digest().unwrap(),
            source_physical.digest().unwrap()
        );
        assert_eq!(
            physical.structural_fingerprint().unwrap(),
            source_physical.structural_fingerprint().unwrap()
        );
        assert!(physical.structurally_equivalent(&source_physical).unwrap());
        let program = physical.program();
        for (kind, expected) in [
            (EntityKind::Domain, 1),
            (EntityKind::Port, 3),
            (EntityKind::Relation, 1),
            (EntityKind::Connection, 1),
        ] {
            assert_eq!(
                program
                    .nodes()
                    .filter(|node| node.id().kind() == kind)
                    .count(),
                expected,
                "Python physical authoring lost a {kind:?} declaration"
            );
        }
        let relation = program
            .nodes()
            .find_map(|node| match node {
                KernelNode::Relation(relation) => Some(relation),
                _ => None,
            })
            .expect("the replayed physical Model must retain its Relation");
        assert_eq!(
            relation.residuals().roots().len(),
            2,
            "multi-residual meaning was lost during Python artifact replay"
        );

        let spatial = replay_python_model(&locals, "spatial_model");
        let source_spatial = eqiora::api::ModelDocument::compile(
            "python-native-poisson.eqi",
            include_str!("../../../verify/interfaces/python-native-modeling/models/poisson.eqi"),
        )
        .unwrap();
        assert_ne!(spatial.digest().unwrap(), source_spatial.digest().unwrap());
        assert_eq!(
            spatial.structural_fingerprint().unwrap(),
            source_spatial.structural_fingerprint().unwrap()
        );
        let environment = ScalarEllipticExecutionEnvironment::host_serial();
        let plan = spatial
            .preview_scalar_elliptic_run(
                ScalarEllipticIntent::new(
                    RealizationRevision::new(1),
                    ScalarEllipticMethod::FiniteElement,
                    NonZeroUsize::new(4).unwrap(),
                    NonZeroUsize::MIN,
                ),
                environment,
            )
            .unwrap();
        let result = spatial.run_scalar_elliptic_plan(plan, environment).unwrap();
        assert_eq!(result.field().value_count(), 5);

        assert_eq!(
            locals
                .get_item("component")?
                .unwrap()
                .getattr("residuals")?
                .len()?,
            2
        );
        assert_eq!(
            locals.get_item("flow")?.unwrap().repr()?.to_str()?,
            "Relation(\"flow\", activation='continuous')"
        );

        assert_rejected_without_model(
            py,
            module,
            c_str!(
                r#"
included = eqiora.Field("x", initial=1.0)
same_named_foreign = eqiora.Field("x", initial=1.0)
relation = eqiora.Relation("flow", residual=same_named_foreign)
rejected_model = eqiora.Model.define("foreign_symbol", included, relation)
"#
            ),
            "EQ0603",
            &["foreign_symbol", "flow"],
            None,
        )?;

        assert_rejected_without_model(
            py,
            module,
            c_str!(
                r#"
temperature = eqiora.Field(
    "temperature",
    dimension=eqiora.Dimension(temperature=1),
    initial=293.0,
)
duration = eqiora.Parameter(
    "duration",
    dimension=eqiora.Dimension(time=1),
    value=1.0,
)
invalid = eqiora.Relation("invalid", residual=temperature + duration)
rejected_model = eqiora.Model.define("dimension_mismatch", temperature, duration, invalid)
"#
            ),
            "EQ0603",
            &["dimension_mismatch", "invalid"],
            None,
        )?;

        assert_rejected_without_model(
            py,
            module,
            c_str!(
                r#"
voltage = eqiora.Dimension(mass=1, length=2, time=-3, current=-1)
current = eqiora.Dimension(current=1)
left_domain = eqiora.PhysicalDomain(
    "electrical_left",
    across_dimension=voltage,
    through_dimension=current,
)
equal_but_foreign = eqiora.PhysicalDomain(
    "electrical_foreign",
    across_dimension=voltage,
    through_dimension=current,
)
left = eqiora.ConservingPort("left", domain=left_domain)
foreign = eqiora.ConservingPort("foreign", domain=equal_but_foreign)
bad_connection = eqiora.connect(left, foreign)
rejected_model = eqiora.Model.define(
    "nominal_domain_mismatch",
    left_domain,
    equal_but_foreign,
    left,
    foreign,
    eqiora.Relation("left_owner", residual=eqiora.across(left)),
    eqiora.Relation("foreign_owner", residual=eqiora.across(foreign)),
    bad_connection,
)
"#
            ),
            "EQ0603",
            &["nominal_domain_mismatch"],
            Some("exact same"),
        )?;

        assert_rejected_without_model(
            py,
            module,
            c_str!(
                r#"
voltage = eqiora.Dimension(mass=1, length=2, time=-3, current=-1)
current = eqiora.Dimension(current=1)
electrical = eqiora.PhysicalDomain(
    "electrical",
    across_dimension=voltage,
    through_dimension=current,
)
left = eqiora.ConservingPort("left", domain=electrical)
omitted = eqiora.ConservingPort("omitted", domain=electrical)
bad_connection = eqiora.connect(left, omitted)
rejected_model = eqiora.Model.define(
    "omitted_connection_member",
    electrical,
    left,
    eqiora.Relation("left_owner", residual=eqiora.across(left)),
    bad_connection,
)
"#
            ),
            "EQ0603",
            &["omitted_connection_member"],
            None,
        )?;

        assert_rejected_without_model(
            py,
            module,
            c_str!(
                r#"
included = eqiora.Domain.box("interval", (0.0, 1.0))
same_named_foreign = eqiora.Domain.box("interval", (0.0, 1.0))
space = eqiora.Representation.continuum("space")
field = eqiora.Field(
    "u",
    domain=same_named_foreign,
    representation=space,
)
rejected_model = eqiora.Model.define("foreign_domain", included, space, field)
"#
            ),
            "EQ0603",
            &["foreign_domain", "u"],
            Some("foreign or omitted Domain"),
        )?;

        assert_rejected_without_model(
            py,
            module,
            c_str!(
                r#"
interval = eqiora.Domain.box("interval", (0.0, 1.0))
included = eqiora.Representation.continuum("space")
same_named_foreign = eqiora.Representation.continuum("space")
field = eqiora.Field("u", domain=interval, representation=same_named_foreign)
rejected_model = eqiora.Model.define(
    "foreign_representation",
    interval,
    included,
    field,
)
"#
            ),
            "EQ0603",
            &["foreign_representation", "u"],
            Some("foreign or omitted Representation"),
        )?;

        assert_rejected_without_model(
            py,
            module,
            c_str!(
                r#"
included = eqiora.Domain.box("interval", (0.0, 1.0))
same_named_foreign = eqiora.Domain.box("interval", (0.0, 1.0))
relation = eqiora.Relation(
    "balance",
    domain=same_named_foreign,
    residual=1.0,
)
rejected_model = eqiora.Model.define("foreign_relation_domain", included, relation)
"#
            ),
            "EQ0603",
            &["foreign_relation_domain", "balance"],
            Some("foreign or omitted Domain"),
        )?;

        assert_rejected_without_model(
            py,
            module,
            c_str!(
                r#"
included = eqiora.Domain.box("interval", (0.0, 1.0))
same_named_foreign = eqiora.Domain.box("interval", (0.0, 1.0))
lower = same_named_foreign.boundary(
    "lower",
    axis=0,
    side=eqiora.BoundarySide.Lower,
)
rejected_model = eqiora.Model.define("foreign_parent", included, lower)
"#
            ),
            "EQ0603",
            &["foreign_parent", "lower"],
            Some("foreign or omitted parent Domain"),
        )?;

        assert_rejected_without_model(
            py,
            module,
            c_str!(
                r#"
interval = eqiora.Domain.box("interval", (0.0, 1.0))
space = eqiora.Representation.continuum("space")
field = eqiora.Field("u", domain=interval, representation=space)
invalid = eqiora.Relation(
    "invalid",
    domain=interval,
    residual=eqiora.trace(field),
)
rejected_model = eqiora.Model.define(
    "support_mismatch",
    interval,
    space,
    field,
    invalid,
)
"#
            ),
            "EQ0302",
            &["semantic", "Relation"],
            None,
        )?;

        Ok(())
    })
}

fn replay_python_model(locals: &Bound<'_, PyDict>, name: &str) -> eqiora::api::ModelDocument {
    let bytes: Vec<u8> = locals
        .get_item(name)
        .unwrap()
        .unwrap()
        .call_method0("to_json")
        .unwrap()
        .extract()
        .unwrap();
    let document = ExactModelCodec::CURRENT.replay(&bytes).unwrap();
    assert_eq!(document.canonical_json().unwrap(), bytes);
    document
}

fn assert_rejected_without_model(
    py: Python<'_>,
    module: &Bound<'_, PyModule>,
    code: &std::ffi::CStr,
    expected_code: &str,
    expected_path_prefix: &[&str],
    expected_message_fragment: Option<&str>,
) -> PyResult<()> {
    let locals = PyDict::new(py);
    locals.set_item("eqiora", module)?;
    let error = py
        .run(code, None, Some(&locals))
        .expect_err("the Python native definition must fail closed");
    assert!(error.is_instance(py, &module.getattr("ValidationError")?));
    assert!(
        !locals.contains("rejected_model")?,
        "a rejected definition exposed a partial Model"
    );
    let diagnostics = error.value(py).getattr("diagnostics")?;
    assert!(diagnostics.len()? > 0);
    let diagnostic = diagnostics.get_item(0)?;
    assert_eq!(
        diagnostic.getattr("code")?.extract::<String>()?,
        expected_code
    );
    let graph_path = diagnostic
        .getattr("graph_path")?
        .extract::<Option<Vec<String>>>()?
        .expect("native construction diagnostics must retain a declaration path");
    assert!(
        graph_path.len() >= expected_path_prefix.len()
            && graph_path
                .iter()
                .zip(expected_path_prefix)
                .all(|(actual, expected)| actual == expected),
        "unexpected graph path {graph_path:?}"
    );
    if let Some(expected) = expected_message_fragment {
        let message = diagnostic.getattr("message")?.extract::<String>()?;
        assert!(
            message.contains(expected),
            "diagnostic {message:?} did not prove the intended falsifier"
        );
    }
    assert!(diagnostic.getattr("source_span")?.is_none());
    Ok(())
}
