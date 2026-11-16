//! Debug Adapter Protocol client.
//!
//! Spawns any DAP adapter over stdio (lldb-dap, debugpy, ...), drives the session
//! (initialize -> launch -> breakpoints -> configurationDone), and exposes the stopped
//! state (call stack, scopes, variables) to the UI.

use crate::config::Config;
use crate::util::no_window;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

enum Msg {
    Json(Value),
    Stderr(String),
    Closed,
}

struct DapClient {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Msg>,
    seq: i64,
}

pub(crate) fn read_message<R: BufRead + Read>(r: &mut R) -> Option<Value> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            len = v.trim().parse().ok();
        }
    }
    let mut buf = vec![0u8; len?];
    r.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

impl DapClient {
    fn spawn(cmd: &str, args: &[String], cwd: &Path) -> Result<Self> {
        let mut c = Command::new(cmd);
        c.args(args).current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        no_window(&mut c);
        let mut child = c.spawn().map_err(|e| anyhow!("cannot start debug adapter `{cmd}`: {e}"))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
        let stderr = child.stderr.take();
        let (tx, rx) = channel();
        let tx2 = tx.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            while let Some(v) = read_message(&mut r) {
                if tx.send(Msg::Json(v)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Msg::Closed);
        });
        if let Some(e) = stderr {
            std::thread::spawn(move || {
                for l in BufReader::new(e).lines().map_while(std::result::Result::ok) {
                    let _ = tx2.send(Msg::Stderr(l));
                }
            });
        }
        Ok(Self { child, stdin, rx, seq: 0 })
    }

    fn write(&mut self, v: &Value) {
        let s = v.to_string();
        let _ = write!(self.stdin, "Content-Length: {}\r\n\r\n{}", s.len(), s);
        let _ = self.stdin.flush();
    }

    fn request(&mut self, command: &str, arguments: Value) -> i64 {
        self.seq += 1;
        let seq = self.seq;
        self.write(&json!({"seq": seq, "type": "request", "command": command, "arguments": arguments}));
        seq
    }

    fn respond(&mut self, req_seq: i64, command: &str, success: bool, body: Value) {
        self.seq += 1;
        let seq = self.seq;
        self.write(&json!({"seq": seq, "type": "response", "request_seq": req_seq, "success": success, "command": command, "body": body}));
    }
}

impl Drop for DapClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DbgState {
    Idle,
    Starting,
    Running,
    Stopped,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub id: i64,
    pub name: String,
    pub path: Option<PathBuf>,
    pub line: usize, // 1-based
}

#[derive(Clone, Debug)]
pub struct Var {
    pub name: String,
    pub value: String,
    pub ty: String,
    pub var_ref: i64,
}

#[derive(Clone, Debug)]
pub struct Scope {
    pub name: String,
    pub var_ref: i64,
}

#[derive(Debug, Clone)]
pub enum DbgEvent {
    /// Execution stopped at `path:line` (1-based).
    Stopped { path: PathBuf, line: usize },
    Started,
    Ended(String),
}

#[derive(Debug)]
enum Pending {
    Initialize,
    Launch,
    StackTrace,
    Scopes,
    Variables(i64),
    Evaluate(String),
    Threads,
    Other,
}

pub struct Debugger {
    pub state: DbgState,
    client: Option<DapClient>,
    pending: HashMap<i64, Pending>,
    pub console: Vec<String>,
    pub frames: Vec<Frame>,
    pub selected_frame: usize,
    pub scopes: Vec<Scope>,
    pub vars: HashMap<i64, Vec<Var>>,
    pub open_vars: HashSet<i64>,
    pub thread_id: Option<i64>,
    pub program: String,
    pub args: String,
    pub eval_input: String,
    breakpoints: Vec<(PathBuf, Vec<usize>)>,
    launch_args: Value,
    adapter_kind: String,
    events: Vec<DbgEvent>,
}

impl Default for Debugger {
    fn default() -> Self {
        Self {
            state: DbgState::Idle,
            client: None,
            pending: HashMap::new(),
            console: vec![],
            frames: vec![],
            selected_frame: 0,
            scopes: vec![],
            vars: HashMap::new(),
            open_vars: HashSet::new(),
            thread_id: None,
            program: String::new(),
            args: String::new(),
            eval_input: String::new(),
            breakpoints: vec![],
            launch_args: Value::Null,
            adapter_kind: String::new(),
            events: vec![],
        }
    }
}

