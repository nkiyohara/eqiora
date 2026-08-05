//! Worker termination, bounded projection, and panic-containment oracles.

use super::*;
use std::process::Command;

pub(super) type CapturedCompilerInputs = Arc<Mutex<Vec<(String, String)>>>;

pub(super) fn tool_definition() -> Value {
    serde_json::from_str(TOOL_DEFINITION_SOURCE).expect("frozen tool-definition JSON")
}

pub(super) fn response_result<'a>(response: &'a Value, id: &Value) -> &'a Value {
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], *id);
    assert!(!response.as_object().unwrap().contains_key("error"));
    &response["result"]
}

pub(super) fn assert_protocol_error(response: &Value, id: Option<&Value>, code: i64) {
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response.get("id"), id);
    assert_eq!(response["error"]["code"], code);
    assert!(response.get("result").is_none());
}

pub(super) fn accepted_source() -> (String, String) {
    let fixture: Value = serde_json::from_slice(ACCEPTED_CONTROL_REQUEST).unwrap();
    (
        fixture["filename"].as_str().unwrap().to_owned(),
        fixture["source"].as_str().unwrap().to_owned(),
    )
}

pub(super) fn counting_harness(control: Arc<OracleControl>) -> (Harness, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    (
        Harness::start(
            move |filename, source| {
                observed.fetch_add(1, Ordering::SeqCst);
                ModelDocument::compile(filename, source)
            },
            control,
        ),
        calls,
    )
}

pub(super) fn deterministic_harness(
    filename: &str,
    source: &str,
) -> (Harness, CapturedCompilerInputs) {
    let document = ModelDocument::compile(filename, source).unwrap();
    let inputs = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&inputs);
    let harness = Harness::start(
        move |filename, source| {
            observed
                .lock()
                .unwrap()
                .push((filename.to_owned(), source.to_owned()));
            Ok(document.clone())
        },
        Arc::new(OracleControl::new()),
    );
    (harness, inputs)
}

pub(super) fn call(id: Value, arguments: Value) -> Value {
    let mut params = Map::new();
    params.insert("name".to_owned(), expected()["toolName"].clone());
    params.insert("arguments".to_owned(), arguments);
    request(id, "tools/call", params)
}

fn raw_call_with_progress_token(
    id: &str,
    progress_token: &str,
    filename: &str,
    source: &str,
) -> Vec<u8> {
    let mut request = call_with_meta(
        json!(id),
        json!({"filename":filename,"source":source}),
        request_meta(),
    );
    request["params"]["_meta"]["progressToken"] = json!(0);
    let encoded = serde_json::to_string(&request).unwrap();
    let marker = "\"progressToken\":0";
    assert_eq!(encoded.matches(marker).count(), 1);
    format!(
        "{}\n",
        encoded.replacen(marker, &format!("\"progressToken\":{progress_token}"), 1)
    )
    .into_bytes()
}

