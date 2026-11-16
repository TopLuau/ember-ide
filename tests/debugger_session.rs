//! End-to-end test of the debugger client against the built-in scripted adapter.

use ember::config::Config;
use ember::dap::{DbgEvent, DbgState, Debugger};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn pump(d: &mut Debugger, events: &mut Vec<DbgEvent>, what: &str, cond: impl Fn(&Debugger) -> bool) {
    let t = Instant::now();
    while !cond(d) {
        events.extend(d.poll());
        assert!(t.elapsed() < Duration::from_secs(15), "timed out waiting for: {what}\nconsole: {:#?}", d.console);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn full_debug_session_with_breakpoint_stack_variables_stepping_and_exit() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut cfg = Config::default();
    {
        let a = cfg.debug_adapters.get_mut("rust").unwrap();
        a.command = env!("CARGO_BIN_EXE_ember").to_string();
        a.args = vec!["--mock-dap".into()];
    }
    let mut d = Debugger::default();
    d.program = "demo".into();
    let file = root.join("src").join("editor.rs");
    d.start(&cfg, &root, Some(&file), vec![(file.clone(), vec![30])]).expect("start");
    assert_eq!(d.state, DbgState::Starting);

    let mut events = Vec::new();
    pump(&mut d, &mut events, "stopped at breakpoint with variables", |d| d.state == DbgState::Stopped && d.vars.contains_key(&10));

    // location & call stack
    assert!(events.iter().any(|e| matches!(e, DbgEvent::Started)));
    let (p, line) = d.current_location().expect("location");
    assert!(p.ends_with("editor.rs"), "{p:?}");
    assert_eq!(line, 31);
    assert_eq!(d.frames.len(), 2);
    assert_eq!(d.frames[0].name, "line_start");

    // variables, with lazy expansion of a nested one
    let locals = &d.vars[&10];
    assert_eq!(locals[0].name, "byte");
    assert_eq!(locals[0].value, "42");
    let items_ref = locals[2].var_ref;
    assert!(items_ref > 0);
    d.expand(items_ref);
    pump(&mut d, &mut events, "children of items", |d| d.vars.contains_key(&items_ref));
    assert_eq!(d.vars[&items_ref].len(), 3);

    // evaluate in the debug console
    d.evaluate("byte");
    pump(&mut d, &mut events, "evaluate result", |d| d.console.iter().any(|l| l == "42"));

    // step over -> stopped again one line further
    d.step_over();
    assert_eq!(d.state, DbgState::Running);
    pump(&mut d, &mut events, "stopped after step", |d| d.state == DbgState::Stopped && d.frames.first().map(|f| f.line) == Some(32));

    // continue -> program output, then the session ends cleanly
    d.cont();
    pump(&mut d, &mut events, "session end", |d| !d.active());
    assert!(d.console.iter().any(|l| l.contains("program output")));
    assert!(events.iter().any(|e| matches!(e, DbgEvent::Ended(_))));
    assert_eq!(d.state, DbgState::Idle);
}

#[test]
fn stop_kills_the_session() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut cfg = Config::default();
    {
        let a = cfg.debug_adapters.get_mut("rust").unwrap();
        a.command = env!("CARGO_BIN_EXE_ember").to_string();
        a.args = vec!["--mock-dap".into()];
    }
    let mut d = Debugger::default();
    d.program = "demo".into();
    d.start(&cfg, &root, Some(Path::new("x.rs")), vec![]).unwrap();
    let mut events = Vec::new();
    pump(&mut d, &mut events, "paused", |d| d.state == DbgState::Stopped);
    d.stop();
    assert!(!d.active());
}

#[test]
fn missing_program_is_reported_before_spawning() {
    let mut d = Debugger::default();
    let r = d.start(&Config::default(), Path::new("."), Some(Path::new("x.rs")), vec![]);
    assert!(r.unwrap_err().to_string().contains("program"));
}
