//! Colour themes. Twelve are built in; more can be dropped into `<config>/ember/themes/*.toml`.

use crate::config::config_dir;
use eframe::egui::{self, Color32, Stroke, Vec2};
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tok {
    Plain,
    Comment,
    Str,
    Number,
    Keyword,
    Function,
    Type,
    Constant,
    Variable,
    Operator,
    Punct,
}

#[derive(Clone)]
pub struct Theme {
    pub name: String,
    pub dark: bool,
    pub panel: Color32,
    pub editor: Color32,
    pub fg: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub border: Color32,
    pub line_hl: Color32,
    syn: [Color32; 9], // comment string number keyword function type constant variable operator
}

pub fn hex(s: &str) -> Color32 {
    let s = s.trim().trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0xff00ff);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

impl Theme {
    #[allow(clippy::too_many_arguments)]
    fn build(name: &str, dark: bool, ui: [&str; 5], syn: [&str; 9]) -> Theme {
        let panel = hex(ui[0]);
        let editor = hex(ui[1]);
        let fg = hex(ui[2]);
        let accent = hex(ui[3]);
        let selection = hex(ui[4]);
        Theme {
            name: name.to_string(),
            dark,
            panel,
            editor,
            fg,
            dim: blend(fg, editor, 0.45),
            accent,
            selection,
            border: blend(panel, fg, 0.14),
            line_hl: blend(editor, fg, 0.05),
            syn: syn.map(hex),
        }
    }

    pub fn tok(&self, t: Tok) -> Color32 {
        match t {
            Tok::Plain => self.fg,
            Tok::Comment => self.syn[0],
            Tok::Str => self.syn[1],
            Tok::Number => self.syn[2],
            Tok::Keyword => self.syn[3],
            Tok::Function => self.syn[4],
            Tok::Type => self.syn[5],
            Tok::Constant => self.syn[6],
            Tok::Variable => self.syn[7],
            Tok::Operator => self.syn[8],
            Tok::Punct => blend(self.fg, self.editor, 0.25),
        }
    }

    pub fn hover(&self) -> Color32 {
        blend(self.panel, self.fg, 0.10)
    }

    pub fn raised(&self) -> Color32 {
        blend(self.panel, self.editor, 0.6)
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let mut v = if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.panel_fill = self.panel;
        v.window_fill = self.raised();
        v.extreme_bg_color = self.editor;
        v.faint_bg_color = blend(self.panel, self.fg, 0.04);
        v.code_bg_color = blend(self.editor, self.fg, 0.08);
        v.hyperlink_color = self.accent;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.window_rounding = 8.0.into();
        v.menu_rounding = 6.0.into();
        v.selection.bg_fill = self.selection;
        v.selection.stroke = Stroke::new(1.0, self.accent);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.panel;
        w.noninteractive.weak_bg_fill = self.panel;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.fg);
        let lift = |t: f32| blend(self.panel, self.fg, t);
        for (wv, t) in [(&mut w.inactive, 0.07), (&mut w.hovered, 0.14), (&mut w.active, 0.22), (&mut w.open, 0.14)] {
            wv.bg_fill = lift(t);
            wv.weak_bg_fill = lift(t);
            wv.bg_stroke = Stroke::new(1.0, blend(self.border, self.fg, 0.05));
            wv.fg_stroke = Stroke::new(1.0, self.fg);
            wv.rounding = 5.0.into();
        }
        w.hovered.bg_stroke = Stroke::new(1.0, self.accent);
        w.active.bg_stroke = Stroke::new(1.0, self.accent);
        ctx.set_visuals(v);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = Vec2::new(8.0, 5.0);
            s.spacing.button_padding = Vec2::new(9.0, 4.0);
            s.interaction.selectable_labels = false;
        });
    }
}

#[derive(Deserialize)]
struct ThemeFile {
    name: String,
    #[serde(default = "yes")]
    dark: bool,
    panel: String,
    editor: String,
    fg: String,
    accent: String,
    selection: String,
    comment: String,
    string: String,
    number: String,
    keyword: String,
    function: String,
    r#type: String,
    constant: String,
    variable: String,
    operator: String,
}

fn yes() -> bool {
    true
}

