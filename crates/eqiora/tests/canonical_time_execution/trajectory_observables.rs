use super::*;
use eqiora::backends::diffsol::DIFFSOL_TIME_BACKEND;
use eqiora_numerics::{
    CommonOdePlan, CommonOdeRunRequest, CommonTrajectory, CommonTsitouras45,
    CommonTsitourasTolerance, ResolvedCommonPlan, TimeFunctionalQuadrature,
};

fn fixture(rate: u32) -> (ModelEnvelope, KernelProgram) {
    let source = format!(
        r#"
model Decay() {{
  state x: 1;
  initial {{ x = 3; }}
  parameter rate: 1/s = {rate};
  relation flow {{ derivative(x) = -rate*x; }}
  observable sample: 1 = x;
}}
"#
    );
    let compiled = eqiora::compiler::compile("functional-decay.eqi", &source)
        .unwrap()
        .pop()
        .unwrap();
    let (transaction, model, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let kernel = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    (ModelEnvelope::from_program(&kernel).unwrap(), kernel)
}

use eqiora::sem::KernelProgram;

#[test]
fn ordinary_time_functional_uses_native_steps_and_exact_trajectory_lineage() {
    let (model, kernel) = fixture(2);
    let field = kernel
        .nodes()
        .find_map(|node| match node {
            KernelNode::Field(value) => Some(value.id()),
            _ => None,
        })
        .unwrap();
    let observable = kernel
        .nodes()
        .find_map(|node| match node {
            KernelNode::Observable(value) => Some(value.id()),
            _ => None,
        })
        .unwrap();
    let temporal = CommonTsitouras45::new(
        1e-3,
        1e-11,
        vec![CommonTsitourasTolerance::new(field, 1e-13).unwrap()],
    )
    .unwrap();
    let plan = CommonOdePlan::resolve(&model, &kernel, temporal, DIFFSOL_TIME_BACKEND).unwrap();
    assert_eq!(plan.field_ids().len(), 1, "functional adds no ODE state");
    let run = |output_times| {
        let request = CommonOdeRunRequest::new(
            plan.clone(),
            plan.initial_state().unwrap(),
            1.0,
            output_times,
        )
        .unwrap();
        let solution = DiffsolTimeBackend::new()
            .solve(&request.problem().unwrap(), request.time_plan())
            .unwrap();
        CommonTrajectory::accept_ode(request, solution).unwrap()
    };
    let sparse = run(vec![0.25]);
    let dense = run((1..10).map(|index| f64::from(index) / 10.0).collect());
    assert_eq!(sparse.ode_states().unwrap().len(), 1);
    assert_eq!(
        sparse.ode_history(),
        dense.ode_history(),
        "requested samples do not choose accepted integration steps"
    );
    let rule = TimeFunctionalQuadrature::AcceptedStepSimpson;
    let integrated = sparse
        .observe_time_integral(&model, observable, rule)
        .unwrap();
    let dense_integrated = dense
        .observe_time_integral(&model, observable, rule)
        .unwrap();
    assert_eq!(integrated.value(), dense_integrated.value());
    assert_eq!(integrated.trajectory_identity(), sparse.identity());
    assert_eq!(integrated.quadrature(), Some(rule));
    assert_eq!(integrated.interval_s(), [0.0, 1.0]);
    assert_eq!(integrated.endpoint_convention(), "fixed-interval-dt");
    // Solve x'= -2x, x(0)=3 independently: x=3exp(-2t), J=3(1-exp(-2))/2.
    let expected = 1.5 * (1.0 - (-2.0_f64).exp());
    assert!((integrated.value().real_scalar_value().unwrap().value() - expected).abs() < 1e-8);
    assert_eq!(
        integrated.value().value_type().dimension(),
        DimExponents::from_integers([0, 0, 1, 0, 0, 0, 0]).unwrap()
    );
    let terminal = sparse.observe_terminal(&model, observable).unwrap();
    assert!(
        (terminal.value().real_scalar_value().unwrap().value() - 3.0 * (-2.0_f64).exp()).abs()
            < 1e-9
    );
    assert_eq!(terminal.quadrature(), None);
    assert_eq!(terminal.interval_s(), [1.0, 1.0]);
    assert_eq!(terminal.endpoint_convention(), "terminal-fixed-time");
    // Samples without accepted history remain valid outputs but cannot own integrals.
    let request =
        CommonOdeRunRequest::new(plan.clone(), plan.initial_state().unwrap(), 1.0, vec![1.0])
            .unwrap();
    let report = eqiora::time::TimeExecutionReport::new(
        DIFFSOL_TIME_BACKEND,
        TimeMethod::Tsitouras45,
        TimeEquationClass::ExplicitOde,
        InitialConditionPolicy::Provided,
    );
    let samples =
        eqiora::time::TimeSolution::accepted(1, vec![1.0], vec![3.0 * (-2.0_f64).exp()], report)
            .unwrap();
    let sampled = CommonTrajectory::accept_ode(request, samples).unwrap();
    assert!(sampled.observe_terminal(&model, observable).is_err());
    assert!(
        sampled
            .observe_time_integral(&model, observable, rule)
            .is_err()
    );
    let resolved = ResolvedCommonPlan::Ode(Box::new(plan));
    let bytes = sparse.to_bytes().unwrap();
    let replayed = CommonTrajectory::from_bytes(&bytes, &resolved).unwrap();
    assert_eq!(replayed, sparse);
    assert_eq!(
        replayed
            .observe_time_integral(&model, observable, rule)
            .unwrap(),
        integrated
    );
    let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    forged["payload"]["history"][0]["midpoint_state"][0] = serde_json::json!(8.0);
    assert!(
        CommonTrajectory::from_bytes(&serde_json::to_vec(&forged).unwrap(), &resolved).is_err()
    );
    assert!(
        sparse
            .observe_time_integral(&fixture(3).0, observable, rule)
            .is_err()
    );
    assert!(
        sparse
            .observe_time_integral(&model, Id::new(), rule)
            .is_err()
    );
}

#[test]
fn event_driven_observables_reject_the_smooth_ode_plan() {
    let compiled = eqiora::compiler::compile(
        "event-functional.eqi",
        r#"
model Reset() {
  state x: 1;
  initial { x = 0; }
  relation flow { derivative(x) = 1[1/s]; }
  event hit = crossing(x - 0.4, direction=rising);
  relation reset at hit { next(x) = 0; }
  observable sample: 1 = x;
}
"#,
    )
    .unwrap()
    .pop()
    .unwrap();
    let (transaction, model_id, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let kernel = KernelProgram::from_snapshot(&store.snapshot(), model_id).unwrap();
    let model = ModelEnvelope::from_program(&kernel).unwrap();
    let field = kernel
        .nodes()
        .find_map(|node| match node {
            KernelNode::Field(field) => Some(field.id()),
            _ => None,
        })
        .unwrap();
    let temporal = CommonTsitouras45::new(
        1e-3,
        1e-11,
        vec![CommonTsitourasTolerance::new(field, 1e-13).unwrap()],
    )
    .unwrap();
    let error =
        CommonOdePlan::resolve(&model, &kernel, temporal, DIFFSOL_TIME_BACKEND).unwrap_err();
    assert!(error.message().contains("activation family"));
}
