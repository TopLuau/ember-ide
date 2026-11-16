//! Small helpers shared across the IDE.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;

pub fn hash_of<T: Hash + ?Sized>(v: &T) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// Open a file or folder with the operating system's default handler.
pub fn open_in_os(path: &Path) {
    #[cfg(windows)]
    {
        let _ = Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }
}

/// Open a web address in the default browser.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", url]);
        no_window(&mut c);
        let _ = c.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}

/// Stop a console window flashing up when spawning child processes on Windows.
#[allow(unused_variables)]
pub fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
}

/// Remove ANSI escape sequences from terminal output.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for n in chars.by_ref() {
                        if n.is_ascii_alphabetic() || n == '~' {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    for n in chars.by_ref() {
                        if n == '\x07' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

pub fn char_to_byte(s: &str, c: usize) -> usize {
    s.char_indices().nth(c).map(|(b, _)| b).unwrap_or(s.len())
}

pub fn byte_to_char(s: &str, b: usize) -> usize {
    s[..b.min(s.len())].chars().count()
}

pub fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

/// Directories that are never worth walking into for search / quick-open.
pub fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "target" | "node_modules" | ".idea" | ".vscode" | "__pycache__" | ".venv" | "venv" | "dist" | "build" | ".next"
    )
}

/// Subsequence fuzzy matcher. Higher is better.
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let name_start = t.iter().rposition(|c| *c == '/' || *c == '\\').map(|i| i + 1).unwrap_or(0);
    let mut qi = 0;
    let mut score = 0i32;
    let mut prev = false;
    for (i, c) in t.iter().enumerate() {
        if qi < q.len() && *c == q[qi] {
            score += 10;
            if prev {
                score += 15;
            }
            if i == 0 || matches!(t[i - 1], '/' | '\\' | '_' | '-' | '.' | ' ') {
                score += 20;
            }
            if i >= name_start {
                score += 6;
            }
            qi += 1;
            prev = true;
        } else {
            prev = false;
        }
    }
    if qi == q.len() {
        Some(score - (t.len() as i32) / 6)
    } else {
        None
    }
}

/// Line comment prefix for a file extension.
pub fn comment_prefix(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "rs" | "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" | "java" | "go" | "cs" | "swift"
        | "kt" | "kts" | "scala" | "dart" | "php" | "zig" | "json5" | "glsl" | "wgsl" => "//",
        "py" | "rb" | "sh" | "bash" | "zsh" | "toml" | "yml" | "yaml" | "ps1" | "r" | "pl" | "mk" | "cmake" | "conf" | "ini" | "gitignore"
        | "dockerfile" | "nim" => "#",
        "lua" | "sql" | "hs" | "elm" => "--",
        "vim" => "\"",
        "bat" | "cmd" => "REM",
        "lisp" | "clj" | "el" | "scm" => ";",
        "tex" => "%",
        _ => return None,
    })
}

/// Human readable language name for Discord presence and the status bar fallback.
pub fn lang_label(ext: &str) -> &'static str {
    match ext {
        "rs" => "Rust",
        "py" => "Python",
        "js" | "mjs" | "cjs" | "jsx" => "JavaScript",
        "ts" | "tsx" => "TypeScript",
        "c" | "h" => "C",
        "cpp" | "cc" | "cxx" | "hpp" => "C++",
        "cs" => "C#",
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "swift" => "Swift",
        "rb" => "Ruby",
        "php" => "PHP",
        "lua" => "Lua",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "json" => "JSON",
        "toml" => "TOML",
        "yml" | "yaml" => "YAML",
        "md" => "Markdown",
        "sh" | "bash" => "Shell",
        "rhai" => "Rhai",
        _ => "Plain Text",
    }
}

/// Name of the Rich Presence art asset for a file extension (see assets/presence).
pub fn lang_key(ext: &str) -> &'static str {
    match ext {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "cs" => "csharp",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "rb" => "ruby",
        "php" => "php",
        "lua" => "lua",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "yml" | "yaml" => "yaml",
        "md" => "markdown",
        "sh" | "bash" => "shell",
        "rhai" => "rhai",
        _ => "file",
    }
}

/// Current UTC time as `YYYY-MM-DD HH:MM:SS` (no external date crate needed).
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // civil_from_days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "util_tests.rs"]
mod tests;
