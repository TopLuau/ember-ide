//! User configuration, stored as TOML in the platform config directory.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AdapterCfg {
    pub command: String,
    pub args: Vec<String>,
    /// "lldb" (native programs) or "python" (debugpy). Decides the launch arguments.
    pub kind: String,
}

/// Application id shipped with release builds so Rich Presence works out of the box.
/// Set it in CI with the `EMBER_DISCORD_APP_ID` environment variable at build time,
/// or paste your own id in Settings.
pub const DEFAULT_DISCORD_APP_ID: &str = match option_env!("EMBER_DISCORD_APP_ID") {
    Some(id) if !id.is_empty() => id,
    _ => "1571923030861553766",
};

/// The id Rich Presence uses: the user's own if set, otherwise the one built into this release.
pub fn effective_discord_id(user: &str) -> String {
    let u = user.trim();
    if u.is_empty() { DEFAULT_DISCORD_APP_ID.to_string() } else { u.to_string() }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub font_size: f32,
    pub tab_width: usize,
    pub word_wrap: bool,
    pub auto_save: bool,
    pub auto_pair: bool,
    pub discord_enabled: bool,
    /// Discord application (client) id. Create one at https://discord.com/developers/applications
    pub discord_app_id: String,
    pub discord_show_file: bool,
    pub last_folder: Option<PathBuf>,
    pub recent_folders: Vec<PathBuf>,
    pub disabled_plugins: Vec<String>,
    pub debug_adapters: BTreeMap<String, AdapterCfg>,
}

impl Default for AdapterCfg {
    fn default() -> Self {
        Self { command: String::new(), args: vec![], kind: "lldb".into() }
    }
}

impl Default for Config {
    fn default() -> Self {
        let lldb = AdapterCfg { command: "lldb-dap".into(), args: vec![], kind: "lldb".into() };
        let mut adapters = BTreeMap::new();
        adapters.insert("rust".to_string(), lldb.clone());
        adapters.insert("c".to_string(), lldb.clone());
        adapters.insert("cpp".to_string(), lldb);
        adapters.insert(
            "python".to_string(),
            AdapterCfg {
                command: if cfg!(windows) { "python".into() } else { "python3".into() },
                args: vec!["-m".into(), "debugpy.adapter".into()],
                kind: "python".into(),
            },
        );
        Self {
            theme: "Ember Dark".into(),
            font_size: 14.5,
            tab_width: 4,
            word_wrap: false,
            auto_save: false,
            auto_pair: true,
            discord_enabled: true,
            discord_app_id: DEFAULT_DISCORD_APP_ID.to_string(),
            discord_show_file: true,
            last_folder: None,
            recent_folders: vec![],
            disabled_plugins: vec![],
            debug_adapters: adapters,
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("ember")
}

fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(config_dir());
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = std::fs::write(config_path(), s);
        }
    }

    pub fn push_recent(&mut self, p: PathBuf) {
        self.recent_folders.retain(|x| x != &p);
        self.recent_folders.insert(0, p.clone());
        self.recent_folders.truncate(8);
        self.last_folder = Some(p);
    }
}
