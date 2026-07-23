use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

const REFINEMENTS: [usize; 7] = [64, 128, 256, 512, 1024, 2048, 4096];
const REPETITIONS: usize = 9;
const REGISTERED_SOURCE_COMMIT: &str = "5696f62ed84eba5457e2ff99f40fd2080c808d69";
const MAX_CSV_BYTES: usize = 256 * 1024;
const MAX_JSON_BYTES: usize = 64 * 1024;
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Summary {
    schema: String,
    comparison: String,
    outcome: String,
    durable_crossing_refinement: Option<usize>,
    summaries: Vec<RefinementSummary>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefinementSummary {
    refinement: usize,
    rows: usize,
    nonzeros: usize,
    cpu_median_ns: u128,
    cuda_median_ns: u128,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Environment {
    absolute_tolerance: f64,
    binding_toolkit: String,
    cpu_affinity_count: usize,
    cpu_frequency_policy: CpuFrequencyPolicy,
    cpu_model: String,
    cuda_comparison: String,
    cuda_driver: i32,
    cudarc: String,
    cusparse: i32,
    device_residency: String,
    eqiora_device_ordinal: u16,
    gpu_compute_process_count_after: usize,
    gpu_compute_process_count_before: usize,
    gpu_memory_bytes: u64,
    gpu_name: String,
    gpu_operating_after: GpuOperatingPoint,
    gpu_operating_before: GpuOperatingPoint,
    host_memory: String,
    index: String,
    kernel: String,
    matrix: String,
    os_release: String,
    policy: String,
    profile: String,
    reference: String,
    refinements: Vec<usize>,
    relative_tolerance: f64,
    repetitions: usize,
    rustc: String,
    rustflags_present: bool,
    scalar: String,
    schema: String,
    source_clean: bool,
    source_commit: String,
    system_load_after: SystemLoad,
    system_load_before: SystemLoad,
    warmups: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CpuFrequencyPolicy {
    governor: String,
    maximum_khz: u64,
    minimum_khz: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SystemLoad {
    fifteen_minutes: f64,
    five_minutes: f64,
    one_minute: f64,
    runnable_processes: usize,
    total_processes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuOperatingPoint {
    maximum_memory_clock_mhz: u64,
    maximum_sm_clock_mhz: u64,
    memory_clock_mhz: u64,
    memory_used_mib: u64,
    performance_state: String,
    power_draw_watts: f64,
    power_limit_watts: f64,
    sm_clock_mhz: u64,
    temperature_celsius: f64,
    utilization_percent: f64,
}

#[test]
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

#[test]
fn committed_json_schemas_are_closed_and_inputs_are_bounded() {
    let root = case_root();
    let environment_bytes =
        read_bounded(&root.join("observations/environment.json"), MAX_JSON_BYTES);
    let mut environment: serde_json::Value =
        serde_json::from_slice(&environment_bytes).expect("environment JSON");
    environment["host_label"] = serde_json::json!("must-not-be-admitted");
    assert!(
        decode_closed::<Environment>(&serde_json::to_vec(&environment).unwrap()).is_err(),
        "an unknown top-level environment field must fail closed"
    );

    let summary_bytes = read_bounded(&root.join("expected/summary.json"), MAX_JSON_BYTES);
    let mut summary: serde_json::Value =
        serde_json::from_slice(&summary_bytes).expect("summary JSON");
    summary["summaries"][0]["collector_note"] = serde_json::json!("must-not-be-admitted");
    assert!(
        decode_closed::<Summary>(&serde_json::to_vec(&summary).unwrap()).is_err(),
        "an unknown nested summary field must fail closed"
    );

    assert!(read_bounded_bytes(&[], MAX_JSON_BYTES).is_err());
    assert!(read_bounded_bytes(&vec![b' '; MAX_JSON_BYTES + 1], MAX_JSON_BYTES).is_err());
}

fn case_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verify/performance/cuda-csr-transfer-threshold")
}

fn read_samples(path: &Path) -> Vec<Sample> {
    let bytes = read_bounded(path, MAX_CSV_BYTES);
    let source = std::str::from_utf8(&bytes).expect("evidence CSV must be UTF-8");
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
    let summary: Summary =
        decode_closed(&read_bounded(path, MAX_JSON_BYTES)).expect("closed summary JSON");
    assert_eq!(
        summary.schema,
        "eqiora.cuda-csr-transfer-threshold.summary.v2"
    );
    assert_eq!(
        summary.comparison,
        "median-cold-cuda-execution-wall-vs-serial-host-action"
    );
    assert_eq!(
        summary.outcome,
        if durable_crossing.is_some() {
            "durable-crossing-observed"
        } else {
            "no-crossing-in-measured-range"
        }
    );
    match durable_crossing {
        Some(index) => assert_eq!(summary.durable_crossing_refinement, Some(replayed[index].0)),
        None => assert_eq!(summary.durable_crossing_refinement, None),
    }
    assert_eq!(summary.summaries.len(), replayed.len());
    for (value, replayed) in summary.summaries.iter().zip(replayed) {
        assert_eq!(value.refinement, replayed.0);
        assert_eq!(value.rows, replayed.1);
        assert_eq!(value.nonzeros, replayed.2);
        assert_eq!(value.cpu_median_ns, replayed.3);
        assert_eq!(value.cuda_median_ns, replayed.4);
    }
}

fn replay_environment(path: &Path) {
    let environment: Environment =
        decode_closed(&read_bounded(path, MAX_JSON_BYTES)).expect("closed environment JSON");
    assert_eq!(
        environment.schema,
        "eqiora.cuda-csr-transfer-threshold.environment.v3"
    );
    let source_commit = environment.source_commit.as_str();
    assert_eq!(source_commit.len(), 40);
    assert!(
        source_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    assert_eq!(
        source_commit, REGISTERED_SOURCE_COMMIT,
        "register the exact public source commit with the observation"
    );
    assert!(environment.source_clean);
    assert_eq!(environment.profile, "release");
    assert_eq!(environment.scalar, "f64");
    assert_eq!(environment.index, "i64");
    assert_eq!(environment.policy, "backend-native");
    assert_eq!(environment.host_memory, "pageable");
    assert_eq!(environment.device_residency, "fresh-per-sample");
    assert_eq!(
        environment.cuda_comparison,
        "outer-verified-call-minus-recorded-reference-comparison"
    );
    assert_eq!(
        environment.reference,
        "precomputed-serial-host-action; both timed actions independently accepted"
    );
    assert_eq!(
        environment.matrix,
        "2d-dirichlet-five-point-row-major-sorted-csr"
    );
    assert_eq!(environment.eqiora_device_ordinal, 0);
    assert_eq!(environment.warmups, 2);
    assert_eq!(environment.repetitions, REPETITIONS);
    assert_eq!(environment.refinements, REFINEMENTS);
    for field in [
        environment.rustc.as_str(),
        environment.kernel.as_str(),
        environment.os_release.as_str(),
        environment.cpu_model.as_str(),
        environment.gpu_name.as_str(),
        environment.cudarc.as_str(),
        environment.binding_toolkit.as_str(),
    ] {
        assert!(!field.trim().is_empty());
    }
    let _rustflags_present = environment.rustflags_present;
    assert_eq!(environment.cpu_affinity_count, 1);
    assert!(!environment.cpu_frequency_policy.governor.trim().is_empty());
    assert!(environment.cpu_frequency_policy.minimum_khz > 0);
    assert!(environment.cpu_frequency_policy.maximum_khz > 0);
    validate_system_load(&environment.system_load_before);
    validate_system_load(&environment.system_load_after);
    validate_gpu_operating_point(&environment.gpu_operating_before);
    validate_gpu_operating_point(&environment.gpu_operating_after);
    let _gpu_compute_process_counts = (
        environment.gpu_compute_process_count_before,
        environment.gpu_compute_process_count_after,
    );
    assert!(environment.gpu_memory_bytes > 0);
    assert!(environment.cuda_driver > 0);
    assert!(environment.cusparse > 0);
    assert_eq!(environment.absolute_tolerance, 1.0e-11);
    assert_eq!(environment.relative_tolerance, 1.0e-11);
}

fn validate_system_load(system_load: &SystemLoad) {
    for value in [
        system_load.one_minute,
        system_load.five_minutes,
        system_load.fifteen_minutes,
    ] {
        assert!(value >= 0.0);
    }
    assert!(system_load.runnable_processes > 0);
    assert!(system_load.total_processes > 0);
}

fn validate_gpu_operating_point(gpu: &GpuOperatingPoint) {
    assert!(!gpu.performance_state.trim().is_empty());
    for value in [
        gpu.temperature_celsius,
        gpu.utilization_percent,
        gpu.power_draw_watts,
        gpu.power_limit_watts,
    ] {
        assert!(value >= 0.0);
    }
    let _memory_used_mib = gpu.memory_used_mib;
    for value in [
        gpu.sm_clock_mhz,
        gpu.memory_clock_mhz,
        gpu.maximum_sm_clock_mhz,
        gpu.maximum_memory_clock_mhz,
    ] {
        assert!(value > 0);
    }
}

fn read_bounded(path: &Path, maximum: usize) -> Vec<u8> {
    let mut file = fs::File::open(path)
        .unwrap_or_else(|error| panic!("cannot open bounded evidence {}: {error}", path.display()));
    let read_limit = u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    file.by_ref()
        .take(read_limit)
        .read_to_end(&mut bytes)
        .unwrap_or_else(|error| panic!("cannot read bounded evidence {}: {error}", path.display()));
    read_bounded_bytes(&bytes, maximum)
        .unwrap_or_else(|error| panic!("invalid bounded evidence {}: {error}", path.display()));
    bytes
}

fn read_bounded_bytes(bytes: &[u8], maximum: usize) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(format!(
            "evidence bytes require 1..={maximum} bytes, found {}",
            bytes.len()
        ));
    }
    Ok(())
}

fn decode_closed<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let decoded = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(decoded)
}
