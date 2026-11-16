//! Floating UI: command palette / quick open, find bar, settings, confirmations, toasts.

use crate::app::*;
use crate::explorer::ExplorerEvent;
use crate::glyph::{self, Glyph};
use crate::theme::Theme;
use crate::util::*;
use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, FontId, Frame, Id, Key, Layout, Margin, Modifiers, Order, RichText, Sense, Stroke, TextEdit, Ui};

impl App {
    pub fn dialogs(&mut self, ctx: &egui::Context, theme: &Theme) {
        let mut evs: Vec<ExplorerEvent> = Vec::new();
        self.explorer.show_dialogs(ctx, &mut evs);
        self.handle_explorer_events(evs);
        self.palette_ui(ctx, theme);
        self.settings_ui(ctx, theme);
        self.about_ui(ctx, theme);
        self.confirm_ui(ctx);
        self.toasts_ui(ctx, theme);
    }

    // --------------------------------------------------------------- palette

    fn palette_ui(&mut self, ctx: &egui::Context, theme: &Theme) {
        let Some(mut pal) = self.palette.take() else { return };
        let mut close = false;
        let mut run: Option<Cmd> = None;

        let items: Vec<(String, String, Cmd)> = match pal.mode {
            PaletteMode::Commands => {
                let q = pal.query.trim().to_string();
                let mut v: Vec<(i32, (String, String, Cmd))> =
                    self.palette_commands().into_iter().filter_map(|it| fuzzy_score(&q, &it.0).map(|s| (s, it))).collect();
                v.sort_by(|a, b| b.0.cmp(&a.0));
                v.into_iter().map(|x| x.1).take(60).collect()
            }
            PaletteMode::Files => {
                let q = pal.query.trim().to_string();
                let mut v: Vec<(i32, std::path::PathBuf)> = self
                    .project_file_list()
                    .iter()
                    .filter_map(|p| fuzzy_score(&q, &self.rel(p)).map(|s| (s, p.clone())))
                    .collect();
                v.sort_by(|a, b| b.0.cmp(&a.0));
                v.into_iter()
                    .take(60)
                    .map(|(_, p)| {
                        let rel = self.rel(&p);
                        let dir = p.parent().map(|d| self.rel(d)).unwrap_or_default();
                        let _ = rel;
                        (file_name(&p), dir, Cmd::OpenPath(p))
                    })
                    .collect()
            }
            PaletteMode::Goto => vec![],
        };
        pal.sel = pal.sel.min(items.len().saturating_sub(1));

        let (up, down, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if up {
            pal.sel = pal.sel.saturating_sub(1);
        }
        if down && pal.sel + 1 < items.len() {
            pal.sel += 1;
        }

        let hint = match pal.mode {
            PaletteMode::Commands => "Type a command…",
            PaletteMode::Files => {
                if self.explorer.root.is_none() {
                    "Open a folder first to search its files"
                } else {
                    "Type a file name…"
                }
            }
            PaletteMode::Goto => "Line number…",
        };
        let total_lines = self.docs.get(self.active).map(|d| d.line_count()).unwrap_or(1);

        let area = egui::Area::new(Id::new("palette")).anchor(Align2::CENTER_TOP, [0.0, 64.0]).order(Order::Foreground).show(ctx, |ui| {
            Frame::popup(ui.style()).fill(theme.raised()).stroke(Stroke::new(1.0, theme.border)).rounding(10.0).inner_margin(Margin::same(8.0)).show(ui, |ui| {
                ui.set_width(580.0);
                let r = ui.add(TextEdit::singleline(&mut pal.query).hint_text(hint).font(FontId::proportional(15.0)).desired_width(f32::INFINITY).margin(vec2(8.0, 6.0)));
                if pal.first {
                    r.request_focus();
                    pal.first = false;
                }
                if r.changed() {
                    pal.sel = 0;
                }
                ui.add_space(4.0);
                if pal.mode == PaletteMode::Goto {
                    ui.label(RichText::new(format!("Go to line 1 – {total_lines}")).color(theme.dim));
                    return;
                }
                egui::ScrollArea::vertical().max_height(380.0).auto_shrink([true, true]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, (label, detail, cmd)) in items.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                        if i == pal.sel {
                            ui.painter().rect_filled(rect, 5.0, theme.selection);
                            if up || down {
                                resp.scroll_to_me(None);
                            }
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, 5.0, theme.hover());
                        }
                        ui.painter().text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(13.5), theme.fg);
                        if !detail.is_empty() {
                            ui.painter().text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, detail, FontId::proportional(12.0), theme.dim);
                        }
                        if resp.clicked() {
                            run = Some(cmd.clone());
                        }
                    }
                    if items.is_empty() {
                        ui.add_space(6.0);
                        ui.label(RichText::new("No matches").color(theme.dim));
                    }
                });
            });
        });

        if enter {
            match pal.mode {
                PaletteMode::Goto => {
                    if let Ok(n) = pal.query.trim().parse::<usize>() {
                        if let Some(d) = self.docs.get_mut(self.active) {
                            d.goto = Some(n.saturating_sub(1).min(total_lines.saturating_sub(1)));
                            d.want_focus = true;
                        }
                    }
                    close = true;
                }
                _ => {
                    if let Some(it) = items.get(pal.sel) {
                        run = Some(it.2.clone());
                    }
                }
            }
        }
        if esc {
            close = true;
        }
        let clicked_outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.pointer_interact_pos().map(|p| !area.response.rect.contains(p)).unwrap_or(false);
        if clicked_outside {
            close = true;
        }
        if run.is_some() {
            close = true;
        }
        if !close {
            self.palette = Some(pal);
        }
        if let Some(c) = run {
            self.exec(ctx, c);
        }
    }

    // ------------------------------------------------------------- find bar

    pub fn find_bar(&mut self, ui: &mut Ui, theme: &Theme) {
        let mut close = false;
        Frame::none().fill(theme.panel).inner_margin(Margin::symmetric(10.0, 6.0)).stroke(Stroke::new(1.0, theme.border)).show(ui, |ui| {
            ui.horizontal(|ui| {
                if glyph::button(ui, if self.find.replace_mode { Glyph::Down } else { Glyph::Collapse }, "Toggle replace", None).clicked() {
                    self.find.replace_mode = !self.find.replace_mode;
                }
                let id = Id::new("find_query");
                let r = ui.add(TextEdit::singleline(&mut self.find.query).id(id).hint_text("Find").desired_width(240.0));
                if self.find.focus {
                    r.request_focus();
                    self.find.focus = false;
                }
                let mut changed = r.changed();
                if r.has_focus() {
                    let (sh_enter, enter, esc) = ui.input_mut(|i| {
                        (i.consume_key(Modifiers::SHIFT, Key::Enter), i.consume_key(Modifiers::NONE, Key::Enter), i.consume_key(Modifiers::NONE, Key::Escape))
                    });
                    if sh_enter {
                        self.find_step(false);
                    } else if enter {
                        self.find_step(true);
                    }
                    if esc {
                        close = true;
                    }
                }
                if glyph::toggle(ui, "Aa", self.find.case, "Match case", theme.accent).clicked() {
                    self.find.case = !self.find.case;
                    changed = true;
                }
                if glyph::toggle(ui, ".*", self.find.regex, "Regular expression", theme.accent).clicked() {
                    self.find.regex = !self.find.regex;
                    changed = true;
                }
                if changed {
                    self.find.current = 0;
                    self.find.key = None;
                    self.update_find();
                    self.find_select_current();
                }
                let n = self.find.matches.len();
                let msg = if let Some(e) = &self.find.error {
                    e.clone()
                } else if self.find.query.is_empty() {
                    String::new()
                } else if n == 0 {
                    "No results".to_string()
                } else {
                    format!("{} of {}", self.find.current + 1, n)
                };
                ui.label(RichText::new(msg).size(12.0).color(theme.dim));
                if glyph::button(ui, Glyph::Up, "Previous (Shift+F3)", None).clicked() {
                    self.find_step(false);
                }
                if glyph::button(ui, Glyph::Down, "Next (F3)", None).clicked() {
                    self.find_step(true);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if glyph::button(ui, Glyph::Close, "Close (Esc)", None).clicked() {
                        close = true;
                    }
                });
            });
            if self.find.replace_mode {
                ui.horizontal(|ui| {
                    ui.add_space(30.0);
                    ui.add(TextEdit::singleline(&mut self.find.replace).hint_text("Replace").desired_width(240.0));
                    if ui.button("Replace").clicked() {
                        self.replace_current();
                    }
                    if ui.button("Replace All").clicked() {
                        self.replace_all();
                    }
                });
            }
        });
        if close {
            self.find.open = false;
            if let Some(d) = self.docs.get_mut(self.active) {
                d.want_focus = true;
                d.select = None;
            }
        }
    }

    // ------------------------------------------------------------- settings

    fn settings_ui(&mut self, ctx: &egui::Context, theme: &Theme) {
        if !self.show_settings {
            return;
        }
        let before = format!("{:?}", self.cfg);
        let mut open = true;
        let mut new_theme: Option<usize> = None;
        let mut cmd: Option<Cmd> = None;
        egui::Window::new("Settings").open(&mut open).default_size([560.0, 560.0]).resizable(true).collapsible(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let h = |ui: &mut Ui, t: &str| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(t).size(15.0).strong().color(theme.accent));
                    ui.add_space(2.0);
                };
                ui.spacing_mut().slider_width = 220.0;
                h(ui, "Appearance");
                egui::Grid::new("set_appearance").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                    ui.label("Theme");
                    egui::ComboBox::from_id_salt("theme_combo").selected_text(self.themes[self.theme_idx].name.clone()).width(220.0).show_ui(ui, |ui| {
                        for (i, t) in self.themes.iter().enumerate() {
                            if ui.selectable_label(i == self.theme_idx, &t.name).clicked() {
                                new_theme = Some(i);
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Editor font size");
                    ui.add(egui::Slider::new(&mut self.cfg.font_size, 9.0..=32.0).step_by(0.5));
                    ui.end_row();
                    ui.label("Tab width");
                    ui.add(egui::DragValue::new(&mut self.cfg.tab_width).range(1..=8));
                    ui.end_row();
                });
                ui.checkbox(&mut self.cfg.word_wrap, "Word wrap");
                ui.checkbox(&mut self.cfg.auto_save, "Auto save after 1 second of inactivity");
                ui.checkbox(&mut self.cfg.auto_pair, "Auto-close brackets and quotes");

                h(ui, "Discord Rich Presence");
                ui.checkbox(&mut self.cfg.discord_enabled, "Show what I'm working on in Discord");
                ui.add_enabled_ui(self.cfg.discord_enabled, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Application ID");
                        ui.add(TextEdit::singleline(&mut self.cfg.discord_app_id).hint_text("leave empty to use the built-in one").desired_width(260.0));
                    });
                    ui.checkbox(&mut self.cfg.discord_show_file, "Show file and folder names");
                    ui.label(RichText::new(format!("Status: {}", self.presence.status())).size(12.0).color(theme.dim));
                    let id = self.cfg.discord_app_id.trim();
                    if !id.is_empty() && !id.chars().all(|c| c.is_ascii_digit()) {
                        ui.label(RichText::new("The Application ID should be a long number (digits only).").size(12.0).color(egui::Color32::from_rgb(240, 170, 60)));
                    }
                    if ui.button("Open Discord developer portal").clicked() {
                        crate::util::open_url("https://discord.com/developers/applications");
                    }
                    ui.label(RichText::new("Works automatically while the Discord desktop app is running - no login needed. Optional: use your own app. Create a free one at discord.com/developers/applications, copy its Application ID here, and upload an image named “ember” under Rich Presence > Art Assets.").size(11.5).color(theme.dim));
                });

                h(ui, "Debug adapters");
                ui.label(RichText::new("Command and arguments for each language's DAP adapter.").size(12.0).color(theme.dim));
                let keys: Vec<String> = self.cfg.debug_adapters.keys().cloned().collect();
                egui::Grid::new("set_adapters").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
                    for k in keys {
                        ui.label(RichText::new(&k).monospace());
                        if let Some(a) = self.cfg.debug_adapters.get_mut(&k) {
                            ui.add_sized([130.0, 22.0], TextEdit::singleline(&mut a.command));
                            let buf = self.adapter_args_buf.entry(k.clone()).or_insert_with(|| a.args.join(" "));
                            if ui.add_sized([220.0, 22.0], TextEdit::singleline(buf)).changed() {
                                a.args = buf.split_whitespace().map(|s| s.to_string()).collect();
                            }
                        }
                        ui.end_row();
                    }
                });

                h(ui, "Folders");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Config").clicked() {
                        cmd = Some(Cmd::OpenConfigDir);
                    }
                    if ui.button("Plugins").clicked() {
                        cmd = Some(Cmd::OpenPluginsDir);
                    }
                    if ui.button("Custom themes").clicked() {
                        cmd = Some(Cmd::OpenThemesDir);
                    }
                });
            });
        });
        self.show_settings = open;
        if let Some(i) = new_theme {
            self.set_theme(ctx, i);
        }
        if let Some(c) = cmd {
            self.exec(ctx, c);
        }
        if format!("{:?}", self.cfg) != before {
            self.cfg.save();
            self.presence.configure(self.cfg.discord_enabled, &crate::config::effective_discord_id(&self.cfg.discord_app_id));
        }
    }

    fn about_ui(&mut self, ctx: &egui::Context, theme: &Theme) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        egui::Window::new("About Ember IDE").open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                if let Some(t) = &self.logo {
                    ui.image((t.id(), vec2(84.0, 84.0)));
                }
                ui.label(RichText::new("Ember IDE").size(24.0).strong());
                ui.label(RichText::new(format!("version {}", env!("CARGO_PKG_VERSION"))).color(theme.dim));
                ui.add_space(6.0);
                ui.label("Written in Rust with egui. MIT licensed.");
                ui.label(RichText::new("Plugins (Rhai) · DAP debugger · 12 themes · Discord Rich Presence").size(12.0).color(theme.dim));
            });
        });
        self.show_about = open;
    }

    fn confirm_ui(&mut self, ctx: &egui::Context) {
        let Some(conf) = self.confirm.clone() else { return };
        let mut choice: Option<u8> = None; // 0 save, 1 discard, 2 cancel
        let (title, msg, save_label, discard_label) = match &conf {
            Confirm::CloseTab(i) => (
                "Unsaved changes",
                format!("Save changes to “{}” before closing?", self.docs.get(*i).map(|d| d.title()).unwrap_or_default()),
                "Save",
                "Don't Save",
            ),
            Confirm::Quit => {
                let n = self.docs.iter().filter(|d| d.dirty).count();
                ("Unsaved changes", format!("{n} file(s) have unsaved changes."), "Save All & Quit", "Quit Without Saving")
            }
        };
        egui::Window::new(title).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(msg);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(save_label).clicked() {
                    choice = Some(0);
                }
                if ui.button(discard_label).clicked() {
                    choice = Some(1);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(2);
                }
            });
        });
        match (choice, conf) {
            (Some(0), Confirm::CloseTab(i)) => {
                self.save_doc(i, true);
                if i < self.docs.len() && !self.docs[i].dirty {
                    self.close_tab(i, true);
                }
                self.confirm = None;
            }
            (Some(1), Confirm::CloseTab(i)) => {
                self.close_tab(i, true);
                self.confirm = None;
            }
            (Some(0), Confirm::Quit) => {
                for i in 0..self.docs.len() {
                    if self.docs[i].dirty {
                        self.save_doc(i, true);
                    }
                }
                if self.docs.iter().all(|d| !d.dirty) {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                self.confirm = None;
            }
            (Some(1), Confirm::Quit) => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                self.confirm = None;
            }
            (Some(_), _) => self.confirm = None,
            _ => {}
        }
    }

    fn toasts_ui(&mut self, ctx: &egui::Context, theme: &Theme) {
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(Id::new("toasts")).anchor(Align2::RIGHT_BOTTOM, [-16.0, -40.0]).order(Order::Tooltip).interactable(false).show(ctx, |ui| {
            for t in &self.toasts {
                Frame::popup(ui.style()).fill(theme.raised()).stroke(Stroke::new(1.0, theme.accent.linear_multiply(0.7))).rounding(8.0).inner_margin(Margin::symmetric(12.0, 8.0)).show(ui, |ui| {
                    ui.set_max_width(380.0);
                    ui.label(RichText::new(&t.text).color(theme.fg));
                });
                ui.add_space(4.0);
            }
        });
    }
}