#[test]
fn request_progress_token_uses_exact_decimal_integer_semantics_before_admission() {
    let contract = expected();
    let witnesses = &contract["metadata"]["requestProgressTokenRawNumberWitnesses"];
    assert_eq!(
        witnesses["falsifiedPredicates"],
        json!([
            "general-is-number",
            "lexical-dot-or-exponent",
            "rounded-f64-fract-equals-zero"
        ])
    );
    assert_eq!(
        witnesses["invalidOutcome"],
        json!({"code":-32602,"compilations":0})
    );
    assert_eq!(
        witnesses["decoderRejectedOutcome"],
        json!({"code":-32700,"echoesId":false,"compilations":0})
    );
    assert_eq!(
        contract["metadata"]["notificationProgressTokenBehavior"],
        "open-unrecognized-ignored"
    );
    let (filename, source) = accepted_source();
    let (harness, inputs) = deterministic_harness(&filename, &source);

    for (index, token) in witnesses["validIntegers"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let token = token.as_str().unwrap();
        let id = format!("valid-progress-{index}");
        harness.send_raw(raw_call_with_progress_token(&id, token, &filename, &source));
        let response = harness.recv();
        assert_eq!(response_result(&response, &json!(id))["isError"], false);
        assert_eq!(
            inputs.lock().unwrap().len(),
            index + 1,
            "valid raw progress token `{token}` did not compile exactly once"
        );
    }
    let valid_compilations = inputs.lock().unwrap().len();

    for (index, token) in witnesses["decoderRejected"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let token = token.as_str().unwrap();
        let id = format!("decoder-rejected-progress-{index}");
        harness.send_raw(raw_call_with_progress_token(&id, token, &filename, &source));
        let response = harness.recv();
        assert_protocol_error(&response, None, -32700);
        assert_eq!(
            inputs.lock().unwrap().len(),
            valid_compilations,
            "decoder-rejected raw progress token `{token}` compiled"
        );
    }
    for (index, token) in witnesses["invalidNonIntegers"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let token = token.as_str().unwrap();
        let id = format!("invalid-progress-{index}");
        harness.send_raw(raw_call_with_progress_token(&id, token, &filename, &source));
        let response = harness.recv();
        assert_protocol_error(&response, Some(&json!(id)), -32602);
        assert_eq!(
            inputs.lock().unwrap().len(),
            valid_compilations,
            "invalid raw progress token `{token}` compiled"
        );
    }
    harness.finish();
}

pub(super) fn wait_flag(receiver: &mpsc::Receiver<()>, name: &str) {
    receiver
        .recv_timeout(timeout("ordinaryResponseMs"))
        .unwrap_or_else(|error| panic!("{name} was not reached: {error}"));
}

pub(super) fn rerun_capturing_stderr(environment: &str, test: &str, markers: &[&str]) -> bool {
    if std::env::var_os(environment).is_some() {
        return false;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env(environment, "1")
        .output()
        .expect("run stderr-capture witness");
    assert!(
        output.status.success(),
        "stderr child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for marker in markers {
        assert!(!stderr.contains(marker), "stderr leaked `{marker}`");
    }
    true
}

fn parse_raw_value(source: &str, start: usize) -> (Value, usize) {
    let mut values = serde_json::Deserializer::from_str(&source[start..]).into_iter::<Value>();
    let value = values.next().unwrap().unwrap();
    (value, start + values.byte_offset())
}

fn skip_whitespace(source: &str, mut cursor: usize) -> usize {
    while source
        .as_bytes()
        .get(cursor)
        .is_some_and(u8::is_ascii_whitespace)
    {
        cursor += 1;
    }
    cursor
}

fn raw_object_members(source: &str) -> Vec<(String, &str)> {
    let mut cursor = skip_whitespace(source, 0);
    assert_eq!(source.as_bytes().get(cursor), Some(&b'{'));
    cursor += 1;
    let mut members = Vec::new();
    loop {
        cursor = skip_whitespace(source, cursor);
        if source.as_bytes().get(cursor) == Some(&b'}') {
            cursor = skip_whitespace(source, cursor + 1);
            assert_eq!(cursor, source.len());
            return members;
        }
        let (key, key_end) = parse_raw_value(source, cursor);
        let key = key.as_str().unwrap().to_owned();
        cursor = skip_whitespace(source, key_end);
        assert_eq!(source.as_bytes().get(cursor), Some(&b':'));
        let value_start = skip_whitespace(source, cursor + 1);
        let (_, value_end) = parse_raw_value(source, value_start);
        members.push((key, &source[value_start..value_end]));
        cursor = skip_whitespace(source, value_end);
        match source.as_bytes().get(cursor) {
            Some(b',') => cursor += 1,
            Some(b'}') => {}
            other => panic!("invalid raw object separator {other:?}"),
        }
    }
}

fn raw_array_values(source: &str) -> Vec<&str> {
    let mut cursor = skip_whitespace(source, 0);
    assert_eq!(source.as_bytes().get(cursor), Some(&b'['));
    cursor += 1;
    let mut values = Vec::new();
    loop {
        cursor = skip_whitespace(source, cursor);
        if source.as_bytes().get(cursor) == Some(&b']') {
            cursor = skip_whitespace(source, cursor + 1);
            assert_eq!(cursor, source.len());
            return values;
        }
        let start = cursor;
        let (_, end) = parse_raw_value(source, start);
        values.push(&source[start..end]);
        cursor = skip_whitespace(source, end);
        match source.as_bytes().get(cursor) {
            Some(b',') => cursor += 1,
            Some(b']') => {}
            other => panic!("invalid raw array separator {other:?}"),
        }
    }
}

fn canonical_json(source: &str) -> String {
    match serde_json::from_str::<Value>(source).unwrap() {
        Value::Object(_) => {
            let body = raw_object_members(source)
                .iter()
                .map(|(name, raw)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(name).unwrap(),
                        canonical_json(raw)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
        Value::Array(_) => {
            let body = raw_array_values(source)
                .iter()
                .map(|raw| canonical_json(raw))
                .collect::<Vec<_>>()
                .join(",");
            format!("[{body}]")
        }
        value => serde_json::to_string(&value).unwrap(),
    }
}

fn raw_member<'a>(members: &[(String, &'a str)], name: &str) -> &'a str {
    members
        .iter()
        .find(|(candidate, _)| candidate == name)
        .unwrap_or_else(|| panic!("missing raw member `{name}`"))
        .1
}

