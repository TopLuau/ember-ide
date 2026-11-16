//! Application state and behaviour. The widgets live in `panels.rs` and `dialogs.rs`.

use crate::config::{config_dir, Config};
use crate::dap::{guess_rust_program, DbgEvent, DbgState, Debugger};
use crate::doc::Doc;
use crate::explorer::{Explorer, ExplorerEvent};
use crate::highlight::Highlighter;
use crate::plugins::{PluginAction, PluginHost};
use crate::presence::{Presence, PresenceInfo};
use crate::search::ProjectSearch;
use crate::terminal::Terminal;
use crate::theme::{all_themes, Theme};
use crate::util::*;
use eframe::egui::{self, Key, Modifiers};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SideTab {
    Explorer,
    Search,
    Debug,
    Plugins,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Bottom {
    Terminal,
    DebugConsole,
    Output,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    Commands,
    Files,
    Goto,
}

#[derive(Clone, PartialEq)]
pub enum Confirm {
    CloseTab(usize),
    Quit,
}

#[derive(Clone, PartialEq)]
pub enum Cmd {
    NewFile,
    OpenFile,
    OpenFolder,
    OpenRecent(PathBuf),
    OpenPath(PathBuf),
    Save,
    SaveAs,
    SaveAll,
    CloseTab,
    CloseFolder,
    Quit,
    ToggleSidebar,
    TogglePanel,
    ShowSide(SideTab),
    ShowBottom(Bottom),
    Find,
    Replace,
    FindNext,
    FindPrev,
    ProjectSearch,
    QuickOpen,
    Palette,
    GotoLine,
    Settings,
    About,
    SetTheme(usize),
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleWrap,
    DebugStart,
    DebugStop,
    DebugContinue,
    DebugOver,
    DebugIn,
    DebugOut,
    DebugPause,
    ToggleBreakpoint,
    RunFile,
    Plugin(String, String),
    ReloadPlugins,
    OpenPluginsDir,
    OpenThemesDir,
    OpenConfigDir,
    NextTab,
    PrevTab,
    ClearTerminal,
}

pub struct Palette {
    pub mode: PaletteMode,
    pub query: String,
    pub sel: usize,
    pub first: bool,
}

pub struct Toast {
    pub text: String,
    pub at: Instant,
}

#[derive(Default)]
pub struct FindState {
    pub open: bool,
    pub replace_mode: bool,
    pub query: String,
    pub replace: String,
    pub case: bool,
    pub regex: bool,
    pub matches: Vec<(usize, usize)>,
    pub current: usize,
    pub key: Option<(u64, u64, u64, bool, bool)>,
    pub focus: bool,
    pub error: Option<String>,
}

pub struct App {
    pub cfg: Config,
    pub themes: Vec<Theme>,
    pub theme_idx: usize,
    pub docs: Vec<Doc>,
    pub active: usize,
    pub next_id: u64,
    pub untitled_counter: usize,
    pub explorer: Explorer,
    pub hl: Highlighter,
    pub side_tab: SideTab,
    pub show_side: bool,
    pub show_bottom: bool,
    pub bottom: Bottom,
    pub palette: Option<Palette>,
    pub find: FindState,
    pub search: ProjectSearch,
    pub terminal: Terminal,
    pub plugins: PluginHost,
    pub dbg: Debugger,
    pub presence: Presence,
    pub toasts: Vec<Toast>,
    pub show_settings: bool,
    pub show_about: bool,
    pub confirm: Option<Confirm>,
    pub allow_close: bool,
    pub git_branch: Option<String>,
    pub git_checked: Instant,
    pub project_files: Option<Vec<PathBuf>>,
    pub last_title: String,
    pub logo: Option<egui::TextureHandle>,
    pub adapter_args_buf: std::collections::HashMap<String, String>,
    pub focus_terminal: bool,
    pub focus_search: bool,
    pub last_active: usize,
    pub terminal_focus: bool,
    pub rec: Option<crate::record::Recorder>,
    pub last_input: std::time::Instant,
    pub file_since: (u64, i64),
    pub shot: Option<(PathBuf, u32)>,
}

fn plugin_dirs() -> Vec<PathBuf> {
    let user = config_dir().join("plugins");
    let _ = std::fs::create_dir_all(&user);
    let mut v = vec![user];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            v.push(d.join("plugins"));
            v.push(d.join("../Resources/plugins"));
            v.push(d.join("../share/ember/plugins"));
        }
    }
    if let Some(m) = option_env!("CARGO_MANIFEST_DIR") {
        v.push(PathBuf::from(m).join("plugins"));
    }
    v.push(PathBuf::from("/usr/share/ember/plugins"));
    v.retain(|p| p.is_dir());
    v
}

