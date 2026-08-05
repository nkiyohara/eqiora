use std::io::{self, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;

use eqiora::Diagnostic;
use eqiora::api::ModelDocument;
use serde_json::{Map, Value};

use super::framing::{self, InputEvent, Line};
use super::tool;

type Hook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync + 'static>;

struct HookState {
    users: usize,
    saved: Option<Hook>,
}

static HOOK_STATE: OnceLock<Mutex<HookState>> = OnceLock::new();

struct QuietPanics;

impl QuietPanics {
    fn enter() -> Self {
        let state = HOOK_STATE.get_or_init(|| {
            Mutex::new(HookState {
                users: 0,
                saved: None,
            })
        });
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.users == 0 {
            state.saved = Some(std::panic::take_hook());
            std::panic::set_hook(Box::new(|_| {}));
        }
        state.users += 1;
        Self
    }
}

impl Drop for QuietPanics {
    fn drop(&mut self) {
        let Some(lock) = HOOK_STATE.get() else {
            return;
        };
        let mut state = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.users -= 1;
        if state.users == 0
            && let Some(saved) = state.saved.take()
        {
            std::panic::set_hook(saved);
        }
    }
}

#[derive(Clone)]
pub(super) struct Signals {
    #[cfg(test)]
    before: Arc<dyn Fn() + Send + Sync>,
    #[cfg(test)]
    cancelled: Arc<dyn Fn(&Value) + Send + Sync>,
    #[cfg(test)]
    decided: Arc<DecisionSignal>,
}

#[cfg(test)]
type DecisionSignal = dyn Fn(&Value, bool) + Send + Sync;

impl Signals {
    pub(super) fn ordinary() -> Self {
        Self {
            #[cfg(test)]
            before: Arc::new(|| {}),
            #[cfg(test)]
            cancelled: Arc::new(|_| {}),
            #[cfg(test)]
            decided: Arc::new(|_, _| {}),
        }
    }

    #[cfg(test)]
    pub(super) fn testing<B, C, D>(before: B, cancelled: C, decided: D) -> Self
    where
        B: Fn() + Send + Sync + 'static,
        C: Fn(&Value) + Send + Sync + 'static,
        D: Fn(&Value, bool) + Send + Sync + 'static,
    {
        Self {
            before: Arc::new(before),
            cancelled: Arc::new(cancelled),
            decided: Arc::new(decided),
        }
    }

    fn before(&self) {
        #[cfg(test)]
        (self.before)();
    }

    fn cancelled(&self, id: &Value) {
        #[cfg(test)]
        (self.cancelled)(id);
        #[cfg(not(test))]
        let _ = id;
    }

    fn decided(&self, id: &Value, commit: bool) {
        #[cfg(test)]
        (self.decided)(id, commit);
        #[cfg(not(test))]
        let _ = (id, commit);
    }
}

enum Event {
    Input(InputEvent),
    Finished { serial: u64, outcome: WorkerOutcome },
}

enum WorkerOutcome {
    Structured(String),
    Internal,
    Cancelled,
}

struct Active {
    id: Value,
    serial: u64,
    cancelled: Arc<AtomicBool>,
}

pub(super) fn run<R, W, C>(
    reader: R,
    writer: W,
    compiler: C,
    version: &'static str,
    signals: Signals,
) -> io::Result<()>
where
    R: Read + Send + 'static,
    W: Write,
    C: Fn(&str, &str) -> Result<ModelDocument, Vec<Diagnostic>> + Send + Sync + 'static,
{
    let _quiet = QuietPanics::enter();
    let compiler = Arc::new(compiler);
    let (sender, receiver) = mpsc::sync_channel::<Event>(8);
    let input_sender = sender.clone();
    thread::spawn(move || {
        framing::read_lines(reader, |event| {
            input_sender.send(Event::Input(event)).is_ok()
        });
    });
    serve(writer, compiler, sender, receiver, version, signals)
}

fn serve<W, C>(
    mut writer: W,
    compiler: Arc<C>,
    sender: mpsc::SyncSender<Event>,
    receiver: mpsc::Receiver<Event>,
    version: &str,
    signals: Signals,
) -> io::Result<()>
where
    W: Write,
    C: Fn(&str, &str) -> Result<ModelDocument, Vec<Diagnostic>> + Send + Sync + 'static,
{
    let mut active: Option<Active> = None;
    let mut serial = 0_u64;
    loop {
        match receiver.recv() {
            Ok(Event::Input(InputEvent::End)) => return Ok(()),
            Ok(Event::Input(InputEvent::ReadFailure)) => return Err(framing::reader_failure()),
            Ok(Event::Input(InputEvent::Line(line))) => {
                if let Some(response) = handle_line(
                    line,
                    &mut active,
                    &mut serial,
                    &compiler,
                    &sender,
                    version,
                    &signals,
                ) {
                    write_response(&mut writer, &response)?;
                }
            }
            Ok(Event::Finished {
                serial: finished,
                outcome,
            }) => {
                let Some(current) = active.as_ref() else {
                    continue;
                };
                if current.serial != finished {
                    continue;
                }
                let commit = !current.cancelled.load(Ordering::SeqCst)
                    && !matches!(outcome, WorkerOutcome::Cancelled);
                signals.decided(&current.id, commit);
                if commit {
                    let response = match outcome {
                        WorkerOutcome::Structured(structured) => success(
                            &current.id,
                            &tool_result(
                                &structured,
                                structured.contains("\"status\":\"rejected\""),
                                version,
                            ),
                        ),
                        WorkerOutcome::Internal => {
                            error(Some(&current.id), -32603, "Internal error", "null")
                        }
                        WorkerOutcome::Cancelled => unreachable!(),
                    };
                    active = None;
                    write_response(&mut writer, &response)?;
                } else {
                    active = None;
                }
            }
            Err(_) => return Ok(()),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_line<C>(
    line: Line,
    active: &mut Option<Active>,
    serial: &mut u64,
    compiler: &Arc<C>,
    sender: &mpsc::SyncSender<Event>,
    version: &str,
    signals: &Signals,
) -> Option<String>
where
    C: Fn(&str, &str) -> Result<ModelDocument, Vec<Diagnostic>> + Send + Sync + 'static,
{
    let decoded = match line {
        Line::ParseFailure => return Some(error(None, -32700, "Parse error", "null")),
        Line::Invalid { id } => {
            return Some(error(id.as_ref(), -32600, "Invalid Request", "null"));
        }
        Line::Overlong => {
            return Some(error(
                None,
                -32600,
                "MCP message exceeds 67108864 encoded bytes",
                "null",
            ));
        }
        Line::Message(decoded) => decoded,
    };
    let request_progress_token_is_integer = decoded.request_progress_token_is_integer;
    let Some(object) = decoded.value.as_object() else {
        return Some(error(None, -32600, "Invalid Request", "null"));
    };
    let candidate_id = framing::safe_id(object.get("id"));
    let has_id = object.contains_key("id");
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "jsonrpc" | "id" | "method" | "params"))
        || object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || object.get("method").and_then(Value::as_str).is_none()
    {
        return request_error(has_id, candidate_id.as_ref(), -32600, "Invalid Request");
    }
    if has_id && candidate_id.is_none() {
        return Some(error(None, -32600, "Invalid Request", "null"));
    }
    let method = object.get("method").and_then(Value::as_str).unwrap();
    if !has_id {
        if method == "notifications/cancelled" || method.starts_with("notifications/") {
            handle_notification(object, method, active, signals);
            return None;
        }
        return matches!(
            method,
            "server/discover" | "tools/list" | "tools/call" | "initialize"
        )
        .then(|| error(None, -32600, "Invalid Request", "null"));
    }
    let id = candidate_id.as_ref().unwrap();
    if active.as_ref().is_some_and(|current| current.id == *id) {
        return Some(error(Some(id), -32600, "Invalid Request", "null"));
    }
    if method == "initialize" {
        return Some(error(
            Some(id),
            -32601,
            "This server supports only MCP 2026-07-28",
            "null",
        ));
    }
    let Some(params) = object.get("params").and_then(Value::as_object) else {
        return Some(error(Some(id), -32602, "Invalid params", "null"));
    };
    let metadata = match params.get("_meta").and_then(Value::as_object) {
        Some(metadata)
            if validate_request_metadata(metadata, request_progress_token_is_integer) =>
        {
            metadata
        }
        _ => return Some(error(Some(id), -32602, "Invalid params", "null")),
    };
    let requested = metadata
        .get("io.modelcontextprotocol/protocolVersion")
        .and_then(Value::as_str)
        .unwrap();
    if requested != "2026-07-28" {
        let data = format!(
            "{{\"supported\":[\"2026-07-28\"],\"requested\":{}}}",
            quoted(requested)
        );
        return Some(error(
            Some(id),
            -32022,
            "Unsupported protocol version",
            &data,
        ));
    }
    match method {
        "server/discover" => {
            if params.len() != 1 {
                Some(error(Some(id), -32602, "Invalid params", "null"))
            } else {
                Some(success(id, &discover(version)))
            }
        }
        "tools/list" => {
            if params.len() != 1 {
                Some(error(Some(id), -32602, "Invalid params", "null"))
            } else {
                Some(success(id, &list(version)))
            }
        }
        "tools/call" => handle_call(
            id, params, active, serial, compiler, sender, version, signals,
        ),
        _ => Some(error(Some(id), -32601, "Method not found", "null")),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_call<C>(
    id: &Value,
    params: &Map<String, Value>,
    active: &mut Option<Active>,
    serial: &mut u64,
    compiler: &Arc<C>,
    sender: &mpsc::SyncSender<Event>,
    version: &str,
    signals: &Signals,
) -> Option<String>
where
    C: Fn(&str, &str) -> Result<ModelDocument, Vec<Diagnostic>> + Send + Sync + 'static,
{
    if params
        .keys()
        .any(|key| !matches!(key.as_str(), "name" | "arguments" | "_meta"))
    {
        return Some(error(Some(id), -32602, "Invalid params", "null"));
    }
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Some(error(Some(id), -32602, "Invalid tool name", "null"));
    };
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Some(error(Some(id), -32602, "Invalid tool name", "null"));
    }
    if name != tool::NAME {
        return Some(error(
            Some(id),
            -32602,
            &format!("Unknown tool: {name}"),
            "null",
        ));
    }
    let empty = Map::new();
    let arguments = match params.get("arguments") {
        None => &empty,
        Some(Value::Object(arguments)) => arguments,
        _ => return Some(error(Some(id), -32602, "Invalid params", "null")),
    };
    let admitted = match tool::admit(arguments) {
        Ok(admitted) => admitted,
        Err(structured) => return Some(success(id, &tool_result(&structured, true, version))),
    };
    if let Some(current) = active.as_ref() {
        if current.id == *id {
            return Some(error(Some(id), -32600, "Invalid Request", "null"));
        }
        return Some(error(
            Some(id),
            -32800,
            "Eqiora MCP server is busy",
            "{\"activeLimit\":1}",
        ));
    }
    *serial = serial.wrapping_add(1);
    let current_serial = *serial;
    let cancelled = Arc::new(AtomicBool::new(false));
    *active = Some(Active {
        id: id.clone(),
        serial: current_serial,
        cancelled: Arc::clone(&cancelled),
    });
    let compiler = Arc::clone(compiler);
    let sender = sender.clone();
    let signals = signals.clone();
    thread::spawn(move || {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            signals.before();
            if cancelled.load(Ordering::SeqCst) {
                return WorkerOutcome::Cancelled;
            }
            match tool::project(compiler(&admitted.filename, &admitted.source)) {
                Ok(structured) => WorkerOutcome::Structured(structured),
                Err(()) => WorkerOutcome::Internal,
            }
        }))
        .unwrap_or(WorkerOutcome::Internal);
        let _ = sender.send(Event::Finished {
            serial: current_serial,
            outcome,
        });
    });
    None
}