fn frozen_order<'a>(contract: &'a Value, name: &str) -> Vec<&'a str> {
    contract["resultProjection"][name]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member.as_str().unwrap())
        .collect()
}

fn assert_raw_order(members: &[(String, &str)], expected: &[&str]) {
    assert_eq!(
        members
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        expected
    );
}

pub(super) fn assert_exact_result_wire(bytes: &[u8]) {
    let response: Value = serde_json::from_slice(bytes).unwrap();
    if response["result"].get("content").is_none() {
        return;
    }
    let contract = expected();
    let text = std::str::from_utf8(bytes)
        .unwrap()
        .strip_suffix('\n')
        .expect("one response LF");
    assert_eq!(
        canonical_json(text),
        text,
        "response JSON must be canonical compact JSON"
    );
    let response_members = raw_object_members(text);
    let result_members = raw_object_members(raw_member(&response_members, "result"));
    assert_raw_order(
        &result_members,
        &frozen_order(&contract, "resultMemberOrder"),
    );
    let content = raw_array_values(raw_member(&result_members, "content"));
    assert_eq!(content.len(), 1);
    let content_members = raw_object_members(content[0]);
    assert_raw_order(&content_members, &["type", "text"]);
    let text: String = serde_json::from_str(raw_member(&content_members, "text")).unwrap();
    let structured_raw = raw_member(&result_members, "structuredContent");
    let canonical = canonical_json(structured_raw);
    assert_eq!(structured_raw.as_bytes(), canonical.as_bytes());
    assert_eq!(text.as_bytes(), canonical.as_bytes());
    let structured = raw_object_members(structured_raw);
    if response["result"]["structuredContent"]["status"] == "accepted" {
        assert_raw_order(
            &structured,
            &frozen_order(&contract, "acceptedStructuredMemberOrder"),
        );
        let model = raw_object_members(raw_member(&structured, "model"));
        assert_raw_order(&model, &frozen_order(&contract, "acceptedModelMemberOrder"));
        let fingerprint = raw_object_members(raw_member(&model, "structuralFingerprint"));
        assert_raw_order(
            &fingerprint,
            &frozen_order(&contract, "fingerprintMemberOrder"),
        );
    } else {
        assert_raw_order(
            &structured,
            &frozen_order(&contract, "rejectedStructuredMemberOrder"),
        );
        for diagnostic in raw_array_values(raw_member(&structured, "diagnostics")) {
            let diagnostic = raw_object_members(diagnostic);
            assert_raw_order(
                &diagnostic,
                &frozen_order(&contract, "diagnosticMemberOrder"),
            );
            let span = raw_member(&diagnostic, "span");
            if span != "null" {
                assert_raw_order(
                    &raw_object_members(span),
                    &frozen_order(&contract, "spanMemberOrder"),
                );
            }
            let patch = raw_member(&diagnostic, "patch");
            if patch != "null" {
                assert_raw_order(
                    &raw_object_members(patch),
                    &frozen_order(&contract, "patchMemberOrder"),
                );
            }
        }
    }
}