/// Which adapter entry in the config should be used for a file extension.
pub fn adapter_key(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("cpp"),
        "py" => Some("python"),
        _ => None,
    }
}

/// Guess the executable a Rust project builds (target/debug/<package>).
pub fn guess_rust_program(root: &Path) -> Option<String> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let v: toml::Value = toml::from_str(&manifest).ok()?;
    let name = v.get("package")?.get("name")?.as_str()?.to_string();
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name };
    Some(root.join("target").join("debug").join(exe).display().to_string())
}

impl Debugger {
    pub fn active(&self) -> bool {
        self.state != DbgState::Idle
    }

    fn log(&mut self, s: impl Into<String>) {
        self.console.push(s.into());
        if self.console.len() > 5000 {
            self.console.drain(0..1000);
        }
    }

    pub fn start(&mut self, cfg: &Config, root: &Path, file: Option<&Path>, breakpoints: Vec<(PathBuf, Vec<usize>)>) -> Result<()> {
        if self.active() {
            bail!("a debug session is already running");
        }
        let ext = file.map(crate::util::ext_of).unwrap_or_default();
        let key = adapter_key(&ext).ok_or_else(|| anyhow!("no debug adapter for .{ext} files (see Settings → Debugger)"))?;
        let adapter = cfg.debug_adapters.get(key).ok_or_else(|| anyhow!("no adapter `{key}` configured"))?.clone();
        if adapter.command.trim().is_empty() {
            bail!("adapter `{key}` has no command configured");
        }
        let args: Vec<String> = self.args.split_whitespace().map(|s| s.to_string()).collect();
        self.launch_args = match adapter.kind.as_str() {
            "python" => {
                let f = file.ok_or_else(|| anyhow!("open a Python file to debug"))?;
                json!({"program": f.display().to_string(), "args": args, "cwd": root.display().to_string(),
                       "console": "internalConsole", "justMyCode": false, "stopOnEntry": false})
            }
            _ => {
                if self.program.trim().is_empty() {
                    bail!("set the program to debug in the Debug panel (e.g. target/debug/app)");
                }
                json!({"program": self.program.clone(), "args": args, "cwd": root.display().to_string(), "stopOnEntry": false})
            }
        };
        let client = DapClient::spawn(&adapter.command, &adapter.args, root)?;
        self.client = Some(client);
        self.adapter_kind = adapter.kind.clone();
        self.breakpoints = breakpoints;
        self.state = DbgState::Starting;
        self.console.clear();
        self.frames.clear();
        self.scopes.clear();
        self.vars.clear();
        self.pending.clear();
        self.log(format!("Starting {} {}", adapter.command, adapter.args.join(" ")));
        let init = json!({"clientID": "ember", "clientName": "Ember IDE", "adapterID": adapter.kind,
            "linesStartAt1": true, "columnsStartAt1": true, "pathFormat": "path",
            "supportsVariableType": true, "supportsRunInTerminalRequest": true});
        self.send("initialize", init, Pending::Initialize);
        Ok(())
    }

    fn send(&mut self, cmd: &str, args: Value, p: Pending) {
        if let Some(c) = &mut self.client {
            let seq = c.request(cmd, args);
            self.pending.insert(seq, p);
        }
    }

    fn thread(&self) -> i64 {
        self.thread_id.unwrap_or(1)
    }

    pub fn cont(&mut self) {
        let t = self.thread();
        self.send("continue", json!({"threadId": t}), Pending::Other);
        self.mark_running();
    }
    pub fn step_over(&mut self) {
        let t = self.thread();
        self.send("next", json!({"threadId": t}), Pending::Other);
        self.mark_running();
    }
    pub fn step_in(&mut self) {
        let t = self.thread();
        self.send("stepIn", json!({"threadId": t}), Pending::Other);
        self.mark_running();
    }
    pub fn step_out(&mut self) {
        let t = self.thread();
        self.send("stepOut", json!({"threadId": t}), Pending::Other);
        self.mark_running();
    }
    pub fn pause(&mut self) {
        let t = self.thread();
        self.send("pause", json!({"threadId": t}), Pending::Other);
    }

    fn mark_running(&mut self) {
        self.state = DbgState::Running;
        self.frames.clear();
        self.scopes.clear();
        self.vars.clear();
    }

    pub fn stop(&mut self) {
        if let Some(c) = &mut self.client {
            c.request("disconnect", json!({"terminateDebuggee": true}));
        }
        self.finish("Debug session stopped");
    }

