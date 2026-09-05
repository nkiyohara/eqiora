//! Transport-independent packaged DC-drive presentation composition.

use eqiora::artifact::{ArtifactDigest, RunManifestV1};
use eqiora::package::ModelPackageIdentityV1;
use eqiora::sem::{Interpreter, ReferenceConfig, Trajectory};
use eqiora::{DimExponents, RawId};
use serde::Serialize;

use super::packages::PreparedPackages;

const DEMO_PROTOCOL: &str = "eqiora.studio.packaged-dc-drive-demo/v2";
const EXAMPLE_ID: &str = "packaged-dc-motor-control";
const SCIENTIFIC_CASE_ID: &str = "hybrid.packaged-dc-motor-controller";

const END_TIME_S: f64 = 0.1;
const MAXIMUM_STEP_S: f64 = 0.001;
const SAMPLE_PERIOD_STEPS: usize = 10;
const ACCEPTED_STEPS: usize = 100;
const MAXIMUM_NONLINEAR_ITERATIONS: usize = 32;
const MAXIMUM_SEMANTIC_STEPS: usize = 1_000_000;
const NONLINEAR_ABSOLUTE_TOLERANCE: f64 = 1.0e-10;
const NONLINEAR_RELATIVE_TOLERANCE: f64 = 1.0e-10;

const CURRENT_DIMENSION: DimExponents =
    DimExponents::from_integers([0, 0, 0, 1, 0, 0, 0]).expect("current dimension");
const ANGULAR_SPEED_DIMENSION: DimExponents =
    DimExponents::from_integers([0, 0, -1, 0, 0, 0, 0]).expect("angular speed dimension");
