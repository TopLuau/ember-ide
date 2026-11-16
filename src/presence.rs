//! Discord Rich Presence on a background thread (so a missing / slow Discord never blocks the UI).

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct PresenceInfo {
    pub details: String,
    pub state: String,
    pub small_key: String,
    pub small_text: String,
    pub since: i64,
}

enum Msg {
    Update(PresenceInfo),
    Shutdown,
}

pub struct Presence {
    tx: Option<Sender<Msg>>,
    app_id: String,
    pub status: Arc<Mutex<String>>,
    last: Option<PresenceInfo>,
}

impl Presence {
    pub fn new() -> Self {
        Self { tx: None, app_id: String::new(), status: Arc::new(Mutex::new("off".into())), last: None }
    }

    pub fn status(&self) -> String {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Start, stop or restart the worker according to the settings.
    pub fn configure(&mut self, enabled: bool, app_id: &str) {
        let app_id = app_id.trim().to_string();
        let want = enabled && !app_id.is_empty();
        if want && self.tx.is_some() && self.app_id == app_id {
            return;
        }
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Msg::Shutdown);
        }
        self.last = None;
        if !want {
            *self.status.lock().unwrap() = if enabled { "no application id set".into() } else { "off".into() };
            return;
        }
        self.app_id = app_id.clone();
        let (tx, rx) = channel::<Msg>();
        self.tx = Some(tx);
        let status = self.status.clone();
        *status.lock().unwrap() = "connecting…".into();
        std::thread::spawn(move || {
            let mut client: Option<DiscordIpcClient> = None;
            let mut current: Option<PresenceInfo> = None;
            let mut dirty = true;
            let mut last_sent = Instant::now() - Duration::from_secs(60);
            let mut last_try = Instant::now() - Duration::from_secs(60);
            loop {
                match rx.recv_timeout(Duration::from_secs(2)) {
                    Ok(Msg::Update(i)) => {
                        current = Some(i);
                        dirty = true;
                    }
                    Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                        if let Some(mut c) = client.take() {
                            let _ = c.clear_activity();
                            let _ = c.close();
                        }
                        return;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                }
                if client.is_none() && last_try.elapsed() >= Duration::from_secs(15) {
                    last_try = Instant::now();
                    match DiscordIpcClient::new(&app_id) {
                        Ok(mut c) => match c.connect() {
                            Ok(_) => {
                                client = Some(c);
                                dirty = true;
                                *status.lock().unwrap() = "connected".into();
                            }
                            Err(_) => *status.lock().unwrap() = "Discord not running".into(),
                        },
                        Err(e) => *status.lock().unwrap() = format!("error: {e}"),
                    }
                }
                if dirty && last_sent.elapsed() >= Duration::from_secs(6) {
                    if let (Some(c), Some(info)) = (client.as_mut(), current.as_ref()) {
                        let mut act = activity::Activity::new().timestamps(activity::Timestamps::new().start(info.since));
                        if !info.details.is_empty() {
                            act = act.details(&info.details);
                        }
                        if !info.state.is_empty() {
                            act = act.state(&info.state);
                        }
                        act = act.assets(activity::Assets::new().large_image("ember").large_text("Ember IDE").small_image(&info.small_key).small_text(&info.small_text));
                        match c.set_activity(act) {
                            Ok(_) => {
                                dirty = false;
                                last_sent = Instant::now();
                            }
                            Err(_) => {
                                client = None;
                                *status.lock().unwrap() = "connection lost, retrying…".into();
                            }
                        }
                    } else if client.is_some() {
                        dirty = false;
                    }
                }
            }
        });
    }

    pub fn update(&mut self, info: PresenceInfo) {
        if self.last.as_ref() == Some(&info) {
            return;
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Update(info.clone()));
            self.last = Some(info);
        }
    }
}

impl Drop for Presence {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Msg::Shutdown);
            std::thread::sleep(Duration::from_millis(150));
        }
    }
}
