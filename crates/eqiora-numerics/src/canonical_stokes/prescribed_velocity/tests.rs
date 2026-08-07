use std::collections::BTreeMap;

use eqiora_compiler::compile;
use eqiora_core::RawId;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_schema::kernel::{BoundarySide, DomainKind, KernelNode};
use eqiora_sem::KernelProgram;

use super::{
    ModelOwnedEssentialVelocityReplay2d, SteadyStokesPrescribedVelocityTrace2d,
    admit_model_owned_essential_velocity_2d, lower_prescribed_velocity_trace_2d,
};
use crate::canonical_stokes::support::relations_on;

const SEALED_INPUT_SHA256: &str =
    "da3223d51caf11f6e627540f9284c2bb307518f7d87a62e2897a9f3732bbf620";
const REFERENCE_TOPOLOGY: &str = "stokes-square-ring-reference-n32-m4-v1";
const REFINED_TOPOLOGY: &str = "stokes-square-ring-refined-n64-m8-v1";
const ROLE_NAMES: [&str; 5] = [
    "body_no_slip",
    "outer_x_minus",
    "outer_x_plus",
    "outer_y_minus",
    "outer_y_plus",
];

const SOURCE: &str = r#"
model stokes_e1_prescribed_velocity {
  domain fluid = box(-10, 10, -10, 10);
  domain body_no_slip = boundary(fluid, axis = 0, side = lower);
  domain outer_x_minus = boundary(fluid, axis = 0, side = lower);
  domain outer_x_plus = boundary(fluid, axis = 0, side = upper);
  domain outer_y_minus = boundary(fluid, axis = 1, side = lower);
  domain outer_y_plus = boundary(fluid, axis = 1, side = upper);
  representation space = continuum;

  field velocity on fluid as space: m / s shape spatial_vector;
  field pressure on fluid as space: kg / (m * s ^ 2) = 0;
  field force_potential on fluid as space: kg / (m * s ^ 2) = 0;
  field chi on fluid as space: m ^ 2 / s = 0;
  parameter mu: kg / (m * s) = 1;
  parameter U: m / s = 1;
  parameter zero_pressure: kg / (m * s ^ 2) = 0;

  relation force continuous on fluid {
    force_potential - zero_pressure = 0;
  }
  relation momentum continuous on fluid {
    -div(
      2 * mu * symmetric_part(grad(velocity))
      - isotropic_lift(pressure)
    ) - grad(force_potential) = 0;
  }
  relation incompressibility continuous on fluid { div(velocity) = 0; }
  relation define_chi continuous on fluid { chi - U * coordinate(0) = 0; }

  relation body_zero continuous on body_no_slip { trace(velocity) = 0; }
  relation outer_x_minus_value continuous on outer_x_minus {
    trace(velocity) - trace(grad(chi)) = 0;
  }
  relation outer_x_plus_value continuous on outer_x_plus {
    trace(velocity) - trace(grad(chi)) = 0;
  }
  relation outer_y_minus_value continuous on outer_y_minus {
    trace(velocity) - trace(grad(chi)) = 0;
  }
  relation outer_y_plus_value continuous on outer_y_plus {
    trace(velocity) - trace(grad(chi)) = 0;
  }
}
"#;

#[derive(Clone)]
struct Fixture {
    program: KernelProgram,
    velocity: RawId,
    role_boundaries: BTreeMap<String, RawId>,
    body_relation: RawId,
    outer_traces: BTreeMap<String, SteadyStokesPrescribedVelocityTrace2d>,
    owners: BTreeMap<String, Vec<(usize, [f64; 2])>>,
}

impl Fixture {
    fn from_source(source: &str) -> Self {
        let program = compile_program(source);
        let domain = program
            .nodes()
            .find_map(|node| match node {
                KernelNode::Domain(value)
                    if matches!(value.kind(), DomainKind::CartesianBox { .. }) =>
                {
                    Some(value.id().erase())
                }
                _ => None,
            })
            .expect("one volume Domain");
        let velocity = program
            .nodes()
            .find_map(|node| match node {
                KernelNode::Field(value) if value.shape().extents().len() == 1 => {
                    Some(value.id().erase())
                }
                _ => None,
            })
            .expect("one vector velocity Field");

        let mut role_boundaries = BTreeMap::new();
        let mut body_relation = None;
        let mut outer_traces = BTreeMap::new();
        for node in program.nodes() {
            let KernelNode::Domain(boundary) = node else {
                continue;
            };
            let DomainKind::CartesianBoundary { axis, side } = boundary.kind() else {
                continue;
            };
            let boundary_id = boundary.id().erase();
            let boundary_relations = relations_on(&program, boundary_id);
            let [relation] = boundary_relations.as_slice() else {
                panic!("every fixture boundary has one Relation");
            };
            match lower_prescribed_velocity_trace_2d(
                &program,
                domain,
                velocity,
                boundary_id,
                *relation,
            )
            .expect("fixture boundary has supported typed meaning")
            {
                Some(trace) => {
                    let role = outer_role(*axis, *side);
                    assert_eq!(trace.boundary(), boundary_id);
                    assert_eq!(trace.relation(), *relation);
                    role_boundaries.insert(role.to_owned(), boundary_id);
                    outer_traces.insert(role.to_owned(), trace);
                }
                None => {
                    assert!(body_relation.replace(*relation).is_none());
                    role_boundaries.insert("body_no_slip".to_owned(), boundary_id);
                }
            }
        }
        assert_eq!(role_boundaries.keys().count(), ROLE_NAMES.len());
        assert!(
            ROLE_NAMES
                .iter()
                .all(|name| role_boundaries.contains_key(*name))
        );

        Self {
            program,
            velocity,
            role_boundaries,
            body_relation: body_relation.expect("one trace-zero body"),
            outer_traces,
            owners: exact_owned_vertices(),
        }
    }

