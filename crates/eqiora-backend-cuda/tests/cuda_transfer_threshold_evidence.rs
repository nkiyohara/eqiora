use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const REFINEMENTS: [usize; 7] = [64, 128, 256, 512, 1024, 2048, 4096];
const REPETITIONS: usize = 9;
// Set this to the exact public source commit when the observation is collected,
// then remove the test's ignore marker in the same evidence-registration change.
const REGISTERED_SOURCE_COMMIT: Option<&str> = None;
const CSV_HEADER: &str = "refinement,rows,nonzeros,repetition,order,cpu_ns,cuda_setup_ns,cuda_h2d_ns,cuda_action_ns,cuda_d2h_ns,cuda_phase_sum_ns,cuda_execution_wall_ns,cuda_verification_ns,cuda_verified_wall_ns,maximum_absolute_error,maximum_scaled_error,host_to_device_bytes,device_to_host_bytes,workspace_bytes";

#[derive(Debug, Clone)]
struct Sample {
    refinement: usize,
    rows: usize,
    nonzeros: usize,
    repetition: usize,
    order: String,
    cpu_ns: u128,
    cuda_setup_ns: u128,
    cuda_h2d_ns: u128,
    cuda_action_ns: u128,
    cuda_d2h_ns: u128,
    cuda_phase_sum_ns: u128,
    cuda_execution_wall_ns: u128,
    cuda_verification_ns: u128,
    cuda_verified_wall_ns: u128,
    maximum_absolute_error: f64,
    maximum_scaled_error: f64,
    host_to_device_bytes: usize,
    device_to_host_bytes: usize,
    workspace_bytes: usize,
}

#[test]
#[ignore = "physical evidence is recollected from the first public commit before registration"]
fn committed_cuda_transfer_threshold_evidence_replays() {
    let root = case_root();
    let samples = read_samples(&root.join("observations/repetitions.csv"));
    assert_eq!(samples.len(), REFINEMENTS.len() * REPETITIONS);

    let mut by_refinement = BTreeMap::<usize, Vec<&Sample>>::new();
    for sample in &samples {
        by_refinement
            .entry(sample.refinement)
            .or_default()
            .push(sample);
    }
    assert_eq!(
        by_refinement.keys().copied().collect::<Vec<_>>(),
        REFINEMENTS
    );

    let mut replayed = Vec::with_capacity(REFINEMENTS.len());
    for refinement in REFINEMENTS {
        let samples = by_refinement.get_mut(&refinement).unwrap();
        samples.sort_by_key(|sample| sample.repetition);
        assert_eq!(samples.len(), REPETITIONS);
        let rows = refinement.checked_mul(refinement).unwrap();
        let nonzeros = rows
            .checked_mul(5)
            .unwrap()
            .checked_sub(refinement.checked_mul(4).unwrap())
            .unwrap();
        for (repetition, sample) in samples.iter().enumerate() {
            assert_eq!(sample.repetition, repetition);
            assert_eq!(sample.rows, rows);
            assert_eq!(sample.nonzeros, nonzeros);
            assert_eq!(
                sample.order,
                if repetition % 2 == 0 {
                    "cpu-first"
                } else {
                    "cuda-first"
                }
            );
            assert!(sample.cpu_ns > 0);
            assert!(sample.cuda_setup_ns > 0);
            assert!(sample.cuda_h2d_ns > 0);
            assert!(sample.cuda_action_ns > 0);
            assert!(sample.cuda_d2h_ns > 0);
            assert!(sample.cuda_verification_ns > 0);
            assert_eq!(
                sample.cuda_phase_sum_ns,
                sample.cuda_setup_ns
                    + sample.cuda_h2d_ns
                    + sample.cuda_action_ns
                    + sample.cuda_d2h_ns
            );
            assert!(sample.cuda_execution_wall_ns >= sample.cuda_phase_sum_ns);
            assert_eq!(
                sample.cuda_verified_wall_ns,
                sample.cuda_execution_wall_ns + sample.cuda_verification_ns
            );
            assert!(sample.maximum_absolute_error.is_finite());
            assert!(sample.maximum_absolute_error >= 0.0);
            assert!(sample.maximum_scaled_error.is_finite());
            assert!(sample.maximum_scaled_error >= 0.0);
            assert!(sample.maximum_scaled_error <= 1.0);
            assert_eq!(
                sample.host_to_device_bytes,
                (rows + 1) * size_of::<i64>()
                    + nonzeros * size_of::<i64>()
                    + nonzeros * size_of::<f64>()
                    + rows * size_of::<f64>()
            );
            assert_eq!(sample.device_to_host_bytes, rows * size_of::<f64>());
            assert!(sample.workspace_bytes > 0);
        }
        replayed.push((
            refinement,
            rows,
            nonzeros,
            median(samples.iter().map(|sample| sample.cpu_ns)),
            median(samples.iter().map(|sample| sample.cuda_execution_wall_ns)),
        ));
    }

    let durable_crossing = replayed.iter().enumerate().position(|(index, candidate)| {
        index + 1 < replayed.len()
            && candidate.4 < candidate.3
            && replayed
                .iter()
                .skip_while(|summary| summary.0 < candidate.0)
                .all(|summary| summary.4 < summary.3)
    });
    assert_eq!(durable_crossing, None);
    replay_summary(
        &root.join("expected/summary.json"),
        &replayed,
        durable_crossing,
    );
    replay_environment(&root.join("observations/environment.json"));
}