fn handle_notification(
    object: &Map<String, Value>,
    method: &str,
    active: &mut Option<Active>,
    signals: &Signals,
) {
    if method != "notifications/cancelled" {
        return;
    }
    let Some(params) = object.get("params").and_then(Value::as_object) else {
        return;
    };
    if params
        .keys()
        .any(|key| !matches!(key.as_str(), "requestId" | "reason" | "_meta"))
    {
        return;
    }
    let Some(request_id) = framing::safe_id(params.get("requestId")) else {
        return;
    };
    if let Some(reason) = params.get("reason") {
        let Some(reason) = reason.as_str() else {
            return;
        };
        if reason.chars().count() > 4096 || reason.len() > 4096 {
            return;
        }
    }
    if let Some(metadata) = params.get("_meta") {
        let Some(metadata) = metadata.as_object() else {
            return;
        };
        if !validate_cancel_metadata(metadata) {
            return;
        }
    }
    let Some(current) = active.as_ref() else {
        return;
    };
    if current.id == request_id {
        current.cancelled.store(true, Ordering::SeqCst);
        signals.cancelled(&current.id);
    }
}

fn validate_request_metadata(
    metadata: &Map<String, Value>,
    request_progress_token_is_integer: bool,
) -> bool {
    let Some(version) = metadata
        .get("io.modelcontextprotocol/protocolVersion")
        .and_then(Value::as_str)
    else {
        return false;
    };
    if version.chars().count() > 128 || version.len() > 128 {
        return false;
    }
    let Some(capabilities) = metadata
        .get("io.modelcontextprotocol/clientCapabilities")
        .and_then(Value::as_object)
    else {
        return false;
    };
    if !validate_capabilities(capabilities) {
        return false;
    }
    for (name, value) in metadata {
        let valid = match name.as_str() {
            "io.modelcontextprotocol/protocolVersion"
            | "io.modelcontextprotocol/clientCapabilities" => true,
            "io.modelcontextprotocol/clientInfo" => validate_client_info(value),
            "progressToken" => {
                value.is_string() || (value.is_number() && request_progress_token_is_integer)
            }
            "io.modelcontextprotocol/logLevel" => value.as_str().is_some_and(|level| {
                matches!(
                    level,
                    "debug"
                        | "info"
                        | "notice"
                        | "warning"
                        | "error"
                        | "critical"
                        | "alert"
                        | "emergency"
                )
            }),
            "traceparent" => value.as_str().is_some_and(valid_traceparent),
            "tracestate" => value.as_str().is_some_and(valid_tracestate),
            "baggage" => value.as_str().is_some_and(valid_baggage),
            _ => valid_metadata_name(name),
        };
        if !valid {
            return false;
        }
    }
    true
}

