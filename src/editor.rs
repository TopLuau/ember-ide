//! The code editor widget: gutter, breakpoints, highlighting, smart editing commands.

use crate::config::Config;
use crate::doc::Doc;
use crate::highlight::Highlighter;
use crate::theme::Theme;
use crate::util::{char_to_byte, comment_prefix, hash_of};
use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, Context, FontId, Id, Key, Margin, Modifiers, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder,
};
use std::sync::Arc;

pub struct EditorEnv<'a> {
    pub theme: &'a Theme,
    pub hl: &'a Highlighter,
    pub cfg: &'a Config,
    /// Find matches as char ranges (sorted).
    pub matches: &'a [(usize, usize)],
    pub current_match: Option<usize>,
    /// 0-based line the debugger is stopped on, if in this file.
    pub exec_line: Option<usize>,
}

#[derive(Default)]
pub struct EditorOut {
    pub toggled_bp: Option<usize>,
    pub focused: bool,
}

fn line_start(text: &str, byte: usize) -> usize {
    text[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn line_end(text: &str, byte: usize) -> usize {
    text[byte..].find('\n').map(|i| byte + i).unwrap_or(text.len())
}

fn set_cursor(ctx: &Context, id: Id, mut state: egui::text_edit::TextEditState, anchor: usize, primary: usize) {
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(anchor), CCursor::new(primary))));
    state.store(ctx, id);
}

/// Replace the full lines touched by `[blo, bhi]` using `f` on every line. Returns char range of new block.
fn map_lines(text: &mut String, blo: usize, bhi: usize, f: impl Fn(&str) -> String) -> (usize, usize) {
    let s = line_start(text, blo);
    let e = if bhi > blo && text[..bhi].ends_with('\n') { bhi - 1 } else { line_end(text, bhi) };
    let block = text[s..e].to_string();
    let new_block = block.split('\n').map(f).collect::<Vec<_>>().join("\n");
    text.replace_range(s..e, &new_block);
    let a = text[..s].chars().count();
    (a, a + new_block.chars().count())
}

