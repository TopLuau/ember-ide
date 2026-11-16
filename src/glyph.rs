//! Tiny vector glyphs for toolbar buttons (so we never depend on font coverage).

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Response, Sense, Shape, Stroke, Ui};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Close,
    Up,
    Down,
    Play,
    Stop,
    Pause,
    StepOver,
    StepIn,
    StepOut,
    Refresh,
    Plus,
    NewFile,
    NewFolder,
    Clear,
    Collapse,
}

fn poly(p: &egui::Painter, pts: &[Pos2], st: Stroke) {
    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], st);
    }
}

fn tri(p: &egui::Painter, a: Pos2, b: Pos2, c: Pos2, col: Color32) {
    p.add(Shape::convex_polygon(vec![a, b, c], col, Stroke::NONE));
}

pub fn paint(p: &egui::Painter, c: Pos2, g: Glyph, col: Color32) {
    let st = Stroke::new(1.6, col);
    let v = |x: f32, y: f32| c + vec2(x, y);
    match g {
        Glyph::Close => {
            p.line_segment([v(-4.0, -4.0), v(4.0, 4.0)], st);
            p.line_segment([v(-4.0, 4.0), v(4.0, -4.0)], st);
        }
        Glyph::Up => poly(p, &[v(-4.5, 2.0), v(0.0, -2.5), v(4.5, 2.0)], st),
        Glyph::Down => poly(p, &[v(-4.5, -2.0), v(0.0, 2.5), v(4.5, -2.0)], st),
        Glyph::Play => tri(p, v(-3.5, -5.5), v(-3.5, 5.5), v(5.5, 0.0), col),
        Glyph::Stop => {
            p.rect_filled(egui::Rect::from_center_size(c, vec2(9.0, 9.0)), 1.5, col);
        }
        Glyph::Pause => {
            p.rect_filled(egui::Rect::from_center_size(v(-3.0, 0.0), vec2(3.0, 10.0)), 1.0, col);
            p.rect_filled(egui::Rect::from_center_size(v(3.0, 0.0), vec2(3.0, 10.0)), 1.0, col);
        }
        Glyph::StepOver => {
            poly(p, &[v(-6.0, 1.0), v(-5.5, -3.0), v(-2.0, -5.0), v(2.0, -5.0), v(5.0, -2.5)], st);
            tri(p, v(2.5, -2.0), v(7.5, -2.0), v(5.0, 2.0), col);
            p.circle_filled(v(0.0, 5.5), 1.9, col);
        }
        Glyph::StepIn => {
            p.line_segment([v(0.0, -7.0), v(0.0, 0.5)], st);
            tri(p, v(-3.5, -0.5), v(3.5, -0.5), v(0.0, 4.0), col);
            p.circle_filled(v(0.0, 6.5), 1.9, col);
        }
        Glyph::StepOut => {
            p.line_segment([v(0.0, 1.0), v(0.0, 7.0)], st);
            tri(p, v(-3.5, 1.5), v(3.5, 1.5), v(0.0, -3.0), col);
            p.circle_filled(v(0.0, -6.0), 1.9, col);
        }
        Glyph::Refresh => {
            let mut pts = Vec::new();
            for i in 0..=14 {
                let a = -0.3 + i as f32 * (std::f32::consts::TAU * 0.82 / 14.0);
                pts.push(v(5.5 * a.cos(), 5.5 * a.sin()));
            }
            poly(p, &pts, st);
            tri(p, v(5.5, -5.5), v(5.5, 0.5), v(0.5, -1.5), col);
        }
        Glyph::Plus => {
            p.line_segment([v(-5.0, 0.0), v(5.0, 0.0)], st);
            p.line_segment([v(0.0, -5.0), v(0.0, 5.0)], st);
        }
        Glyph::NewFile => {
            let r = egui::Rect::from_center_size(v(-1.0, 0.0), vec2(10.0, 13.0));
            p.rect_stroke(r, 1.5, Stroke::new(1.3, col));
            p.line_segment([v(4.0, 4.0), v(8.0, 4.0)], Stroke::new(1.5, col));
            p.line_segment([v(6.0, 2.0), v(6.0, 6.0)], Stroke::new(1.5, col));
        }
        Glyph::NewFolder => {
            let r = egui::Rect::from_center_size(v(-1.0, 1.0), vec2(14.0, 10.0));
            p.rect_stroke(r, 1.5, Stroke::new(1.3, col));
            p.line_segment([v(-8.0, -4.0), v(-3.0, -4.0)], Stroke::new(1.3, col));
            p.line_segment([v(4.5, 4.5), v(8.5, 4.5)], Stroke::new(1.5, col));
            p.line_segment([v(6.5, 2.5), v(6.5, 6.5)], Stroke::new(1.5, col));
        }
        Glyph::Clear => {
            p.circle_stroke(c, 5.5, Stroke::new(1.4, col));
            p.line_segment([v(-3.5, 3.5), v(3.5, -3.5)], Stroke::new(1.4, col));
        }
        Glyph::Collapse => {
            poly(p, &[v(-4.0, -1.5), v(0.0, -5.0), v(4.0, -1.5)], st);
            poly(p, &[v(-4.0, 1.5), v(0.0, 5.0), v(4.0, 1.5)], st);
        }
    }
}

/// A 24x24 flat icon button with hover background and tooltip.
pub fn button(ui: &mut Ui, g: Glyph, tip: &str, color: Option<Color32>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let base = color.unwrap_or(ui.visuals().widgets.inactive.fg_stroke.color);
    let col = if resp.hovered() { base } else { base.linear_multiply(0.85) };
    paint(ui.painter(), rect.center(), g, col);
    resp.on_hover_text(tip)
}

/// A small text toggle such as "Aa" or ".*".
pub fn toggle(ui: &mut Ui, label: &str, on: bool, tip: &str, accent: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, 4.0, accent.linear_multiply(0.30));
        p.rect_stroke(rect, 4.0, Stroke::new(1.0, accent));
    } else if resp.hovered() {
        p.rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let col = if on { ui.visuals().widgets.inactive.fg_stroke.color } else { ui.visuals().widgets.inactive.fg_stroke.color.linear_multiply(0.75) };
    p.text(rect.center(), Align2::CENTER_CENTER, label, FontId::monospace(12.5), col);
    resp.on_hover_text(tip)
}