fn validate_capabilities(capabilities: &Map<String, Value>) -> bool {
    for (name, value) in capabilities {
        let valid = match name.as_str() {
            "experimental" => value
                .as_object()
                .is_some_and(|members| members.values().all(Value::is_object)),
            "extensions" => value.as_object().is_some_and(|members| {
                members
                    .iter()
                    .all(|(name, value)| valid_prefixed_metadata_name(name) && value.is_object())
            }),
            "roots" => value.is_object(),
            "sampling" => closed_object(value, &["context", "tools"], |value| value.is_object()),
            "elicitation" => closed_object(value, &["form", "url"], |value| value.is_object()),
            _ => true,
        };
        if !valid {
            return false;
        }
    }
    true
}

fn closed_object<F>(value: &Value, names: &[&str], validate: F) -> bool
where
    F: Fn(&Value) -> bool,
{
    value.as_object().is_some_and(|object| {
        object
            .iter()
            .all(|(name, value)| names.contains(&name.as_str()) && validate(value))
    })
}

fn validate_client_info(value: &Value) -> bool {
    let Some(info) = value.as_object() else {
        return false;
    };
    if info.get("name").and_then(Value::as_str).is_none()
        || info.get("version").and_then(Value::as_str).is_none()
        || info.keys().any(|name| {
            !matches!(
                name.as_str(),
                "name" | "version" | "icons" | "title" | "description" | "websiteUrl"
            )
        })
    {
        return false;
    }
    for name in ["title", "description", "websiteUrl"] {
        if info.get(name).is_some_and(|value| !value.is_string()) {
            return false;
        }
    }
    info.get("icons").is_none_or(|icons| {
        icons.as_array().is_some_and(|icons| {
            icons.iter().all(|icon| {
                let Some(icon) = icon.as_object() else {
                    return false;
                };
                icon.get("src").and_then(Value::as_str).is_some()
                    && icon.get("mimeType").is_none_or(Value::is_string)
                    && icon.get("sizes").is_none_or(|sizes| {
                        sizes
                            .as_array()
                            .is_some_and(|sizes| sizes.iter().all(Value::is_string))
                    })
                    && icon.get("theme").is_none_or(|theme| {
                        theme
                            .as_str()
                            .is_some_and(|theme| matches!(theme, "light" | "dark"))
                    })
            })
        })
    })
}

