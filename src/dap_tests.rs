use super::*;
use std::io::Cursor;

#[test]
fn reads_framed_messages() {
    let body1 = r#"{"seq":1,"type":"event","event":"initialized"}"#;
    let body2 = r#"{"seq":2,"type":"response","request_seq":1,"success":true}"#;
    let data = format!("Content-Length: {}\r\n\r\n{}Content-Length: {}\r\n\r\n{}", body1.len(), body1, body2.len(), body2);
    let mut c = Cursor::new(data.into_bytes());
    let a = read_message(&mut c).unwrap();
    assert_eq!(a["event"], "initialized");
    let b = read_message(&mut c).unwrap();
    assert_eq!(b["request_seq"], 1);
    assert!(read_message(&mut c).is_none());
}

#[test]
fn adapter_selection() {
    assert_eq!(adapter_key("rs"), Some("rust"));
    assert_eq!(adapter_key("py"), Some("python"));
    assert_eq!(adapter_key("txt"), None);
}

#[test]
fn stack_trace_response_populates_frames() {
    let mut d = Debugger::default();
    d.state = DbgState::Stopped;
    d.pending.insert(7, Pending::StackTrace);
    d.on_response(&json!({"request_seq": 7, "success": true, "command": "stackTrace",
        "body": {"stackFrames": [{"id": 1, "name": "main", "line": 12, "source": {"path": "Cargo.toml"}}]}}));
    assert_eq!(d.frames.len(), 1);
    assert_eq!(d.frames[0].line, 12);
}

#[test]
fn variables_response_is_stored() {
    let mut d = Debugger::default();
    d.pending.insert(3, Pending::Variables(55));
    d.on_response(&json!({"request_seq": 3, "success": true,
        "body": {"variables": [{"name": "x", "value": "5", "type": "i32", "variablesReference": 0}]}}));
    assert_eq!(d.vars[&55][0].name, "x");
    assert_eq!(d.vars[&55][0].value, "5");
}

#[test]
fn start_fails_cleanly_without_adapter_binary() {
    let mut cfg = Config::default();
    cfg.debug_adapters.get_mut("python").unwrap().command = "definitely-not-a-real-binary-xyz".into();
    let mut d = Debugger::default();
    let r = d.start(&cfg, Path::new("."), Some(Path::new("x.py")), vec![]);
    assert!(r.is_err());
    assert!(!d.active());
}
