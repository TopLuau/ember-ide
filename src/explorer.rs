//! File explorer tree with icons, context menu and create / rename / delete prompts.

use crate::icons;
use crate::theme::Theme;
use crate::util::{file_name, is_ignored_dir, open_in_os};
use eframe::egui::{self, pos2, vec2, Align2, FontId, Sense, Ui};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone)]
struct Entry {
    path: PathBuf,
    name: String,
    is_dir: bool,
}

#[derive(Clone, PartialEq)]
pub enum PromptKind {
    NewFile,
    NewFolder,
    Rename,
}

pub struct Prompt {
    pub kind: PromptKind,
    pub target: PathBuf, // parent dir for New*, the item for Rename
    pub input: String,
    pub first_frame: bool,
}

pub enum ExplorerEvent {
    Open(PathBuf),
    Created(PathBuf),
    Deleted(PathBuf),
    Renamed(PathBuf, PathBuf),
    Error(String),
}

#[derive(Default)]
pub struct Explorer {
    pub root: Option<PathBuf>,
    expanded: HashSet<PathBuf>,
    cache: HashMap<PathBuf, Vec<Entry>>,
    pub selected: Option<PathBuf>,
    pub prompt: Option<Prompt>,
    pub confirm_delete: Option<PathBuf>,
}

impl Explorer {
    pub fn set_root(&mut self, root: PathBuf) {
        self.expanded.clear();
        self.cache.clear();
        self.expanded.insert(root.clone());
        self.root = Some(root);
        self.selected = None;
    }

    pub fn refresh(&mut self) {
        self.cache.clear();
    }

    fn entries(&mut self, dir: &Path) -> Vec<Entry> {
        if let Some(c) = self.cache.get(dir) {
            return c.clone();
        }
        let mut v: Vec<Entry> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name == ".git" {
                            return None;
                        }
                        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        Some(Entry { path: e.path(), name, is_dir })
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        self.cache.insert(dir.to_path_buf(), v.clone());
        v
    }

    pub fn reveal(&mut self, path: &Path) {
        let Some(root) = self.root.clone() else { return };
        let mut p = path.parent();
        while let Some(d) = p {
            if !d.starts_with(&root) {
                break;
            }
            self.expanded.insert(d.to_path_buf());
            p = d.parent();
        }
        self.selected = Some(path.to_path_buf());
    }

    pub fn show(&mut self, ui: &mut Ui, theme: &Theme, events: &mut Vec<ExplorerEvent>) {
        let Some(root) = self.root.clone() else { return };
        self.draw_dir(ui, theme, &root, 0, events);
    }

    fn draw_dir(&mut self, ui: &mut Ui, theme: &Theme, dir: &Path, depth: usize, events: &mut Vec<ExplorerEvent>) {
        for e in self.entries(dir) {
            let h = 22.0;
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
            let selected = self.selected.as_deref() == Some(e.path.as_path());
            let p = ui.painter();
            if selected {
                p.rect_filled(rect, 0.0, theme.selection.linear_multiply(0.8));
            } else if resp.hovered() {
                p.rect_filled(rect, 0.0, theme.hover());
            }
            let x0 = rect.left() + 8.0 + depth as f32 * 14.0;
            let open = self.expanded.contains(&e.path);
            let dim = e.is_dir && is_ignored_dir(&e.name);
            if e.is_dir {
                icons::paint_chevron(p, pos2(x0 + 4.0, rect.center().y), open, theme.dim);
                icons::paint_folder(p, egui::Rect::from_center_size(pos2(x0 + 21.0, rect.center().y), vec2(18.0, 16.0)), &e.name, open);
            } else {
                icons::paint_file(p, egui::Rect::from_center_size(pos2(x0 + 21.0, rect.center().y), vec2(18.0, 16.0)), &e.name);
            }
            let col = if dim { theme.dim } else { theme.fg };
            p.text(pos2(x0 + 36.0, rect.center().y), Align2::LEFT_CENTER, &e.name, FontId::proportional(13.5), col);

            if resp.clicked() {
                self.selected = Some(e.path.clone());
                if e.is_dir {
                    if open {
                        self.expanded.remove(&e.path);
                    } else {
                        self.expanded.insert(e.path.clone());
                    }
                } else {
                    events.push(ExplorerEvent::Open(e.path.clone()));
                }
            }
            resp.context_menu(|ui| {
                let parent = if e.is_dir { e.path.clone() } else { e.path.parent().map(|p| p.to_path_buf()).unwrap_or_default() };
                if ui.button("New File…").clicked() {
                    self.prompt = Some(Prompt { kind: PromptKind::NewFile, target: parent.clone(), input: String::new(), first_frame: true });
                    ui.close_menu();
                }
                if ui.button("New Folder…").clicked() {
                    self.prompt = Some(Prompt { kind: PromptKind::NewFolder, target: parent, input: String::new(), first_frame: true });
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Rename…").clicked() {
                    self.prompt = Some(Prompt { kind: PromptKind::Rename, target: e.path.clone(), input: e.name.clone(), first_frame: true });
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    self.confirm_delete = Some(e.path.clone());
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Reveal in File Manager").clicked() {
                    open_in_os(if e.is_dir { &e.path } else { e.path.parent().unwrap_or(&e.path) });
                    ui.close_menu();
                }
                if ui.button("Copy Path").clicked() {
                    ui.ctx().copy_text(e.path.display().to_string());
                    ui.close_menu();
                }
            });
            if e.is_dir && open {
                self.draw_dir(ui, theme, &e.path, depth + 1, events);
            }
        }
    }

