//! An open document (a tab).

use crate::highlight::HlCache;
use crate::util::{ext_of, file_name};
use anyhow::{bail, Result};
use eframe::egui::text::LayoutJob;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

pub struct Doc {
    pub id: u64,
    pub path: Option<PathBuf>,
    pub untitled: usize,
    pub text: String,
    pub dirty: bool,
    pub crlf: bool,
    /// Bumped on every text change; used to invalidate find results.
    pub rev: u64,
    /// 0-based line numbers.
    pub breakpoints: BTreeSet<usize>,
    pub goto: Option<usize>,
    /// Char range to select and scroll to.
    pub select: Option<(usize, usize)>,
    pub want_focus: bool,
    pub last_edit: Instant,
    /// 1-based (line, column) of the caret, and selected char count.
    pub cursor: (usize, usize),
    pub sel_len: usize,
    pub hl: HlCache,
    pub job: Option<(u64, LayoutJob)>,
    /// Set for images and other binary files, which are shown read-only instead of edited.
    pub view: Option<crate::viewer::Viewer>,
}

impl Doc {
    pub fn untitled(id: u64, n: usize) -> Doc {
        Doc {
            id,
            path: None,
            untitled: n,
            text: String::new(),
            dirty: false,
            crlf: cfg!(windows),
            rev: 0,
            breakpoints: BTreeSet::new(),
            goto: None,
            select: None,
            want_focus: true,
            last_edit: Instant::now(),
            cursor: (1, 1),
            sel_len: 0,
            hl: HlCache::default(),
            job: None,
            view: None,
        }
    }

    pub fn open(id: u64, path: PathBuf) -> Result<Doc> {
        let bytes = std::fs::read(&path)?;
        if let Some(view) = crate::viewer::Viewer::for_bytes(&ext_of(&path), &bytes) {
            let mut d = Doc::untitled(id, 0);
            d.path = Some(path);
            d.view = Some(view);
            d.want_focus = false;
            return Ok(d);
        }
        let raw = String::from_utf8_lossy(&bytes).into_owned();
        let crlf = raw.contains("\r\n");
        let text = if crlf { raw.replace("\r\n", "\n") } else { raw };
        let mut d = Doc::untitled(id, 0);
        d.path = Some(path);
        d.text = text;
        d.crlf = crlf;
        Ok(d)
    }

    pub fn title(&self) -> String {
        match &self.path {
            Some(p) => file_name(p),
            None => format!("Untitled-{}", self.untitled),
        }
    }

    pub fn ext(&self) -> String {
        self.path.as_deref().map(ext_of).unwrap_or_default()
    }

    pub fn touch(&mut self) {
        self.dirty = true;
        self.rev += 1;
        self.last_edit = Instant::now();
    }

    pub fn save(&mut self) -> Result<()> {
        if self.view.is_some() {
            return Ok(());
        }
        let Some(path) = &self.path else { bail!("document has no path") };
        let out = if self.crlf { self.text.replace('\n', "\r\n") } else { self.text.clone() };
        std::fs::write(path, out)?;
        self.dirty = false;
        Ok(())
    }

    pub fn line_count(&self) -> usize {
        self.text.bytes().filter(|b| *b == b'\n').count() + 1
    }
}
