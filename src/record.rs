//! Scripted demo recorder (`ember --record DIR`). It drives the real editor through a scripted
//! session — typing, saving, the palette, theme switches, the terminal and the debugger — and
//! writes one PNG per step. `tools/make-demo.py` turns those frames into the website GIFs.
//! The script advances per captured frame (not per wall-clock tick), so the result is deterministic.

use crate::app::{App, Bottom, Palette, PaletteMode, SideTab};
use eframe::egui;
use std::path::PathBuf;

pub enum Act {
    Type(String),
    Save,
    PaletteOpen,
    PaletteQuery(String),
    PaletteClose,
    Theme(usize),
    ShowBottom(Bottom),
    HideBottom,
    TermRun(String),
    Breakpoint(usize),
    DebugStart,
    StepOver,
    Cont,
    Wait(u32),
    Mark(String),
    SleepMs(u64),
    Quit,
}

pub struct Recorder {
    pub dir: PathBuf,
    pub script: Vec<Act>,
    pub pos: usize,
    pub frame: u32,
    pub settle: u32,
    pub waiting: bool,
    pub wait_left: u32,
    pub marks: Vec<(u32, String)>,
}

const MAIN_RS: &str = "use std::collections::HashMap;

#[derive(Debug)]
struct Task {
    id: u32,
    title: String,
    done: bool,
}

