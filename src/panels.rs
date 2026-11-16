//! Main window panels: menu bar, activity bar, sidebars, bottom panel, status bar, editor area.

use crate::app::*;
use crate::dap::{DbgState, Debugger};
use crate::editor::{self, EditorEnv};
use crate::explorer::{ExplorerEvent, Prompt, PromptKind};
use crate::glyph::{self, Glyph};
use crate::icons::{self, Activity};
use crate::theme::{blend, Theme};
use crate::util::*;
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, FontId, Frame, Id, Layout, Margin, Rect, RichText, Sense, Stroke, TextEdit, Ui,
};

fn contrast(c: Color32) -> Color32 {
    let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if lum > 150.0 {
        Color32::from_rgb(20, 20, 24)
    } else {
        Color32::WHITE
    }
}

fn mi(ui: &mut Ui, label: &str, shortcut: &str) -> bool {
    let r = ui.add(egui::Button::new(label).shortcut_text(shortcut));
    if r.clicked() {
        ui.close_menu();
        true
    } else {
        false
    }
}

fn hint(ui: &mut Ui, theme: &Theme, text: &str) {
    Frame::none().inner_margin(Margin::symmetric(10.0, 2.0)).show(ui, |ui| {
        ui.label(RichText::new(text).size(11.5).color(theme.dim));
    });
}

fn section_header(ui: &mut Ui, theme: &Theme, title: &str, buttons: impl FnOnce(&mut Ui)) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(title).size(11.5).strong().color(theme.dim));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(4.0);
            buttons(ui);
        });
    });
    ui.add_space(2.0);
}

fn draw_vars(ui: &mut Ui, dbg: &mut Debugger, theme: &Theme, var_ref: i64, depth: usize) {
    let Some(list) = dbg.vars.get(&var_ref).cloned() else {
        ui.weak("loading…");
        return;
    };
    for v in list {
        if v.var_ref > 0 && depth < 6 {
            let title = format!("{} = {}", v.name, v.value.chars().take(60).collect::<String>());
            egui::CollapsingHeader::new(RichText::new(title).monospace().size(12.5))
                .id_salt(("var", var_ref, v.name.clone()))
                .show(ui, |ui| {
                    dbg.expand(v.var_ref);
                    draw_vars(ui, dbg, theme, v.var_ref, depth + 1);
                });
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&v.name).monospace().size(12.5).color(theme.tok(crate::theme::Tok::Variable)));
                ui.label(RichText::new(v.value.chars().take(80).collect::<String>()).monospace().size(12.5).color(theme.tok(crate::theme::Tok::Str)));
                if !v.ty.is_empty() {
                    ui.label(RichText::new(&v.ty).monospace().size(11.0).color(theme.dim));
                }
            });
        }
    }
}

impl App {
    // ------------------------------------------------------------------ menu