    fn finish(&mut self, why: &str) {
        if self.client.take().is_some() {
            self.log(why.to_string());
        }
        self.state = DbgState::Idle;
        self.frames.clear();
        self.scopes.clear();
        self.vars.clear();
        self.pending.clear();
        self.events.push(DbgEvent::Ended(why.to_string()));
    }

    pub fn set_breakpoints(&mut self, path: &Path, lines: Vec<usize>) {
        self.breakpoints.retain(|(p, _)| p != path);
        self.breakpoints.push((path.to_path_buf(), lines.clone()));
        if self.active() && self.state != DbgState::Starting {
            self.send_breakpoints_for(path, &lines);
        }
    }

    fn send_breakpoints_for(&mut self, path: &Path, lines: &[usize]) {
        let bps: Vec<Value> = lines.iter().map(|l| json!({"line": l + 1})).collect();
        self.send(
            "setBreakpoints",
            json!({"source": {"path": path.display().to_string()}, "breakpoints": bps}),
            Pending::Other,
        );
    }

    pub fn evaluate(&mut self, expr: &str) {
        if self.state != DbgState::Stopped {
            self.log("Evaluate is only available while stopped at a breakpoint.");
            return;
        }
        let frame = self.frames.get(self.selected_frame).map(|f| f.id);
        self.log(format!("> {expr}"));
        let mut a = json!({"expression": expr, "context": "repl"});
        if let Some(f) = frame {
            a["frameId"] = json!(f);
        }
        self.send("evaluate", a, Pending::Evaluate(expr.to_string()));
    }

    pub fn expand(&mut self, var_ref: i64) {
        if var_ref > 0 && !self.vars.contains_key(&var_ref) {
            self.send("variables", json!({"variablesReference": var_ref}), Pending::Variables(var_ref));
        }
    }

    pub fn select_frame(&mut self, idx: usize) {
        if let Some(f) = self.frames.get(idx).cloned() {
            self.selected_frame = idx;
            self.scopes.clear();
            self.vars.clear();
            self.send("scopes", json!({"frameId": f.id}), Pending::Scopes);
            if let Some(path) = f.path {
                self.events.push(DbgEvent::Stopped { path, line: f.line });
            }
        }
    }

    pub fn current_location(&self) -> Option<(PathBuf, usize)> {
        if self.state != DbgState::Stopped {
            return None;
        }
        let f = self.frames.get(self.selected_frame)?;
        Some((f.path.clone()?, f.line))
    }

    pub fn poll(&mut self) -> Vec<DbgEvent> {
        let mut msgs = Vec::new();
        if let Some(c) = &self.client {
            while let Ok(m) = c.rx.try_recv() {
                msgs.push(m);
            }
        }
        for m in msgs {
            match m {
                Msg::Json(v) => self.handle(v),
                Msg::Stderr(s) => self.log(s),
                Msg::Closed => {
                    if self.client.is_some() {
                        self.finish("Debug adapter exited");
                    }
                }
            }
        }
        std::mem::take(&mut self.events)
    }

    fn handle(&mut self, v: Value) {
        match v["type"].as_str().unwrap_or("") {
            "response" => self.on_response(&v),
            "event" => self.on_event(&v),
            "request" => self.on_reverse_request(&v),
            _ => {}
        }
    }

