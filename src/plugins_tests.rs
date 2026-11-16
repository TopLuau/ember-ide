use super::*;

fn host() -> PluginHost {
    let mut h = PluginHost::new();
    h.reload(&[PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins")], &[]);
    h
}

fn set_text(actions: Vec<PluginAction>) -> String {
    actions
        .into_iter()
        .find_map(|a| if let PluginAction::SetText(s) = a { Some(s) } else { None })
        .expect("plugin should call set_editor_text")
}

#[test]
fn loads_all_bundled_plugins_without_errors() {
    let h = host();
    assert_eq!(h.plugins.len(), 4);
    for p in &h.plugins {
        assert!(p.error.is_none(), "{} failed: {:?}", p.id, p.error);
        assert!(!p.commands.is_empty(), "{} registered no commands", p.id);
    }
}

#[test]
fn line_tools_sort_dedupe_reverse() {
    let mut h = host();
    let out = set_text(h.run_command("line-tools", "line-tools.sort", "b\na\nc", "", ""));
    assert_eq!(out, "a\nb\nc");
    let out = set_text(h.run_command("line-tools", "line-tools.dedupe", "a\nb\na\nb\nc", "", ""));
    assert_eq!(out, "a\nb\nc");
    let out = set_text(h.run_command("line-tools", "line-tools.reverse", "1\n2\n3", "", ""));
    assert_eq!(out, "3\n2\n1");
    let out = set_text(h.run_command("line-tools", "line-tools.upper", "abc", "", ""));
    assert_eq!(out, "ABC");
}

#[test]
fn trim_whitespace_and_word_count() {
    let mut h = host();
    let out = set_text(h.run_command("trim-whitespace", "trim-whitespace.run", "a  \nb\t\nc", "", ""));
    assert_eq!(out, "a\nb\nc");
    let acts = h.run_command("word-count", "word-count.count", "one two three\nfour", "f.txt", "");
    let msg = acts.iter().find_map(|a| if let PluginAction::Notify(s) = a { Some(s.clone()) } else { None }).unwrap();
    assert!(msg.starts_with("4 words, 2 lines"), "{msg}");
}

#[test]
fn on_open_hook_fires() {
    let mut h = host();
    let acts = h.fire("on_open", "fn main() {\n  // TODO: x\n  // FIXME y\n}", "main.rs", "");
    println!("log: {:?}\nactions: {:?}\nerrors: {:?}", h.log, acts, h.plugins.iter().map(|p| (&p.id, &p.error)).collect::<Vec<_>>());
    assert!(acts.iter().any(|a| matches!(a, PluginAction::Notify(s) if s.starts_with("2 TODO"))));
}

#[test]
fn runaway_script_is_stopped() {
    let dir = std::env::temp_dir().join("ember_plugin_test");
    let p = dir.join("loop");
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(p.join("plugin.toml"), "name = \"loop\"").unwrap();
    std::fs::write(p.join("main.rhai"), "fn init() { register_command(\"loop.run\", \"Loop\"); }\nfn on_command(id) { while true { } }").unwrap();
    let mut h = PluginHost::new();
    h.reload(&[dir.clone()], &[]);
    let acts = h.run_command("loop", "loop.run", "", "", "");
    assert!(acts.iter().any(|a| matches!(a, PluginAction::Notify(s) if s.contains("failed"))));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn disabled_plugins_are_not_loaded() {
    let mut h = PluginHost::new();
    h.reload(&[PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins")], &["line-tools".to_string()]);
    let p = h.plugins.iter().find(|p| p.id == "line-tools").unwrap();
    assert!(!p.enabled && p.commands.is_empty());
}
