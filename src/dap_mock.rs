//! A tiny scripted Debug Adapter (`ember --mock-dap`). It pretends to debug a program that
//! stops at `src/editor.rs:31`, exposes a few variables, and exits when you continue.
//! It exists so the debugger client can be tested end-to-end without LLVM or Python.

use crate::dap::read_message;
use serde_json::{json, Value};
use std::io::{BufReader, Write};

fn send(v: Value) {
    let s = v.to_string();
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{}", s.len(), s);
    let _ = out.flush();
}

pub fn run() {
    let mut r = BufReader::new(std::io::stdin());
    let mut seq = 0;
    let mut cwd = std::env::current_dir().unwrap_or_default();
    let mut line: i64 = std::env::var("EMBER_MOCK_LINE").ok().and_then(|v| v.parse().ok()).unwrap_or(31);
    let file = std::env::var("EMBER_MOCK_FILE").unwrap_or_else(|_| "src/editor.rs".into());
    let mut next = move || {
        seq += 1;
        seq
    };
    while let Some(m) = read_message(&mut r) {
        if m["type"] != "request" {
            continue;
        }
        let cmd = m["command"].as_str().unwrap_or("").to_string();
        let rs = m["seq"].as_i64().unwrap_or(0);
        let ok = |body: Value, seq: i64| json!({"seq": seq, "type": "response", "request_seq": rs, "success": true, "command": cmd, "body": body});
        let event = |name: &str, body: Value, seq: i64| json!({"seq": seq, "type": "event", "event": name, "body": body});
        let src = cwd.join(&file).display().to_string();
        match cmd.as_str() {
            "initialize" => send(ok(json!({"supportsConfigurationDoneRequest": true}), next())),
            "launch" => {
                if let Some(c) = m["arguments"]["cwd"].as_str() {
                    cwd = c.into();
                }
                send(ok(json!({}), next()));
                send(event("initialized", json!({}), next()));
            }
            "setBreakpoints" => {
                let n = m["arguments"]["breakpoints"].as_array().map(|a| a.len()).unwrap_or(0);
                let bps: Vec<Value> = (0..n).map(|_| json!({"verified": true})).collect();
                send(ok(json!({"breakpoints": bps}), next()));
            }
            "configurationDone" => {
                send(ok(json!({}), next()));
                send(event("stopped", json!({"reason": "breakpoint", "threadId": 1, "allThreadsStopped": true}), next()));
            }
            "threads" => send(ok(json!({"threads": [{"id": 1, "name": "main"}]}), next())),
            "stackTrace" => send(ok(
                json!({"stackFrames": [
                    {"id": 1, "name": "line_start", "line": line, "column": 1, "source": {"path": src}},
                    {"id": 2, "name": "main", "line": 12, "column": 1, "source": {"path": src}}
                ], "totalFrames": 2}),
                next(),
            )),
            "scopes" => send(ok(json!({"scopes": [{"name": "Locals", "variablesReference": 10}, {"name": "Registers", "variablesReference": 11, "expensive": true}]}), next())),
            "variables" => {
                let r = m["arguments"]["variablesReference"].as_i64().unwrap_or(0);
                let vars = match r {
                    10 => json!([
                        {"name": "byte", "value": "42", "type": "usize", "variablesReference": 0},
                        {"name": "text", "value": "\"fn main() {}\"", "type": "&str", "variablesReference": 0},
                        {"name": "items", "value": "Vec<u32> (len 3)", "type": "Vec<u32>", "variablesReference": 12}
                    ]),
                    12 => json!([
                        {"name": "[0]", "value": "1", "type": "u32", "variablesReference": 0},
                        {"name": "[1]", "value": "2", "type": "u32", "variablesReference": 0},
                        {"name": "[2]", "value": "3", "type": "u32", "variablesReference": 0}
                    ]),
                    _ => json!([]),
                };
                send(ok(json!({"variables": vars}), next()));
            }
            "evaluate" => {
                let e = m["arguments"]["expression"].as_str().unwrap_or("");
                send(ok(json!({"result": if e == "byte" { "42".to_string() } else { format!("<{e}>") }, "variablesReference": 0}), next()));
            }
            "next" | "stepIn" | "stepOut" => {
                line += 1;
                send(ok(json!({}), next()));
                send(event("stopped", json!({"reason": "step", "threadId": 1}), next()));
            }
            "continue" => {
                send(ok(json!({"allThreadsContinued": true}), next()));
                send(event("output", json!({"category": "stdout", "output": "program output\n"}), next()));
                send(event("exited", json!({"exitCode": 0}), next()));
                send(event("terminated", json!({}), next()));
            }
            "disconnect" => {
                send(ok(json!({}), next()));
                return;
            }
            _ => send(ok(json!({}), next())),
        }
    }
}
