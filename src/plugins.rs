//! Plugin system. Plugins are folders containing `plugin.toml` and a Rhai script
//! (`main.rhai`). Scripts run in a sandboxed engine with an operation budget and can
//! only talk to the editor through the host API registered below.
//!
//! Hooks a script may define: `init()`, `on_command(id)`, `on_open(path)`,
//! `on_save(path)` and `on_before_save(path)`.

use crate::util::now_iso;
use rhai::{Dynamic, Engine, EvalAltResult, FuncArgs, Scope, AST};
use serde::Deserialize;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum PluginAction {
    Notify(String),
    SetText(String),
    Log(String),
    OpenFile(String),
}

#[derive(Default)]
struct Ctx {
    text: String,
    path: String,
    root: String,
    current_plugin: String,
    registered: Vec<(String, String, String)>, // (plugin id, command id, title)
    actions: Vec<PluginAction>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Manifest {
    name: String,
    version: String,
    description: String,
    author: String,
    main: String,
}

pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub dir: PathBuf,
    pub enabled: bool,
    pub error: Option<String>,
    pub commands: Vec<(String, String)>, // (command id, title)
    ast: Option<AST>,
}

pub struct PluginHost {
    engine: Engine,
    ctx: Rc<RefCell<Ctx>>,
    pub plugins: Vec<Plugin>,
    pub log: Vec<String>,
}

impl PluginHost {
    pub fn new() -> Self {
        let ctx = Rc::new(RefCell::new(Ctx::default()));
        let mut engine = Engine::new();
        engine.set_max_operations(3_000_000);
        engine.set_max_call_levels(48);
        engine.set_max_expr_depths(128, 64);
        engine.set_max_string_size(32 * 1024 * 1024);
        engine.set_max_array_size(2_000_000);

        let c = ctx.clone();
        engine.register_fn("notify", move |s: String| c.borrow_mut().actions.push(PluginAction::Notify(s)));
        let c = ctx.clone();
        engine.register_fn("log", move |s: String| c.borrow_mut().actions.push(PluginAction::Log(s)));
        let c = ctx.clone();
        engine.register_fn("editor_text", move || c.borrow().text.clone());
        let c = ctx.clone();
        engine.register_fn("set_editor_text", move |s: String| c.borrow_mut().actions.push(PluginAction::SetText(s)));
        let c = ctx.clone();
        engine.register_fn("current_file", move || c.borrow().path.clone());
        let c = ctx.clone();
        engine.register_fn("project_root", move || c.borrow().root.clone());
        let c = ctx.clone();
        engine.register_fn("open_file", move |s: String| c.borrow_mut().actions.push(PluginAction::OpenFile(s)));
        let c = ctx.clone();
        engine.register_fn("register_command", move |id: String, title: String| {
            let mut b = c.borrow_mut();
            let pid = b.current_plugin.clone();
            b.registered.push((pid, id, title));
        });
        engine.register_fn("now", now_iso);
        engine.register_fn("rtrim", |s: String| s.trim_end().to_string());
        engine.register_fn("join", |a: rhai::Array, sep: String| a.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(&sep));

        Self { engine, ctx, plugins: vec![], log: vec![] }
    }