fn case_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verify/performance/cuda-csr-transfer-threshold")
}

fn read_samples(path: &Path) -> Vec<Sample> {
    let source = fs::read_to_string(path).unwrap();
    let mut lines = source.lines();
    assert_eq!(lines.next(), Some(CSV_HEADER));
    lines
        .enumerate()
        .map(|(index, line)| parse_sample(index + 2, line))
        .collect()
}

fn parse_sample(line_number: usize, line: &str) -> Sample {
    let fields = line.split(',').collect::<Vec<_>>();
    assert_eq!(fields.len(), 19, "CSV line {line_number}");
    Sample {
        refinement: parse(fields[0], line_number),
        rows: parse(fields[1], line_number),
        nonzeros: parse(fields[2], line_number),
        repetition: parse(fields[3], line_number),
        order: fields[4].to_owned(),
        cpu_ns: parse(fields[5], line_number),
        cuda_setup_ns: parse(fields[6], line_number),
        cuda_h2d_ns: parse(fields[7], line_number),
        cuda_action_ns: parse(fields[8], line_number),
        cuda_d2h_ns: parse(fields[9], line_number),
        cuda_phase_sum_ns: parse(fields[10], line_number),
        cuda_execution_wall_ns: parse(fields[11], line_number),
        cuda_verification_ns: parse(fields[12], line_number),
        cuda_verified_wall_ns: parse(fields[13], line_number),
        maximum_absolute_error: parse(fields[14], line_number),
        maximum_scaled_error: parse(fields[15], line_number),
        host_to_device_bytes: parse(fields[16], line_number),
        device_to_host_bytes: parse(fields[17], line_number),
        workspace_bytes: parse(fields[18], line_number),
    }
}

fn parse<T>(value: &str, line_number: usize) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .unwrap_or_else(|error| panic!("invalid CSV value on line {line_number}: {error}"))
}

fn median(values: impl IntoIterator<Item = u128>) -> u128 {
    let mut values = values.into_iter().collect::<Vec<_>>();
    assert!(!values.is_empty());
    assert_eq!(values.len() % 2, 1);
    values.sort_unstable();
    values[values.len() / 2]
}

fn replay_summary(
    path: &Path,
    replayed: &[(usize, usize, usize, u128, u128)],
    durable_crossing: Option<usize>,
) {
    let summary: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        summary["schema"],
        "eqiora.cuda-csr-transfer-threshold.summary.v2"
    );
    assert_eq!(
        summary["comparison"],
        "median-cold-cuda-execution-wall-vs-serial-host-action"
    );
    assert_eq!(
        summary["outcome"],
        if durable_crossing.is_some() {
            "durable-crossing-observed"
        } else {
            "no-crossing-in-measured-range"
        }
    );
    match durable_crossing {
        Some(index) => assert_eq!(
            summary["durable_crossing_refinement"].as_u64(),
            Some(u64::try_from(replayed[index].0).unwrap())
        ),
        None => assert!(summary["durable_crossing_refinement"].is_null()),
    }
    let committed = summary["summaries"].as_array().unwrap();
    assert_eq!(committed.len(), replayed.len());
    for (value, replayed) in committed.iter().zip(replayed) {
        assert_eq!(value["refinement"].as_u64(), Some(replayed.0 as u64));
        assert_eq!(value["rows"].as_u64(), Some(replayed.1 as u64));
        assert_eq!(value["nonzeros"].as_u64(), Some(replayed.2 as u64));
        assert_eq!(value["cpu_median_ns"].as_u64(), Some(replayed.3 as u64));
        assert_eq!(value["cuda_median_ns"].as_u64(), Some(replayed.4 as u64));
    }
}