fn editor_keys(ctx: &Context, id: Id, doc: &mut Doc, cfg: &Config) {
    if !ctx.memory(|m| m.has_focus(id)) {
        return;
    }
    let Some(state) = egui::TextEdit::load_state(ctx, id) else { return };
    let Some(range) = state.cursor.char_range() else { return };
    let (pa, sa) = (range.primary.index, range.secondary.index);
    let (lo, hi) = (pa.min(sa), pa.max(sa));
    let blo = char_to_byte(&doc.text, lo);
    let bhi = char_to_byte(&doc.text, hi);
    let ext = doc.ext();
    let indent_unit = " ".repeat(cfg.tab_width.max(1));

    let tab_s = ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab));
    let tab = !tab_s && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
    let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
    let comment = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Slash));
    let dup = ctx.input_mut(|i| i.consume_key(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowDown));
    let del_line = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::K));
    let move_down = !dup && ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowDown));
    let move_up = ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowUp));

    let mut typed: Option<char> = None;
    if cfg.auto_pair {
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Text(t) if typed.is_none() && t.chars().count() == 1 => {
                    let c = t.chars().next().unwrap();
                    if matches!(c, '(' | '[' | '{' | ')' | ']' | '}' | '"' | '\'' | '`') {
                        typed = Some(c);
                        false
                    } else {
                        true
                    }
                }
                _ => true,
            })
        });
    }

    let multiline_sel = doc.text[blo..bhi].contains('\n');
    let mut result: Option<(usize, usize)> = None; // (anchor, primary) in chars

    if tab && !multiline_sel {
        let ls = line_start(&doc.text, blo);
        let col = doc.text[ls..blo].chars().count();
        let n = cfg.tab_width.max(1) - col % cfg.tab_width.max(1);
        doc.text.replace_range(blo..bhi, &" ".repeat(n));
        result = Some((lo + n, lo + n));
    } else if tab {
        let (a, b) = map_lines(&mut doc.text, blo, bhi, |l| format!("{indent_unit}{l}"));
        result = Some((a, b));
    } else if tab_s {
        let tw = cfg.tab_width.max(1);
        let (a, b) = map_lines(&mut doc.text, blo, bhi, |l| {
            if let Some(r) = l.strip_prefix('\t') {
                r.to_string()
            } else {
                let n = l.chars().take(tw).take_while(|c| *c == ' ').count();
                l[n..].to_string()
            }
        });
        result = Some((a, b));
    } else if enter {
        let ls = line_start(&doc.text, blo);
        let base: String = doc.text[ls..blo].chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        let prev = doc.text[..blo].chars().next_back();
        let next = doc.text[bhi..].chars().next();
        let opens = matches!(prev, Some('{') | Some('(') | Some('[')) || (ext == "py" && prev == Some(':'));
        let closes = matches!((prev, next), (Some('{'), Some('}')) | (Some('('), Some(')')) | (Some('['), Some(']')));
        let ins = if opens && closes {
            format!("\n{base}{indent_unit}\n{base}")
        } else if opens {
            format!("\n{base}{indent_unit}")
        } else {
            format!("\n{base}")
        };
        let caret = lo + if opens && closes { 1 + base.chars().count() + indent_unit.len() } else { ins.chars().count() };
        doc.text.replace_range(blo..bhi, &ins);
        result = Some((caret, caret));
    } else if comment {
        if let Some(prefix) = comment_prefix(&ext) {
            let s = line_start(&doc.text, blo);
            let e = if bhi > blo && doc.text[..bhi].ends_with('\n') { bhi - 1 } else { line_end(&doc.text, bhi) };
            let block = doc.text[s..e].to_string();
            let non_empty: Vec<&str> = block.split('\n').filter(|l| !l.trim().is_empty()).collect();
            let all_commented = !non_empty.is_empty() && non_empty.iter().all(|l| l.trim_start().starts_with(prefix));
            let min_indent = non_empty.iter().map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
            let prefix = prefix.to_string();
            let (a, b) = map_lines(&mut doc.text, blo, bhi, |l| {
                if l.trim().is_empty() {
                    l.to_string()
                } else if all_commented {
                    let ind = l.len() - l.trim_start().len();
                    let rest = &l[ind + prefix.len()..];
                    format!("{}{}", &l[..ind], rest.strip_prefix(' ').unwrap_or(rest))
                } else {
                    format!("{}{} {}", &l[..min_indent], prefix, &l[min_indent..])
                }
            });
            result = Some((a, b));
        }
    } else if dup {
        let s = line_start(&doc.text, blo);
        let e = if bhi > blo && doc.text[..bhi].ends_with('\n') { bhi - 1 } else { line_end(&doc.text, bhi) };
        let block = doc.text[s..e].to_string();
        doc.text.insert_str(e, &format!("\n{block}"));
        let a = doc.text[..e + 1].chars().count();
        result = Some((a, a + block.chars().count()));
    } else if del_line {
        let s = line_start(&doc.text, blo);
        let e = line_end(&doc.text, bhi);
        let (rs, re) = if e < doc.text.len() { (s, e + 1) } else { (s.saturating_sub(if s > 0 { 1 } else { 0 }), e) };
        doc.text.replace_range(rs..re, "");
        let c = doc.text[..rs.min(doc.text.len())].chars().count();
        result = Some((c, c));
    } else if move_down || move_up {
        let s = line_start(&doc.text, blo);
        let e = if bhi > blo && doc.text[..bhi].ends_with('\n') { bhi - 1 } else { line_end(&doc.text, bhi) };
        let block = doc.text[s..e].to_string();
        if move_down && e < doc.text.len() {
            let ne = line_end(&doc.text, e + 1);
            let next = doc.text[e + 1..ne].to_string();
            doc.text.replace_range(s..ne, &format!("{next}\n{block}"));
            let a = doc.text[..s + next.len() + 1].chars().count();
            result = Some((a, a + block.chars().count()));
        } else if move_up && s > 0 {
            let ps = line_start(&doc.text, s - 1);
            let prev = doc.text[ps..s - 1].to_string();
            doc.text.replace_range(ps..e, &format!("{block}\n{prev}"));
            let a = doc.text[..ps].chars().count();
            result = Some((a, a + block.chars().count()));
        }
    } else if let Some(c) = typed {
        let next = doc.text[bhi..].chars().next();
        let prev = doc.text[..blo].chars().next_back();
        let has_sel = bhi > blo;
        let closer = match c {
            '(' => Some(')'),
            '[' => Some(']'),
            '{' => Some('}'),
            '"' | '`' => Some(c),
            '\'' if !matches!(ext.as_str(), "rs" | "ml" | "lisp" | "clj" | "el") => Some(c),
            _ => None,
        };
        if matches!(c, ')' | ']' | '}') || (closer == Some(c) && !has_sel && next == Some(c)) {
            if next == Some(c) && !has_sel {
                result = Some((lo + 1, lo + 1));
            } else {
                doc.text.replace_range(blo..bhi, &c.to_string());
                result = Some((lo + 1, lo + 1));
            }
        } else if let Some(cl) = closer {
            let word_before = prev.map(|p| p.is_alphanumeric() || p == '_').unwrap_or(false);
            let word_after = next.map(|n| n.is_alphanumeric() || n == '_').unwrap_or(false);
            let is_quote = matches!(c, '"' | '\'' | '`');
            if !has_sel && (word_after || (is_quote && word_before)) {
                doc.text.replace_range(blo..bhi, &c.to_string());
                result = Some((lo + 1, lo + 1));
            } else if has_sel {
                let inner = doc.text[blo..bhi].to_string();
                doc.text.replace_range(blo..bhi, &format!("{c}{inner}{cl}"));
                result = Some((lo + 1, hi + 1));
            } else {
                doc.text.replace_range(blo..bhi, &format!("{c}{cl}"));
                result = Some((lo + 1, lo + 1));
            }
        } else {
            doc.text.replace_range(blo..bhi, &c.to_string());
            result = Some((lo + 1, lo + 1));
        }
    }

    if let Some((a, p)) = result {
        doc.touch();
        set_cursor(ctx, id, state, a, p);
    }
}

