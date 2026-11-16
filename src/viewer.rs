//! Read-only viewers for files that are not text: images are shown as images (with zoom and a
//! checkerboard for transparency) and other binaries as a hex dump.

use crate::theme::Theme;
use eframe::egui::{Align, Color32, ColorImage, Layout, Pos2, Rect, RichText, Sense, TextureHandle, TextureOptions, Ui, Vec2};

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tif", "tiff"];

pub enum Viewer {
    Image(ImageView),
    Binary(BinaryView),
}

pub struct ImageView {
    pub image: Option<ColorImage>,
    pub tex: Option<TextureHandle>,
    pub size: [usize; 2],
    pub file_size: u64,
    pub format: String,
    /// `None` = fit to window.
    pub zoom: Option<f32>,
    pub error: Option<String>,
}

pub struct BinaryView {
    pub file_size: u64,
    pub head: Vec<u8>,
}

impl Viewer {
    /// Try to build a viewer for a file's raw bytes; `None` means "treat as text".
    pub fn for_bytes(ext: &str, bytes: &[u8]) -> Option<Viewer> {
        let file_size = bytes.len() as u64;
        if IMAGE_EXTS.contains(&ext) {
            let mut v = ImageView { image: None, tex: None, size: [0, 0], file_size, format: ext.to_uppercase(), zoom: None, error: None };
            match image::load_from_memory(bytes) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    v.size = size;
                    v.image = Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()));
                }
                Err(e) => v.error = Some(e.to_string()),
            }
            return Some(Viewer::Image(v));
        }
        if bytes[..bytes.len().min(8192)].contains(&0) {
            return Some(Viewer::Binary(BinaryView { file_size, head: bytes[..bytes.len().min(4096)].to_vec() }));
        }
        None
    }

    pub fn describe(&self) -> String {
        match self {
            Viewer::Image(i) if i.error.is_none() => format!("{} × {} · {}", i.size[0], i.size[1], human_size(i.file_size)),
            Viewer::Image(i) => human_size(i.file_size),
            Viewer::Binary(b) => format!("Binary · {}", human_size(b.file_size)),
        }
    }
}

pub fn human_size(n: u64) -> String {
    let n = n as f64;
    if n < 1024.0 {
        format!("{n} B")
    } else if n < 1024.0 * 1024.0 {
        format!("{:.1} KB", n / 1024.0)
    } else {
        format!("{:.2} MB", n / 1024.0 / 1024.0)
    }
}

pub fn show(ui: &mut Ui, view: &mut Viewer, title: &str, theme: &Theme) {
    match view {
        Viewer::Image(v) => show_image(ui, v, title, theme),
        Viewer::Binary(b) => show_binary(ui, b, theme),
    }
}

fn show_image(ui: &mut Ui, v: &mut ImageView, title: &str, theme: &Theme) {
    if v.tex.is_none() {
        if let Some(img) = v.image.take() {
            v.tex = Some(ui.ctx().load_texture(format!("img-{title}"), img, TextureOptions::LINEAR));
        }
    }
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        if v.error.is_none() {
            ui.label(RichText::new(format!("{} × {} px", v.size[0], v.size[1])).color(theme.fg));
            ui.label(RichText::new(format!("{} · {}", v.format, human_size(v.file_size))).color(theme.dim));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            if ui.button("Actual size").clicked() {
                v.zoom = Some(1.0);
            }
            if ui.button("Fit").clicked() {
                v.zoom = None;
            }
            if ui.button("+").clicked() {
                v.zoom = Some((v.zoom.unwrap_or(1.0) * 1.25).min(32.0));
            }
            if ui.button("−").clicked() {
                v.zoom = Some((v.zoom.unwrap_or(1.0) / 1.25).max(0.05));
            }
            if let Some(z) = v.zoom {
                ui.label(RichText::new(format!("{:.0}%", z * 100.0)).color(theme.dim));
            }
        });
    });
    ui.separator();
    if let Some(e) = &v.error {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| ui.label(RichText::new(format!("Cannot display this image: {e}")).color(theme.dim)));
        return;
    }
    let Some(tex) = &v.tex else { return };
    let avail = ui.available_size();
    let (rect, resp) = ui.allocate_exact_size(avail, Sense::hover());
    if resp.hovered() {
        let (scroll, ctrl) = ui.input(|i| (i.smooth_scroll_delta.y, i.modifiers.command));
        if ctrl && scroll != 0.0 {
            let fit = fit_zoom(v.size, avail);
            let z = v.zoom.unwrap_or(fit);
            v.zoom = Some((z * (1.0 + scroll * 0.002)).clamp(0.05, 32.0));
        }
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme.editor);
    let zoom = v.zoom.unwrap_or_else(|| fit_zoom(v.size, avail));
    let size = Vec2::new(v.size[0] as f32 * zoom, v.size[1] as f32 * zoom);
    let img_rect = Rect::from_center_size(rect.center(), size);
    // Checkerboard behind transparent pixels
    let visible = img_rect.intersect(rect);
    if visible.is_positive() {
        let cell = 12.0;
        let (a, b) = (Color32::from_gray(58), Color32::from_gray(74));
        let mut y = visible.top();
        let mut row = 0;
        while y < visible.bottom() {
            let mut x = visible.left();
            let mut col = 0;
            while x < visible.right() {
                let r = Rect::from_min_max(Pos2::new(x, y), Pos2::new((x + cell).min(visible.right()), (y + cell).min(visible.bottom())));
                painter.rect_filled(r, 0.0, if (row + col) % 2 == 0 { a } else { b });
                x += cell;
                col += 1;
            }
            y += cell;
            row += 1;
        }
    }
    // Pixel art stays crisp when magnified.
    let mut mesh_tex = tex.clone();
    let _ = &mut mesh_tex;
    painter.image(tex.id(), img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
}

fn fit_zoom(size: [usize; 2], avail: Vec2) -> f32 {
    if size[0] == 0 || size[1] == 0 {
        return 1.0;
    }
    let z = (avail.x - 24.0).max(10.0) / size[0] as f32;
    let z = z.min((avail.y - 24.0).max(10.0) / size[1] as f32);
    z.min(1.0).max(0.05)
}

fn show_binary(ui: &mut Ui, b: &BinaryView, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("Binary file").color(theme.fg).strong());
        ui.label(RichText::new(format!("{} · showing the first {} bytes", human_size(b.file_size), b.head.len())).color(theme.dim));
    });
    ui.separator();
    eframe::egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, chunk) in b.head.chunks(16).enumerate() {
            let hex: String = (0..16).map(|k| chunk.get(k).map(|b| format!("{b:02x} ")).unwrap_or_else(|| "   ".into())).collect();
            let ascii: String = chunk.iter().map(|&c| if (32..127).contains(&c) { c as char } else { '.' }).collect();
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(RichText::new(format!("{:08x}", i * 16)).monospace().color(theme.dim));
                ui.label(RichText::new(hex).monospace().color(theme.fg));
                ui.label(RichText::new(ascii).monospace().color(theme.accent));
            });
        }
    });
}