fn assert_exact_overflow(result: &Value) {
    let expected = expected();
    let expected_members = expected["resultProjection"]["resultMemberOrder"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member.as_str().unwrap())
        .collect::<Vec<_>>();
    let mut actual_members = result
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    actual_members.sort_unstable();
    let mut expected_members_sorted = expected_members.clone();
    expected_members_sorted.sort_unstable();
    assert_eq!(actual_members, expected_members_sorted);
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["status"], "rejected");
    assert_eq!(
        result["structuredContent"]["diagnostics"],
        json!([expected["overflowDiagnostic"]])
    );
    assert_eq!(
        result["structuredContent"]["schema"],
        tool_definition()["outputSchema"]["oneOf"][1]["properties"]["schema"]["const"]
    );
    assert_eq!(result["content"].as_array().unwrap().len(), 1);
    assert_eq!(result["content"][0]["type"], "text");
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, result["structuredContent"]);
    assert!(
        !result["structuredContent"]
            .as_object()
            .unwrap()
            .contains_key("model")
    );
}

#[test]
fn closing_input_suppresses_an_unsent_active_response_without_joining_the_worker() {
    let calls = Arc::new(AtomicUsize::new(0));
    let compiler_calls = Arc::clone(&calls);
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let release_receiver = Arc::new(Mutex::new(release_receiver));
    let compiler_release = Arc::clone(&release_receiver);
    let mut harness = Harness::start(
        move |filename, source| {
            compiler_calls.fetch_add(1, Ordering::SeqCst);
            started_sender.send(()).unwrap();
            compiler_release.lock().unwrap().recv().unwrap();
            ModelDocument::compile(filename, source)
        },
        Arc::new(OracleControl::new()),
    );
    let (filename, source) = accepted_source();
    harness.send(call(
        json!("shutdown-active"),
        json!({"filename":filename,"source":source}),
    ));
    wait_flag(&started_receiver, "shutdown compiler");
    harness.input.take();
    let outcome = harness
        .done
        .recv_timeout(timeout("shutdownMs"))
        .expect("server returns while compiler is blocked");
    outcome.unwrap();
    harness.thread.take().unwrap().join().unwrap();
    assert!(
        harness
            .output
            .recv_timeout(timeout("silenceProbeMs"))
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    release_sender.send(()).unwrap();
}

fn run_diagnostic_case(name: &str, diagnostics: Vec<Diagnostic>) -> Value {
    let calls = Arc::new(AtomicUsize::new(0));
    let compiler_calls = Arc::clone(&calls);
    let diagnostics = Arc::new(diagnostics);
    let compiler_diagnostics = Arc::clone(&diagnostics);
    let harness = Harness::start(
        move |_, _| {
            compiler_calls.fetch_add(1, Ordering::SeqCst);
            Err(compiler_diagnostics.as_ref().clone())
        },
        Arc::new(OracleControl::new()),
    );
    harness.send(call(
        json!(name),
        json!({"filename":"overflow.eqi","source":"admitted"}),
    ));
    let response = harness.recv_with(timeout("largeResponseMs"));
    let result = response_result(&response, &json!(name)).clone();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    harness.finish();
    result
}

fn run_overflow_case(name: &str, diagnostics: Vec<Diagnostic>) {
    assert_exact_overflow(&run_diagnostic_case(name, diagnostics));
}

fn assert_exact_diagnostics(
    name: &str,
    diagnostics: Vec<Diagnostic>,
    expected_count: usize,
) -> Value {
    let result = run_diagnostic_case(name, diagnostics);
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["status"], "rejected");
    assert_eq!(
        result["structuredContent"]["schema"],
        tool_definition()["outputSchema"]["oneOf"][1]["properties"]["schema"]["const"]
    );
    assert_eq!(
        result["structuredContent"]["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        expected_count
    );
    assert_ne!(
        result["structuredContent"]["diagnostics"],
        json!([expected()["overflowDiagnostic"]])
    );
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, result["structuredContent"]);
    result
}