fn read_git_branch(root: &Path) -> Option<String> {
    let head = std::fs::read_to_string(root.join(".git").join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(r) = head.strip_prefix("ref: refs/heads/") {
        Some(r.to_string())
    } else {
        Some(head.chars().take(7).collect())
    }
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !(e.file_type().is_dir() && e.depth() > 0 && is_ignored_dir(&e.file_name().to_string_lossy())))
        .flatten()
        .filter(|e| e.file_type().is_file())
        .take(40_000)
        .map(|e| e.into_path())
        .collect()
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: crate::Opts) -> Self {
        let arg = opts.path.clone();
        let cfg = Config::load();
        let themes = all_themes();
        let wanted = opts.theme.clone().unwrap_or_else(|| cfg.theme.clone());
        let theme_idx = themes.iter().position(|t| t.name.eq_ignore_ascii_case(&wanted)).unwrap_or(0);
        themes[theme_idx].apply(&cc.egui_ctx);
        cc.egui_ctx.style_mut(|s| {
            use egui::{FontId, TextStyle::*};
            s.text_styles.insert(Body, FontId::proportional(13.5));
            s.text_styles.insert(Button, FontId::proportional(13.5));
            s.text_styles.insert(Heading, FontId::proportional(20.0));
            s.text_styles.insert(Small, FontId::proportional(11.5));
            s.text_styles.insert(Monospace, FontId::monospace(13.0));
        });
        let logo = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok().map(|d| {
            cc.egui_ctx.load_texture(
                "logo",
                egui::ColorImage::from_rgba_unmultiplied([d.width as usize, d.height as usize], &d.rgba),
                Default::default(),
            )
        });
        let mut app = App {
            cfg,
            themes,
            theme_idx,
            docs: vec![],
            active: 0,
            next_id: 1,
            untitled_counter: 0,
            explorer: Explorer::default(),
            hl: Highlighter::new(),
            side_tab: SideTab::Explorer,
            show_side: true,
            show_bottom: false,
            bottom: Bottom::Terminal,
            palette: None,
            find: FindState::default(),
            search: ProjectSearch::default(),
            terminal: Terminal::default(),
            plugins: PluginHost::new(),
            dbg: Debugger::default(),
            presence: Presence::new(),
            toasts: vec![],
            show_settings: false,
            show_about: false,
            confirm: None,
            allow_close: false,
            git_branch: None,
            git_checked: Instant::now(),
            project_files: None,
            last_title: String::new(),
            logo,
            adapter_args_buf: Default::default(),
            focus_terminal: false,
            focus_search: false,
            last_active: usize::MAX,
            terminal_focus: false,
            rec: None,
            last_input: std::time::Instant::now(),
            file_since: (0, crate::util::unix_now()),
            shot: opts.screenshot.clone().map(|p| (p, 0)),
        };
        app.reload_plugins();
        app.presence.configure(app.cfg.discord_enabled, &crate::config::effective_discord_id(&app.cfg.discord_app_id));
        match arg {
            Some(p) if p.is_dir() => app.open_folder(p),
            Some(p) if p.is_file() => {
                if let Some(parent) = p.parent() {
                    app.open_folder(parent.to_path_buf());
                }
                app.open_file(&p);
            }
            _ => {
                if opts.ui.as_deref() != Some("welcome") {
                    if let Some(f) = app.cfg.last_folder.clone() {
                        if f.is_dir() {
                            app.open_folder(f);
                        }
                    }
                }
            }
        }
        app.apply_demo_state(opts.ui.as_deref());
        if let Some(dir) = opts.record.clone() {
            let names: Vec<String> = app.themes.iter().map(|t| t.name.clone()).collect();
            let (rec, project) = crate::record::Recorder::new(dir, &names);
            app.open_folder(project.clone());
            app.open_file(&project.join("src").join("main.rs"));
            if let Some(i) = app.themes.iter().position(|t| t.name == "Ember Dark") {
                app.theme_idx = i;
                app.themes[i].apply(&cc.egui_ctx);
            }
            app.rec = Some(rec);
        }
        app
    }

    /// Pre-open a screen. Only used by `--ui` (screenshots and demos).
    fn apply_demo_state(&mut self, ui: Option<&str>) {
        let Some(ui) = ui else { return };
        if !matches!(ui, "welcome" | "image") {
            // Most demo screens look better with a real source file open behind them.
            let p = self.work_dir().join("src").join("editor.rs");
            if p.is_file() {
                if let Some(i) = self.open_file(&p) {
                    self.docs[i].goto = Some(34);
                }
            }
        }
        match ui {
            "palette" => self.palette = Some(Palette { mode: PaletteMode::Commands, query: String::new(), sel: 2, first: true }),
            "quickopen" => {
                if let Some(r) = self.explorer.root.clone() {
                    self.project_files = Some(collect_files(&r));
                }
                self.palette = Some(Palette { mode: PaletteMode::Files, query: "ap".into(), sel: 0, first: true });
            }
            "settings" => self.show_settings = true,
            "find" => {
                self.open_find(true);
                self.find.query = "use".into();
                self.find.replace = "using".into();
            }
            "debug" => {
                self.side_tab = SideTab::Debug;
                if let Some(d) = self.docs.get_mut(self.active) {
                    d.breakpoints.extend([2usize, 9, 31]);
                }
            }
            "plugins" => self.side_tab = SideTab::Plugins,
            "search" => {
                self.side_tab = SideTab::Search;
                self.search.query = "Highlighter".into();
                let root = self.work_dir();
                self.search.start(&root);
            }
            "image" => {
                let p = self.work_dir().join("assets").join("icon.png");
                self.open_file(&p);
            }
            "terminal" => {
                self.show_bottom = true;
                self.bottom = Bottom::Terminal;
                let root = self.work_dir();
                self.terminal.run(if cfg!(windows) { "echo 'Hello from the Ember terminal'; Get-ChildItem | Select-Object -First 8" } else { "echo Hello from the Ember terminal; ls" }, &root);
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------- helpers

    pub fn theme(&self) -> Theme {
        self.themes[self.theme_idx].clone()
    }

    /// Append a timestamped line to the Output panel.
    pub fn log_out(&mut self, s: impl AsRef<str>) {
        let t = crate::util::now_iso();
        self.plugins.log.push(format!("[{}] {}", t.get(11..).unwrap_or(&t), s.as_ref()));
        if self.plugins.log.len() > 2000 {
            self.plugins.log.drain(0..500);
        }
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        let s = s.into();
        self.log_out(&s);
        self.toasts.push(Toast { text: s, at: Instant::now() });
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    pub fn work_dir(&self) -> PathBuf {
        self.explorer
            .root
            .clone()
            .or_else(|| self.docs.get(self.active).and_then(|d| d.path.as_ref()).and_then(|p| p.parent().map(|x| x.to_path_buf())))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    pub fn rel(&self, p: &Path) -> String {
        match &self.explorer.root {
            Some(r) => p.strip_prefix(r).unwrap_or(p).display().to_string(),
            None => p.display().to_string(),
        }
    }

    pub fn set_theme(&mut self, ctx: &egui::Context, idx: usize) {
        if idx < self.themes.len() {
            self.theme_idx = idx;
            self.cfg.theme = self.themes[idx].name.clone();
            self.themes[idx].apply(ctx);
            self.cfg.save();
        }
    }

    pub fn reload_plugins(&mut self) {
        let dirs = plugin_dirs();
        self.plugins.reload(&dirs, &self.cfg.disabled_plugins);
    }

    pub fn plugin_ctx(&self) -> (String, String, String) {
        let root = self.explorer.root.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        match self.docs.get(self.active) {
            Some(d) => (d.text.clone(), d.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(), root),
            None => (String::new(), String::new(), root),
        }
    }

    pub fn apply_actions(&mut self, doc_idx: Option<usize>, actions: Vec<PluginAction>) {
        for a in actions {
            match a {
                PluginAction::Notify(s) => self.toast(s),
                PluginAction::SetText(s) => {
                    if let Some(i) = doc_idx.or(Some(self.active)) {
                        if let Some(d) = self.docs.get_mut(i) {
                            if d.text != s {
                                d.text = s;
                                d.touch();
                            }
                        }
                    }
                }
                PluginAction::OpenFile(p) => {
                    self.open_file(Path::new(&p));
                }
                PluginAction::Log(_) => {}
            }
        }
    }

    // ------------------------------------------------------------ documents

    pub fn open_folder(&mut self, path: PathBuf) {
        self.log_out(format!("Opened folder {}", path.display()));
        self.explorer.set_root(path.clone());
        self.cfg.push_recent(path.clone());
        self.cfg.save();
        self.git_branch = read_git_branch(&path);
        self.project_files = None;
        self.search.results.clear();
        if let Some(prog) = guess_rust_program(&path) {
            self.dbg.program = prog;
        }
        self.show_side = true;
        self.side_tab = SideTab::Explorer;
    }

    pub fn open_file(&mut self, path: &Path) -> Option<usize> {
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(path)) {
            self.active = i;
            self.explorer.reveal(path);
            return Some(i);
        }
        let id = self.next_id;
        match Doc::open(id, path.to_path_buf()) {
            Ok(doc) => {
                self.next_id += 1;
                let (text, p) = (doc.text.clone(), path.display().to_string());
                // Restore breakpoints that were set before the tab was opened.
                self.docs.push(doc);
                self.active = self.docs.len() - 1;
                self.explorer.reveal(path);
                let root = self.explorer.root.as_ref().map(|r| r.display().to_string()).unwrap_or_default();
                let acts = self.plugins.fire("on_open", &text, &p, &root);
                let idx = self.active;
                self.apply_actions(Some(idx), acts);
                Some(self.active)
            }
            Err(e) => {
                self.toast(format!("Cannot open {}: {e}", file_name(path)));
                None
            }
        }
    }

    pub fn open_at(&mut self, path: &Path, line: usize) {
        if let Some(i) = self.open_file(path) {
            self.docs[i].goto = Some(line);
            self.docs[i].want_focus = true;
        }
    }

    pub fn new_file(&mut self) {
        self.untitled_counter += 1;
        let d = Doc::untitled(self.next_id, self.untitled_counter);
        self.next_id += 1;
        self.docs.push(d);
        self.active = self.docs.len() - 1;
    }

    pub fn save_doc(&mut self, idx: usize, hooks: bool) {
        if idx >= self.docs.len() {
            return;
        }
        if self.docs[idx].path.is_none() {
            self.save_as(idx);
            return;
        }
        let path = self.docs[idx].path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let root = self.explorer.root.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        if hooks {
            let text = self.docs[idx].text.clone();
            let acts = self.plugins.fire("on_before_save", &text, &path, &root);
            self.apply_actions(Some(idx), acts);
        }
        match self.docs[idx].save() {
            Ok(()) => {
                if hooks {
                    let text = self.docs[idx].text.clone();
                    let acts = self.plugins.fire("on_save", &text, &path, &root);
                    self.apply_actions(Some(idx), acts);
                }
            }
            Err(e) => self.toast(format!("Save failed: {e}")),
        }
    }

    pub fn save_as(&mut self, idx: usize) {
        if idx >= self.docs.len() {
            return;
        }
        let mut dlg = rfd::FileDialog::new().set_file_name(self.docs[idx].title());
        if let Some(r) = &self.explorer.root {
            dlg = dlg.set_directory(r);
        }
        if let Some(p) = dlg.save_file() {
            self.docs[idx].path = Some(p);
            self.docs[idx].job = None;
            self.save_doc(idx, false);
            self.explorer.refresh();
            self.project_files = None;
        }
    }

    pub fn close_tab(&mut self, idx: usize, force: bool) {
        if idx >= self.docs.len() {
            return;
        }
        if self.docs[idx].dirty && !force {
            self.confirm = Some(Confirm::CloseTab(idx));
            return;
        }
        self.docs.remove(idx);
        if self.active >= self.docs.len() && self.active > 0 {
            self.active = self.docs.len().saturating_sub(1);
        } else if idx < self.active {
            self.active -= 1;
        }
    }

    pub fn handle_explorer_events(&mut self, events: Vec<ExplorerEvent>) {
        for e in events {
            match e {
                ExplorerEvent::Open(p) => {
                    self.open_file(&p);
                    if let Some(d) = self.docs.get_mut(self.active) {
                        d.want_focus = true;
                    }
                }
                ExplorerEvent::Created(p) => {
                    self.project_files = None;
                    if p.is_file() {
                        self.open_file(&p);
                    }
                    self.explorer.reveal(&p);
                }
                ExplorerEvent::Deleted(p) => {
                    self.project_files = None;
                    let ids: Vec<usize> = self.docs.iter().enumerate().filter(|(_, d)| d.path.as_deref().map(|x| x.starts_with(&p)).unwrap_or(false)).map(|(i, _)| i).collect();
                    for i in ids.into_iter().rev() {
                        self.close_tab(i, true);
                    }
                }
                ExplorerEvent::Renamed(old, new) => {
                    self.project_files = None;
                    for d in &mut self.docs {
                        if d.path.as_deref() == Some(old.as_path()) {
                            d.path = Some(new.clone());
                            d.job = None;
                        }
                    }
                }
                ExplorerEvent::Error(s) => self.toast(s),
            }
        }
    }

    // ------------------------------------------------------------------ find

    fn find_regex(&self) -> Option<regex::Regex> {
        let pat = if self.find.regex { self.find.query.clone() } else { regex::escape(&self.find.query) };
        regex::RegexBuilder::new(&pat).case_insensitive(!self.find.case).multi_line(true).build().ok()
    }

    pub fn update_find(&mut self) {
        if !self.find.open || self.find.query.is_empty() || self.docs.get(self.active).is_none() {
            self.find.matches.clear();
            self.find.key = None;
            self.find.error = None;
            return;
        }
        let d = &self.docs[self.active];
        let key = (d.id, d.rev, hash_of(&self.find.query), self.find.case, self.find.regex);
        if self.find.key == Some(key) {
            return;
        }
        self.find.key = Some(key);
        let Some(re) = self.find_regex() else {
            self.find.error = Some("invalid regex".into());
            self.find.matches.clear();
            return;
        };
        self.find.error = None;
        let d = &self.docs[self.active];
        let mut out = Vec::new();
        let (mut last_b, mut last_c) = (0usize, 0usize);
        for m in re.find_iter(&d.text).take(20_000) {
            if m.start() == m.end() {
                continue;
            }
            let a = last_c + d.text[last_b..m.start()].chars().count();
            let b = a + d.text[m.start()..m.end()].chars().count();
            out.push((a, b));
            last_b = m.end();
            last_c = b;
        }
        self.find.matches = out;
        if self.find.current >= self.find.matches.len() {
            self.find.current = 0;
        }
    }

    pub fn find_step(&mut self, forward: bool) {
        self.update_find();
        let n = self.find.matches.len();
        if n == 0 {
            return;
        }
        self.find.current = if forward { (self.find.current + 1) % n } else { (self.find.current + n - 1) % n };
        let m = self.find.matches[self.find.current];
        if let Some(d) = self.docs.get_mut(self.active) {
            d.select = Some(m);
        }
    }

    pub fn find_select_current(&mut self) {
        if let Some(m) = self.find.matches.get(self.find.current).copied() {
            if let Some(d) = self.docs.get_mut(self.active) {
                d.select = Some(m);
            }
        }
    }

    pub fn replace_current(&mut self) {
        self.update_find();
        let Some(&(a, b)) = self.find.matches.get(self.find.current) else { return };
        let Some(re) = self.find_regex() else { return };
        let rep = self.find.replace.clone();
        let regex_mode = self.find.regex;
        let d = &mut self.docs[self.active];
        let (ba, bb) = (char_to_byte(&d.text, a), char_to_byte(&d.text, b));
        let new = if regex_mode { re.replace(&d.text[ba..bb], rep.as_str()).into_owned() } else { rep };
        d.text.replace_range(ba..bb, &new);
        d.touch();
        self.find.key = None;
        self.update_find();
        self.find_select_current();
    }

    pub fn replace_all(&mut self) {
        let Some(re) = self.find_regex() else { return };
        if self.find.query.is_empty() {
            return;
        }
        let rep = self.find.replace.clone();
        let count = self.find.matches.len();
        let regex_mode = self.find.regex;
        let d = &mut self.docs[self.active];
        let new = if regex_mode { re.replace_all(&d.text, rep.as_str()).into_owned() } else { re.replace_all(&d.text, regex::NoExpand(rep.as_str())).into_owned() };
        d.text = new;
        d.touch();
        self.find.key = None;
        self.toast(format!("Replaced {count} occurrence(s)"));
    }

    pub fn open_find(&mut self, replace: bool) {
        if self.docs.is_empty() {
            return;
        }
        self.find.open = true;
        self.find.replace_mode = replace;
        self.find.focus = true;
    }

    // -------------------------------------------------------------- run/debug

    pub fn run_current(&mut self) {
        let Some(d) = self.docs.get(self.active) else { return };
        let Some(path) = d.path.clone() else {
            self.toast("Save the file before running it");
            return;
        };
        if d.dirty {
            self.save_doc(self.active, true);
        }
        let ext = ext_of(&path);
        let p = format!("\"{}\"", path.display());
        let root = self.work_dir();
        let tmp = if cfg!(windows) { "ember_tmp.exe" } else { "./ember_tmp" };
        let cmd = match ext.as_str() {
            "rs" if root.join("Cargo.toml").exists() => "cargo run".to_string(),
            "rs" => format!("rustc {p} -o ember_tmp{} && {tmp}", if cfg!(windows) { ".exe" } else { "" }),
            "py" => format!("{} {p}", if cfg!(windows) { "python" } else { "python3" }),
            "js" | "mjs" | "cjs" => format!("node {p}"),
            "ts" => format!("npx tsx {p}"),
            "go" => format!("go run {p}"),
            "sh" | "bash" => format!("bash {p}"),
            "ps1" => format!("powershell -NoProfile -File {p}"),
            "bat" | "cmd" => p.clone(),
            "rb" => format!("ruby {p}"),
            "lua" => format!("lua {p}"),
            "java" => format!("java {p}"),
            "html" | "htm" => {
                open_in_os(&path);
                return;
            }
            _ => {
                self.toast(format!("Don't know how to run .{ext} files"));
                return;
            }
        };
        self.show_bottom = true;
        self.bottom = Bottom::Terminal;
        self.log_out(format!("Run: {cmd}"));
        self.terminal.run(&cmd, &root);
    }

    pub fn all_breakpoints(&self) -> Vec<(PathBuf, Vec<usize>)> {
        self.docs.iter().filter_map(|d| d.path.clone().map(|p| (p, d.breakpoints.iter().cloned().collect::<Vec<_>>()))).filter(|(_, l)| !l.is_empty()).collect()
    }

    pub fn sync_breakpoints(&mut self, idx: usize) {
        if let Some(d) = self.docs.get(idx) {
            if let Some(p) = d.path.clone() {
                let lines: Vec<usize> = d.breakpoints.iter().cloned().collect();
                self.dbg.set_breakpoints(&p, lines);
            }
        }
    }

    pub fn toggle_breakpoint_at_cursor(&mut self) {
        let idx = self.active;
        let Some(d) = self.docs.get_mut(idx) else { return };
        let line = d.cursor.0.saturating_sub(1);
        if !d.breakpoints.remove(&line) {
            d.breakpoints.insert(line);
        }
        self.sync_breakpoints(idx);
    }

    pub fn debug_start(&mut self) {
        if self.dbg.state == DbgState::Stopped {
            self.dbg.cont();
            return;
        }
        if self.dbg.active() {
            return;
        }
        if self.docs.get(self.active).map(|d| d.dirty && d.path.is_some()).unwrap_or(false) {
            self.save_doc(self.active, true);
        }
        let root = self.work_dir();
        let file = self.docs.get(self.active).and_then(|d| d.path.clone());
        let bps = self.all_breakpoints();
        match self.dbg.start(&self.cfg, &root, file.as_deref(), bps) {
            Ok(()) => {
                self.side_tab = SideTab::Debug;
                self.show_side = true;
                self.bottom = Bottom::DebugConsole;
                self.show_bottom = true;
            }
            Err(e) => {
                self.toast(e.to_string());
                self.side_tab = SideTab::Debug;
                self.show_side = true;
            }
        }
    }

    fn handle_debug_events(&mut self, events: Vec<DbgEvent>) {
        for e in events {
            match e {
                DbgEvent::Stopped { path, line } => {
                    if path.exists() {
                        self.open_at(&path, line.saturating_sub(1));
                    }
                }
                DbgEvent::Started => self.toast("Debugger attached"),
                DbgEvent::Ended(m) => self.toast(m),
            }
        }
    }

    // ----------------------------------------------------------------- commands

    pub fn exec(&mut self, ctx: &egui::Context, cmd: Cmd) {
        match cmd {
            Cmd::NewFile => self.new_file(),
            Cmd::OpenFile => {
                let mut d = rfd::FileDialog::new();
                if let Some(r) = &self.explorer.root {
                    d = d.set_directory(r);
                }
                if let Some(files) = d.pick_files() {
                    for f in files {
                        self.open_file(&f);
                    }
                }
            }
            Cmd::OpenFolder => {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    self.open_folder(p);
                }
            }
            Cmd::OpenRecent(p) => {
                if p.is_dir() {
                    self.open_folder(p);
                } else {
                    self.toast("That folder no longer exists");
                }
            }
            Cmd::OpenPath(p) => {
                self.open_file(&p);
                if let Some(d) = self.docs.get_mut(self.active) {
                    d.want_focus = true;
                }
            }
            Cmd::Save => self.save_doc(self.active, true),
            Cmd::SaveAs => self.save_as(self.active),
            Cmd::SaveAll => {
                for i in 0..self.docs.len() {
                    if self.docs[i].dirty {
                        self.save_doc(i, true);
                    }
                }
            }
            Cmd::CloseTab => self.close_tab(self.active, false),
            Cmd::CloseFolder => {
                self.explorer.root = None;
                self.git_branch = None;
                self.project_files = None;
            }
            Cmd::Quit => {
                if self.docs.iter().any(|d| d.dirty) {
                    self.confirm = Some(Confirm::Quit);
                } else {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Cmd::ToggleSidebar => self.show_side = !self.show_side,
            Cmd::TogglePanel => {
                self.show_bottom = !self.show_bottom;
                if self.show_bottom && self.bottom == Bottom::Terminal {
                    self.focus_terminal = true;
                }
            }
            Cmd::ShowSide(t) => {
                if self.side_tab == t && self.show_side {
                    self.show_side = false;
                } else {
                    self.side_tab = t;
                    self.show_side = true;
                }
            }
            Cmd::ShowBottom(b) => {
                self.bottom = b;
                self.show_bottom = true;
                if b == Bottom::Terminal {
                    self.focus_terminal = true;
                }
            }
            Cmd::Find => self.open_find(false),
            Cmd::Replace => self.open_find(true),
            Cmd::FindNext => self.find_step(true),
            Cmd::FindPrev => self.find_step(false),
            Cmd::ProjectSearch => {
                self.side_tab = SideTab::Search;
                self.show_side = true;
                self.focus_search = true;
            }
            Cmd::QuickOpen => {
                if self.project_files.is_none() {
                    if let Some(r) = self.explorer.root.clone() {
                        self.project_files = Some(collect_files(&r));
                    }
                }
                self.palette = Some(Palette { mode: PaletteMode::Files, query: String::new(), sel: 0, first: true });
            }
            Cmd::Palette => self.palette = Some(Palette { mode: PaletteMode::Commands, query: String::new(), sel: 0, first: true }),
            Cmd::GotoLine => {
                if !self.docs.is_empty() {
                    self.palette = Some(Palette { mode: PaletteMode::Goto, query: String::new(), sel: 0, first: true });
                }
            }
            Cmd::Settings => self.show_settings = true,
            Cmd::About => self.show_about = true,
            Cmd::SetTheme(i) => self.set_theme(ctx, i),
            Cmd::ZoomIn => {
                self.cfg.font_size = (self.cfg.font_size + 1.0).min(40.0);
                self.cfg.save();
            }
            Cmd::ZoomOut => {
                self.cfg.font_size = (self.cfg.font_size - 1.0).max(8.0);
                self.cfg.save();
            }
            Cmd::ZoomReset => {
                self.cfg.font_size = 14.5;
                self.cfg.save();
            }
            Cmd::ToggleWrap => {
                self.cfg.word_wrap = !self.cfg.word_wrap;
                self.cfg.save();
            }
            Cmd::DebugStart => self.debug_start(),
            Cmd::DebugStop => self.dbg.stop(),
            Cmd::DebugContinue => self.dbg.cont(),
            Cmd::DebugOver => self.dbg.step_over(),
            Cmd::DebugIn => self.dbg.step_in(),
            Cmd::DebugOut => self.dbg.step_out(),
            Cmd::DebugPause => self.dbg.pause(),
            Cmd::ToggleBreakpoint => self.toggle_breakpoint_at_cursor(),
            Cmd::RunFile => self.run_current(),
            Cmd::Plugin(pid, cid) => {
                let (t, p, r) = self.plugin_ctx();
                let acts = self.plugins.run_command(&pid, &cid, &t, &p, &r);
                let idx = self.active;
                self.apply_actions(Some(idx), acts);
            }
            Cmd::ReloadPlugins => {
                self.reload_plugins();
                self.toast(format!("Loaded {} plugin(s)", self.plugins.plugins.len()));
            }
            Cmd::OpenPluginsDir => {
                let d = config_dir().join("plugins");
                let _ = std::fs::create_dir_all(&d);
                open_in_os(&d);
            }
            Cmd::OpenThemesDir => {
                let d = config_dir().join("themes");
                let _ = std::fs::create_dir_all(&d);
                open_in_os(&d);
            }
            Cmd::OpenConfigDir => {
                let d = config_dir();
                let _ = std::fs::create_dir_all(&d);
                open_in_os(&d);
            }
            Cmd::NextTab => {
                if !self.docs.is_empty() {
                    self.active = (self.active + 1) % self.docs.len();
                }
            }
            Cmd::PrevTab => {
                if !self.docs.is_empty() {
                    self.active = (self.active + self.docs.len() - 1) % self.docs.len();
                }
            }
            Cmd::ClearTerminal => self.terminal.clear(),
        }
    }

    pub fn palette_commands(&self) -> Vec<(String, String, Cmd)> {
        let s = |l: &str, k: &str, c: Cmd| (l.to_string(), k.to_string(), c);
        let mut v = vec![
            s("File: New File", "Ctrl+N", Cmd::NewFile),
            s("File: Open File…", "Ctrl+O", Cmd::OpenFile),
            s("File: Open Folder…", "Ctrl+Shift+O", Cmd::OpenFolder),
            s("File: Save", "Ctrl+S", Cmd::Save),
            s("File: Save As…", "Ctrl+Shift+S", Cmd::SaveAs),
            s("File: Save All", "", Cmd::SaveAll),
            s("File: Close Tab", "Ctrl+W", Cmd::CloseTab),
            s("File: Close Folder", "", Cmd::CloseFolder),
            s("File: Quit", "", Cmd::Quit),
            s("Go to File…", "Ctrl+P", Cmd::QuickOpen),
            s("Go to Line…", "Ctrl+G", Cmd::GotoLine),
            s("Find in File", "Ctrl+F", Cmd::Find),
            s("Replace in File", "Ctrl+H", Cmd::Replace),
            s("Search in Project", "Ctrl+Shift+F", Cmd::ProjectSearch),
            s("View: Toggle Sidebar", "Ctrl+B", Cmd::ToggleSidebar),
            s("View: Toggle Panel", "Ctrl+J", Cmd::TogglePanel),
            s("View: Show Explorer", "", Cmd::ShowSide(SideTab::Explorer)),
            s("View: Show Debug", "", Cmd::ShowSide(SideTab::Debug)),
            s("View: Show Plugins", "", Cmd::ShowSide(SideTab::Plugins)),
            s("View: Show Terminal", "", Cmd::ShowBottom(Bottom::Terminal)),
            s("View: Show Debug Console", "", Cmd::ShowBottom(Bottom::DebugConsole)),
            s("View: Show Output", "", Cmd::ShowBottom(Bottom::Output)),
            s("View: Toggle Word Wrap", "", Cmd::ToggleWrap),
            s("View: Zoom In", "Ctrl+=", Cmd::ZoomIn),
            s("View: Zoom Out", "Ctrl+-", Cmd::ZoomOut),
            s("View: Reset Zoom", "", Cmd::ZoomReset),
            s("Run: Run Current File", "Ctrl+F5", Cmd::RunFile),
            s("Debug: Start / Continue", "F5", Cmd::DebugStart),
            s("Debug: Stop", "Shift+F5", Cmd::DebugStop),
            s("Debug: Step Over", "F10", Cmd::DebugOver),
            s("Debug: Step Into", "F11", Cmd::DebugIn),
            s("Debug: Step Out", "Shift+F11", Cmd::DebugOut),
            s("Debug: Pause", "", Cmd::DebugPause),
            s("Debug: Toggle Breakpoint", "F9", Cmd::ToggleBreakpoint),
            s("Terminal: Clear", "", Cmd::ClearTerminal),
            s("Plugins: Reload", "", Cmd::ReloadPlugins),
            s("Plugins: Open Plugins Folder", "", Cmd::OpenPluginsDir),
            s("Preferences: Open Settings", "Ctrl+,", Cmd::Settings),
            s("Preferences: Open Themes Folder", "", Cmd::OpenThemesDir),
            s("Preferences: Open Config Folder", "", Cmd::OpenConfigDir),
            s("Help: About Ember IDE", "", Cmd::About),
        ];
        for (i, t) in self.themes.iter().enumerate() {
            v.push((format!("Theme: {}", t.name), String::new(), Cmd::SetTheme(i)));
        }
        for p in &self.plugins.plugins {
            if p.enabled {
                for (cid, title) in &p.commands {
                    v.push((title.clone(), format!("plugin · {}", p.name), Cmd::Plugin(p.id.clone(), cid.clone())));
                }
            }
        }
        for r in &self.cfg.recent_folders {
            v.push((format!("Open Recent: {}", r.display()), String::new(), Cmd::OpenRecent(r.clone())));
        }
        v
    }

    pub fn project_file_list(&self) -> &[PathBuf] {
        self.project_files.as_deref().unwrap_or(&[])
    }

    // ------------------------------------------------------------ per-frame

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let c = Modifiers::COMMAND;
        let cs = Modifiers::COMMAND | Modifiers::SHIFT;
        let sh = Modifiers::SHIFT;
        let none = Modifiers::NONE;
        let mut cmds: Vec<Cmd> = Vec::new();
        if self.terminal_focus {
            // While typing in the terminal only a couple of global keys apply.
            ctx.input_mut(|i| {
                if i.consume_key(c, Key::J) {
                    cmds.push(Cmd::TogglePanel);
                }
                if i.consume_key(cs, Key::P) {
                    cmds.push(Cmd::Palette);
                }
            });
            for cmd in cmds {
                self.exec(ctx, cmd);
            }
            return;
        }
        ctx.input_mut(|i| {
            let mut k = |m: Modifiers, key: Key, cmd: Cmd, out: &mut Vec<Cmd>| {
                if i.consume_key(m, key) {
                    out.push(cmd);
                }
            };
            k(cs, Key::P, Cmd::Palette, &mut cmds);
            k(cs, Key::O, Cmd::OpenFolder, &mut cmds);
            k(cs, Key::S, Cmd::SaveAs, &mut cmds);
            k(cs, Key::F, Cmd::ProjectSearch, &mut cmds);
            k(cs, Key::E, Cmd::ShowSide(SideTab::Explorer), &mut cmds);
            k(cs, Key::D, Cmd::ShowSide(SideTab::Debug), &mut cmds);
            k(cs, Key::X, Cmd::ShowSide(SideTab::Plugins), &mut cmds);
            k(c, Key::P, Cmd::QuickOpen, &mut cmds);
            k(c, Key::O, Cmd::OpenFile, &mut cmds);
            k(c, Key::N, Cmd::NewFile, &mut cmds);
            k(c, Key::S, Cmd::Save, &mut cmds);
            k(c, Key::W, Cmd::CloseTab, &mut cmds);
            k(c, Key::F, Cmd::Find, &mut cmds);
            k(c, Key::H, Cmd::Replace, &mut cmds);
            k(c, Key::G, Cmd::GotoLine, &mut cmds);
            k(c, Key::B, Cmd::ToggleSidebar, &mut cmds);
            k(c, Key::J, Cmd::TogglePanel, &mut cmds);
            k(c, Key::Comma, Cmd::Settings, &mut cmds);
            k(c, Key::Equals, Cmd::ZoomIn, &mut cmds);
            k(c, Key::Minus, Cmd::ZoomOut, &mut cmds);
            k(c, Key::Num0, Cmd::ZoomReset, &mut cmds);
            k(c, Key::PageDown, Cmd::NextTab, &mut cmds);
            k(c, Key::PageUp, Cmd::PrevTab, &mut cmds);
            k(c, Key::F5, Cmd::RunFile, &mut cmds);
            k(sh, Key::F5, Cmd::DebugStop, &mut cmds);
            k(sh, Key::F11, Cmd::DebugOut, &mut cmds);
            k(sh, Key::F3, Cmd::FindPrev, &mut cmds);
            k(none, Key::F3, Cmd::FindNext, &mut cmds);
            k(none, Key::F5, Cmd::DebugStart, &mut cmds);
            k(none, Key::F9, Cmd::ToggleBreakpoint, &mut cmds);
            k(none, Key::F10, Cmd::DebugOver, &mut cmds);
            k(none, Key::F11, Cmd::DebugIn, &mut cmds);
        });
        for cmd in cmds {
            self.exec(ctx, cmd);
        }
    }

    fn frame_start(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| !i.events.is_empty() || i.pointer.delta() != egui::Vec2::ZERO) {
            self.last_input = std::time::Instant::now();
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(5));
        self.terminal.poll();
        self.search.poll();
        let evs = self.dbg.poll();
        self.handle_debug_events(evs);

        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            if let Some(p) = f.path {
                if p.is_dir() {
                    self.open_folder(p);
                } else {
                    self.open_file(&p);
                }
            }
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.docs.iter().any(|d| d.dirty) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm = Some(Confirm::Quit);
            } else {
                self.on_exit();
            }
        }

        if self.cfg.auto_save {
            let due: Vec<usize> = self
                .docs
                .iter()
                .enumerate()
                .filter(|(_, d)| d.dirty && d.path.is_some() && d.last_edit.elapsed() > Duration::from_millis(1200))
                .map(|(i, _)| i)
                .collect();
            for i in due {
                self.save_doc(i, false);
            }
        }

        if self.git_checked.elapsed() > Duration::from_secs(3) {
            self.git_checked = Instant::now();
            self.git_branch = self.explorer.root.as_ref().and_then(|r| read_git_branch(r));
        }
        self.toasts.retain(|t| t.at.elapsed() < Duration::from_millis(4500));
    }

    fn on_exit(&mut self) {
        self.dbg.stop();
        self.cfg.save();
    }

    /// `--screenshot out.png`: render a few frames, save the window as PNG and quit.
    /// Used for README images and automated smoke tests.
    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some((path, frames)) = &mut self.shot else { return };
        *frames += 1;
        ctx.request_repaint();
        if *frames == 45 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        let img = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = img {
            let mut raw = Vec::with_capacity(img.pixels.len() * 4);
            for p in &img.pixels {
                raw.extend_from_slice(&p.to_array());
            }
            let _ = image::save_buffer(&*path, &raw, img.size[0] as u32, img.size[1] as u32, image::ColorType::Rgba8);
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn update_title_and_presence(&mut self, ctx: &egui::Context) {
        let folder = self.explorer.root.as_ref().map(|p| file_name(p)).unwrap_or_default();
        let (title, dirty, ext) = match self.docs.get(self.active) {
            Some(d) => (d.title(), d.dirty, d.ext()),
            None => (String::new(), false, String::new()),
        };
        let mut t = String::new();
        if !title.is_empty() {
            t.push_str(&format!("{}{}", if dirty { "● " } else { "" }, title));
            t.push_str(" — ");
        }
        if !folder.is_empty() {
            t.push_str(&format!("{folder} — "));
        }
        t.push_str("Ember IDE");
        if t != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(t.clone()));
            self.last_title = t;
        }

        let debugging = self.dbg.active();
        let idle = self.last_input.elapsed() > std::time::Duration::from_secs(180);
        let show = self.cfg.discord_show_file;
        let cur = self.docs.get(self.active).map(|d| (d.id, d.cursor));
        if let Some((id, _)) = cur {
            if id != self.file_since.0 {
                self.file_since = (id, crate::util::unix_now());
            }
        }
        let lang = if ext.is_empty() { String::new() } else { lang_label(&ext).to_string() };
        let info = if title.is_empty() {
            PresenceInfo { details: "Idling in Ember IDE".into(), state: if folder.is_empty() || !show { String::new() } else { format!("Workspace: {folder}") }, small_key: "idle".into(), small_text: "Idle".into(), since: self.file_since.1 }
        } else {
            let verb = if debugging { "Debugging" } else if idle { "Idling on" } else { "Editing" };
            let details = if show { format!("{verb} {title}") } else { format!("{verb} code") };
            let (line, col) = cur.map(|(_, c)| c).unwrap_or((1, 1));
            let mut state = if idle { "Away from keyboard".to_string() } else { format!("Line {line}, Col {col}") };
            if show && !folder.is_empty() {
                state.push_str(&format!(" · {folder}"));
            }
            PresenceInfo {
                details,
                state,
                small_key: crate::util::lang_key(&ext).into(),
                small_text: if lang.is_empty() { "Ember IDE".into() } else { lang },
                since: self.file_since.1,
            }
        };
        self.presence.update(info);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_start(ctx);
        self.record_step(ctx);
        self.shortcuts(ctx);
        self.update_find();
        let theme = self.theme();
        self.menu_bar(ctx, &theme);
        self.status_bar(ctx, &theme);
        self.activity_bar(ctx, &theme);
        if self.show_side {
            self.side_panel(ctx, &theme);
        }
        if self.show_bottom {
            self.bottom_panel(ctx, &theme);
        }
        self.central(ctx, &theme);
        self.dialogs(ctx, &theme);
        self.update_title_and_presence(ctx);

        self.screenshot_step(ctx);
        let busy = self.search.running || self.dbg.active() || !self.toasts.is_empty();
        ctx.request_repaint_after(Duration::from_millis(if busy { 60 } else { 700 }));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.on_exit();
    }
}