fn validate_cancel_metadata(metadata: &Map<String, Value>) -> bool {
    metadata.iter().all(|(name, value)| match name.as_str() {
        "io.modelcontextprotocol/subscriptionId" => framing::safe_id(Some(value)).is_some(),
        "traceparent" => value.as_str().is_some_and(valid_traceparent),
        "tracestate" => value.as_str().is_some_and(valid_tracestate),
        "baggage" => value.as_str().is_some_and(valid_baggage),
        _ => valid_metadata_name(name),
    })
}

fn valid_metadata_name(name: &str) -> bool {
    name.split_once('/')
        .is_some_and(|_| valid_prefixed_metadata_name(name))
        || (!name.contains('/') && valid_metadata_member(name))
}

fn valid_prefixed_metadata_name(name: &str) -> bool {
    let Some((owner, member)) = name.split_once('/') else {
        return false;
    };
    let labels = owner.split('.').collect::<Vec<_>>();
    if labels.is_empty()
        || labels.iter().any(|label| !valid_prefix_label(label))
        || labels.get(1).is_some_and(|label| {
            label.eq_ignore_ascii_case("mcp") || label.eq_ignore_ascii_case("modelcontextprotocol")
        })
    {
        return false;
    }
    valid_metadata_member(member)
}

fn valid_prefix_label(text: &str) -> bool {
    let bytes = text.as_bytes();
    let Some(first) = bytes.first() else {
        return false;
    };
    let Some(last) = bytes.last() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && last.is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
}