fn main() {
    let mut tasks: HashMap<u32, Task> = HashMap::new();
    tasks.insert(1, Task { id: 1, title: \"Ship Ember\".into(), done: false });
    tasks.insert(2, Task { id: 2, title: \"Write docs\".into(), done: true });

    let total = tasks.len();
    let finished = tasks.values().filter(|t| t.done).count();
    println!(\"{finished}/{total} tasks done\");
}
";

const TYPED: &str = "
fn summary(tasks: &HashMap<u32, Task>) -> String {
    let mut titles: Vec<_> = tasks.values().map(|t| t.title.as_str()).collect();
    titles.sort();
    format!(\"{} tasks: {}\", titles.len(), titles.join(\", \"))
}
";

impl Recorder {
    /// Create the demo project on disk and build the script.
    pub fn new(dir: PathBuf, theme_names: &[String]) -> (Recorder, PathBuf) {
        let project = dir.join("demo-project");
        let _ = std::fs::create_dir_all(project.join("src"));
        let _ = std::fs::write(project.join("src").join("main.rs"), MAIN_RS);
        let _ = std::fs::write(project.join("Cargo.toml"), "[package]\nname = \"tasks\"\nversion = \"0.1.0\"\nedition = \"2021\"\n");
        let _ = std::fs::write(project.join("README.md"), "# tasks\n\nA tiny task tracker.\n");
        let _ = std::fs::create_dir_all(dir.join("frames"));

        let mut s: Vec<Act> = Vec::new();
        s.push(Act::Mark("start".into()));
        s.push(Act::Wait(8));
        // 1. type a function
        s.push(Act::Mark("typing".into()));
        let chars: Vec<char> = TYPED.chars().collect();
        for chunk in chars.chunks(3) {
            s.push(Act::Type(chunk.iter().collect()));
        }
        s.push(Act::Wait(6));
        // 2. save
        s.push(Act::Mark("save".into()));
        s.push(Act::Save);
        s.push(Act::Wait(14));
        // 3. palette -> theme
        s.push(Act::Mark("palette".into()));
        s.push(Act::PaletteOpen);
        s.push(Act::Wait(5));
        for q in ["t", "th", "the", "them", "theme"] {
            s.push(Act::PaletteQuery(q.into()));
            s.push(Act::Wait(1));
        }
        s.push(Act::Wait(6));
        s.push(Act::PaletteClose);
        // 4. cycle themes
        for (i, n) in theme_names.iter().enumerate() {
            s.push(Act::Mark(format!("theme:{n}")));
            s.push(Act::Theme(i));
            s.push(Act::Wait(7));
        }
        s.push(Act::Mark("theme:end".into()));
        s.push(Act::Theme(0));
        s.push(Act::Wait(4));
        // 5. terminal
        s.push(Act::Mark("terminal".into()));
        s.push(Act::ShowBottom(Bottom::Terminal));
        s.push(Act::Wait(10));
        s.push(Act::TermRun("rustc --version; cargo --version".into()));
        for _ in 0..22 {
            s.push(Act::SleepMs(90));
            s.push(Act::Wait(1));
        }
        s.push(Act::HideBottom);
        s.push(Act::Wait(3));
        // 6. debugger
        s.push(Act::Mark("debug".into()));
        s.push(Act::Breakpoint(15));
        s.push(Act::Wait(8));
        s.push(Act::DebugStart);
        for _ in 0..26 {
            s.push(Act::SleepMs(80));
            s.push(Act::Wait(1));
        }
        s.push(Act::Wait(12));
        s.push(Act::StepOver);
        for _ in 0..12 {
            s.push(Act::SleepMs(80));
            s.push(Act::Wait(1));
        }
        s.push(Act::Wait(10));
        s.push(Act::Cont);
        for _ in 0..10 {
            s.push(Act::SleepMs(80));
            s.push(Act::Wait(1));
        }
        s.push(Act::Mark("end".into()));
        s.push(Act::Wait(8));
        s.push(Act::Quit);

        (Recorder { dir, script: s, pos: 0, frame: 0, settle: 0, waiting: false, wait_left: 0, marks: vec![] }, project)
    }
}

impl App {
    /// Called once per UI frame while recording.
    pub fn record_step(&mut self, ctx: &egui::Context) {
        let Some(mut rec) = self.rec.take() else { return };
        ctx.request_repaint();

        // Save a captured frame when it arrives.
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
            let path = rec.dir.join("frames").join(format!("f{:05}.png", rec.frame));
            let _ = image::save_buffer(&path, &raw, img.size[0] as u32, img.size[1] as u32, image::ColorType::Rgba8);
            rec.frame += 1;
            rec.waiting = false;
        }

        if rec.waiting {
            self.rec = Some(rec);
            return;
        }
        if rec.settle > 0 {
            rec.settle -= 1;
            if rec.settle == 0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                rec.waiting = true;
            }
            self.rec = Some(rec);
            return;
        }

        // Run script acts until one wants a frame captured.
        loop {
            if rec.wait_left > 0 {
                rec.wait_left -= 1;
                rec.settle = 3;
                break;
            }
            let Some(act) = rec.script.get(rec.pos) else {
                let marks: Vec<String> = rec.marks.iter().map(|(f, n)| format!("{f} {n}")).collect();
                let _ = std::fs::write(rec.dir.join("marks.txt"), marks.join("\n"));
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            };
            rec.pos += 1;
            let mut capture = true;
            match act {
                Act::Mark(n) => {
                    rec.marks.push((rec.frame, n.clone()));
                    capture = false;
                }
                Act::Wait(n) => {
                    rec.wait_left = n.saturating_sub(1);
                    rec.settle = 3;
                    break;
                }
                Act::SleepMs(ms) => {
                    std::thread::sleep(std::time::Duration::from_millis(*ms));
                    capture = false;
                }
                Act::Type(t) => {
                    let idx = self.active;
                    if let Some(d) = self.docs.get_mut(idx) {
                        d.text.push_str(t);
                        d.touch();
                        let n = d.text.chars().count();
                        d.select = Some((n, n));
                        d.want_focus = true;
                    }
                }
                Act::Save => self.save_doc(self.active, true),
                Act::PaletteOpen => self.palette = Some(Palette { mode: PaletteMode::Commands, query: String::new(), sel: 0, first: true }),
                Act::PaletteQuery(q) => {
                    if let Some(p) = self.palette.as_mut() {
                        p.query = q.clone();
                        p.first = false;
                    }
                }
                Act::PaletteClose => self.palette = None,
                Act::Theme(i) => {
                    if *i < self.themes.len() {
                        self.theme_idx = *i;
                        self.themes[*i].apply(ctx);
                    }
                }
                Act::ShowBottom(b) => {
                    self.bottom = *b;
                    self.show_bottom = true;
                    self.focus_terminal = true;
                }
                Act::HideBottom => self.show_bottom = false,
                Act::TermRun(c) => {
                    let root = self.work_dir();
                    self.terminal.ctx = Some(ctx.clone());
                    self.terminal.run(c, &root);
                }
                Act::Breakpoint(line) => {
                    let idx = self.active;
                    if let Some(d) = self.docs.get_mut(idx) {
                        d.breakpoints.insert(*line);
                    }
                }
                Act::DebugStart => {
                    std::env::set_var("EMBER_MOCK_FILE", "src/main.rs");
                    std::env::set_var("EMBER_MOCK_LINE", "16");
                    if let Ok(exe) = std::env::current_exe() {
                        if let Some(a) = self.cfg.debug_adapters.get_mut("rust") {
                            a.command = exe.display().to_string();
                            a.args = vec!["--mock-dap".into()];
                        }
                    }
                    self.dbg.program = "tasks".into();
                    self.side_tab = SideTab::Debug;
                    self.show_side = true;
                    self.debug_start();
                }
                Act::StepOver => self.dbg.step_over(),
                Act::Cont => self.dbg.cont(),
                Act::Quit => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    capture = false;
                }
            }
            if capture {
                rec.settle = 3;
                break;
            }
        }
        self.rec = Some(rec);
    }
}
