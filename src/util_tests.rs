use super::*;

#[test]
fn fuzzy_prefers_tighter_matches() {
    let a = fuzzy_score("app", "src/app.rs").unwrap();
    let b = fuzzy_score("app", "src/a_long_path/p/p.rs").unwrap();
    assert!(a > b);
    assert!(fuzzy_score("xyz", "src/app.rs").is_none());
    assert_eq!(fuzzy_score("", "anything"), Some(0));
}

#[test]
fn strips_ansi() {
    assert_eq!(strip_ansi("\x1b[31mred\x1b[0m text\r\n"), "red text\n");
}

#[test]
fn iso_time_shape() {
    let s = now_iso();
    assert_eq!(s.len(), 19, "{s}");
    assert!(s.starts_with("20"));
}

#[test]
fn char_byte_roundtrip_with_unicode() {
    let s = "aé😀b";
    assert_eq!(char_to_byte(s, 3), "aé😀".len());
    assert_eq!(byte_to_char(s, "aé😀".len()), 3);
}

#[test]
fn comment_prefixes() {
    assert_eq!(comment_prefix("rs"), Some("//"));
    assert_eq!(comment_prefix("py"), Some("#"));
    assert_eq!(comment_prefix("zzz"), None);
}
