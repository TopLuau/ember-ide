//! Ember IDE as a library, so the binary and the integration tests share one code base.

pub mod app;
pub mod config;
pub mod dap;
pub mod dap_mock;
mod dialogs;
pub mod doc;
pub mod editor;
pub mod explorer;
mod glyph;
pub mod highlight;
mod icons;
mod panels;
pub mod plugins;
pub mod presence;
pub mod record;
pub mod search;
pub mod terminal;
pub mod theme;
pub mod util;
pub mod viewer;

/// Command line options: `ember [PATH] [--theme NAME] [--screenshot OUT.png] [--ui STATE]`.
#[derive(Default, Clone)]
pub struct Opts {
    pub path: Option<std::path::PathBuf>,
    pub theme: Option<String>,
    pub screenshot: Option<std::path::PathBuf>,
    /// Start with a particular screen open (used for screenshots / demos).
    pub ui: Option<String>,
    /// Record a scripted demo session into this folder (see `record.rs`).
    pub record: Option<std::path::PathBuf>,
}

impl Opts {
    pub fn parse() -> Opts {
        let mut o = Opts::default();
        let mut it = std::env::args().skip(1);
        while let Some(a) = it.next() {
            match a.as_str() {
                "--theme" => o.theme = it.next(),
                "--screenshot" => o.screenshot = it.next().map(std::path::PathBuf::from),
                "--ui" => o.ui = it.next(),
                "--record" => o.record = it.next().map(std::path::PathBuf::from),
                _ if !a.starts_with("--") => o.path = Some(std::path::PathBuf::from(a)),
                _ => {}
            }
        }
        o
    }
}