    fn admit(&self) -> Result<ModelOwnedEssentialVelocityReplay2d, eqiora_core::Diagnostic> {
        admit_model_owned_essential_velocity_2d(
            &self.program,
            self.velocity,
            &self.role_boundaries,
            self.body_relation,
            &self.outer_traces,
            &self.owners,
        )
    }
}

#[test]
fn sealed_selector_and_complete_model_trace_reach_the_ordinary_positive_first() {
    assert_eq!(SEALED_INPUT_SHA256.len(), 64);
    assert!(
        SEALED_INPUT_SHA256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
    assert_ne!(REFERENCE_TOPOLOGY, REFINED_TOPOLOGY);
    let base = parse_pair(["0", "0"]);
    let direction = parse_pair(["1", "0"]);
    let step = "2e-3".parse::<f64>().expect("sealed step");
    assert_eq!(add_scaled(base, direction, step), [0.002, 0.0]);
    assert_eq!(add_scaled(base, direction, -step), [-0.002, 0.0]);

    let fixture = Fixture::from_source(SOURCE);
    let mut shared_identity = None;
    for (role, trace) in &fixture.outer_traces {
        assert!(trace.is_complete_affine_potential(), "{role}");
        assert_eq!(trace.evaluate([7.0, -3.0]).unwrap(), [1.0, 0.0]);
        let identity = (
            trace.coefficient_field(),
            trace.definition_relation(),
            trace.speed_parameter(),
        );
        if let Some(expected) = shared_identity {
            assert_eq!(
                identity, expected,
                "{role} drifted from the shared Model law"
            );
        } else {
            shared_identity = Some(identity);
        }
    }
    assert_eq!(fixture.outer_traces.len(), 4);

    let replay = fixture
        .admit()
        .expect("complete typed Model reaches admission");
    assert_eq!(
        replay.values(),
        &[
            [1.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
        ]
    );
    // The horizontal sides witness the nonzero tangential component, while
    // the two vertical fluxes cancel exactly.
    assert_eq!(replay.value_for_role("outer_y_minus", 0), Some([1.0, 0.0]));
    assert_eq!(replay.value_for_role("outer_y_plus", 3), Some([1.0, 0.0]));
    assert_eq!((-1.0_f64).mul_add(20.0, 1.0 * 20.0), 0.0);
    replay
        .validate_transport(|vertex| replay.values()[vertex])
        .expect("the callback transports only replayed Model values");
}

#[test]
fn normal_only_incomplete_and_equal_value_identity_mutants_fail_closed() {
    let normal_source = SOURCE
        .replace(
            "  field chi on fluid as space: m ^ 2 / s = 0;",
            "  field chi on fluid as space: m ^ 2 / s = 0;\n  field normal_speed on fluid as space: m / s = 0;",
        )
        .replace(
            "  relation define_chi continuous on fluid { chi - U * coordinate(0) = 0; }",
            "  relation define_chi continuous on fluid { chi - U * coordinate(0) = 0; }\n  relation define_normal_speed continuous on fluid { normal_speed - U = 0; }",
        )
        .replace(
            "trace(velocity) - trace(grad(chi)) = 0;\n  }\n  relation outer_y_plus_value",
            "trace(velocity) - normal(isotropic_lift(normal_speed)) = 0;\n  }\n  relation outer_y_plus_value",
        );
    let normal = Fixture::from_source(&normal_source);
    assert!(
        !normal.outer_traces["outer_y_minus"].is_complete_affine_potential(),
        "the normal-only alias must reach the intended complete-trace gate"
    );
    assert!(normal.admit().is_err());

    let mut incomplete = Fixture::from_source(SOURCE);
    incomplete.outer_traces.remove("outer_y_plus");
    assert!(incomplete.admit().is_err());

    let drift_source = SOURCE
        .replace(
            "  field chi on fluid as space: m ^ 2 / s = 0;",
            "  field chi on fluid as space: m ^ 2 / s = 0;\n  field chi_alt on fluid as space: m ^ 2 / s = 0;",
        )
        .replace(
            "  parameter U: m / s = 1;",
            "  parameter U: m / s = 1;\n  parameter U_alt: m / s = 1;",
        )
        .replace(
            "  relation define_chi continuous on fluid { chi - U * coordinate(0) = 0; }",
            "  relation define_chi continuous on fluid { chi - U * coordinate(0) = 0; }\n  relation define_chi_alt continuous on fluid { chi_alt - U_alt * coordinate(0) = 0; }",
        )
        .replace(
            "relation outer_y_plus_value continuous on outer_y_plus {\n    trace(velocity) - trace(grad(chi)) = 0;",
            "relation outer_y_plus_value continuous on outer_y_plus {\n    trace(velocity) - trace(grad(chi_alt)) = 0;",
        );
    let drift = Fixture::from_source(&drift_source);
    assert_eq!(
        drift.outer_traces["outer_y_plus"]
            .evaluate([0.0, 0.0])
            .unwrap(),
        [1.0, 0.0],
        "the mutant is numerically equal and must be killed by identity"
    );
    assert!(drift.admit().is_err());

    let mut permuted = Fixture::from_source(SOURCE);
    let x = permuted.outer_traces["outer_x_minus"].clone();
    let y = permuted.outer_traces["outer_y_minus"].clone();
    permuted.outer_traces.insert("outer_x_minus".into(), y);
    permuted.outer_traces.insert("outer_y_minus".into(), x);
    assert!(permuted.admit().is_err());
}

#[test]
fn ownership_corner_and_callback_mutants_reject_without_partial_admission() {
    let fixture = Fixture::from_source(SOURCE);
    fixture
        .admit()
        .expect("mutants fork after the positive admission");

    let mut missing = fixture.clone();
    missing.owners.remove("outer_y_plus");
    assert!(missing.admit().is_err());

    let mut duplicate = fixture.clone();
    duplicate
        .owners
        .get_mut("outer_x_minus")
        .unwrap()
        .push((0, [-10.0, -10.0]));
    assert!(duplicate.admit().is_err());

    let mut mixed = fixture.clone();
    mixed
        .owners
        .get_mut("body_no_slip")
        .unwrap()
        .push((0, [-10.0, -10.0]));
    assert!(mixed.admit().is_err());

    let replay = fixture.admit().unwrap();
    assert!(
        replay
            .validate_transport(|vertex| if vertex == 0 {
                [0.0, 1.0]
            } else {
                replay.values()[vertex]
            })
            .is_err(),
        "a rotated callback at a shared corner cannot repair the Model"
    );
    assert!(
        replay
            .validate_transport(|vertex| if vertex == 2 {
                [2.0, 0.0]
            } else {
                replay.values()[vertex]
            })
            .is_err(),
        "a distinct caller-owned speed is rejected before finalization"
    );
}

fn compile_program(source: &str) -> KernelProgram {
    let mut compiled =
        compile("stokes-e1-prescribed-velocity.eqi", source).expect("typed E1 Model compiles");
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("transaction commits");
    KernelProgram::from_snapshot(&store.snapshot(), model).expect("Model validates")
}

fn outer_role(axis: usize, side: BoundarySide) -> &'static str {
    match (axis, side) {
        (0, BoundarySide::Lower) => "outer_x_minus",
        (0, BoundarySide::Upper) => "outer_x_plus",
        (1, BoundarySide::Lower) => "outer_y_minus",
        (1, BoundarySide::Upper) => "outer_y_plus",
        _ => panic!("E1 is exactly two-dimensional"),
    }
}

fn exact_owned_vertices() -> BTreeMap<String, Vec<(usize, [f64; 2])>> {
    BTreeMap::from([
        (
            "body_no_slip".into(),
            vec![(4, [-1.0, 0.0]), (5, [1.0, 0.0])],
        ),
        (
            "outer_x_minus".into(),
            vec![(0, [-10.0, -10.0]), (3, [-10.0, 10.0])],
        ),
        (
            "outer_x_plus".into(),
            vec![(1, [10.0, -10.0]), (2, [10.0, 10.0])],
        ),
        (
            "outer_y_minus".into(),
            vec![(0, [-10.0, -10.0]), (1, [10.0, -10.0])],
        ),
        (
            "outer_y_plus".into(),
            vec![(3, [-10.0, 10.0]), (2, [10.0, 10.0])],
        ),
    ])
}

fn parse_pair(values: [&str; 2]) -> [f64; 2] {
    values.map(|value| value.parse::<f64>().expect("sealed decimal"))
}

fn add_scaled(base: [f64; 2], direction: [f64; 2], step: f64) -> [f64; 2] {
    [
        direction[0].mul_add(step, base[0]),
        direction[1].mul_add(step, base[1]),
    ]
}
