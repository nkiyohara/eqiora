use std::{
    io::Write,
    process::{Command, Stdio},
};

use serde_json::{Value, json};

const SERVER: &str = env!("CARGO_BIN_EXE_eqiora-language-server");

#[test]
fn version_command_reports_the_release() {
    let output = Command::new(SERVER)
        .arg("--version")
        .output()
        .expect("run language-server version command");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 version output"),
        format!("eqiora-language-server {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn stdio_session_syncs_diagnostics_and_serves_editor_requests() {
    let source = "dimension Scalar = 1;\npublic component Part{\n  public parameter gain: Scalar;\n  relation law continuous { gain = 0; }\n}\nmodel Demo{\n  parameter input: Scalar = 1;\n  field state: Scalar = 0;\n  instance part: Part(gain = input);\n  relation balance continuous { state = 0; }\n}\n";
    let uri = "file:///workspace/main.eqi";
    let mut child = Command::new(SERVER)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn language server");

    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"eqiora","version":-3,"text":source}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":uri}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"textDocument/foldingRange","params":{"textDocument":{"uri":uri}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":{"line":8,"character":18}}}),
        json!({"jsonrpc":"2.0","id":5,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":{"line":8,"character":18}}}),
        json!({"jsonrpc":"2.0","id":6,"method":"textDocument/formatting","params":{"textDocument":{"uri":uri},"options":{"tabSize":2,"insertSpaces":true}}}),
        json!({"jsonrpc":"2.0","id":8,"method":"textDocument/formatting","params":{"textDocument":42}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":42}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":-1},"contentChanges":[{"text":"model Broken { nonsense; }\n"}]}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":-2},"contentChanges":[{"text":source}]}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":0},"contentChanges":[{"text":source}]}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}),
        json!({"jsonrpc":"2.0","id":7,"method":"shutdown","params":null}),
        json!({"jsonrpc":"2.0","method":"exit","params":null}),
    ];
    let mut stdin = child.stdin.take().expect("language server stdin");
    for message in messages {
        write_packet(&mut stdin, &message);
    }
    drop(stdin);

    let output = child.wait_with_output().expect("language server output");
    assert!(
        output.status.success(),
        "language server failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lifecycle = String::from_utf8(output.stderr)
        .expect("UTF-8 lifecycle log")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("structured lifecycle log"))
        .collect::<Vec<_>>();
    assert_eq!(lifecycle[0]["event"], "server_started");
    assert_eq!(lifecycle[1]["event"], "server_stopped");
    let messages = parse_packets(&output.stdout);

    let initialize = response(&messages, 1);
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "eqiora-language-server"
    );
    assert_eq!(
        initialize["result"]["capabilities"]["positionEncoding"],
        "utf-16"
    );

    let symbols = response(&messages, 2)["result"]
        .as_array()
        .expect("nested document symbols");
    assert_eq!(symbols[0]["name"], "Scalar");
    assert_eq!(symbols[1]["name"], "Part");
    assert_eq!(symbols[2]["name"], "Demo");
    assert_eq!(symbols[2]["children"][2]["name"], "part");

    let folds = response(&messages, 3)["result"]
        .as_array()
        .expect("folding ranges");
    assert_eq!(folds.len(), 2);
    assert_eq!(folds[0]["collapsedText"], "Part");

    let hover = response(&messages, 4);
    assert!(
        hover["result"]["contents"]["value"]
            .as_str()
            .expect("Markdown hover")
            .contains("**Component** `Part`")
    );

    let definition = response(&messages, 5);
    assert_eq!(definition["result"]["uri"], uri);
    assert_eq!(definition["result"]["range"]["start"]["line"], 1);

    let edits = response(&messages, 6)["result"]
        .as_array()
        .expect("format edits");
    assert_eq!(edits.len(), 1);
    assert!(
        edits[0]["newText"]
            .as_str()
            .expect("formatted source")
            .contains("public component Part {")
    );

    let diagnostics = messages
        .iter()
        .filter(|message| message["method"] == "textDocument/publishDiagnostics")
        .map(|message| &message["params"])
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 4);
    assert_eq!(diagnostics[0]["version"], -3);
    assert_eq!(diagnostics[0]["diagnostics"], json!([]));
    assert_eq!(diagnostics[1]["version"], -1);
    assert!(
        !diagnostics[1]["diagnostics"]
            .as_array()
            .expect("malformed diagnostics")
            .is_empty()
    );
    assert_eq!(diagnostics[2]["version"], 0);
    assert_eq!(diagnostics[2]["diagnostics"], json!([]));
    assert!(diagnostics[3]["version"].is_null());
    assert_eq!(diagnostics[3]["diagnostics"], json!([]));
    assert!(
        messages
            .iter()
            .all(|message| message["params"]["version"] != -2)
    );
    assert!(response(&messages, 7)["result"].is_null());
    assert_eq!(response(&messages, 8)["error"]["code"], -32602);
}

fn write_packet(writer: &mut impl Write, message: &Value) {
    let body = serde_json::to_vec(message).expect("serialize LSP message");
    write!(writer, "Content-Length: {}\r\n\r\n", body.len()).expect("write LSP header");
    writer.write_all(&body).expect("write LSP body");
    writer.flush().expect("flush LSP message");
}

fn parse_packets(mut bytes: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();
    while !bytes.is_empty() {
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("complete LSP header");
        let header = std::str::from_utf8(&bytes[..header_end]).expect("UTF-8 LSP header");
        let length = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .expect("Content-Length header")
            .parse::<usize>()
            .expect("decimal Content-Length");
        let body_start = header_end + 4;
        let body_end = body_start + length;
        messages
            .push(serde_json::from_slice(&bytes[body_start..body_end]).expect("JSON-RPC response"));
        bytes = &bytes[body_end..];
    }
    messages
}

fn response(messages: &[Value], id: i64) -> &Value {
    messages
        .iter()
        .find(|message| message["id"].as_i64() == Some(id))
        .unwrap_or_else(|| panic!("missing response {id}: {messages:#?}"))
}
