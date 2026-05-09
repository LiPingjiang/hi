//! Tests for Editor state: cursor movement, scroll, search, clamp.

use hi::buffer::Buffer;
use hi::config::Config;
use hi::editor::Editor;

fn make_editor(content: &str) -> Editor {
    let cfg = Config::default();
    let mut editor = Editor::new(cfg, 80, 24);
    editor.buffer.insert_str(0, content);
    editor
}

// ── Cursor clamping ───────────────────────────────────────────────────────────

#[test]
fn test_clamp_cursor_empty_buffer() {
    let mut ed = make_editor("");
    ed.cursor_line = 99;
    ed.cursor_col  = 99;
    ed.clamp_cursor();
    assert_eq!(ed.cursor_line, 0);
    assert_eq!(ed.cursor_col,  0);
}

#[test]
fn test_clamp_cursor_within_line() {
    let mut ed = make_editor("hello\nworld");
    ed.cursor_line = 0;
    ed.cursor_col  = 100;
    ed.clamp_cursor();
    // Normal mode: col can be at most len-1
    assert_eq!(ed.cursor_col, 4); // "hello" len=5, max col = 4
}

// ── Motion: move_down / move_up ───────────────────────────────────────────────

#[test]
fn test_move_down_stays_within_bounds() {
    let mut ed = make_editor("a\nb\nc");
    ed.cursor_line = 0;
    ed.move_down(10);
    assert_eq!(ed.cursor_line, 2); // clamped to last line
}

#[test]
fn test_move_up_stays_within_bounds() {
    let mut ed = make_editor("a\nb\nc");
    ed.cursor_line = 2;
    ed.move_up(10);
    assert_eq!(ed.cursor_line, 0);
}

// ── Motion: move_right / move_left ───────────────────────────────────────────

#[test]
fn test_move_right_within_line() {
    let mut ed = make_editor("hello");
    ed.cursor_col = 0;
    ed.move_right(3);
    assert_eq!(ed.cursor_col, 3);
}

#[test]
fn test_move_left_clamps_at_zero() {
    let mut ed = make_editor("hello");
    ed.cursor_col = 1;
    ed.move_left(5);
    assert_eq!(ed.cursor_col, 0);
}

// ── Motion: gg / G ───────────────────────────────────────────────────────────

#[test]
fn test_move_file_top() {
    let mut ed = make_editor("a\nb\nc");
    ed.cursor_line = 2;
    ed.move_file_top();
    assert_eq!(ed.cursor_line, 0);
}

#[test]
fn test_move_file_bottom() {
    let mut ed = make_editor("a\nb\nc");
    ed.move_file_bottom();
    assert_eq!(ed.cursor_line, 2);
}

// ── Motion: move_line_start / end ─────────────────────────────────────────────

#[test]
fn test_move_line_start_and_end() {
    let mut ed = make_editor("  hello world");
    ed.cursor_col = 5;
    ed.move_line_start();
    assert_eq!(ed.cursor_col, 0);

    ed.move_line_end();
    assert_eq!(ed.cursor_col, 12); // "  hello world" len=13, last col=12
}

#[test]
fn test_move_line_start_nonblank() {
    let mut ed = make_editor("   abc");
    ed.cursor_col = 0;
    ed.move_line_start_nonblank();
    assert_eq!(ed.cursor_col, 3); // skip 3 spaces
}

// ── Motion: word forward / back ───────────────────────────────────────────────

#[test]
fn test_move_word_forward() {
    let mut ed = make_editor("hello world foo");
    ed.cursor_col = 0;
    ed.move_word_forward(1);
    assert_eq!(ed.cursor_col, 6); // 'w' of "world"
}

#[test]
fn test_move_word_back() {
    let mut ed = make_editor("hello world foo");
    ed.cursor_col = 6; // 'w' of "world"
    ed.move_word_back(1);
    assert_eq!(ed.cursor_col, 0);
}

// ── Motion: paragraph ────────────────────────────────────────────────────────

#[test]
fn test_move_paragraph_forward() {
    let mut ed = make_editor("a\nb\n\nc\nd");
    ed.cursor_line = 0;
    ed.move_paragraph_forward(1);
    // Should jump to the blank line or just past it
    assert!(ed.cursor_line >= 2, "expected jump past blank line, got {}", ed.cursor_line);
}

// ── Search ───────────────────────────────────────────────────────────────────

#[test]
fn test_run_search_finds_matches() {
    let mut ed = make_editor("foo bar foo baz");
    ed.run_search("foo", false);
    assert_eq!(ed.search_matches.len(), 2);
}

#[test]
fn test_search_next_wraps() {
    let mut ed = make_editor("x y x y x");
    ed.run_search("x", false);
    let count = ed.search_matches.len();
    assert_eq!(count, 3);
    let initial_idx = ed.search_match_idx;
    ed.search_next();
    ed.search_next();
    ed.search_next(); // should wrap around
    assert_eq!(ed.search_match_idx, initial_idx);
}

#[test]
fn test_search_case_insensitive() {
    let mut ed = make_editor("Hello HELLO hello");
    ed.run_search("hello", true);
    assert_eq!(ed.search_matches.len(), 3);
}

#[test]
fn test_search_empty_pattern_clears() {
    let mut ed = make_editor("something");
    ed.run_search("some", false);
    assert!(!ed.search_matches.is_empty());
    ed.run_search("", false);
    assert!(ed.search_matches.is_empty());
}

// ── Jump list ─────────────────────────────────────────────────────────────────

#[test]
fn test_jump_list_back_and_forward() {
    let mut ed = make_editor("a\nb\nc\nd\ne");
    ed.cursor_line = 0; ed.push_jump();
    ed.cursor_line = 3; ed.push_jump();
    ed.cursor_line = 1; // manual move (not pushed)

    let ok = ed.jump_back();
    assert!(ok);
    assert_eq!(ed.cursor_line, 3);

    let ok2 = ed.jump_forward();
    assert!(ok2);
}

// ── scroll_to_cursor ──────────────────────────────────────────────────────────

#[test]
fn test_scroll_follows_cursor_down() {
    // 24 rows terminal → edit height = 22; scroll_off = 5 (default)
    let content: String = (0..50).map(|i| format!("line{}\n", i)).collect();
    let mut ed = make_editor(&content);
    ed.cursor_line = 40;
    ed.scroll_to_cursor();
    // scroll_line should bring cursor into view with scroll_off
    let visible_bottom = ed.scroll_line + ed.edit_height();
    assert!(ed.cursor_line < visible_bottom,
        "cursor {} should be below scroll bottom {}", ed.cursor_line, visible_bottom);
}

// ── cursor_char_idx ───────────────────────────────────────────────────────────

#[test]
fn test_cursor_char_idx() {
    let mut ed = make_editor("abc\nxyz");
    ed.cursor_line = 1;
    ed.cursor_col  = 1;
    // line 0 = "abc\n" (4 chars), so line 1 starts at char 4
    // col 1 → char index 5
    let idx = ed.cursor_char_idx();
    assert_eq!(idx, 5);
}