#[test]
fn every_diagnostic_and_complete_result_overflow_is_substituted_atomically() {
    let valid = || Diagnostic::error(Code("EQ0901"), "bounded");
    assert_ne!(
        canonical_json("{ \"mutant\": \"\\u0061\" }"),
        "{ \"mutant\": \"\\u0061\" }"
    );
    run_overflow_case("zero-count", Vec::new());
    assert_exact_diagnostics("max-count", vec![valid(); 1024], 1024);
    let warning = assert_exact_diagnostics(
        "warning-severity",
        vec![Diagnostic::warning(Code("EQ0901"), "bounded")],
        1,
    );
    assert_eq!(
        warning["structuredContent"]["diagnostics"][0]["severity"],
        "warning"
    );
    run_overflow_case("count", vec![valid(); 1025]);
    assert_exact_diagnostics(
        "max-message-ascii",
        vec![Diagnostic::error(Code("EQ0901"), "x".repeat(1_048_576))],
        1,
    );
    assert_exact_diagnostics(
        "max-message-two-byte",
        vec![Diagnostic::error(Code("EQ0901"), "é".repeat(524_288))],
        1,
    );
    run_overflow_case(
        "message",
        vec![Diagnostic::error(Code("EQ0901"), "x".repeat(1_048_577))],
    );
    run_overflow_case(
        "message-two-byte",
        vec![Diagnostic::error(Code("EQ0901"), "é".repeat(524_289))],
    );
    run_overflow_case("empty-message", vec![Diagnostic::error(Code("EQ0901"), "")]);
    let empty_graph = assert_exact_diagnostics(
        "zero-graph-segments",
        vec![valid().with_graph_path(GraphPath::new(Vec::<String>::new()))],
        1,
    );
    assert_eq!(
        empty_graph["structuredContent"]["diagnostics"][0]["graphPath"],
        json!([])
    );
    run_overflow_case(
        "empty-graph-segment",
        vec![valid().with_graph_path(GraphPath::new([""]))],
    );
    assert_exact_diagnostics(
        "max-graph-count",
        vec![valid().with_graph_path(GraphPath::new(vec!["x"; 256]))],
        1,
    );
    run_overflow_case(
        "graph-count",
        vec![valid().with_graph_path(GraphPath::new(vec!["x"; 257]))],
    );
    let empty_span = assert_exact_diagnostics(
        "empty-span-file",
        vec![valid().with_span(Span {
            file: String::new(),
            start: 0,
            end: 0,
        })],
        1,
    );
    assert_eq!(
        empty_span["structuredContent"]["diagnostics"][0]["span"],
        json!({"file":"","start":0,"end":0})
    );
    for (name, text) in [
        ("max-text-ascii", "x".repeat(4096)),
        ("max-text-two-byte", "é".repeat(2048)),
    ] {
        assert_exact_diagnostics(
            &format!("{name}-graph"),
            vec![valid().with_graph_path(GraphPath::new([text.clone()]))],
            1,
        );
        assert_exact_diagnostics(
            &format!("{name}-span"),
            vec![valid().with_span(Span {
                file: text.clone(),
                start: u32::MAX,
                end: u32::MAX,
            })],
            1,
        );
        assert_exact_diagnostics(
            &format!("{name}-patch"),
            vec![valid().with_suggestion(Patch::new(text))],
            1,
        );
    }
    run_overflow_case(
        "graph-segment",
        vec![valid().with_graph_path(GraphPath::new(["x".repeat(4097)]))],
    );
    run_overflow_case(
        "graph-segment-two-byte",
        vec![valid().with_graph_path(GraphPath::new(["é".repeat(2049)]))],
    );
    run_overflow_case(
        "span-file",
        vec![valid().with_span(Span {
            file: "x".repeat(4097),
            start: 0,
            end: 1,
        })],
    );
    run_overflow_case(
        "span-file-two-byte",
        vec![valid().with_span(Span {
            file: "é".repeat(2049),
            start: 0,
            end: 1,
        })],
    );
    run_overflow_case(
        "span-order",
        vec![valid().with_span(Span {
            file: "x.eqi".to_owned(),
            start: 2,
            end: 1,
        })],
    );
    run_overflow_case(
        "patch-summary",
        vec![valid().with_suggestion(Patch::new("x".repeat(4097)))],
    );
    run_overflow_case(
        "patch-summary-two-byte",
        vec![valid().with_suggestion(Patch::new("é".repeat(2049)))],
    );
    run_overflow_case(
        "empty-patch-summary",
        vec![valid().with_suggestion(Patch::new(""))],
    );
    for (name, code) in [
        ("code-short", Code("bad")),
        ("code-lowercase", Code("eq0901")),
        ("code-nondigit", Code("EQ09A1")),
        ("code-long", Code("EQ09011")),
    ] {
        run_overflow_case(name, vec![Diagnostic::error(code, "bounded")]);
    }
    let maximum = expected()["responseBounds"]["compactStructuredContentMaximumUtf8Bytes"]
        .as_u64()
        .unwrap() as usize;
    let prefix = (0..8)
        .map(|_| Diagnostic::error(Code("EQ0901"), "x".repeat(1_048_576)))
        .collect::<Vec<_>>();
    let structured_prefix = json!({
        "schema":tool_definition()["outputSchema"]["oneOf"][1]["properties"]["schema"]["const"],
        "status":"rejected",
        "diagnostics":prefix.iter().map(|diagnostic| json!({
            "source":"kernel","severity":"error","code":diagnostic.code().to_string(),
            "message":diagnostic.message(),"graphPath":null,"span":null,"patch":null
        })).collect::<Vec<_>>()
    });
    let mut with_empty_last = structured_prefix.clone();
    with_empty_last["diagnostics"]
        .as_array_mut()
        .unwrap()
        .push(json!({"source":"kernel","severity":"error","code":"EQ0901",
            "message":"","graphPath":null,"span":null,"patch":null}));
    let base = serde_json::to_vec(&with_empty_last).unwrap().len();
    let padding = maximum
        .checked_sub(base)
        .expect("frozen aggregate witness fits");
    assert!(padding <= 1_048_576);
    let mut exact = prefix.clone();
    exact.push(Diagnostic::error(Code("EQ0901"), "x".repeat(padding)));
    let exact_result = run_diagnostic_case("complete-result-exact", exact.clone());
    assert_eq!(
        serde_json::to_vec(&exact_result["structuredContent"])
            .unwrap()
            .len(),
        maximum
    );
    assert_ne!(
        exact_result["structuredContent"]["diagnostics"],
        json!([expected()["overflowDiagnostic"]])
    );
    *exact.last_mut().unwrap() = Diagnostic::error(Code("EQ0901"), "x".repeat(padding + 1));
    run_overflow_case("complete-result-over", exact);
}