    pub fn menu_bar(&mut self, ctx: &egui::Context, theme: &Theme) {
        let mut cmd: Option<Cmd> = None;
        egui::TopBottomPanel::top("menubar")
            .frame(Frame::none().fill(theme.panel).inner_margin(Margin::symmetric(8.0, 2.0)).stroke(Stroke::new(1.0, theme.border)))
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    if let Some(t) = &self.logo {
                        ui.image((t.id(), vec2(18.0, 18.0)));
                    }
                    ui.menu_button("File", |ui| {
                        if mi(ui, "New File", "Ctrl+N") {
                            cmd = Some(Cmd::NewFile);
                        }
                        if mi(ui, "Open File…", "Ctrl+O") {
                            cmd = Some(Cmd::OpenFile);
                        }
                        if mi(ui, "Open Folder…", "Ctrl+Shift+O") {
                            cmd = Some(Cmd::OpenFolder);
                        }
                        ui.menu_button("Open Recent", |ui| {
                            if self.cfg.recent_folders.is_empty() {
                                ui.weak("(none)");
                            }
                            for r in &self.cfg.recent_folders {
                                if ui.button(r.display().to_string()).clicked() {
                                    cmd = Some(Cmd::OpenRecent(r.clone()));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.separator();
                        if mi(ui, "Save", "Ctrl+S") {
                            cmd = Some(Cmd::Save);
                        }
                        if mi(ui, "Save As…", "Ctrl+Shift+S") {
                            cmd = Some(Cmd::SaveAs);
                        }
                        if mi(ui, "Save All", "") {
                            cmd = Some(Cmd::SaveAll);
                        }
                        ui.separator();
                        if mi(ui, "Close Tab", "Ctrl+W") {
                            cmd = Some(Cmd::CloseTab);
                        }
                        if mi(ui, "Close Folder", "") {
                            cmd = Some(Cmd::CloseFolder);
                        }
                        ui.separator();
                        if mi(ui, "Settings", "Ctrl+,") {
                            cmd = Some(Cmd::Settings);
                        }
                        if mi(ui, "Exit", "") {
                            cmd = Some(Cmd::Quit);
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if mi(ui, "Find", "Ctrl+F") {
                            cmd = Some(Cmd::Find);
                        }
                        if mi(ui, "Replace", "Ctrl+H") {
                            cmd = Some(Cmd::Replace);
                        }
                        if mi(ui, "Find in Project", "Ctrl+Shift+F") {
                            cmd = Some(Cmd::ProjectSearch);
                        }
                        ui.separator();
                        if mi(ui, "Go to Line…", "Ctrl+G") {
                            cmd = Some(Cmd::GotoLine);
                        }
                        if mi(ui, "Go to File…", "Ctrl+P") {
                            cmd = Some(Cmd::QuickOpen);
                        }
                        ui.separator();
                        ui.weak("Ctrl+/ comment · Alt+Up/Down move line");
                        ui.weak("Shift+Alt+Down duplicate · Ctrl+Shift+K delete");
                    });
                    ui.menu_button("View", |ui| {
                        if mi(ui, "Command Palette…", "Ctrl+Shift+P") {
                            cmd = Some(Cmd::Palette);
                        }
                        ui.separator();
                        if mi(ui, "Explorer", "Ctrl+Shift+E") {
                            cmd = Some(Cmd::ShowSide(SideTab::Explorer));
                        }
                        if mi(ui, "Debug", "Ctrl+Shift+D") {
                            cmd = Some(Cmd::ShowSide(SideTab::Debug));
                        }
                        if mi(ui, "Plugins", "Ctrl+Shift+X") {
                            cmd = Some(Cmd::ShowSide(SideTab::Plugins));
                        }
                        ui.separator();
                        if mi(ui, "Toggle Sidebar", "Ctrl+B") {
                            cmd = Some(Cmd::ToggleSidebar);
                        }
                        if mi(ui, "Toggle Panel", "Ctrl+J") {
                            cmd = Some(Cmd::TogglePanel);
                        }
                        if mi(ui, "Word Wrap", "") {
                            cmd = Some(Cmd::ToggleWrap);
                        }
                        ui.separator();
                        if mi(ui, "Zoom In", "Ctrl+=") {
                            cmd = Some(Cmd::ZoomIn);
                        }
                        if mi(ui, "Zoom Out", "Ctrl+-") {
                            cmd = Some(Cmd::ZoomOut);
                        }
                        ui.separator();
                        ui.menu_button("Theme", |ui| {
                            for (i, t) in self.themes.iter().enumerate() {
                                if ui.radio(i == self.theme_idx, &t.name).clicked() {
                                    cmd = Some(Cmd::SetTheme(i));
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                    ui.menu_button("Run", |ui| {
                        if mi(ui, "Start Debugging / Continue", "F5") {
                            cmd = Some(Cmd::DebugStart);
                        }
                        if mi(ui, "Stop Debugging", "Shift+F5") {
                            cmd = Some(Cmd::DebugStop);
                        }
                        if mi(ui, "Step Over", "F10") {
                            cmd = Some(Cmd::DebugOver);
                        }
                        if mi(ui, "Step Into", "F11") {
                            cmd = Some(Cmd::DebugIn);
                        }
                        if mi(ui, "Step Out", "Shift+F11") {
                            cmd = Some(Cmd::DebugOut);
                        }
                        if mi(ui, "Toggle Breakpoint", "F9") {
                            cmd = Some(Cmd::ToggleBreakpoint);
                        }
                        ui.separator();
                        if mi(ui, "Run Current File", "Ctrl+F5") {
                            cmd = Some(Cmd::RunFile);
                        }
                    });
                    ui.menu_button("Plugins", |ui| {
                        let mut any = false;
                        for p in &self.plugins.plugins {
                            if p.enabled && !p.commands.is_empty() {
                                any = true;
                                ui.menu_button(&p.name, |ui| {
                                    for (cid, title) in &p.commands {
                                        if ui.button(title).clicked() {
                                            cmd = Some(Cmd::Plugin(p.id.clone(), cid.clone()));
                                            ui.close_menu();
                                        }
                                    }
                                });
                            }
                        }
                        if !any {
                            ui.weak("No plugin commands");
                        }
                        ui.separator();
                        if mi(ui, "Manage Plugins", "Ctrl+Shift+X") {
                            cmd = Some(Cmd::ShowSide(SideTab::Plugins));
                        }
                        if mi(ui, "Reload Plugins", "") {
                            cmd = Some(Cmd::ReloadPlugins);
                        }
                        if mi(ui, "Open Plugins Folder", "") {
                            cmd = Some(Cmd::OpenPluginsDir);
                        }
                    });
                    ui.menu_button("Help", |ui| {
                        if mi(ui, "Command Palette", "Ctrl+Shift+P") {
                            cmd = Some(Cmd::Palette);
                        }
                        if mi(ui, "About Ember IDE", "") {
                            cmd = Some(Cmd::About);
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let b = egui::Button::new(RichText::new("Search commands…  Ctrl+Shift+P").size(12.0).color(theme.dim)).min_size(vec2(240.0, 20.0));
                        if ui.add(b).clicked() {
                            cmd = Some(Cmd::Palette);
                        }
                    });
                });
            });
        if let Some(c) = cmd {
            self.exec(ctx, c);
        }
    }

    // ---------------------------------------------------------------- status

    pub fn status_bar(&mut self, ctx: &egui::Context, theme: &Theme) {
        let debugging = self.dbg.active();
        let bg = if debugging { Color32::from_rgb(214, 96, 28) } else { theme.accent };
        let fg = contrast(bg);
        let mut cmd: Option<Cmd> = None;
        let mut toggle_eol = false;
        egui::TopBottomPanel::bottom("status").exact_height(24.0).frame(Frame::none().fill(bg).inner_margin(Margin::symmetric(10.0, 0.0))).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                let txt = |s: String| RichText::new(s).size(12.0).color(fg);
                if let Some(b) = &self.git_branch {
                    ui.label(txt(format!("git: {b}")));
                }
                if debugging {
                    let s = match self.dbg.state {
                        DbgState::Starting => "Debugger starting…",
                        DbgState::Running => "Debugging · running",
                        DbgState::Stopped => "Debugging · paused",
                        DbgState::Idle => "",
                    };
                    ui.label(txt(s.to_string()));
                }
                let bps: usize = self.docs.iter().map(|d| d.breakpoints.len()).sum();
                if bps > 0 {
                    ui.label(txt(format!("{bps} breakpoint{}", if bps == 1 { "" } else { "s" })));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let btn = |ui: &mut Ui, s: String| ui.add(egui::Button::new(RichText::new(s).size(12.0).color(fg)).frame(false));
                    let st = self.presence.status();
                    if self.cfg.discord_enabled {
                        if btn(ui, format!("Discord: {st}")).clicked() {
                            cmd = Some(Cmd::Settings);
                        }
                    }
                    if btn(ui, self.themes[self.theme_idx].name.clone()).clicked() {
                        self.palette = Some(Palette { mode: PaletteMode::Commands, query: "Theme: ".into(), sel: 0, first: true });
                    }
                    if let Some(d) = self.docs.get(self.active) {
                        let lang = self.hl.language_name(&d.ext());
                        ui.label(txt(lang));
                        if btn(ui, if d.crlf { "CRLF".into() } else { "LF".into() }).clicked() {
                            toggle_eol = true;
                        }
                        ui.label(txt("UTF-8".into()));
                        ui.label(txt(format!("Spaces: {}", self.cfg.tab_width)));
                        let sel = if d.sel_len > 0 { format!(" ({} selected)", d.sel_len) } else { String::new() };
                        if btn(ui, format!("Ln {}, Col {}{}", d.cursor.0, d.cursor.1, sel)).clicked() {
                            cmd = Some(Cmd::GotoLine);
                        }
                    }
                });
            });
        });
        if toggle_eol {
            if let Some(d) = self.docs.get_mut(self.active) {
                d.crlf = !d.crlf;
                d.dirty = true;
            }
        }
        if let Some(c) = cmd {
            self.exec(ctx, c);
        }
    }

    // -------------------------------------------------------------- activity

    pub fn activity_bar(&mut self, ctx: &egui::Context, theme: &Theme) {
        let bg = blend(theme.panel, if theme.dark { Color32::BLACK } else { Color32::WHITE }, 0.22);
        let mut cmd: Option<Cmd> = None;
        egui::SidePanel::left("activity").exact_width(46.0).resizable(false).frame(Frame::none().fill(bg)).show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(4.0);
            let items = [
                (Activity::Explorer, SideTab::Explorer, "Explorer (Ctrl+Shift+E)"),
                (Activity::Search, SideTab::Search, "Search (Ctrl+Shift+F)"),
                (Activity::Debug, SideTab::Debug, "Run and Debug (Ctrl+Shift+D)"),
                (Activity::Plugins, SideTab::Plugins, "Plugins (Ctrl+Shift+X)"),
            ];
            for (kind, tab, tip) in items {
                let (rect, resp) = ui.allocate_exact_size(vec2(46.0, 46.0), Sense::click());
                let active = self.side_tab == tab && self.show_side;
                let col = if active || resp.hovered() { theme.fg } else { theme.dim };
                if active {
                    ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(2.5, rect.height())), 0.0, theme.accent);
                }
                icons::paint_activity(ui.painter(), rect, kind, col);
                if kind == Activity::Debug && self.dbg.active() {
                    ui.painter().circle_filled(rect.center() + vec2(9.0, -9.0), 4.0, Color32::from_rgb(255, 160, 40));
                }
                if resp.on_hover_text(tip).clicked() {
                    cmd = Some(Cmd::ShowSide(tab));
                }
            }
            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.add_space(4.0);
                let (rect, resp) = ui.allocate_exact_size(vec2(46.0, 46.0), Sense::click());
                let col = if resp.hovered() { theme.fg } else { theme.dim };
                icons::paint_activity(ui.painter(), rect, Activity::Settings, col);
                if resp.on_hover_text("Settings (Ctrl+,)").clicked() {
                    cmd = Some(Cmd::Settings);
                }
            });
        });
        if let Some(c) = cmd {
            self.exec(ctx, c);
        }
    }

    // --------------------------------------------------------------- sidebar

    pub fn side_panel(&mut self, ctx: &egui::Context, theme: &Theme) {
        egui::SidePanel::left("side")
            .resizable(true)
            .default_width(270.0)
            .width_range(170.0..=620.0)
            .frame(Frame::none().fill(theme.panel).stroke(Stroke::new(1.0, theme.border)))
            .show(ctx, |ui| match self.side_tab {
                SideTab::Explorer => self.ui_explorer(ui, theme),
                SideTab::Search => self.ui_search(ui, theme),
                SideTab::Debug => self.ui_debug(ui, theme),
                SideTab::Plugins => self.ui_plugins(ui, theme),
            });
    }

    fn ui_explorer(&mut self, ui: &mut Ui, theme: &Theme) {
        let mut cmd: Option<Cmd> = None;
        let mut new_prompt: Option<PromptKind> = None;
        let mut refresh = false;
        section_header(ui, theme, "EXPLORER", |ui| {
            if glyph::button(ui, Glyph::Refresh, "Refresh", None).clicked() {
                refresh = true;
            }
            if glyph::button(ui, Glyph::NewFolder, "New Folder", None).clicked() {
                new_prompt = Some(PromptKind::NewFolder);
            }
            if glyph::button(ui, Glyph::NewFile, "New File", None).clicked() {
                new_prompt = Some(PromptKind::NewFile);
            }
        });
        if refresh {
            self.explorer.refresh();
            self.project_files = None;
        }
        if let Some(kind) = new_prompt {
            if let Some(root) = self.explorer.root.clone() {
                let target = match &self.explorer.selected {
                    Some(s) if s.is_dir() => s.clone(),
                    Some(s) => s.parent().map(|p| p.to_path_buf()).unwrap_or(root),
                    None => root,
                };
                self.explorer.prompt = Some(Prompt { kind, target, input: String::new(), first_frame: true });
            }
        }
        if self.explorer.root.is_none() {
            ui.add_space(14.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No folder opened").color(theme.dim));
                ui.add_space(8.0);
                if ui.add_sized([180.0, 30.0], egui::Button::new("Open Folder…")).clicked() {
                    cmd = Some(Cmd::OpenFolder);
                }
            });
            if !self.cfg.recent_folders.is_empty() {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new("RECENT").size(11.0).strong().color(theme.dim));
                });
                for r in self.cfg.recent_folders.clone() {
                    if ui.add(egui::Button::new(RichText::new(file_name(&r)).color(theme.accent)).frame(false)).on_hover_text(r.display().to_string()).clicked() {
                        cmd = Some(Cmd::OpenRecent(r));
                    }
                }
            }
        } else {
            let name = self.explorer.root.as_ref().map(|r| file_name(r)).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(RichText::new(name.to_uppercase()).size(12.0).strong());
            });
            ui.add_space(2.0);
            let mut evs: Vec<ExplorerEvent> = Vec::new();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                self.explorer.show(ui, theme, &mut evs);
            });
            self.handle_explorer_events(evs);
        }
        if let Some(c) = cmd {
            let ctx = ui.ctx().clone();
            self.exec(&ctx, c);
        }
    }

    fn ui_search(&mut self, ui: &mut Ui, theme: &Theme) {
        section_header(ui, theme, "SEARCH", |_| {});
        let mut run = false;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let w = (ui.available_width() - 110.0).max(80.0);
            let r = ui.add(TextEdit::singleline(&mut self.search.query).hint_text("Search in project").desired_width(w).id(Id::new("search_query")));
            if self.focus_search {
                r.request_focus();
                self.focus_search = false;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                run = true;
            }
            if glyph::toggle(ui, "Aa", self.search.case_sensitive, "Match case", theme.accent).clicked() {
                self.search.case_sensitive = !self.search.case_sensitive;
                run = true;
            }
            if glyph::toggle(ui, ".*", self.search.use_regex, "Regular expression", theme.accent).clicked() {
                self.search.use_regex = !self.search.use_regex;
                run = true;
            }
        });
        if run {
            let root = self.work_dir();
            self.search.start(&root);
        }
        if let Some(e) = &self.search.error {
            ui.colored_label(Color32::from_rgb(240, 90, 80), e);
        }
        // group hits by file
        let mut groups: Vec<(std::path::PathBuf, usize, usize)> = Vec::new();
        for (i, h) in self.search.results.iter().enumerate() {
            match groups.last_mut() {
                Some(g) if g.0 == h.path => g.2 = i + 1,
                _ => groups.push((h.path.clone(), i, i + 1)),
            }
        }
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            let msg = if self.search.running {
                format!("Searching… {} results", self.search.results.len())
            } else if self.search.query.is_empty() {
                String::new()
            } else {
                format!("{} results in {} files{}", self.search.results.len(), groups.len(), if self.search.truncated { " (truncated)" } else { "" })
            };
            ui.label(RichText::new(msg).size(12.0).color(theme.dim));
        });
        let mut open: Option<(std::path::PathBuf, usize)> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (path, s, e) in &groups {
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
                icons::paint_file(ui.painter(), Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(18.0, 16.0)), &file_name(path));
                ui.painter().text(pos2(rect.left() + 32.0, rect.center().y), Align2::LEFT_CENTER, format!("{}  {}", file_name(path), e - s), FontId::proportional(13.0), theme.fg);
                for h in &self.search.results[*s..*e] {
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 0.0, theme.hover());
                    }
                    let max_chars = ((r.width() - 56.0) / 6.8) as usize;
                    let t: String = h.text.chars().take(max_chars.max(8)).collect();
                    ui.painter().text(pos2(r.left() + 28.0, r.center().y), Align2::LEFT_CENTER, format!("{}", h.line + 1), FontId::monospace(11.5), theme.dim);
                    ui.painter().text(pos2(r.left() + 56.0, r.center().y), Align2::LEFT_CENTER, t, FontId::monospace(12.0), theme.fg);
                    if resp.on_hover_text(self.rel(&h.path)).clicked() {
                        open = Some((h.path.clone(), h.line));
                    }
                }
            }
        });
        if let Some((p, l)) = open {
            self.open_at(&p, l);
        }
    }

    fn ui_debug(&mut self, ui: &mut Ui, theme: &Theme) {
        let mut cmd: Option<Cmd> = None;
        section_header(ui, theme, "RUN AND DEBUG", |_| {});
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if !self.dbg.active() {
                if ui.add(egui::Button::new(RichText::new("▶  Start Debugging").strong()).fill(theme.accent.linear_multiply(0.35))).on_hover_text("F5").clicked() {
                    cmd = Some(Cmd::DebugStart);
                }
                if ui.button("Run").on_hover_text("Run current file (Ctrl+F5)").clicked() {
                    cmd = Some(Cmd::RunFile);
                }
            } else {
                let green = Color32::from_rgb(90, 200, 120);
                let stopped = self.dbg.state == DbgState::Stopped;
                if glyph::button(ui, Glyph::Play, "Continue (F5)", Some(green)).clicked() {
                    cmd = Some(Cmd::DebugContinue);
                }
                if glyph::button(ui, Glyph::StepOver, "Step Over (F10)", Some(theme.accent)).clicked() && stopped {
                    cmd = Some(Cmd::DebugOver);
                }
                if glyph::button(ui, Glyph::StepIn, "Step Into (F11)", Some(theme.accent)).clicked() && stopped {
                    cmd = Some(Cmd::DebugIn);
                }
                if glyph::button(ui, Glyph::StepOut, "Step Out (Shift+F11)", Some(theme.accent)).clicked() && stopped {
                    cmd = Some(Cmd::DebugOut);
                }
                if glyph::button(ui, Glyph::Pause, "Pause", Some(Color32::from_rgb(255, 190, 60))).clicked() {
                    cmd = Some(Cmd::DebugPause);
                }
                if glyph::button(ui, Glyph::Stop, "Stop (Shift+F5)", Some(Color32::from_rgb(235, 80, 80))).clicked() {
                    cmd = Some(Cmd::DebugStop);
                }
            }
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let lock = self.dbg.active();
            ui.add_enabled_ui(!lock, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label("Program");
                });
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.add(TextEdit::singleline(&mut self.dbg.program).hint_text("target/debug/app (native programs)").desired_width(ui.available_width() - 10.0));
                });
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label("Arguments");
                });
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.add(TextEdit::singleline(&mut self.dbg.args).hint_text("optional").desired_width(ui.available_width() - 10.0));
                });
            });
            hint(ui, theme, "Rust / C / C++ need lldb-dap (from LLVM). Python needs `pip install debugpy`. Adapters are configurable in Settings.");
            ui.add_space(4.0);

            egui::CollapsingHeader::new(RichText::new("CALL STACK").size(11.5).strong()).default_open(true).show(ui, |ui| {
                if self.dbg.frames.is_empty() {
                    ui.weak(if self.dbg.state == DbgState::Stopped { "loading…" } else { "not paused" });
                }
                let mut pick = None;
                for (i, f) in self.dbg.frames.iter().enumerate() {
                    let loc = f.path.as_ref().map(|p| format!("{}:{}", file_name(p), f.line)).unwrap_or_default();
                    let label = format!("{}   {}", f.name, loc);
                    if ui.selectable_label(i == self.dbg.selected_frame, RichText::new(label).monospace().size(12.0)).clicked() {
                        pick = Some(i);
                    }
                }
                if let Some(i) = pick {
                    self.dbg.select_frame(i);
                }
            });
            egui::CollapsingHeader::new(RichText::new("VARIABLES").size(11.5).strong()).default_open(true).show(ui, |ui| {
                if self.dbg.scopes.is_empty() {
                    ui.weak("not paused");
                }
                for sc in self.dbg.scopes.clone() {
                    egui::CollapsingHeader::new(RichText::new(&sc.name).strong()).id_salt(("scope", sc.var_ref)).default_open(true).show(ui, |ui| {
                        self.dbg.expand(sc.var_ref);
                        draw_vars(ui, &mut self.dbg, theme, sc.var_ref, 0);
                    });
                }
            });
            egui::CollapsingHeader::new(RichText::new("BREAKPOINTS").size(11.5).strong()).default_open(true).show(ui, |ui| {
                let mut jump: Option<(std::path::PathBuf, usize)> = None;
                let mut remove: Option<(usize, usize)> = None;
                let mut any = false;
                for (di, d) in self.docs.iter().enumerate() {
                    for l in &d.breakpoints {
                        any = true;
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(12.0, 14.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 4.5, Color32::from_rgb(229, 72, 77));
                            if ui.add(egui::Button::new(format!("{}:{}", d.title(), l + 1)).frame(false)).clicked() {
                                if let Some(p) = &d.path {
                                    jump = Some((p.clone(), *l));
                                }
                            }
                            if glyph::button(ui, Glyph::Close, "Remove breakpoint", None).clicked() {
                                remove = Some((di, *l));
                            }
                        });
                    }
                }
                if !any {
                    ui.weak("Click the gutter or press F9 to add one");
                }
                if let Some((di, l)) = remove {
                    self.docs[di].breakpoints.remove(&l);
                    self.sync_breakpoints(di);
                }
                if let Some((p, l)) = jump {
                    self.open_at(&p, l);
                }
            });
        });
        if let Some(c) = cmd {
            let ctx = ui.ctx().clone();
            self.exec(&ctx, c);
        }
    }

    fn ui_plugins(&mut self, ui: &mut Ui, theme: &Theme) {
        let mut reload = false;
        let mut open_dir = false;
        section_header(ui, theme, "PLUGINS", |ui| {
            if glyph::button(ui, Glyph::Refresh, "Reload plugins", None).clicked() {
                reload = true;
            }
            if glyph::button(ui, Glyph::NewFolder, "Open plugins folder", None).clicked() {
                open_dir = true;
            }
        });
        let mut toggle: Option<(String, bool)> = None;
        let mut run: Option<(String, String)> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if self.plugins.plugins.is_empty() {
                ui.add_space(10.0);
                ui.weak("  No plugins found.");
            }
            for p in &self.plugins.plugins {
                Frame::none().fill(theme.raised()).rounding(6.0).inner_margin(Margin::same(8.0)).outer_margin(Margin::symmetric(8.0, 3.0)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let mut en = p.enabled;
                        if ui.checkbox(&mut en, "").changed() {
                            toggle = Some((p.id.clone(), en));
                        }
                        ui.label(RichText::new(&p.name).strong());
                        ui.label(RichText::new(format!("v{}", p.version)).size(11.0).color(theme.dim));
                    });
                    if !p.description.is_empty() {
                        ui.label(RichText::new(&p.description).size(12.0).color(theme.dim));
                    }
                    if let Some(e) = &p.error {
                        ui.colored_label(Color32::from_rgb(240, 90, 80), format!("error: {e}"));
                    }
                    if p.enabled {
                        for (cid, title) in &p.commands {
                            if ui.add(egui::Button::new(RichText::new(title).size(12.0).color(theme.accent)).frame(false)).clicked() {
                                run = Some((p.id.clone(), cid.clone()));
                            }
                        }
                    }
                });
            }
            ui.add_space(8.0);
            hint(ui, theme, "Plugins are folders with plugin.toml and main.rhai. Put yours in the plugins folder and press reload.");
        });
        if let Some((id, en)) = toggle {
            self.cfg.disabled_plugins.retain(|x| x != &id);
            if !en {
                self.cfg.disabled_plugins.push(id);
            }
            self.cfg.save();
            reload = true;
        }
        let ctx = ui.ctx().clone();
        if let Some((p, c)) = run {
            self.exec(&ctx, Cmd::Plugin(p, c));
        }
        if reload {
            self.exec(&ctx, Cmd::ReloadPlugins);
        }
        if open_dir {
            self.exec(&ctx, Cmd::OpenPluginsDir);
        }
    }

    // ---------------------------------------------------------------- bottom

    pub fn bottom_panel(&mut self, ctx: &egui::Context, theme: &Theme) {
        let mut close = false;
        egui::TopBottomPanel::bottom("bottom")
            .resizable(true)
            .default_height(230.0)
            .height_range(90.0..=640.0)
            .frame(Frame::none().fill(theme.editor).stroke(Stroke::new(1.0, theme.border)))
            .show(ctx, |ui| {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    for (b, name) in [(Bottom::Terminal, "TERMINAL"), (Bottom::DebugConsole, "DEBUG CONSOLE"), (Bottom::Output, "OUTPUT")] {
                        let on = self.bottom == b;
                        let t = RichText::new(name).size(11.5).strong().color(if on { theme.fg } else { theme.dim });
                        let r = ui.add(egui::Button::new(t).frame(false));
                        if on {
                            ui.painter().line_segment([r.rect.left_bottom(), r.rect.right_bottom()], Stroke::new(2.0, theme.accent));
                        }
                        if r.clicked() {
                            self.bottom = b;
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if glyph::button(ui, Glyph::Close, "Close panel", None).clicked() {
                            close = true;
                        }
                        match self.bottom {
                            Bottom::Terminal => {
                                if glyph::button(ui, Glyph::Clear, "Clear", None).clicked() {
                                    self.terminal.clear();
                                }
                                if self.terminal.running() && glyph::button(ui, Glyph::Stop, "Kill shell", Some(Color32::from_rgb(235, 80, 80))).clicked() {
                                    self.terminal.kill();
                                }
                            }
                            Bottom::DebugConsole => {
                                if glyph::button(ui, Glyph::Clear, "Clear", None).clicked() {
                                    self.dbg.console.clear();
                                }
                            }
                            Bottom::Output => {
                                if glyph::button(ui, Glyph::Clear, "Clear", None).clicked() {
                                    self.plugins.log.clear();
                                }
                            }
                        }
                    });
                });
                ui.separator();
                match self.bottom {
                    Bottom::Terminal => self.ui_terminal(ui, theme),
                    Bottom::DebugConsole => self.ui_debug_console(ui, theme),
                    Bottom::Output => {
                        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                            ui.add_space(2.0);
                            for l in &self.plugins.log {
                                ui.label(RichText::new(l).monospace().size(12.5));
                            }
                            if self.plugins.log.is_empty() {
                                ui.weak("  Output from Ember and your plugins appears here (saves, runs, debugger events, log(...) in Rhai).");
                            }
                        });
                    }
                }
            });
        if close {
            self.show_bottom = false;
        }
    }

    fn ui_terminal(&mut self, ui: &mut Ui, theme: &Theme) {
        use egui::{Align2, Event, EventFilter, Sense};
        let cwd = self.work_dir();
        self.terminal.ctx = Some(ui.ctx().clone());
        if !self.terminal.ever_started {
            self.terminal.ensure_started(&cwd);
        }
        let font = FontId::monospace(13.5);
        let (cw, ch) = ui.fonts(|f| (f.glyph_width(&font, 'M'), f.row_height(&font)));
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let inner = rect.shrink2(egui::Vec2::new(10.0, 4.0));
        self.terminal.resize((inner.height() / ch).floor() as u16, (inner.width() / cw).floor() as u16);

        if resp.clicked() || self.focus_terminal {
            resp.request_focus();
            self.focus_terminal = false;
        }
        let focused = resp.has_focus();
        self.terminal_focus = focused;
        if focused {
            ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true }));
            let (app_cursor, bracketed) = {
                let p = self.terminal.parser.lock().unwrap();
                (p.screen().application_cursor(), p.screen().bracketed_paste())
            };
            let events = ui.input(|i| i.events.clone());
            let mut out: Vec<u8> = Vec::new();
            let mut enter = false;
            for e in events {
                match e {
                    Event::Text(t) => out.extend(t.as_bytes()),
                    Event::Paste(t) => {
                        if bracketed {
                            out.extend(b"\x1b[200~");
                            out.extend(t.as_bytes());
                            out.extend(b"\x1b[201~");
                        } else {
                            out.extend(t.replace("\r\n", "\r").replace('\n', "\r").as_bytes());
                        }
                    }
                    Event::Copy => out.push(0x03),
                    Event::Cut => out.push(0x18),
                    Event::Key { key, pressed: true, modifiers, .. } => {
                        if key == egui::Key::Enter {
                            enter = true;
                        }
                        out.extend(term_key_bytes(key, modifiers, app_cursor));
                    }
                    _ => {}
                }
            }
            if !out.is_empty() {
                if self.terminal.running() {
                    self.terminal.write(&out);
                    if let Ok(mut p) = self.terminal.parser.lock() {
                        p.set_scrollback(0);
                    }
                } else if enter {
                    self.terminal.ensure_started(&cwd);
                }
            }
        }
        if resp.hovered() {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            let lines = (dy / ch).round() as i64;
            if lines != 0 {
                if let Ok(mut p) = self.terminal.parser.lock() {
                    let cur = p.screen().scrollback() as i64;
                    p.set_scrollback((cur + lines).max(0) as usize);
                }
            }
        }

        let painter = ui.painter_at(rect);
        let bg = theme.editor;
        painter.rect_filled(rect, 0.0, bg);
        {
            let parser = self.terminal.parser.lock().unwrap();
            let screen = parser.screen();
            let (rows, cols) = screen.size();
            for r in 0..rows {
                let y = inner.top() + r as f32 * ch;
                let mut c = 0u16;
                while c < cols {
                    let start = c;
                    let (fg, cbg) = cell_colors(screen.cell(r, c), theme.fg, bg);
                    let mut text = String::new();
                    while c < cols {
                        let Some(cell) = screen.cell(r, c) else { break };
                        if cell_colors(Some(cell), theme.fg, bg) != (fg, cbg) {
                            break;
                        }
                        if !cell.is_wide_continuation() {
                            let t = cell.contents();
                            text.push_str(if t.is_empty() { " " } else { &t });
                        }
                        c += 1;
                    }
                    if c == start {
                        c += 1;
                        continue;
                    }
                    let x = inner.left() + start as f32 * cw;
                    if cbg != bg {
                        painter.rect_filled(Rect::from_min_size(egui::Pos2::new(x, y), egui::Vec2::new((c - start) as f32 * cw + 0.5, ch)), 0.0, cbg);
                    }
                    if text.trim().is_empty() {
                        continue;
                    }
                    painter.text(egui::Pos2::new(x, y), Align2::LEFT_TOP, text, font.clone(), fg);
                }
            }
            if !screen.hide_cursor() && screen.scrollback() == 0 {
                let (cr, cc) = screen.cursor_position();
                let r = Rect::from_min_size(egui::Pos2::new(inner.left() + cc as f32 * cw, inner.top() + cr as f32 * ch), egui::Vec2::new(cw, ch));
                if focused {
                    painter.rect_filled(r, 0.0, theme.accent.gamma_multiply(0.75));
                } else {
                    painter.rect_stroke(r, 0.0, Stroke::new(1.0, theme.dim));
                }
            }
        }
        if !self.terminal.running() {
            let msg = self.terminal.error.clone().unwrap_or_else(|| "Shell closed - press Enter (or click here) to start a new one".into());
            painter.text(egui::Pos2::new(rect.center().x, rect.bottom() - 18.0), Align2::CENTER_CENTER, msg, FontId::proportional(12.5), theme.dim);
            if resp.clicked() {
                self.terminal.ensure_started(&cwd);
            }
        }
    }

    fn ui_debug_console(&mut self, ui: &mut Ui, theme: &Theme) {
        let row_h = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
        let avail = ui.available_height() - 34.0;
        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(avail.max(40.0)).stick_to_bottom(true).show_rows(ui, row_h, self.dbg.console.len(), |ui, range| {
            for i in range {
                ui.add(egui::Label::new(RichText::new(&self.dbg.console[i]).monospace().size(12.5).color(theme.fg)).wrap_mode(egui::TextWrapMode::Extend));
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new("›").monospace().strong().color(theme.accent));
            let r = ui.add(TextEdit::singleline(&mut self.dbg.eval_input).font(FontId::monospace(13.0)).frame(false).hint_text("evaluate expression (while paused)…").desired_width(f32::INFINITY));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let e = std::mem::take(&mut self.dbg.eval_input);
                if !e.trim().is_empty() {
                    self.dbg.evaluate(e.trim());
                }
                r.request_focus();
            }
        });
    }

    // --------------------------------------------------------------- central

    pub fn central(&mut self, ctx: &egui::Context, theme: &Theme) {
        egui::CentralPanel::default().frame(Frame::none().fill(theme.editor)).show(ctx, |ui| {
            if self.docs.is_empty() {
                self.welcome(ui, theme);
                return;
            }
            self.tab_bar(ui, theme);
            self.breadcrumbs(ui, theme);
            if self.find.open {
                self.find_bar(ui, theme);
            }
            self.editor_area(ui, theme);
        });
    }

    fn tab_bar(&mut self, ui: &mut Ui, theme: &Theme) {
        let strip = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), 34.0));
        ui.painter().rect_filled(strip, 0.0, theme.panel);
        ui.painter().line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, theme.border));
        let mut activate: Option<usize> = None;
        let mut close: Option<usize> = None;
        let mut close_others: Option<usize> = None;
        let scroll_to = self.last_active != self.active;
        egui::ScrollArea::horizontal().id_salt("tabs").auto_shrink([false, true]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for i in 0..self.docs.len() {
                    let d = &self.docs[i];
                    let title = d.title();
                    let is_active = i == self.active;
                    let tcol = if is_active { theme.fg } else { theme.dim };
                    let galley = ui.fonts(|f| f.layout_no_wrap(title.clone(), FontId::proportional(13.0), tcol));
                    let w = 12.0 + 18.0 + 6.0 + galley.size().x + 10.0 + 20.0 + 6.0;
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
                    let p = ui.painter();
                    if is_active {
                        p.rect_filled(rect, 0.0, theme.editor);
                        p.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)), 0.0, theme.accent);
                    } else if resp.hovered() {
                        p.rect_filled(rect, 0.0, theme.hover());
                    }
                    p.line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0, theme.border));
                    icons::paint_file(p, Rect::from_center_size(pos2(rect.left() + 21.0, rect.center().y), vec2(18.0, 16.0)), &title);
                    p.galley(pos2(rect.left() + 12.0 + 18.0 + 6.0, rect.center().y - galley.size().y / 2.0), galley, tcol);
                    let cc = pos2(rect.right() - 16.0, rect.center().y);
                    let over_close = resp.hover_pos().map(|hp| (hp - cc).length() < 9.0).unwrap_or(false);
                    if over_close {
                        p.circle_filled(cc, 9.0, theme.hover().linear_multiply(1.6));
                    }
                    if d.dirty && !over_close {
                        p.circle_filled(cc, 4.0, theme.fg);
                    } else if is_active || resp.hovered() || over_close {
                        glyph::paint(p, cc, Glyph::Close, if over_close { theme.fg } else { theme.dim });
                    }
                    if is_active && scroll_to {
                        resp.scroll_to_me(None);
                    }
                    if resp.clicked() {
                        if over_close {
                            close = Some(i);
                        } else {
                            activate = Some(i);
                        }
                    }
                    if resp.middle_clicked() {
                        close = Some(i);
                    }
                    let path = d.path.clone();
                    resp.context_menu(|ui| {
                        if ui.button("Close").clicked() {
                            close = Some(i);
                            ui.close_menu();
                        }
                        if ui.button("Close Others").clicked() {
                            close_others = Some(i);
                            ui.close_menu();
                        }
                        if let Some(pth) = &path {
                            ui.separator();
                            if ui.button("Copy Path").clicked() {
                                ui.ctx().copy_text(pth.display().to_string());
                                ui.close_menu();
                            }
                            if ui.button("Reveal in File Manager").clicked() {
                                open_in_os(pth.parent().unwrap_or(pth));
                                ui.close_menu();
                            }
                        }
                    });
                }
            });
        });
        self.last_active = self.active;
        if let Some(i) = activate {
            self.active = i;
            self.docs[i].want_focus = true;
        }
        if let Some(keep) = close_others {
            // Keep the chosen tab and anything with unsaved changes.
            let keep_id = self.docs[keep].id;
            self.docs.retain(|d| d.id == keep_id || d.dirty);
            self.active = self.docs.iter().position(|d| d.id == keep_id).unwrap_or(0);
        }
        if let Some(i) = close {
            self.close_tab(i, false);
        }
    }

    fn breadcrumbs(&mut self, ui: &mut Ui, theme: &Theme) {
        let Some(d) = self.docs.get(self.active) else { return };
        let text = match &d.path {
            Some(p) => self.rel(p).replace('\\', "  /  ").replace('/', "  /  ").replace("  /    /  ", "  /  "),
            None => "unsaved".to_string(),
        };
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new(text).size(12.0).color(theme.dim));
        });
        ui.add_space(2.0);
    }

    fn editor_area(&mut self, ui: &mut Ui, theme: &Theme) {
        let idx = self.active;
        if self.docs[idx].view.is_some() {
            let title = self.docs[idx].title();
            if let Some(v) = self.docs[idx].view.as_mut() {
                crate::viewer::show(ui, v, &title, theme);
            }
            return;
        }
        let exec = self.dbg.current_location();
        let doc_path = self.docs[idx].path.clone();
        let exec_line = match (&exec, &doc_path) {
            (Some((p, l)), Some(dp)) if p == dp => Some(l.saturating_sub(1)),
            _ => None,
        };
        let empty: [(usize, usize); 0] = [];
        let matches: &[(usize, usize)] = if self.find.open { &self.find.matches } else { &empty };
        let current = if self.find.open && !self.find.matches.is_empty() { Some(self.find.current) } else { None };
        let env = EditorEnv { theme, hl: &self.hl, cfg: &self.cfg, matches, current_match: current, exec_line };
        let out = editor::show(ui, &mut self.docs[idx], &env);
        if out.toggled_bp.is_some() {
            self.sync_breakpoints(idx);
        }
    }

    fn welcome(&mut self, ui: &mut Ui, theme: &Theme) {
        let mut cmd: Option<Cmd> = None;
        let top = (ui.available_height() * 0.12).max(10.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(top);
                if let Some(t) = &self.logo {
                    ui.image((t.id(), vec2(96.0, 96.0)));
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Ember IDE").size(34.0).strong());
                ui.label(RichText::new("A fast, hackable code editor written in Rust").size(14.0).color(theme.dim));
                ui.add_space(18.0);
                for (label, c) in [("Open Folder…", Cmd::OpenFolder), ("New File", Cmd::NewFile), ("Go to File…", Cmd::QuickOpen), ("Command Palette", Cmd::Palette)] {
                    if ui.add_sized([230.0, 32.0], egui::Button::new(label)).clicked() {
                        cmd = Some(c);
                    }
                }
                if !self.cfg.recent_folders.is_empty() {
                    ui.add_space(16.0);
                    ui.label(RichText::new("Recent").size(12.0).strong().color(theme.dim));
                    for r in self.cfg.recent_folders.clone() {
                        if ui.add(egui::Button::new(RichText::new(r.display().to_string()).color(theme.accent)).frame(false)).clicked() {
                            cmd = Some(Cmd::OpenRecent(r));
                        }
                    }
                }
                ui.add_space(20.0);
                ui.allocate_ui_with_layout(vec2(340.0, 0.0), Layout::top_down(Align::LEFT), |ui| {
                    egui::Grid::new("welcome_keys").num_columns(2).spacing([24.0, 5.0]).show(ui, |ui| {
                        for (a, k) in [
                            ("Command palette", "Ctrl+Shift+P"),
                            ("Go to file", "Ctrl+P"),
                            ("Find / Replace", "Ctrl+F / Ctrl+H"),
                            ("Search project", "Ctrl+Shift+F"),
                            ("Toggle terminal", "Ctrl+J"),
                            ("Start debugging", "F5"),
                            ("Run file", "Ctrl+F5"),
                        ] {
                            ui.label(RichText::new(a).color(theme.dim));
                            ui.label(RichText::new(k).monospace().size(12.5));
                            ui.end_row();
                        }
                    });
                });
            });
        });
        if let Some(c) = cmd {
            let ctx = ui.ctx().clone();
            self.exec(&ctx, c);
        }
    }
}