fn replay_environment(path: &Path) {
    let environment: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        environment["schema"],
        "eqiora.cuda-csr-transfer-threshold.environment.v3"
    );
    let source_commit = environment["source_commit"].as_str().unwrap();
    assert_eq!(source_commit.len(), 40);
    assert!(
        source_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    assert_eq!(
        Some(source_commit),
        REGISTERED_SOURCE_COMMIT,
        "register the exact public source commit with the observation"
    );
    assert_eq!(environment["source_clean"], true);
    assert_eq!(environment["profile"], "release");
    assert_eq!(environment["scalar"], "f64");
    assert_eq!(environment["index"], "i64");
    assert_eq!(environment["policy"], "backend-native");
    assert_eq!(environment["host_memory"], "pageable");
    assert_eq!(environment["device_residency"], "fresh-per-sample");
    assert_eq!(
        environment["cuda_comparison"],
        "outer-verified-call-minus-recorded-reference-comparison"
    );
    assert_eq!(
        environment["reference"],
        "precomputed-serial-host-action; both timed actions independently accepted"
    );
    assert_eq!(
        environment["matrix"],
        "2d-dirichlet-five-point-row-major-sorted-csr"
    );
    assert_eq!(environment["eqiora_device_ordinal"], 0);
    assert_eq!(environment["warmups"], 2);
    assert_eq!(environment["repetitions"], REPETITIONS);
    assert_eq!(
        environment["refinements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect::<Vec<_>>(),
        REFINEMENTS
    );
    for field in [
        "rustc",
        "kernel",
        "os_release",
        "cpu_model",
        "gpu_name",
        "cudarc",
        "binding_toolkit",
    ] {
        assert!(!environment[field].as_str().unwrap().trim().is_empty());
    }
    assert!(environment["rustflags_present"].is_boolean());
    assert_eq!(environment["cpu_affinity_count"], 1);
    assert!(
        !environment["cpu_frequency_policy"]["governor"]
            .as_str()
            .unwrap()
            .trim()
            .is_empty()
    );
    for field in ["minimum_khz", "maximum_khz"] {
        assert!(environment["cpu_frequency_policy"][field].as_u64().unwrap() > 0);
    }
    for moment in ["before", "after"] {
        let system_load_key = format!("system_load_{moment}");
        let system_load = &environment[system_load_key.as_str()];
        for field in ["one_minute", "five_minutes", "fifteen_minutes"] {
            assert!(system_load[field].as_f64().unwrap() >= 0.0);
        }
        for field in ["runnable_processes", "total_processes"] {
            assert!(system_load[field].as_u64().unwrap() > 0);
        }

        let gpu_key = format!("gpu_operating_{moment}");
        let gpu = &environment[gpu_key.as_str()];
        assert!(!gpu["performance_state"].as_str().unwrap().is_empty());
        for field in [
            "temperature_celsius",
            "utilization_percent",
            "power_draw_watts",
            "power_limit_watts",
        ] {
            assert!(gpu[field].as_f64().unwrap() >= 0.0);
        }
        assert!(gpu["memory_used_mib"].as_u64().is_some());
        for field in [
            "sm_clock_mhz",
            "memory_clock_mhz",
            "maximum_sm_clock_mhz",
            "maximum_memory_clock_mhz",
        ] {
            assert!(gpu[field].as_u64().unwrap() > 0);
        }
        let process_count_key = format!("gpu_compute_process_count_{moment}");
        assert!(environment[process_count_key.as_str()].as_u64().is_some());
    }
    assert!(environment["gpu_memory_bytes"].as_u64().unwrap() > 0);
    assert!(environment["cuda_driver"].as_i64().unwrap() > 0);
    assert!(environment["cusparse"].as_i64().unwrap() > 0);
    assert_eq!(environment["absolute_tolerance"].as_f64(), Some(1.0e-11));
    assert_eq!(environment["relative_tolerance"].as_f64(), Some(1.0e-11));
    for forbidden in [
        "hostname",
        "visible_device",
        "gpu_snapshot_before",
        "gpu_snapshot_after",
        "gpu_compute_processes_before",
        "gpu_compute_processes_after",
        "load_average_before",
        "load_average_after",
    ] {
        assert!(environment.get(forbidden).is_none());
    }
}
