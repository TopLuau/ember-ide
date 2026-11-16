//! Integrated terminal: a real pseudo-terminal (ConPTY on Windows, openpty on Unix) running the
//! user's shell, with a `vt100` emulator so colours, cursor movement, full-screen programs,
//! line editing and history all work like in any other terminal.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct Terminal {
    pub parser: Arc<Mutex<vt100::Parser>>,
    writer: Option<Box<dyn Write + Send>>,
    master: Option<Box<dyn MasterPty + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    alive: Arc<AtomicBool>,
    /// Size of the grid in cells (rows, cols).
    pub size: (u16, u16),
    cwd: Option<PathBuf>,
    pub error: Option<String>,
    pub ever_started: bool,
    pub ctx: Option<eframe::egui::Context>,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            parser: Arc::new(Mutex::new(vt100::Parser::new(24, 80, 5000))),
            writer: None,
            master: None,
            child: None,
            alive: Arc::new(AtomicBool::new(false)),
            size: (24, 80),
            cwd: None,
            error: None,
            ever_started: false,
            ctx: None,
        }
    }
}

fn shell_command() -> CommandBuilder {
    #[cfg(windows)]
    {
        let mut c = CommandBuilder::new("powershell.exe");
        c.arg("-NoLogo");
        c
    }
    #[cfg(not(windows))]
    {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
        let mut c = CommandBuilder::new(sh);
        c.arg("-i");
        c
    }
}

impl Terminal {
    /// True while a shell is attached.
    pub fn running(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Nothing to poll any more (the reader thread wakes the UI itself); kept for the frame loop.
    pub fn poll(&mut self) -> bool {
        false
    }

    /// Start a shell in `cwd` if none is running.
    pub fn ensure_started(&mut self, cwd: &Path) {
        if self.running() {
            return;
        }
        self.error = None;
        if let Err(e) = self.start(cwd) {
            self.error = Some(format!("Could not start a shell: {e}"));
        }
    }

    fn start(&mut self, cwd: &Path) -> anyhow::Result<()> {
        let (rows, cols) = self.size;
        let pty = native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut cmd = shell_command();
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "EmberIDE");
        let child = pty.slave.spawn_command(cmd).map_err(|e| anyhow::anyhow!("{e}"))?;
        drop(pty.slave);
        let mut reader = pty.master.try_clone_reader().map_err(|e| anyhow::anyhow!("{e}"))?;
        let writer = pty.master.take_writer().map_err(|e| anyhow::anyhow!("{e}"))?;

        *self.parser.lock().unwrap() = vt100::Parser::new(rows, cols, 5000);
        let parser = self.parser.clone();
        let alive = self.alive.clone();
        alive.store(true, Ordering::Relaxed);
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Ok(mut p) = parser.lock() {
                            p.process(&buf[..n]);
                        }
                        if let Some(c) = &ctx {
                            c.request_repaint();
                        }
                    }
                }
            }
            alive.store(false, Ordering::Relaxed);
            if let Some(c) = &ctx {
                c.request_repaint();
            }
        });
        self.writer = Some(writer);
        self.master = Some(pty.master);
        self.child = Some(child);
        self.cwd = Some(cwd.to_path_buf());
        self.ever_started = true;
        Ok(())
    }

    /// Resize the pseudo terminal when the panel changes size.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        let (rows, cols) = (rows.max(2), cols.max(10));
        if (rows, cols) == self.size {
            return;
        }
        self.size = (rows, cols);
        if let Some(m) = &self.master {
            let _ = m.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        }
        if let Ok(mut p) = self.parser.lock() {
            p.set_size(rows, cols);
        }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        if let Some(w) = self.writer.as_mut() {
            let _ = w.write_all(bytes);
            let _ = w.flush();
        }
    }

    /// Type a command into the shell (used by Run / Ctrl+F5), changing directory first if needed.
    pub fn run(&mut self, cmd: &str, cwd: &Path) {
        self.ensure_started(cwd);
        if !self.running() {
            return;
        }
        if self.cwd.as_deref() != Some(cwd) {
            #[cfg(windows)]
            let cd = format!("Set-Location -LiteralPath '{}'\r", cwd.display().to_string().replace('\'', "''"));
            #[cfg(not(windows))]
            let cd = format!("cd '{}'\r", cwd.display().to_string().replace('\'', "'\\''"));
            self.write(cd.as_bytes());
            self.cwd = Some(cwd.to_path_buf());
        }
        self.write(format!("{cmd}\r").as_bytes());
    }

    /// Clear the screen and scrollback; the shell redraws its prompt.
    pub fn clear(&mut self) {
        if let Ok(mut p) = self.parser.lock() {
            p.process(b"\x1b[2J\x1b[3J\x1b[H");
        }
        self.write(b"\r");
    }

    /// End the shell; a new one starts the next time the terminal is used.
    pub fn kill(&mut self) {
        if let Some(c) = self.child.as_mut() {
            let _ = c.kill();
        }
        self.child = None;
        self.writer = None;
        self.master = None;
        self.alive.store(false, Ordering::Relaxed);
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.kill();
    }
}
