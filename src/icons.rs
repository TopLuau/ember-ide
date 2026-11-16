//! Vector icons painted directly with egui (no icon font needed): file-type badges,
//! folder icons and the activity bar glyphs.

use crate::theme::hex;
use eframe::egui::{pos2, vec2, Align2, Color32, FontId, Painter, Rect, Shape, Stroke};

fn badge(name: &str) -> (&'static str, &'static str) {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "cargo.toml" | "cargo.lock" => return ("CG", "#dea584"),
        "dockerfile" | ".dockerignore" => return ("DK", "#2496ed"),
        "makefile" | "cmakelists.txt" => return ("MK", "#6d8086"),
        "license" | "licence" => return ("LI", "#d0b24a"),
        ".gitignore" | ".gitattributes" | ".gitmodules" => return ("GI", "#f1502f"),
        "readme.md" | "readme" => return ("RM", "#42a5f5"),
        _ => {}
    }
    let ext = lower.rsplit('.').next().unwrap_or("");
    if !lower.contains('.') {
        return ("··", "#7d8590");
    }
    match ext {
        "rs" => ("RS", "#dea584"),
        "py" | "pyw" => ("PY", "#4b8bbe"),
        "js" | "mjs" | "cjs" => ("JS", "#f1e05a"),
        "ts" => ("TS", "#3178c6"),
        "jsx" => ("JX", "#61dafb"),
        "tsx" => ("TX", "#3178c6"),
        "json" | "json5" | "jsonc" => ("{}", "#cbcb41"),
        "toml" => ("TM", "#9c4221"),
        "yml" | "yaml" => ("YM", "#cb171e"),
        "md" | "markdown" => ("MD", "#519aba"),
        "html" | "htm" => ("<>", "#e34c26"),
        "css" => ("CS", "#563d7c"),
        "scss" | "sass" => ("SC", "#c6538c"),
        "c" => ("C", "#6f8cff"),
        "h" => ("H", "#a074c4"),
        "cpp" | "cc" | "cxx" => ("C+", "#f34b7d"),
        "hpp" => ("H+", "#a074c4"),
        "cs" => ("C#", "#178600"),
        "go" => ("GO", "#00add8"),
        "java" => ("JV", "#b07219"),
        "kt" | "kts" => ("KT", "#a97bff"),
        "swift" => ("SW", "#f05138"),
        "rb" => ("RB", "#cc342d"),
        "php" => ("PH", "#777bb4"),
        "lua" => ("LU", "#51a0cf"),
        "sh" | "bash" | "zsh" => ("SH", "#89e051"),
        "ps1" | "bat" | "cmd" => (">_", "#5391fe"),
        "sql" => ("SQ", "#e38c00"),
        "xml" | "svg" => ("XM", "#e37933"),
        "lock" => ("LK", "#7d8590"),
        "txt" | "log" => ("TX", "#9aa4af"),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "bmp" => ("IM", "#a074c4"),
        "rhai" => ("RH", "#ff8a50"),
        "wgsl" | "glsl" | "vert" | "frag" => ("SH", "#5db0d7"),
        "zip" | "tar" | "gz" | "7z" => ("ZP", "#b0a060"),
        "exe" | "dll" | "so" | "dylib" => ("BN", "#8b949e"),
        _ => ("··", "#7d8590"),
    }
}

pub fn paint_file(p: &Painter, rect: Rect, name: &str) {
    let (label, color) = badge(name);
    let c = hex(color);
    let r = Rect::from_center_size(rect.center(), vec2(rect.width().min(18.0), 13.0));
    p.rect_filled(r, 3.0, c);
    let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    let tc = if lum > 150.0 { Color32::from_rgb(25, 25, 28) } else { Color32::WHITE };
    p.text(r.center() + vec2(0.0, 0.3), Align2::CENTER_CENTER, label, FontId::monospace(8.5), tc);
}

fn folder_color(name: &str) -> Color32 {
    match name {
        "src" | "source" | "lib" => hex("#4fa3e6"),
        "target" | "build" | "dist" | "node_modules" | ".git" | "out" => hex("#6c7480"),
        "assets" | "images" | "img" | "resources" | "static" | "public" => hex("#e0a030"),
        "docs" | "doc" => hex("#58b87a"),
        "tests" | "test" | "spec" => hex("#e0634f"),
        "plugins" | "extensions" | "packages" => hex("#b07ae0"),
        ".github" | ".vscode" | ".idea" => hex("#8aa0b8"),
        _ => hex("#d9a441"),
    }
}