fn char_index_of_line(text: &str, line: usize) -> usize {
    let mut idx = 0;
    for (n, l) in text.split('\n').enumerate() {
        if n == line {
            return idx;
        }
        idx += l.chars().count() + 1;
    }
    text.chars().count()
}

pub fn show(ui: &mut Ui, doc: &mut Doc, env: &EditorEnv) -> EditorOut {
    let theme = env.theme;
    let font = FontId::monospace(env.cfg.font_size);
    let (row_h, char_w) = ui.fonts(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
    let digits = doc.line_count().to_string().len().max(3);
    let gutter_w = 26.0 + digits as f32 * char_w + 14.0;
    let full = ui.available_rect_before_wrap();
    let gutter_rect = Rect::from_min_size(full.min, vec2(gutter_w, full.height()));
    let ed_rect = Rect::from_min_max(pos2(full.min.x + gutter_w, full.min.y), full.max);
    let id = Id::new(("editor", doc.id));

    editor_keys(ui.ctx(), id, doc, env.cfg);

    let ext = doc.ext();
    let wrap = env.cfg.word_wrap;
    let mut out = EditorOut::default();
    let mut info: Option<(Arc<egui::Galley>, egui::Pos2)> = None;

    ui.allocate_new_ui(UiBuilder::new().max_rect(ed_rect), |ui| {
        ScrollArea::both().auto_shrink([false, false]).id_salt(("scroll", doc.id)).show(ui, |ui| {
            let avail_w = ui.available_width();
            let min_rows = ((ui.available_height() / row_h) as usize).max(1);
            let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| -> Arc<egui::Galley> {
                let ww = if wrap { wrap_width } else { f32::INFINITY };
                let key = hash_of(&(hash_of(text), theme.name.as_str(), env.cfg.font_size.to_bits(), ww.to_bits()));
                let job = match &doc.job {
                    Some((k, j)) if *k == key => j.clone(),
                    _ => {
                        let j = env.hl.build_job(&mut doc.hl, &ext, text, theme, &font, ww);
                        doc.job = Some((key, j.clone()));
                        j
                    }
                };
                ui.fonts(|f| f.layout_job(job))
            };
            let output = egui::TextEdit::multiline(&mut doc.text)
                .id(id)
                .font(font.clone())
                .frame(false)
                .lock_focus(true)
                .desired_width(if wrap { avail_w } else { f32::INFINITY })
                .desired_rows(min_rows)
                .margin(Margin::symmetric(8.0, 4.0))
                .layouter(&mut layouter)
                .show(ui);
            if output.response.changed() {
                doc.touch();
            }
            out.focused = output.response.has_focus();
            let galley = output.galley.clone();
            let gpos = output.galley_pos;
            let clip = ui.clip_rect();
            let painter = ui.painter();

            if let Some(cr) = &output.cursor_range {
                let p = cr.primary.pcursor;
                doc.cursor = (p.paragraph + 1, p.offset + 1);
                doc.sel_len = cr.primary.ccursor.index.abs_diff(cr.secondary.ccursor.index);
                if cr.is_empty() {
                    if let Some(row) = galley.rows.get(cr.primary.rcursor.row) {
                        let r = row.rect.translate(gpos.to_vec2());
                        painter.rect_filled(Rect::from_x_y_ranges(clip.x_range(), r.y_range()), 0.0, theme.accent.linear_multiply(0.07));
                    }
                }
            }

            // Pending jump / selection requests (search results, debugger, find, plugins).
            let mut jump: Option<(usize, usize)> = None;
            if let Some(line) = doc.goto.take() {
                let c = char_index_of_line(&doc.text, line);
                jump = Some((c, c));
            }
            if let Some(sel) = doc.select.take() {
                jump = Some(sel);
            }
            if let Some((a, b)) = jump {
                set_cursor(ui.ctx(), id, output.state.clone(), a, b);
                let r = galley.pos_from_ccursor(CCursor::new(b)).translate(gpos.to_vec2());
                ui.scroll_to_rect(Rect::from_min_size(r.min, vec2(2.0, row_h)), Some(Align::Center));
                if doc.want_focus {
                    ui.memory_mut(|m| m.request_focus(id));
                    doc.want_focus = false;
                }
            }

            // Find-match highlights (only those in the visible char range).
            if !env.matches.is_empty() {
                let top = galley.cursor_from_pos(clip.min - gpos).ccursor.index;
                let bot = galley.cursor_from_pos(clip.max - gpos).ccursor.index;
                let first = env.matches.partition_point(|m| m.1 < top);
                for (i, (a, b)) in env.matches.iter().enumerate().skip(first) {
                    if *a > bot {
                        break;
                    }
                    let r1 = galley.pos_from_ccursor(CCursor::new(*a));
                    let r2 = galley.pos_from_ccursor(CCursor::new(*b));
                    if (r1.min.y - r2.min.y).abs() < 1.0 {
                        let r = Rect::from_min_max(r1.min, pos2(r2.min.x, r1.max.y)).translate(gpos.to_vec2());
                        let cur = env.current_match == Some(i);
                        let c = if cur { Color32::from_rgba_unmultiplied(255, 190, 40, 110) } else { Color32::from_rgba_unmultiplied(255, 190, 40, 50) };
                        painter.rect_filled(r, 2.0, c);
                        if cur {
                            painter.rect_stroke(r, 2.0, Stroke::new(1.0, Color32::from_rgb(255, 190, 40)));
                        }
                    }
                }
            }

            // Debugger: executing line highlight.
            if let Some(line) = env.exec_line {
                let mut l = 0usize;
                let mut new_line = true;
                for row in &galley.rows {
                    if new_line && l == line {
                        let r = row.rect.translate(gpos.to_vec2());
                        painter.rect_filled(
                            Rect::from_x_y_ranges(clip.x_range(), r.y_range()),
                            0.0,
                            Color32::from_rgba_unmultiplied(255, 214, 10, 46),
                        );
                        break;
                    }
                    if row.ends_with_newline {
                        l += 1;
                        new_line = true;
                    } else {
                        new_line = false;
                    }
                }
            }
            info = Some((galley, gpos));
        });
    });

    // ---- gutter ----
    let gp = ui.painter_at(gutter_rect);
    gp.rect_filled(gutter_rect, 0.0, theme.editor);
    gp.line_segment([gutter_rect.right_top(), gutter_rect.right_bottom()], Stroke::new(1.0, theme.border));
    let resp = ui.interact(gutter_rect, Id::new(("gutter", doc.id)), Sense::click());
    let click_y = if resp.clicked() { resp.interact_pointer_pos().map(|p| p.y) } else { None };
    let hover_y = resp.hover_pos().map(|p| p.y);
    if let Some((galley, gpos)) = info {
        let red = Color32::from_rgb(229, 72, 77);
        let cur_line = doc.cursor.0.saturating_sub(1);
        let mut line = 0usize;
        let mut new_line = true;
        for row in &galley.rows {
            let top = gpos.y + row.rect.min.y;
            let bottom = gpos.y + row.rect.max.y;
            let yc = (top + bottom) / 2.0;
            if new_line {
                if click_y.map(|y| y >= top && y < bottom).unwrap_or(false) {
                    if !doc.breakpoints.remove(&line) {
                        doc.breakpoints.insert(line);
                    }
                    out.toggled_bp = Some(line);
                }
                if yc > gutter_rect.top() - row_h && yc < gutter_rect.bottom() + row_h {
                    let col = if line == cur_line { theme.fg } else { theme.dim };
                    gp.text(pos2(gutter_rect.right() - 10.0, yc), Align2::RIGHT_CENTER, (line + 1).to_string(), font.clone(), col);
                    let c = pos2(gutter_rect.left() + 13.0, yc);
                    if doc.breakpoints.contains(&line) {
                        gp.circle_filled(c, 5.0, red);
                    } else if hover_y.map(|y| y >= top && y < bottom).unwrap_or(false) {
                        gp.circle_filled(c, 5.0, red.linear_multiply(0.35));
                    }
                    if env.exec_line == Some(line) {
                        crate::icons::paint_arrow_right(&gp, pos2(gutter_rect.left() + 13.0, yc), Color32::from_rgb(255, 214, 10));
                    }
                }
            }
            if row.ends_with_newline {
                line += 1;
                new_line = true;
            } else {
                new_line = false;
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