#[test]
fn compiler_diagnostic_text_and_filename_are_confined_to_the_bounded_tool_result() {
    const CHILD_ENV: &str = "EQIORA_MCP_DIAGNOSTIC_SECRECY_ORACLE_CHILD";
    const TEST_NAME: &str = "oracle::worker_outcomes::compiler_diagnostic_text_and_filename_are_confined_to_the_bounded_tool_result";
    let markers = [
        "compiler-diagnostic-secret-marker",
        "diagnostic-filename-secret-marker",
    ];
    if rerun_capturing_stderr(CHILD_ENV, TEST_NAME, &markers) {
        return;
    }
    let security = &expected()["security"];
    assert_eq!(
        security["sensitiveApplicationPayloadInProtocolErrorOrStderr"],
        false
    );
    assert!(
        security
            .get("callerPayloadInProtocolErrorOrStderr")
            .is_none()
    );
    let harness = Harness::start(
        |_, _| {
            Err(vec![
                Diagnostic::error(Code("EQ0901"), "compiler-diagnostic-secret-marker").with_span(
                    Span {
                        file: "diagnostic-filename-secret-marker".to_owned(),
                        start: 0,
                        end: 1,
                    },
                ),
            ])
        },
        Arc::new(OracleControl::new()),
    );
    harness.send(call(
        json!("diagnostic-safe-id"),
        json!({"filename":"input.eqi","source":"invalid"}),
    ));
    let tool_response = harness.recv();
    assert_eq!(tool_response["id"], "diagnostic-safe-id");
    let encoded_tool_result = serde_json::to_string(&tool_response).unwrap();
    for marker in markers {
        assert!(encoded_tool_result.contains(marker));
    }
    harness.send(request(
        json!("protocol-after-diagnostic"),
        "unknown",
        Map::new(),
    ));
    let protocol_error = harness.recv();
    assert_protocol_error(
        &protocol_error,
        Some(&json!("protocol-after-diagnostic")),
        -32601,
    );
    let encoded_protocol_error = serde_json::to_string(&protocol_error).unwrap();
    for marker in markers {
        assert!(!encoded_protocol_error.contains(marker));
    }
    harness.finish();
}