pub fn all_themes() -> Vec<Theme> {
    let mut t = builtin();
    if let Ok(rd) = std::fs::read_dir(config_dir().join("themes")) {
        for e in rd.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) != Some("toml") {
                continue;
            }
            if let Some(f) = std::fs::read_to_string(e.path()).ok().and_then(|s| toml::from_str::<ThemeFile>(&s).ok()) {
                t.push(Theme::build(
                    &f.name,
                    f.dark,
                    [&f.panel, &f.editor, &f.fg, &f.accent, &f.selection],
                    [&f.comment, &f.string, &f.number, &f.keyword, &f.function, &f.r#type, &f.constant, &f.variable, &f.operator],
                ));
            }
        }
    }
    t
}

fn builtin() -> Vec<Theme> {
    //                 panel      editor     fg         accent     selection
    //                 comment    string     number     keyword    function   type       constant   variable   operator
    vec![
        Theme::build("Ember Dark", true, ["#151518", "#1b1b1f", "#d4d4d8", "#ff7a3d", "#4a2f24"],
            ["#6a737d", "#e9a76b", "#f2c14e", "#ff7a59", "#ffd166", "#4ec9b0", "#c792ea", "#9cdcfe", "#ff9f7a"]),
        Theme::build("Ember Light", false, ["#f1eeea", "#fbfaf8", "#2b2b2f", "#e8590c", "#ffd9bf"],
            ["#8a8a92", "#b5521b", "#a05a00", "#d6336c", "#1c6ea4", "#0b7a75", "#7048e8", "#364fc7", "#c2255c"]),
        Theme::build("Dracula", true, ["#21222c", "#282a36", "#f8f8f2", "#bd93f9", "#44475a"],
            ["#6272a4", "#f1fa8c", "#bd93f9", "#ff79c6", "#50fa7b", "#8be9fd", "#bd93f9", "#f8f8f2", "#ff79c6"]),
        Theme::build("Monokai", true, ["#1e1f1c", "#272822", "#f8f8f2", "#a6e22e", "#49483e"],
            ["#75715e", "#e6db74", "#ae81ff", "#f92672", "#a6e22e", "#66d9ef", "#ae81ff", "#f8f8f2", "#f92672"]),
        Theme::build("Nord", true, ["#272c36", "#2e3440", "#d8dee9", "#88c0d0", "#434c5e"],
            ["#616e88", "#a3be8c", "#b48ead", "#81a1c1", "#88c0d0", "#8fbcbb", "#b48ead", "#d8dee9", "#81a1c1"]),
        Theme::build("Solarized Dark", true, ["#00212b", "#002b36", "#93a1a1", "#268bd2", "#073642"],
            ["#586e75", "#2aa198", "#d33682", "#859900", "#268bd2", "#b58900", "#cb4b16", "#93a1a1", "#859900"]),
        Theme::build("Gruvbox Dark", true, ["#1d2021", "#282828", "#ebdbb2", "#fabd2f", "#3c3836"],
            ["#928374", "#b8bb26", "#d3869b", "#fb4934", "#8ec07c", "#fabd2f", "#d3869b", "#83a598", "#fe8019"]),
        Theme::build("Tokyo Night", true, ["#16161e", "#1a1b26", "#a9b1d6", "#7aa2f7", "#33467c"],
            ["#565f89", "#9ece6a", "#ff9e64", "#bb9af7", "#7aa2f7", "#2ac3de", "#ff9e64", "#c0caf5", "#89ddff"]),
        Theme::build("One Dark", true, ["#21252b", "#282c34", "#abb2bf", "#61afef", "#3e4451"],
            ["#5c6370", "#98c379", "#d19a66", "#c678dd", "#61afef", "#e5c07b", "#d19a66", "#e06c75", "#56b6c2"]),
        Theme::build("Catppuccin Mocha", true, ["#181825", "#1e1e2e", "#cdd6f4", "#cba6f7", "#45475a"],
            ["#6c7086", "#a6e3a1", "#fab387", "#cba6f7", "#89b4fa", "#f9e2af", "#fab387", "#f2cdcd", "#89dceb"]),
        Theme::build("GitHub Light", false, ["#f6f8fa", "#ffffff", "#24292f", "#0969da", "#b6e3ff"],
            ["#6e7781", "#0a3069", "#0550ae", "#cf222e", "#8250df", "#953800", "#0550ae", "#24292f", "#cf222e"]),
        Theme::build("Solarized Light", false, ["#eee8d5", "#fdf6e3", "#586e75", "#268bd2", "#e4dcc3"],
            ["#93a1a1", "#2aa198", "#d33682", "#859900", "#268bd2", "#b58900", "#cb4b16", "#586e75", "#859900"]),
    ]
}