    /// Modal dialogs for the create / rename / delete flows.
    pub fn show_dialogs(&mut self, ctx: &egui::Context, events: &mut Vec<ExplorerEvent>) {
        let mut close_prompt = false;
        let mut apply: Option<(PromptKind, PathBuf, String)> = None;
        if let Some(pr) = &mut self.prompt {
            let title = match pr.kind {
                PromptKind::NewFile => "New File",
                PromptKind::NewFolder => "New Folder",
                PromptKind::Rename => "Rename",
            };
            egui::Window::new(title).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut pr.input).desired_width(300.0));
                if pr.first_frame {
                    r.request_focus();
                    pr.first_frame = false;
                }
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if (ui.button("OK").clicked() || enter) && !pr.input.trim().is_empty() {
                        apply = Some((pr.kind.clone(), pr.target.clone(), pr.input.trim().to_string()));
                    }
                    if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close_prompt = true;
                    }
                });
            });
        }
        if let Some((kind, target, name)) = apply {
            close_prompt = true;
            match kind {
                PromptKind::NewFile => {
                    let p = target.join(&name);
                    if let Some(parent) = p.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    match std::fs::OpenOptions::new().write(true).create_new(true).open(&p) {
                        Ok(_) => events.push(ExplorerEvent::Created(p)),
                        Err(e) => events.push(ExplorerEvent::Error(format!("Could not create {name}: {e}"))),
                    }
                }
                PromptKind::NewFolder => {
                    let p = target.join(&name);
                    match std::fs::create_dir_all(&p) {
                        Ok(_) => {
                            self.expanded.insert(target.clone());
                            events.push(ExplorerEvent::Created(p));
                        }
                        Err(e) => events.push(ExplorerEvent::Error(format!("Could not create {name}: {e}"))),
                    }
                }
                PromptKind::Rename => {
                    let new = target.parent().map(|p| p.join(&name)).unwrap_or_else(|| PathBuf::from(&name));
                    match std::fs::rename(&target, &new) {
                        Ok(_) => events.push(ExplorerEvent::Renamed(target, new)),
                        Err(e) => events.push(ExplorerEvent::Error(format!("Rename failed: {e}"))),
                    }
                }
            }
            self.refresh();
        }
        if close_prompt {
            self.prompt = None;
        }

        let mut do_delete: Option<PathBuf> = None;
        let mut cancel = false;
        if let Some(p) = &self.confirm_delete {
            egui::Window::new("Delete").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(format!("Permanently delete “{}”?", file_name(p)));
                ui.weak("This cannot be undone.");
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        do_delete = Some(p.clone());
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        }
        if let Some(p) = do_delete {
            let r = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
            match r {
                Ok(_) => events.push(ExplorerEvent::Deleted(p)),
                Err(e) => events.push(ExplorerEvent::Error(format!("Delete failed: {e}"))),
            }
            self.confirm_delete = None;
            self.refresh();
        }
        if cancel {
            self.confirm_delete = None;
        }
    }
}