#[test]
fn worker_panic_is_contained_without_payload_and_the_server_survives() {
    const CHILD_ENV: &str = "EQIORA_MCP_PANIC_ORACLE_CHILD";
    const TEST_NAME: &str = "oracle::worker_outcomes::worker_panic_is_contained_without_payload_and_the_server_survives";
    if rerun_capturing_stderr(
        CHILD_ENV,
        TEST_NAME,
        &[
            "panic-secret-payload-marker",
            "panic-secret-filename",
            "panic-secret-source",
            "worker_outcomes.rs",
        ],
    ) {
        return;
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let compiler_calls = Arc::clone(&calls);
    let harness = Harness::start(
        move |filename, source| -> Result<ModelDocument, Vec<Diagnostic>> {
            if compiler_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                panic!("panic-secret-payload-marker")
            }
            ModelDocument::compile(filename, source)
        },
        Arc::new(OracleControl::new()),
    );
    harness.send(call(
        json!("panic"),
        json!({"filename":"panic-secret-filename","source":"panic-secret-source"}),
    ));
    let response = harness.recv();
    assert_eq!(response["id"], "panic");
    assert_eq!(response["error"]["code"], -32603);
    let encoded = serde_json::to_string(&response).unwrap();
    for marker in [
        "panic-secret-payload-marker",
        "panic-secret-filename",
        "panic-secret-source",
        "worker_outcomes.rs",
    ] {
        assert!(!encoded.contains(marker));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let (filename, source) = accepted_source();
    harness.send(call(
        json!("after-panic"),
        json!({"filename":filename,"source":source}),
    ));
    let after = harness.recv();
    assert_eq!(after["id"], "after-panic");
    assert_eq!(after["result"]["isError"], false);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    harness.finish();
}
