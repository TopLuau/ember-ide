use super::*;
use crate::theme::all_themes;

#[test]
fn job_text_matches_input_and_incremental_update_is_consistent() {
    let hl = Highlighter::new();
    let theme = all_themes().remove(0);
    let font = FontId::monospace(14.0);
    let mut cache = HlCache::default();
    let src = "fn main() {\n    let x = \"hi\"; // c\n}\n";
    let j = hl.build_job(&mut cache, "rs", src, &theme, &font, f32::INFINITY);
    assert_eq!(j.text, src);
    assert!(j.sections.len() > 3, "expected several coloured sections");
    // edit one line, then compare with a from-scratch highlight
    let src2 = "fn main() {\n    let y = 42; // c\n}\n";
    let inc = hl.build_job(&mut cache, "rs", src2, &theme, &font, f32::INFINITY);
    let fresh = hl.build_job(&mut HlCache::default(), "rs", src2, &theme, &font, f32::INFINITY);
    assert_eq!(inc.text, src2);
    assert_eq!(inc.sections.len(), fresh.sections.len());
    for (a, b) in inc.sections.iter().zip(fresh.sections.iter()) {
        assert_eq!(a.byte_range, b.byte_range);
        assert_eq!(a.format.color, b.format.color);
    }
}

#[test]
fn multiline_constructs_carry_state_to_later_lines() {
    let hl = Highlighter::new();
    let theme = all_themes().remove(0);
    let font = FontId::monospace(14.0);
    let mut cache = HlCache::default();
    let src = "/* a\nb\nc */ let z = 1;\n";
    let j = hl.build_job(&mut cache, "rs", src, &theme, &font, f32::INFINITY);
    let b = src.find('b').unwrap();
    let sec = j.sections.iter().find(|s| s.byte_range.contains(&b)).unwrap();
    assert_eq!(sec.format.color, theme.tok(Tok::Comment));
}

#[test]
fn editing_a_block_comment_updates_following_lines() {
    let hl = Highlighter::new();
    let theme = all_themes().remove(0);
    let font = FontId::monospace(14.0);
    let mut cache = HlCache::default();
    let _ = hl.build_job(&mut cache, "rs", "let a = 1;\nlet b = 2;\n", &theme, &font, f32::INFINITY);
    // opening a comment on line 1 must recolour line 2 even though its text is unchanged
    let src = "/* let a = 1;\nlet b = 2;\n";
    let inc = hl.build_job(&mut cache, "rs", src, &theme, &font, f32::INFINITY);
    let fresh = hl.build_job(&mut HlCache::default(), "rs", src, &theme, &font, f32::INFINITY);
    assert_eq!(inc.sections.len(), fresh.sections.len());
    for (a, b) in inc.sections.iter().zip(fresh.sections.iter()) {
        assert_eq!(a.format.color, b.format.color);
    }
}

#[test]
fn unknown_extension_is_plain() {
    let hl = Highlighter::new();
    let theme = all_themes().remove(0);
    let j = hl.build_job(&mut HlCache::default(), "zzz", "hello", &theme, &FontId::monospace(14.0), 100.0);
    assert_eq!(j.sections.len(), 1);
}