fn valid_metadata_member(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    let bytes = text.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_traceparent(text: &str) -> bool {
    if !text.is_ascii() || text.len() < 55 {
        return false;
    }
    let bytes = text.as_bytes();
    if bytes.get(2) != Some(&b'-') || bytes.get(35) != Some(&b'-') || bytes.get(52) != Some(&b'-') {
        return false;
    }
    let version = &text[0..2];
    let trace = &text[3..35];
    let parent = &text[36..52];
    let flags = &text[53..55];
    if ![version, trace, parent, flags].iter().all(|part| {
        part.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) || version == "ff"
        || trace.bytes().all(|byte| byte == b'0')
        || parent.bytes().all(|byte| byte == b'0')
    {
        return false;
    }
    if version == "00" {
        text.len() == 55
    } else {
        text.len() == 55 || bytes.get(55) == Some(&b'-')
    }
}

fn valid_tracestate(text: &str) -> bool {
    if text.bytes().filter(|byte| *byte == b',').count() >= 32 {
        return false;
    }
    let mut keys: Vec<&str> = Vec::new();
    for member in text.split(',') {
        let member = member.trim_matches([' ', '\t']);
        if member.is_empty() {
            continue;
        }
        if keys.len() == 32 {
            return false;
        }
        let Some((key, value)) = member.split_once('=') else {
            return false;
        };
        if key.is_empty()
            || value.is_empty()
            || value.len() > 256
            || value.starts_with([' ', '\t'])
            || value.ends_with([' ', '\t'])
            || !value
                .bytes()
                .all(|byte| (0x20..=0x7e).contains(&byte) && byte != b',' && byte != b'=')
            || !valid_trace_key(key)
            || keys.contains(&key)
        {
            return false;
        }
        keys.push(key);
    }
    true
}

fn valid_trace_key(key: &str) -> bool {
    if let Some((tenant, system)) = key.split_once('@') {
        tenant.len() <= 241
            && system.len() <= 14
            && trace_key_piece(tenant, true)
            && trace_key_piece(system, false)
    } else {
        key.len() <= 256 && trace_key_piece(key, false)
    }
}

fn trace_key_piece(text: &str, digit_first: bool) -> bool {
    let bytes = text.as_bytes();
    bytes
        .first()
        .is_some_and(|byte| byte.is_ascii_lowercase() || (digit_first && byte.is_ascii_digit()))
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_*/-".contains(byte)
        })
}

