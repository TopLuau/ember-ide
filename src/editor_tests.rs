//! Drives the real editor widget headlessly with synthetic key events.

use super::*;
use crate::theme::all_themes;
use eframe::egui::{CentralPanel, Event, Pos2, RawInput};

struct Harness {
    ctx: Context,
    doc: Doc,
    hl: Highlighter,
    cfg: Config,
    theme: Theme,
    id: Id,
}

impl Harness {
    fn new(text: &str, ext: &str, cursor: usize) -> Self {
        let mut doc = Doc::untitled(1, 1);
        doc.path = Some(std::path::PathBuf::from(format!("t.{ext}")));
        doc.text = text.to_string();
        let mut h = Harness {
            ctx: Context::default(),
            doc,
            hl: Highlighter::new(),
            cfg: Config::default(),
            theme: all_themes().remove(0),
            id: Id::new(("editor", 1u64)),
        };
        h.ctx.memory_mut(|m| m.request_focus(h.id));
        h.frame(vec![]);
        h.set_cursor(cursor, cursor);
        h.frame(vec![]);
        h
    }

    fn set_cursor(&mut self, a: usize, b: usize) {
        let mut st = egui::TextEdit::load_state(&self.ctx, self.id).expect("editor state after first frame");
        st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
        st.store(&self.ctx, self.id);
    }

    fn cursor(&self) -> (usize, usize) {
        let r = egui::TextEdit::load_state(&self.ctx, self.id).unwrap().cursor.char_range().unwrap();
        (r.secondary.index, r.primary.index)
    }

    fn frame(&mut self, events: Vec<Event>) {
        let mut input = RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0)));
        input.events = events;
        input.focused = true;
        let env = EditorEnv { theme: &self.theme, hl: &self.hl, cfg: &self.cfg, matches: &[], current_match: None, exec_line: None };
        let doc = &mut self.doc;
        self.ctx.run(input, |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                show(ui, doc, &env);
            });
        });
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }]);
        self.frame(vec![]);
    }

    fn type_text(&mut self, s: &str) {
        self.frame(vec![Event::Text(s.to_string())]);
        self.frame(vec![]);
    }
}

#[test]
fn enter_keeps_indent_and_adds_level_after_open_brace() {
    let mut h = Harness::new("    fn x() {", "rs", 12);
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.doc.text, "    fn x() {\n        ");
    assert_eq!(h.cursor().1, h.doc.text.chars().count());
}

#[test]
fn enter_between_braces_opens_a_block() {
    let mut h = Harness::new("{}", "rs", 1);
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.doc.text, "{\n    \n}");
    assert_eq!(h.cursor().1, 6);
}

#[test]
fn python_colon_indents() {
    let mut h = Harness::new("if x:", "py", 5);
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.doc.text, "if x:\n    ");
}

#[test]
fn tab_inserts_spaces_to_next_stop_and_shift_tab_removes() {
    let mut h = Harness::new("ab", "rs", 2);
    h.key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.doc.text, "ab  ");
    h.set_cursor(0, 0);
    h.frame(vec![]);
    h.doc.text = "        x".into();
    h.set_cursor(8, 8);
    h.frame(vec![]);
    h.key(Key::Tab, Modifiers::SHIFT);
    assert_eq!(h.doc.text, "    x");
}

#[test]
fn tab_on_multi_line_selection_indents_every_line() {
    let mut h = Harness::new("a\nb\nc", "rs", 0);
    h.set_cursor(0, 5);
    h.frame(vec![]);
    h.key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.doc.text, "    a\n    b\n    c");
}

#[test]
fn brackets_and_quotes_autopair_and_overtype() {
    let mut h = Harness::new("", "rs", 0);
    h.type_text("(");
    assert_eq!(h.doc.text, "()");
    assert_eq!(h.cursor().1, 1);
    h.type_text(")");
    assert_eq!(h.doc.text, "()");
    assert_eq!(h.cursor().1, 2);
    h.type_text("\"");
    assert_eq!(h.doc.text, "()\"\"");
    h.type_text("\"");
    assert_eq!(h.doc.text, "()\"\"");
    assert_eq!(h.cursor().1, 4);
}

#[test]
fn rust_apostrophe_is_not_paired_but_python_quote_is() {
    let mut h = Harness::new("", "rs", 0);
    h.type_text("'");
    assert_eq!(h.doc.text, "'");
    let mut p = Harness::new("", "py", 0);
    p.type_text("'");
    assert_eq!(p.doc.text, "''");
}

#[test]
fn typing_open_bracket_before_a_word_does_not_pair() {
    let mut h = Harness::new("word", "rs", 0);
    h.type_text("(");
    assert_eq!(h.doc.text, "(word");
}

#[test]
fn selection_is_wrapped_in_brackets() {
    let mut h = Harness::new("abc", "rs", 0);
    h.set_cursor(0, 3);
    h.frame(vec![]);
    h.type_text("[");
    assert_eq!(h.doc.text, "[abc]");
}

#[test]
fn comment_toggle_round_trip() {
    let mut h = Harness::new("fn a() {}\n    let b = 1;", "rs", 0);
    h.set_cursor(0, 22);
    h.frame(vec![]);
    h.key(Key::Slash, Modifiers::COMMAND);
    assert_eq!(h.doc.text, "// fn a() {}\n//     let b = 1;");
    h.key(Key::Slash, Modifiers::COMMAND);
    assert_eq!(h.doc.text, "fn a() {}\n    let b = 1;");
}

#[test]
fn move_duplicate_and_delete_lines() {
    let mut h = Harness::new("one\ntwo\nthree", "rs", 0);
    h.key(Key::ArrowDown, Modifiers::ALT);
    assert_eq!(h.doc.text, "two\none\nthree");
    h.key(Key::ArrowUp, Modifiers::ALT);
    assert_eq!(h.doc.text, "one\ntwo\nthree");
    h.key(Key::ArrowDown, Modifiers::ALT | Modifiers::SHIFT);
    assert_eq!(h.doc.text, "one\none\ntwo\nthree");
    h.key(Key::K, Modifiers::COMMAND | Modifiers::SHIFT);
    assert!(h.doc.text == "one\ntwo\nthree" || h.doc.text == "one\none\nthree", "{:?}", h.doc.text);
}

#[test]
fn unicode_text_never_panics() {
    let mut h = Harness::new("é😀 fn ü() {\n  ñ", "rs", 4);
    for k in [Key::Enter, Key::Tab] {
        h.key(k, Modifiers::NONE);
    }
    h.key(Key::Tab, Modifiers::SHIFT);
    h.key(Key::Slash, Modifiers::COMMAND);
    h.key(Key::ArrowDown, Modifiers::ALT);
    h.key(Key::ArrowUp, Modifiers::ALT);
    h.type_text("(");
    h.type_text("\"");
    h.key(Key::K, Modifiers::COMMAND | Modifiers::SHIFT);
    assert!(!h.doc.text.is_empty());
}

#[test]
fn edits_mark_document_dirty_and_bump_revision() {
    let mut h = Harness::new("x", "rs", 1);
    let rev = h.doc.rev;
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.doc.dirty);
    assert!(h.doc.rev > rev);
}

#[test]
fn gutter_reports_line_and_column() {
    let mut h = Harness::new("ab\ncd", "rs", 4);
    h.frame(vec![]);
    assert_eq!(h.doc.cursor, (2, 2));
}
