//! Project-wide text search running on a background thread.

use crate::util::is_ignored_dir;
use regex::RegexBuilder;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

pub struct Hit {
    pub path: PathBuf,
    pub line: usize, // 0-based
    pub text: String,
}

enum Msg {
    Hits(Vec<Hit>),
    Done { truncated: bool },
}

pub struct ProjectSearch {
    pub query: String,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub results: Vec<Hit>,
    pub running: bool,
    pub truncated: bool,
    pub error: Option<String>,
    rx: Option<Receiver<Msg>>,
    cancel: Arc<AtomicBool>,
}

impl Default for ProjectSearch {
    fn default() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            use_regex: false,
            results: vec![],
            running: false,
            truncated: false,
            error: None,
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

const MAX_HITS: usize = 3000;

impl ProjectSearch {
    pub fn start(&mut self, root: &Path) {
        self.cancel.store(true, Ordering::SeqCst);
        self.results.clear();
        self.truncated = false;
        self.error = None;
        if self.query.is_empty() {
            self.running = false;
            return;
        }
        let pat = if self.use_regex { self.query.clone() } else { regex::escape(&self.query) };
        let re = match RegexBuilder::new(&pat).case_insensitive(!self.case_sensitive).build() {
            Ok(r) => r,
            Err(e) => {
                self.error = Some(e.to_string());
                self.running = false;
                return;
            }
        };
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.running = true;
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            let mut total = 0usize;
            let walker = walkdir::WalkDir::new(&root).into_iter().filter_entry(|e| {
                !(e.file_type().is_dir() && e.depth() > 0 && is_ignored_dir(&e.file_name().to_string_lossy()))
            });
            for entry in walker.flatten() {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                if !entry.file_type().is_file() {
                    continue;
                }
                if entry.metadata().map(|m| m.len() > 2_000_000).unwrap_or(true) {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(entry.path()) else { continue };
                let mut hits = Vec::new();
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        hits.push(Hit { path: entry.path().to_path_buf(), line: i, text: line.trim().chars().take(200).collect() });
                        total += 1;
                        if total >= MAX_HITS {
                            break;
                        }
                    }
                }
                if !hits.is_empty() {
                    let _ = tx.send(Msg::Hits(hits));
                }
                if total >= MAX_HITS {
                    let _ = tx.send(Msg::Done { truncated: true });
                    return;
                }
            }
            let _ = tx.send(Msg::Done { truncated: false });
        });
    }

    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Hits(h) => self.results.extend(h),
                Msg::Done { truncated } => {
                    self.running = false;
                    self.truncated = truncated;
                }
            }
        }
    }
}
