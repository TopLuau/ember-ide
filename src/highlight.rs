//! Syntax highlighting.
//!
//! `syntect` is used purely as a *scope parser*; scopes are mapped onto our own token
//! classes so that every IDE theme colours code consistently. Highlighting is incremental:
//! per-line results are cached together with the parser state after the line, and reused
//! while the line text and incoming state are unchanged.

use crate::theme::{Theme, Tok};
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Files above this size are shown without highlighting to keep typing responsive.
const MAX_HIGHLIGHT_BYTES: usize = 1_500_000;

struct LineEntry {
    text: String,
    after: ParseState,
    stack: ScopeStack,
    toks: Vec<(usize, usize, Tok)>,
}

#[derive(Default)]
pub struct HlCache {
    lang: String,
    lines: Vec<Rc<LineEntry>>,
}

pub struct Highlighter {
    ss: SyntaxSet,
    scope_cache: RefCell<HashMap<Scope, Option<Tok>>>,
}

fn map_scope(s: &str) -> Option<Tok> {
    let starts = |p: &str| s == p || s.starts_with(&format!("{p}."));
    // Quotes / comment delimiters take the colour of their parent.
    if s.starts_with("punctuation.definition.string") || s.starts_with("punctuation.definition.comment") {
        return None;
    }
    if starts("comment") {
        return Some(Tok::Comment);
    }
    if starts("string") {
        return Some(Tok::Str);
    }
    if starts("constant.numeric") {
        return Some(Tok::Number);
    }
    if starts("constant") || starts("support.constant") {
        return Some(Tok::Constant);
    }
    if starts("keyword.operator") {
        return Some(Tok::Operator);
    }
    if starts("keyword") || starts("storage") {
        return Some(Tok::Keyword);
    }
    if starts("entity.name.function") || starts("support.function") || starts("variable.function") || starts("meta.function-call") {
        return Some(Tok::Function);
    }
    if starts("entity.name.tag") {
        return Some(Tok::Keyword);
    }
    if starts("entity.name") || starts("entity.other") || starts("support.type") || starts("support.class") {
        return Some(Tok::Type);
    }
    if starts("variable.parameter") || starts("variable") {
        return Some(Tok::Variable);
    }
    if starts("punctuation") {
        return Some(Tok::Punct);
    }
    if starts("markup.heading") {
        return Some(Tok::Keyword);
    }
    if starts("markup.raw") || starts("markup.inline") {
        return Some(Tok::Str);
    }
    if starts("invalid") {
        return Some(Tok::Constant);
    }
    None
}

impl Highlighter {
    pub fn new() -> Self {
        Self { ss: SyntaxSet::load_defaults_newlines(), scope_cache: RefCell::new(HashMap::new()) }
    }

    fn find(&self, ext: &str) -> &SyntaxReference {
        let ext = match ext {
            "ts" | "tsx" | "jsx" | "mjs" | "cjs" => "js",
            "rhai" => "rs",
            "hpp" | "cc" | "cxx" => "cpp",
            "bash" | "zsh" => "sh",
            "yml" => "yaml",
            "markdown" => "md",
            x => x,
        };
        self.ss.find_syntax_by_extension(ext).unwrap_or_else(|| self.ss.find_syntax_plain_text())
    }

    pub fn language_name(&self, ext: &str) -> String {
        self.find(ext).name.clone()
    }

    fn scope_tok(&self, sc: Scope) -> Option<Tok> {
        if let Some(t) = self.scope_cache.borrow().get(&sc) {
            return *t;
        }
        let t = map_scope(&sc.build_string());
        self.scope_cache.borrow_mut().insert(sc, t);
        t
    }

    fn classify(&self, stack: &ScopeStack) -> Tok {
        for sc in stack.as_slice().iter().rev() {
            if let Some(t) = self.scope_tok(*sc) {
                return t;
            }
        }
        Tok::Plain
    }

    fn parse_line(&self, line: &str, ps: &mut ParseState, stack: &mut ScopeStack) -> LineEntry {
        let mut toks = Vec::new();
        match ps.parse_line(line, &self.ss) {
            Ok(ops) => {
                let mut last = 0;
                let mut cur = self.classify(stack);
                for (pos, op) in ops {
                    if pos > last {
                        toks.push((last, pos, cur));
                        last = pos;
                    }
                    let _ = stack.apply(&op);
                    cur = self.classify(stack);
                }
                if last < line.len() {
                    toks.push((last, line.len(), cur));
                }
            }
            Err(_) => toks.push((0, line.len(), Tok::Plain)),
        }
        LineEntry { text: line.to_string(), after: ps.clone(), stack: stack.clone(), toks }
    }

    pub fn build_job(&self, cache: &mut HlCache, ext: &str, text: &str, theme: &Theme, font: &FontId, wrap: f32) -> LayoutJob {
        let mut job = LayoutJob::default();
        job.wrap.max_width = wrap;
        let fmt = |c: Color32| TextFormat { font_id: font.clone(), color: c, ..Default::default() };
        let syntax = self.find(ext);
        if syntax.name == "Plain Text" || text.len() > MAX_HIGHLIGHT_BYTES {
            cache.lines.clear();
            job.append(text, 0.0, fmt(theme.fg));
            return job;
        }
        if cache.lang != syntax.name {
            cache.lang = syntax.name.clone();
            cache.lines.clear();
        }
        let old = std::mem::take(&mut cache.lines);
        let mut new: Vec<Rc<LineEntry>> = Vec::with_capacity(old.len() + 1);
        let mut ps = ParseState::new(syntax);
        let mut stack = ScopeStack::new();
        let mut prev_same = true;
        let mut pending_reuse: Option<Rc<LineEntry>> = None;
        for (i, line) in LinesWithEndings::from(text).enumerate() {
            if prev_same {
                if let Some(o) = old.get(i) {
                    if o.text == line {
                        new.push(o.clone());
                        pending_reuse = Some(o.clone());
                        continue;
                    }
                }
            }
            if let Some(o) = pending_reuse.take() {
                ps = o.after.clone();
                stack = o.stack.clone();
            }
            let entry = self.parse_line(line, &mut ps, &mut stack);
            prev_same = old.get(i).map(|o| o.after == entry.after && o.stack == entry.stack).unwrap_or(false);
            new.push(Rc::new(entry));
        }

        let mut buf = String::new();
        let mut cur: Option<Color32> = None;
        for e in &new {
            for (a, b, t) in &e.toks {
                let col = theme.tok(*t);
                if cur != Some(col) {
                    if let Some(c) = cur {
                        job.append(&buf, 0.0, fmt(c));
                    }
                    buf.clear();
                    cur = Some(col);
                }
                buf.push_str(&e.text[*a..*b]);
            }
        }
        if let Some(c) = cur {
            if !buf.is_empty() {
                job.append(&buf, 0.0, fmt(c));
            }
        }
        cache.lines = new;
        job
    }
}

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod tests;
