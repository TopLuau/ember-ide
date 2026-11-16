#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ember::{app::App, Opts};

fn main() -> eframe::Result<()> {
    // Hidden: act as a scripted debug adapter (used by the test-suite and demos).
    if std::env::args().nth(1).as_deref() == Some("--mock-dap") {
        ember::dap_mock::run();
        return Ok(());
    }
    let opts = Opts::parse();
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Ember IDE")
        .with_inner_size(if opts.record.is_some() { [1280.0, 800.0] } else { [1360.0, 840.0] })
        .with_min_inner_size([760.0, 480.0])
        .with_drag_and_drop(true);
    if let Some(i) = icon {
        viewport = viewport.with_icon(i);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Ember IDE", options, Box::new(move |cc| Ok(Box::new(App::new(cc, opts.clone())))))
}