fn ansi_color(i: u8) -> Color32 {
    const P: [(u8, u8, u8); 16] = [
        (0, 0, 0), (205, 49, 49), (13, 188, 121), (229, 229, 16), (36, 114, 200), (188, 63, 188), (17, 168, 205), (229, 229, 229),
        (102, 102, 102), (241, 76, 76), (35, 209, 139), (245, 245, 67), (59, 142, 234), (214, 112, 214), (41, 184, 219), (255, 255, 255),
    ];
    match i {
        0..=15 => {
            let (r, g, b) = P[i as usize];
            Color32::from_rgb(r, g, b)
        }
        16..=231 => {
            let n = i - 16;
            let l = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Color32::from_rgb(l(n / 36), l((n / 6) % 6), l(n % 6))
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            Color32::from_rgb(v, v, v)
        }
    }
}

fn vt_color(c: vt100::Color, default: Color32) -> Color32 {
    match c {
        vt100::Color::Default => default,
        vt100::Color::Idx(i) => ansi_color(i),
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

fn cell_colors(cell: Option<&vt100::Cell>, dfg: Color32, dbg: Color32) -> (Color32, Color32) {
    let Some(c) = cell else { return (dfg, dbg) };
    let (mut fg, mut bg) = (vt_color(c.fgcolor(), dfg), vt_color(c.bgcolor(), dbg));
    if c.inverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    (fg, bg)
}

/// Bytes a terminal program expects for a non-text key press.
fn term_key_bytes(key: egui::Key, m: egui::Modifiers, app_cursor: bool) -> Vec<u8> {
    use egui::Key::*;
    let ctrl = m.ctrl || m.command;
    let csi = |c: char| -> Vec<u8> {
        let mod_n = 1 + (m.shift as u8) + 2 * (m.alt as u8) + 4 * (ctrl as u8);
        if mod_n > 1 {
            format!("\x1b[1;{mod_n}{c}").into_bytes()
        } else if app_cursor {
            format!("\x1bO{c}").into_bytes()
        } else {
            format!("\x1b[{c}").into_bytes()
        }
    };
    match key {
        Enter => b"\r".to_vec(),
        Backspace => {
            if ctrl {
                vec![0x17]
            } else {
                vec![0x7f]
            }
        }
        Tab => {
            if m.shift {
                b"\x1b[Z".to_vec()
            } else {
                b"\t".to_vec()
            }
        }
        Escape => vec![0x1b],
        ArrowUp => csi('A'),
        ArrowDown => csi('B'),
        ArrowRight => csi('C'),
        ArrowLeft => csi('D'),
        Home => csi('H'),
        End => csi('F'),
        Delete => b"\x1b[3~".to_vec(),
        Insert => b"\x1b[2~".to_vec(),
        PageUp => b"\x1b[5~".to_vec(),
        PageDown => b"\x1b[6~".to_vec(),
        // Ctrl+letter -> control character (C, X and V arrive as Copy/Cut/Paste events instead)
        k if ctrl && !m.alt && !matches!(k, C | X | V) => {
            let name = k.name();
            match name.chars().next() {
                Some(ch) if name.len() == 1 && ch.is_ascii_alphabetic() => vec![ch.to_ascii_uppercase() as u8 - b'A' + 1],
                _ => vec![],
            }
        }
        _ => vec![],
    }
}