const VOLTAGE_DIMENSION: DimExponents =
    DimExponents::from_integers([1, 2, -3, -1, 0, 0, 0]).expect("voltage dimension");

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DcMotorDemoResult {
    protocol: &'static str,
    example_id: &'static str,
    trajectory: TrajectoryProjection,
    package_graph: PackageGraph,
    execution: ExecutionSummary,
    lineage: LineageSummary,
    evidence: EvidenceAttribution,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrajectoryProjection {
    samples: Vec<TrajectorySample>,
    commits: Vec<ControllerCommit>,
    units: TrajectoryUnits,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrajectorySample {
    step: usize,
    time_s: f64,
    current_a: f64,
    angular_speed_per_s: f64,
    held_voltage_v: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControllerCommit {
    step: usize,
    time_s: f64,
    held_voltage_v: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrajectoryUnits {
    current: &'static str,
    angular_speed: &'static str,
    held_voltage: &'static str,
    time: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageGraph {
    root: String,
    resolution_digest: String,
    nodes: Vec<PackageNode>,
    edges: Vec<PackageEdge>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageNode {
    name: String,
    version: String,
    semantic_digest: String,
    source_digest: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageEdge {
    declaring: String,
    target: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionSummary {
    method: &'static str,
    scalar_type: &'static str,
    placement: &'static str,
    end_time_s: f64,
    maximum_step_s: f64,
    sample_period_s: f64,
    accepted_steps: usize,
    hold_intervals: usize,
    controller_commits: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LineageSummary {
    model_digest: String,
    compilation_digest: String,
    run_digest: String,
    run_binding_digest: String,
    semantic_revision: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceAttribution {
    case_id: &'static str,
    status: &'static str,
    physical_port_samples_presented: bool,
}

pub(super) fn prepare_demo() -> Result<DcMotorDemoResult, String> {
    let packages = PreparedPackages::compile()?;
    let trajectory = Interpreter::new()
        .run(packages.document.model().program(), reference_config()?)
        .map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })?;
    let projection = project_trajectory(
        &trajectory,
        field(&packages, "motor.current")?,
        field(&packages, "load.speed")?,
        field(&packages, "controller.held_voltage")?,
    )?;

    // Identity is deliberately unreachable until the complete presentation
    // payload has passed its structural acceptance contract.
    let run = reference_run(&packages)?;
    let binding = packages
        .document
        .bind_run_v1(&run)
        .map_err(|error| error.to_string())?;
    packages
        .document
        .validate_run_v1_binding(&binding, &run, &packages.resolution)
        .map_err(|error| error.to_string())?;

    Ok(DcMotorDemoResult {
        protocol: DEMO_PROTOCOL,
        example_id: EXAMPLE_ID,
        trajectory: projection,
        package_graph: package_graph(&packages)?,
        execution: ExecutionSummary {
            method: "backward-euler",
            scalar_type: "f64",
            placement: "one-host-one-worker",
            end_time_s: END_TIME_S,
            maximum_step_s: MAXIMUM_STEP_S,
            sample_period_s: SAMPLE_PERIOD_STEPS as f64 * MAXIMUM_STEP_S,
            accepted_steps: ACCEPTED_STEPS,
            hold_intervals: ACCEPTED_STEPS / SAMPLE_PERIOD_STEPS,
            controller_commits: ACCEPTED_STEPS / SAMPLE_PERIOD_STEPS + 1,
        },
        lineage: LineageSummary {
            model_digest: packages
                .document
                .model()
                .digest()
                .map_err(|error| error.to_string())?
                .to_owned(),
            compilation_digest: packages
                .document
                .compilation()
                .digest()
                .map_err(|error| error.to_string())?
                .to_hex(),
            run_digest: run.digest().map_err(|error| error.to_string())?.to_string(),
            run_binding_digest: binding
                .digest()
                .map_err(|error| error.to_string())?
                .to_hex(),
            semantic_revision: packages.document.model().program().revision().0,
        },
        evidence: EvidenceAttribution {
            case_id: SCIENTIFIC_CASE_ID,
            status: "historical-a3-only",
            physical_port_samples_presented: false,
        },
    })
}

fn reference_config() -> Result<ReferenceConfig, String> {
    ReferenceConfig::new(END_TIME_S, MAXIMUM_STEP_S)
        .and_then(|config| {
            config.with_nonlinear_tolerances(
                NONLINEAR_ABSOLUTE_TOLERANCE,
                NONLINEAR_RELATIVE_TOLERANCE,
            )
        })
        .and_then(|config| config.with_limits(MAXIMUM_NONLINEAR_ITERATIONS, MAXIMUM_SEMANTIC_STEPS))
        .map_err(|error| error.to_string())
}

fn reference_run(packages: &PreparedPackages) -> Result<RunManifestV1, String> {
    let model_digest = packages
        .document
        .model()
        .digest()
        .map_err(|error| error.to_string())?;
    RunManifestV1::new(
        ArtifactDigest::from_hex(model_digest).map_err(|error| error.to_string())?,
        packages.document.model().program().revision().0,
        "eqiora-sem-reference",
        eqiora::VERSION,
    )
    .and_then(|run| run.with_numerical_setting("execution.topology", "one-host-one-worker"))
    .and_then(|run| {
        run.with_numerical_setting(
            "execution.maximum-semantic-steps",
            MAXIMUM_SEMANTIC_STEPS.to_string(),
        )
    })
    .and_then(|run| {
        run.with_numerical_setting("integration.end-time-seconds", END_TIME_S.to_string())
    })
    .and_then(|run| {
        run.with_numerical_setting(
            "integration.maximum-step-seconds",
            MAXIMUM_STEP_S.to_string(),
        )
    })
    .and_then(|run| run.with_numerical_setting("integration.method", "backward-euler"))
    .and_then(|run| run.with_numerical_setting("scalar.type", "f64"))
    .and_then(|run| {
        run.with_numerical_setting(
            "solver.absolute-tolerance",
            NONLINEAR_ABSOLUTE_TOLERANCE.to_string(),
        )
    })
    .and_then(|run| {
        run.with_numerical_setting(
            "solver.initial-guess",
            "zero-initial-consistency-then-forward-euler-state-and-accepted-algebraics",
        )
    })
    .and_then(|run| {
        run.with_numerical_setting(
            "solver.maximum-iterations",
            MAXIMUM_NONLINEAR_ITERATIONS.to_string(),
        )
    })
    .and_then(|run| run.with_numerical_setting("solver.method", "dense-finite-difference-newton"))
    .and_then(|run| {
        run.with_numerical_setting(
            "solver.relative-tolerance",
            NONLINEAR_RELATIVE_TOLERANCE.to_string(),
        )
    })
    .map_err(|error| error.to_string())
}

fn field(packages: &PreparedPackages, alias: &str) -> Result<RawId, String> {
    packages
        .document
        .model()
        .aliases()
        .get(alias)
        .copied()
        .ok_or_else(|| format!("packaged DC-drive model omitted required alias `{alias}`"))
}

fn project_trajectory(
    trajectory: &Trajectory,
    current: RawId,
    speed: RawId,
    held_voltage: RawId,
) -> Result<TrajectoryProjection, String> {
    let current = field_series(trajectory, current, CURRENT_DIMENSION, "current")?;
    let speed = field_series(trajectory, speed, ANGULAR_SPEED_DIMENSION, "angular speed")?;
    let held_voltage = field_series(trajectory, held_voltage, VOLTAGE_DIMENSION, "held voltage")?;
    if current.len() != ACCEPTED_STEPS + 1
        || speed.len() != current.len()
        || held_voltage.len() != current.len()
    {
        return Err(format!(
            "packaged DC-drive trajectory must expose {} aligned boundaries",
            ACCEPTED_STEPS + 1
        ));
    }

    let mut samples = Vec::with_capacity(ACCEPTED_STEPS + 1);
    for step in 0..=ACCEPTED_STEPS {
        let expected_time = step as f64 * MAXIMUM_STEP_S;
        let (current_time, current_a) = current[step];
        let (speed_time, angular_speed_per_s) = speed[step];
        let (voltage_time, held_voltage_v) = held_voltage[step];
        if !same_time(current_time, expected_time)
            || !same_time(speed_time, expected_time)
            || !same_time(voltage_time, expected_time)
        {
            return Err(format!(
                "packaged DC-drive trajectory is not aligned at integer step {step}"
            ));
        }
        if !current_a.is_finite() || !angular_speed_per_s.is_finite() || !held_voltage_v.is_finite()
        {
            return Err(format!(
                "packaged DC-drive trajectory contains a nonfinite value at step {step}"
            ));
        }
        samples.push(TrajectorySample {
            step,
            time_s: expected_time,
            current_a,
            angular_speed_per_s,
            held_voltage_v,
        });
    }
    validate_zero_order_hold(&samples)?;
    let commits = (0..=ACCEPTED_STEPS / SAMPLE_PERIOD_STEPS)
        .map(|commit| {
            let sample = samples[commit * SAMPLE_PERIOD_STEPS];
            ControllerCommit {
                step: sample.step,
                time_s: sample.time_s,
                held_voltage_v: sample.held_voltage_v,
            }
        })
        .collect();
    Ok(TrajectoryProjection {
        samples,
        commits,
        units: TrajectoryUnits {
            current: "A",
            angular_speed: "s^-1",
            held_voltage: "V",
            time: "s",
        },
    })
}

fn field_series(
    trajectory: &Trajectory,
    field: RawId,
    expected_dimension: DimExponents,
    label: &str,
) -> Result<Vec<(f64, f64)>, String> {
    trajectory
        .samples()
        .iter()
        .filter(|sample| sample.field() == field)
        .map(|sample| {
            if sample.value().dim() != expected_dimension {
                return Err(format!(
                    "packaged DC-drive {label} sample has an unexpected physical dimension"
                ));
            }
            Ok((sample.time(), sample.value().value()))
        })
        .collect()
}

fn same_time(actual: f64, expected: f64) -> bool {
    let allowance = 64.0 * f64::EPSILON * (1.0 + expected.abs());
    actual.is_finite() && (actual - expected).abs() <= allowance
}

fn validate_zero_order_hold(samples: &[TrajectorySample]) -> Result<(), String> {
    for interval in 0..ACCEPTED_STEPS / SAMPLE_PERIOD_STEPS {
        let start = interval * SAMPLE_PERIOD_STEPS;
        let held = samples[start].held_voltage_v.to_bits();
        if samples[start..start + SAMPLE_PERIOD_STEPS]
            .iter()
            .any(|sample| sample.held_voltage_v.to_bits() != held)
        {
            return Err(format!(
                "packaged DC-drive voltage is not zero-order held on interval {interval}"
            ));
        }
    }
    Ok(())
}

fn package_graph(packages: &PreparedPackages) -> Result<PackageGraph, String> {
    let identity_label =
        |identity: &ModelPackageIdentityV1| format!("{}@{}", identity.name, identity.version);
    let nodes = packages
        .resolution
        .nodes()
        .iter()
        .map(|node| PackageNode {
            name: node.identity().name.to_string(),
            version: node.identity().version.to_string(),
            semantic_digest: node.identity().semantic_digest.to_hex(),
            source_digest: node.source_digest().to_hex(),
        })
        .collect();
    let edges = packages
        .resolution
        .edges()
        .iter()
        .map(|edge| PackageEdge {
            declaring: identity_label(edge.declaring()),
            target: identity_label(edge.target()),
        })
        .collect();
    Ok(PackageGraph {
        root: identity_label(packages.resolution.root()),
        resolution_digest: packages
            .resolution
            .digest()
            .map_err(|error| error.to_string())?
            .to_hex(),
        nodes,
        edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_projects_one_structurally_accepted_lineage() {
        let result = prepare_demo().expect("accepted packaged DC-drive demo");
        let repeated = prepare_demo().expect("repeated packaged DC-drive demo");

        assert_eq!(result.trajectory.samples.len(), 101);
        assert_eq!(result.trajectory.commits.len(), 11);
        assert_eq!(result.execution.hold_intervals, 10);
        assert!(!result.evidence.physical_port_samples_presented);
        assert_eq!(result.lineage.model_digest, repeated.lineage.model_digest);
        assert_eq!(
            result.lineage.compilation_digest,
            repeated.lineage.compilation_digest
        );
        assert_eq!(result.lineage.run_digest, repeated.lineage.run_digest);
        assert_eq!(
            result.lineage.run_binding_digest,
            repeated.lineage.run_binding_digest
        );
        assert_eq!(result.evidence.status, "historical-a3-only");
    }
}