fn valid_baggage(text: &str) -> bool {
    let members = text.split(',').collect::<Vec<_>>();
    if members.is_empty() || members.len() > 180 {
        return false;
    }
    members.iter().all(|member| valid_baggage_member(member))
}

fn valid_baggage_member(member: &str) -> bool {
    let pieces = member.split(';').collect::<Vec<_>>();
    let Some(first) = pieces.first() else {
        return false;
    };
    let first = first.trim_matches([' ', '\t']);
    let Some((key, value)) = first.split_once('=') else {
        return false;
    };
    let key = key.trim_matches([' ', '\t']);
    if key.is_empty() || !key.bytes().all(token_byte) || !baggage_value(value) {
        return false;
    }
    pieces[1..].iter().all(|property| {
        let property = property.trim_matches([' ', '\t']);
        if property.is_empty() {
            return false;
        }
        match property.split_once('=') {
            Some((key, value)) => {
                let key = key.trim_matches([' ', '\t']);
                !key.is_empty() && key.bytes().all(token_byte) && baggage_value(value)
            }
            None => property.bytes().all(token_byte),
        }
    })
}

fn token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn baggage_value(text: &str) -> bool {
    let text = text.trim_matches([' ', '\t']);
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if !(0x21..=0x7e).contains(&byte) || matches!(byte, b'"' | b',' | b';' | b'\\') {
            return false;
        }
        if byte == b'%' {
            if bytes
                .get(at + 1)
                .is_none_or(|byte| !byte.is_ascii_hexdigit())
                || bytes
                    .get(at + 2)
                    .is_none_or(|byte| !byte.is_ascii_hexdigit())
            {
                return false;
            }
            at += 3;
        } else {
            at += 1;
        }
    }
    true
}

fn request_error(has_id: bool, id: Option<&Value>, code: i64, message: &str) -> Option<String> {
    has_id.then(|| error(id, code, message, "null"))
}

fn server_meta(version: &str) -> String {
    format!(
        "{{\"io.modelcontextprotocol/serverInfo\":{{\"name\":\"eqiora-mcp\",\"version\":{}}}}}",
        quoted(version)
    )
}

fn discover(version: &str) -> String {
    format!(
        "{{\"resultType\":\"complete\",\"supportedVersions\":[\"2026-07-28\"],\"capabilities\":{{\"tools\":{{\"listChanged\":false}}}},\"_meta\":{},\"instructions\":\"Compile/check Eqiora source in memory. This server does not read files, edit models, run solvers, or create tasks.\",\"ttlMs\":3600000,\"cacheScope\":\"public\"}}",
        server_meta(version)
    )
}

fn list(version: &str) -> String {
    format!(
        "{{\"resultType\":\"complete\",\"tools\":[{}],\"ttlMs\":3600000,\"cacheScope\":\"public\",\"_meta\":{}}}",
        tool::DEFINITION,
        server_meta(version)
    )
}

fn tool_result(structured: &str, is_error: bool, version: &str) -> String {
    format!(
        "{{\"resultType\":\"complete\",\"content\":[{{\"type\":\"text\",\"text\":{}}}],\"structuredContent\":{},\"isError\":{},\"_meta\":{}}}",
        quoted(structured),
        structured,
        is_error,
        server_meta(version)
    )
}

fn success(id: &Value, result: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{result}}}",
        serde_json::to_string(id).expect("request id JSON encoding")
    )
}

fn error(id: Option<&Value>, code: i64, message: &str, data: &str) -> String {
    let error = format!(
        "{{\"code\":{code},\"message\":{},\"data\":{data}}}",
        quoted(message)
    );
    match id {
        Some(id) => format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{error}}}",
            serde_json::to_string(id).expect("request id JSON encoding")
        ),
        None => format!("{{\"jsonrpc\":\"2.0\",\"error\":{error}}}"),
    }
}

fn quoted(text: &str) -> String {
    serde_json::to_string(text).expect("string JSON encoding")
}

fn write_response(writer: &mut impl Write, response: &str) -> io::Result<()> {
    writer.write_all(response.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}