    /// Reads every plugin found in `dirs`. The first plugin with a given id wins.
    pub fn reload(&mut self, dirs: &[PathBuf], disabled: &[String]) {
        self.plugins.clear();
        self.ctx.borrow_mut().registered.clear();
        for dir in dirs {
            let Ok(rd) = std::fs::read_dir(dir) else { continue };
            let mut entries: Vec<_> = rd.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let p = e.path();
                if !p.is_dir() || !p.join("plugin.toml").exists() {
                    continue;
                }
                let id = e.file_name().to_string_lossy().to_string();
                if self.plugins.iter().any(|x| x.id == id) {
                    continue;
                }
                let enabled = !disabled.contains(&id);
                self.load_one(id, &p, enabled);
            }
        }
    }

    fn load_one(&mut self, id: String, dir: &Path, enabled: bool) {
        let manifest: Manifest = std::fs::read_to_string(dir.join("plugin.toml"))
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        let mut plugin = Plugin {
            name: if manifest.name.is_empty() { id.clone() } else { manifest.name.clone() },
            version: manifest.version.clone(),
            description: manifest.description.clone(),
            author: manifest.author.clone(),
            id: id.clone(),
            dir: dir.to_path_buf(),
            enabled,
            error: None,
            commands: vec![],
            ast: None,
        };
        if enabled {
            let main = if manifest.main.is_empty() { "main.rhai".to_string() } else { manifest.main.clone() };
            match std::fs::read_to_string(dir.join(&main)) {
                Err(e) => plugin.error = Some(format!("cannot read {main}: {e}")),
                Ok(src) => match self.engine.compile(&src) {
                    Err(e) => plugin.error = Some(e.to_string()),
                    Ok(ast) => plugin.ast = Some(ast),
                },
            }
        }
        self.plugins.push(plugin);
        let idx = self.plugins.len() - 1;
        if self.plugins[idx].ast.is_some() {
            {
                let mut c = self.ctx.borrow_mut();
                c.current_plugin = id.clone();
                c.registered.retain(|r| r.0 != id);
            }
            if let Err(e) = self.call(idx, "init", ()) {
                self.plugins[idx].error = Some(e);
            }
            let cmds: Vec<(String, String)> =
                self.ctx.borrow().registered.iter().filter(|r| r.0 == id).map(|r| (r.1.clone(), r.2.clone())).collect();
            self.plugins[idx].commands = cmds;
            self.drain_log();
        }
    }

    fn drain_log(&mut self) {
        let actions: Vec<PluginAction> = std::mem::take(&mut self.ctx.borrow_mut().actions);
        for a in actions {
            if let PluginAction::Log(l) = a {
                self.log.push(l);
            }
        }
    }

    fn call(&mut self, idx: usize, func: &str, args: impl FuncArgs) -> Result<(), String> {
        let Some(ast) = &self.plugins[idx].ast else { return Ok(()) };
        let mut scope = Scope::new();
        match self.engine.call_fn::<Dynamic>(&mut scope, ast, func, args) {
            Ok(_) => Ok(()),
            Err(e) => match e.as_ref() {
                EvalAltResult::ErrorFunctionNotFound(sig, _) if sig.starts_with(func) => Ok(()),
                other => Err(other.to_string()),
            },
        }
    }

    fn set_ctx(&self, text: &str, path: &str, root: &str, plugin_id: &str) {
        let mut c = self.ctx.borrow_mut();
        c.text = text.to_string();
        c.path = path.to_string();
        c.root = root.to_string();
        c.current_plugin = plugin_id.to_string();
        c.actions.clear();
    }

    fn collect(&mut self, plugin: &str) -> Vec<PluginAction> {
        let actions: Vec<PluginAction> = std::mem::take(&mut self.ctx.borrow_mut().actions);
        let mut out = Vec::new();
        for a in actions {
            match a {
                PluginAction::Log(l) => self.log.push(format!("[{plugin}] {l}")),
                other => out.push(other),
            }
        }
        out
    }

    pub fn run_command(&mut self, plugin_id: &str, cmd_id: &str, text: &str, path: &str, root: &str) -> Vec<PluginAction> {
        let Some(idx) = self.plugins.iter().position(|p| p.id == plugin_id && p.enabled) else { return vec![] };
        self.set_ctx(text, path, root, plugin_id);
        if let Err(e) = self.call(idx, "on_command", (cmd_id.to_string(),)) {
            self.log.push(format!("[{plugin_id}] error: {e}"));
            return vec![PluginAction::Notify(format!("Plugin {plugin_id} failed: {e}"))];
        }
        self.collect(plugin_id)
    }

    /// Fire an event (`on_open`, `on_save`, `on_before_save`) on every enabled plugin.
    pub fn fire(&mut self, event: &str, text: &str, path: &str, root: &str) -> Vec<PluginAction> {
        let mut all = Vec::new();
        for idx in 0..self.plugins.len() {
            if !self.plugins[idx].enabled || self.plugins[idx].ast.is_none() {
                continue;
            }
            let id = self.plugins[idx].id.clone();
            self.set_ctx(text, path, root, &id);
            if let Err(e) = self.call(idx, event, (path.to_string(),)) {
                self.log.push(format!("[{id}] error in {event}: {e}"));
                continue;
            }
            all.extend(self.collect(&id));
        }
        all
    }
}

#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