pub fn paint_folder(p: &Painter, rect: Rect, name: &str, open: bool) {
    let c = folder_color(name);
    let dark = Color32::from_rgba_unmultiplied(0, 0, 0, 40);
    let c0 = rect.center() + vec2(-8.0, -6.0);
    // back tab
    p.rect_filled(Rect::from_min_size(c0, vec2(7.0, 4.0)), 1.5, c);
    // body
    let body = Rect::from_min_size(c0 + vec2(0.0, 2.5), vec2(16.0, 10.0));
    p.rect_filled(body, 2.0, c);
    if open {
        let front = vec![
            body.left_bottom(),
            body.left_bottom() + vec2(2.5, -6.0),
            body.right_bottom() + vec2(2.0, -6.0),
            body.right_bottom(),
        ];
        p.add(Shape::convex_polygon(front, c, Stroke::NONE));
        p.rect_filled(Rect::from_min_size(body.min + vec2(0.0, 0.0), vec2(16.0, 3.0)), 2.0, dark);
    }
}

pub fn paint_chevron(p: &Painter, center: eframe::egui::Pos2, open: bool, color: Color32) {
    let pts = if open {
        vec![center + vec2(-3.5, -2.0), center + vec2(3.5, -2.0), center + vec2(0.0, 2.5)]
    } else {
        vec![center + vec2(-2.0, -3.5), center + vec2(-2.0, 3.5), center + vec2(2.5, 0.0)]
    };
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Explorer,
    Search,
    Debug,
    Plugins,
    Settings,
}

pub fn paint_activity(p: &Painter, rect: Rect, kind: Activity, color: Color32) {
    let c = rect.center();
    let st = Stroke::new(1.7, color);
    match kind {
        Activity::Explorer => {
            let a = Rect::from_center_size(c + vec2(-2.0, 1.5), vec2(11.0, 14.0));
            let b = Rect::from_center_size(c + vec2(3.0, -2.5), vec2(11.0, 14.0));
            p.rect_stroke(a, 2.0, st);
            p.rect_filled(b.shrink(1.0), 2.0, color.linear_multiply(0.35));
            p.rect_stroke(b, 2.0, st);
        }
        Activity::Search => {
            p.circle_stroke(c + vec2(-2.0, -2.0), 6.5, st);
            p.line_segment([c + vec2(2.8, 2.8), c + vec2(8.5, 8.5)], Stroke::new(2.2, color));
        }
        Activity::Debug => {
            p.circle_stroke(c + vec2(0.0, 1.5), 6.0, st);
            p.circle_filled(c + vec2(0.0, -5.5), 2.8, color);
            for s in [-1.0f32, 1.0] {
                p.line_segment([c + vec2(6.0 * s, -1.0), c + vec2(10.0 * s, -3.0)], st);
                p.line_segment([c + vec2(6.5 * s, 2.0), c + vec2(10.5 * s, 2.0)], st);
                p.line_segment([c + vec2(5.5 * s, 5.5), c + vec2(9.5 * s, 8.0)], st);
            }
            p.line_segment([c + vec2(0.0, -2.0), c + vec2(0.0, 8.0)], Stroke::new(1.2, color));
        }
        Activity::Plugins => {
            let s = 7.5;
            p.rect_stroke(Rect::from_min_size(c + vec2(-9.0, -1.0), vec2(s, s)), 1.5, st);
            p.rect_stroke(Rect::from_min_size(c + vec2(-1.0, -1.0), vec2(s, s)), 1.5, st);
            p.rect_stroke(Rect::from_min_size(c + vec2(-9.0, -9.0), vec2(s, s)), 1.5, st);
            p.rect_filled(Rect::from_min_size(c + vec2(2.5, -9.5), vec2(s, s)), 1.5, color);
        }
        Activity::Settings => {
            p.circle_stroke(c, 5.0, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([c + d * 7.0, c + d * 10.0], Stroke::new(2.4, color));
            }
            p.circle_filled(c, 1.8, color);
        }
    }
}

pub fn paint_dot(p: &Painter, center: eframe::egui::Pos2, r: f32, color: Color32) {
    p.circle_filled(center, r, color);
}

pub fn paint_arrow_right(p: &Painter, c: eframe::egui::Pos2, color: Color32) {
    let pts = vec![pos2(c.x - 4.0, c.y - 4.5), pos2(c.x - 4.0, c.y + 4.5), pos2(c.x + 4.5, c.y)];
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}