    fn on_reverse_request(&mut self, v: &Value) {
        let seq = v["seq"].as_i64().unwrap_or(0);
        let cmd = v["command"].as_str().unwrap_or("").to_string();
        if cmd == "runInTerminal" {
            let args: Vec<String> = v["arguments"]["args"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let cwd = v["arguments"]["cwd"].as_str().unwrap_or(".").to_string();
            if let Some((prog, rest)) = args.split_first() {
                let mut c = Command::new(prog);
                c.args(rest).current_dir(cwd);
                if let Some(env) = v["arguments"]["env"].as_object() {
                    for (k, val) in env {
                        if let Some(s) = val.as_str() {
                            c.env(k, s);
                        }
                    }
                }
                match c.spawn() {
                    Ok(child) => {
                        let pid = child.id();
                        if let Some(cl) = &mut self.client {
                            cl.respond(seq, &cmd, true, json!({"processId": pid}));
                        }
                        return;
                    }
                    Err(e) => self.log(format!("runInTerminal failed: {e}")),
                }
            }
        }
        if let Some(cl) = &mut self.client {
            cl.respond(seq, &cmd, false, json!({}));
        }
    }

    fn on_response(&mut self, v: &Value) {
        let rid = v["request_seq"].as_i64().unwrap_or(-1);
        let pending = self.pending.remove(&rid).unwrap_or(Pending::Other);
        let ok = v["success"].as_bool().unwrap_or(false);
        let body = &v["body"];
        if !ok {
            let msg = v["message"].as_str().unwrap_or("request failed").to_string();
            match pending {
                Pending::Launch | Pending::Initialize => {
                    self.log(format!("Debug launch failed: {msg}"));
                    self.finish("Launch failed");
                }
                Pending::Evaluate(_) => self.log(format!("error: {msg}")),
                _ => self.log(format!("{}: {msg}", v["command"].as_str().unwrap_or("request"))),
            }
            return;
        }
        match pending {
            Pending::Initialize => {
                let la = self.launch_args.clone();
                self.send("launch", la, Pending::Launch);
            }
            Pending::Threads => {
                if let Some(id) = body["threads"].get(0).and_then(|t| t["id"].as_i64()) {
                    self.thread_id = Some(id);
                }
            }
            Pending::StackTrace => {
                self.frames = body["stackFrames"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|f| Frame {
                                id: f["id"].as_i64().unwrap_or(0),
                                name: f["name"].as_str().unwrap_or("?").to_string(),
                                path: f["source"]["path"].as_str().map(PathBuf::from),
                                line: f["line"].as_u64().unwrap_or(0) as usize,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                // Prefer the first frame that has a source file we can show.
                self.selected_frame = self.frames.iter().position(|f| f.path.as_ref().map(|p| p.exists()).unwrap_or(false)).unwrap_or(0);
                self.select_frame(self.selected_frame);
            }
            Pending::Scopes => {
                self.scopes = body["scopes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|s| Scope { name: s["name"].as_str().unwrap_or("").to_string(), var_ref: s["variablesReference"].as_i64().unwrap_or(0) })
                            .collect()
                    })
                    .unwrap_or_default();
                let refs: Vec<i64> = self.scopes.iter().filter(|s| s.var_ref > 0).take(2).map(|s| s.var_ref).collect();
                for r in refs {
                    self.open_vars.insert(r);
                    self.expand(r);
                }
            }
            Pending::Variables(r) => {
                let list: Vec<Var> = body["variables"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|x| Var {
                                name: x["name"].as_str().unwrap_or("").to_string(),
                                value: x["value"].as_str().unwrap_or("").to_string(),
                                ty: x["type"].as_str().unwrap_or("").to_string(),
                                var_ref: x["variablesReference"].as_i64().unwrap_or(0),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                self.vars.insert(r, list);
            }
            Pending::Evaluate(_) => {
                let r = body["result"].as_str().unwrap_or("").to_string();
                self.log(r);
            }
            Pending::Launch | Pending::Other => {}
        }
    }

    fn on_event(&mut self, v: &Value) {
        let body = &v["body"];
        match v["event"].as_str().unwrap_or("") {
            "initialized" => {
                let bps = self.breakpoints.clone();
                for (p, lines) in bps {
                    self.send_breakpoints_for(&p, &lines);
                }
                self.send("setExceptionBreakpoints", json!({"filters": []}), Pending::Other);
                self.send("configurationDone", json!({}), Pending::Other);
                self.state = DbgState::Running;
                self.events.push(DbgEvent::Started);
            }
            "stopped" => {
                self.state = DbgState::Stopped;
                if let Some(t) = body["threadId"].as_i64() {
                    self.thread_id = Some(t);
                } else {
                    self.send("threads", json!({}), Pending::Threads);
                }
                let reason = body["reason"].as_str().unwrap_or("stopped").to_string();
                self.log(format!("Paused ({reason})"));
                let t = self.thread();
                self.send("stackTrace", json!({"threadId": t, "startFrame": 0, "levels": 40}), Pending::StackTrace);
            }
            "continued" => self.mark_running(),
            "output" => {
                let cat = body["category"].as_str().unwrap_or("console");
                if cat != "telemetry" {
                    let out = body["output"].as_str().unwrap_or("").trim_end().to_string();
                    if !out.is_empty() {
                        self.log(out);
                    }
                }
            }
            "exited" => {
                let code = body["exitCode"].as_i64().unwrap_or(0);
                self.log(format!("Process exited with code {code}"));
            }
            "terminated" => {
                if let Some(c) = &mut self.client {
                    c.request("disconnect", json!({}));
                }
                self.finish("Program terminated");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "dap_tests.rs"]
mod tests;
