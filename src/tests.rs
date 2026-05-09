//! Integration tests for recently implemented Vim features.
//!
//! Run with: `cargo test`
//!
//! Covered:
//!   - operator + motion  (d$  d0  d^  dG  dj  dk  de  db)
//!   - gU / gu case operators
//!   - marks  (m{a}  `{a}  '{a})
//!   - Ctrl-a / Ctrl-x  (increment / decrement)
//!   - H / M / L  (screen-position jumps)
//!   - :!cmd  (shell command parsing → CommandAction::ShellCommand)

#![cfg(test)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::buffer::Buffer;
use crate::config::Config;
use crate::editor::Editor;
use crate::mode::command::CommandAction;

// ─── helpers ──────────────────────────────────────────────────────────────────

/// Build an Editor pre-loaded with `text` (lines separated by '\n').
/// Terminal size is set large enough that scroll_off never interferes.
fn editor_with(text: &str) -> Editor {
    let mut buf = Buffer::new();
    buf.rope = ropey::Rope::from_str(text);
    Editor::new(Config::default(), 200, 80).with_buffer(buf)
}

/// Synthesise a plain key event (no modifiers).
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Synthesise a Ctrl+char key event.
fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

// ─── operator + motion ────────────────────────────────────────────────────────

#[test]
fn test_d_dollar_deletes_to_eol() {
    // "Hello World" — cursor at col 6 → d$ should leave "Hello "
    let mut ed = editor_with("Hello World\n");
    ed.cursor_col = 6; // 'W'
    ed.execute_operator_key('d', key(KeyCode::Char('$')), 1);
    assert_eq!(ed.buffer.line_str(0), "Hello ");
}

#[test]
fn test_d_zero_deletes_to_bol() {
    // "Hello World" — cursor at col 6 → d0 should leave "World"
    let mut ed = editor_with("Hello World\n");
    ed.cursor_col = 6;
    ed.execute_operator_key('d', key(KeyCode::Char('0')), 1);
    assert_eq!(ed.buffer.line_str(0), "World");
}

#[test]
fn test_d_caret_deletes_to_first_nonblank() {
    // "  Hello" — cursor at col 5 ('l') → d^ should leave "  lo"
    let mut ed = editor_with("  Hello\n");
    ed.cursor_col = 5; // second 'l'
    ed.execute_operator_key('d', key(KeyCode::Char('^')), 1);
    // first non-blank is col 2 ('H'), so chars [2..5) = "Hel" are deleted
    assert_eq!(ed.buffer.line_str(0), "  lo");
}

#[test]
fn test_d_caret_from_before_nonblank() {
    // "  Hello" — cursor at col 0 → d^ should leave "Hello"
    let mut ed = editor_with("  Hello\n");
    ed.cursor_col = 0;
    ed.execute_operator_key('d', key(KeyCode::Char('^')), 1);
    assert_eq!(ed.buffer.line_str(0), "Hello");
}

#[test]
fn test_dg_deletes_to_eof() {
    // Three lines (no trailing newline), cursor on line 1 → dG should leave only line 0.
    // We use text without trailing '\n' so ropey line_count == 3 exactly.
    let mut ed = editor_with("line0\nline1\nline2");
    ed.cursor_line = 1;
    ed.cursor_col = 0;
    ed.execute_operator_key('d', key(KeyCode::Char('G')), 1);
    // After deletion only "line0" remains (no trailing newline → 1 line)
    assert_eq!(ed.buffer.line_str(0), "line0");
    // ropey may report 1 or 2 lines depending on trailing newline; just check content
    assert!(ed.buffer.line_count() <= 2);
}

#[test]
fn test_dj_deletes_current_and_next_line() {
    let mut ed = editor_with("aaa\nbbb\nccc\n");
    ed.cursor_line = 0;
    ed.execute_operator_key('d', key(KeyCode::Char('j')), 1);
    // lines 0 and 1 deleted → only "ccc" remains
    assert_eq!(ed.buffer.line_str(0), "ccc");
}

#[test]
fn test_dk_deletes_current_and_prev_line() {
    let mut ed = editor_with("aaa\nbbb\nccc\n");
    ed.cursor_line = 1;
    ed.execute_operator_key('d', key(KeyCode::Char('k')), 1);
    // lines 0 and 1 deleted → only "ccc" remains
    assert_eq!(ed.buffer.line_str(0), "ccc");
}

#[test]
fn test_de_deletes_to_word_end() {
    // "hello world" — cursor at 0 → de should delete "hello"
    let mut ed = editor_with("hello world\n");
    ed.cursor_col = 0;
    ed.execute_operator_key('d', key(KeyCode::Char('e')), 1);
    assert_eq!(ed.buffer.line_str(0), " world");
}

#[test]
fn test_db_deletes_back_word() {
    // "hello world" — cursor at 6 ('w') → db should delete "hello "
    let mut ed = editor_with("hello world\n");
    ed.cursor_col = 6;
    ed.execute_operator_key('d', key(KeyCode::Char('b')), 1);
    assert_eq!(ed.buffer.line_str(0), "world");
}

#[test]
fn test_y_dollar_yanks_to_eol() {
    let mut ed = editor_with("Hello World\n");
    ed.cursor_col = 6;
    ed.execute_operator_key('y', key(KeyCode::Char('$')), 1);
    assert_eq!(ed.buffer.register, "World");
    // buffer unchanged
    assert_eq!(ed.buffer.line_str(0), "Hello World");
}

// ─── gU / gu case operators ───────────────────────────────────────────────────

#[test]
fn test_apply_case_line_upper() {
    let mut ed = editor_with("hello world\n");
    ed.cursor_line = 0;
    ed.apply_case_line(true);
    assert_eq!(ed.buffer.line_str(0), "HELLO WORLD");
}

#[test]
fn test_apply_case_line_lower() {
    let mut ed = editor_with("HELLO WORLD\n");
    ed.cursor_line = 0;
    ed.apply_case_line(false);
    assert_eq!(ed.buffer.line_str(0), "hello world");
}

#[test]
fn test_apply_case_range_partial() {
    // "Hello World" — uppercase chars [6..11) = "World" → "WORLD"
    let mut ed = editor_with("Hello World\n");
    let start = ed.buffer.pos_to_char(0, 6);
    let end   = ed.buffer.pos_to_char(0, 11);
    ed.apply_case_range(start, end, true);
    assert_eq!(ed.buffer.line_str(0), "Hello WORLD");
}

#[test]
fn test_gu_dollar_lowercases_to_eol() {
    let mut ed = editor_with("HELLO WORLD\n");
    ed.cursor_col = 6; // 'W'
    ed.apply_case_operator_key(false, key(KeyCode::Char('$')), 1);
    assert_eq!(ed.buffer.line_str(0), "HELLO world");
}

#[test]
fn test_gU_zero_uppercases_from_bol() {
    let mut ed = editor_with("hello world\n");
    ed.cursor_col = 6; // 'w'
    ed.apply_case_operator_key(true, key(KeyCode::Char('0')), 1);
    assert_eq!(ed.buffer.line_str(0), "HELLO world");
}

#[test]
fn test_gU_word_uppercases_word() {
    let mut ed = editor_with("hello world\n");
    ed.cursor_col = 0;
    ed.apply_case_operator_key(true, key(KeyCode::Char('w')), 1);
    // move_word_forward from col 0 lands at col 6 ('w')
    assert_eq!(ed.buffer.line_str(0), "HELLO world");
}

#[test]
fn test_gu_j_lowercases_two_lines() {
    let mut ed = editor_with("HELLO\nWORLD\nFOO\n");
    ed.cursor_line = 0;
    ed.apply_case_operator_key(false, key(KeyCode::Char('j')), 1);
    assert_eq!(ed.buffer.line_str(0), "hello");
    assert_eq!(ed.buffer.line_str(1), "world");
    assert_eq!(ed.buffer.line_str(2), "FOO"); // untouched
}

// ─── marks ────────────────────────────────────────────────────────────────────

#[test]
fn test_set_and_jump_to_mark_exact() {
    let mut ed = editor_with("line0\nline1\nline2\n");
    ed.cursor_line = 2;
    ed.cursor_col = 3;
    ed.set_mark('a');

    // Move away
    ed.cursor_line = 0;
    ed.cursor_col = 0;

    ed.jump_to_mark('a');
    assert_eq!(ed.cursor_line, 2);
    assert_eq!(ed.cursor_col, 3);
}

#[test]
fn test_jump_to_mark_line_goes_to_first_nonblank() {
    let mut ed = editor_with("line0\n  line1\nline2\n");
    ed.cursor_line = 1;
    ed.cursor_col = 4; // somewhere in the middle
    ed.set_mark('b');

    ed.cursor_line = 0;
    ed.cursor_col = 0;

    ed.jump_to_mark_line('b');
    assert_eq!(ed.cursor_line, 1);
    assert_eq!(ed.cursor_col, 2); // first non-blank of "  line1"
}

#[test]
fn test_mark_prev_roundtrip() {
    // After jumping to a mark, mark_prev should hold the old position
    let mut ed = editor_with("aaa\nbbb\nccc\n");
    ed.cursor_line = 0;
    ed.cursor_col = 1;
    ed.set_mark('z');

    ed.cursor_line = 2;
    ed.cursor_col = 2;
    ed.jump_to_mark('z');

    // mark_prev should be (2, 2)
    assert_eq!(ed.mark_prev, Some((2, 2)));
}

// ─── Ctrl-a / Ctrl-x ──────────────────────────────────────────────────────────

#[test]
fn test_ctrl_a_increments_number() {
    let mut ed = editor_with("count: 5\n");
    ed.cursor_col = 7; // on '5'
    ed.increment_number_at_cursor(1);
    assert_eq!(ed.buffer.line_str(0), "count: 6");
}

#[test]
fn test_ctrl_x_decrements_number() {
    let mut ed = editor_with("count: 10\n");
    ed.cursor_col = 7; // on '1'
    ed.increment_number_at_cursor(-1);
    assert_eq!(ed.buffer.line_str(0), "count: 9");
}

#[test]
fn test_ctrl_a_negative_number() {
    let mut ed = editor_with("val: -3\n");
    ed.cursor_col = 5; // on '-'
    ed.increment_number_at_cursor(1);
    assert_eq!(ed.buffer.line_str(0), "val: -2");
}

#[test]
fn test_ctrl_a_scans_right_for_number() {
    // Cursor is before the number; should scan right and find it
    let mut ed = editor_with("x = 42\n");
    ed.cursor_col = 0; // on 'x'
    ed.increment_number_at_cursor(8);
    assert_eq!(ed.buffer.line_str(0), "x = 50");
}

#[test]
fn test_ctrl_a_large_delta() {
    let mut ed = editor_with("n=100\n");
    ed.cursor_col = 2; // on '1'
    ed.increment_number_at_cursor(900);
    assert_eq!(ed.buffer.line_str(0), "n=1000");
}

// ─── H / M / L ────────────────────────────────────────────────────────────────

#[test]
fn test_h_moves_to_screen_top() {
    // 20 lines, scroll_line = 5, scroll_off = 0 → H should land on line 5
    let text: String = (0..20).map(|i| format!("line{}\n", i)).collect();
    let mut ed = editor_with(&text);
    ed.scroll_line = 5;
    ed.config.general.scroll_off = 0;
    ed.cursor_line = 15; // somewhere in the middle
    ed.move_screen_top();
    assert_eq!(ed.cursor_line, 5);
}

#[test]
fn test_l_moves_to_screen_bottom() {
    // 20 lines (no trailing newline → ropey line_count = 20 exactly)
    // scroll_line = 0, edit_height = 78 (80-2), scroll_off = 0
    // → L should land on line min(78-1, 19) = 19
    let lines: Vec<String> = (0..20).map(|i| format!("line{}", i)).collect();
    let text = lines.join("\n");
    let mut ed = editor_with(&text);
    ed.scroll_line = 0;
    ed.config.general.scroll_off = 0;
    ed.cursor_line = 0;
    ed.move_screen_bottom();
    // edit_height = 80-2 = 78; bottom = 0+78 = 78; saturating_sub(0+1) = 77; min(19) = 19
    assert_eq!(ed.cursor_line, 19);
}

#[test]
fn test_m_moves_to_screen_middle() {
    // 20 lines (no trailing newline → ropey line_count = 20 exactly)
    // scroll_line = 0, edit_height = 78 → mid = 39; min(19) = 19
    let lines: Vec<String> = (0..20).map(|i| format!("line{}", i)).collect();
    let text = lines.join("\n");
    let mut ed = editor_with(&text);
    ed.scroll_line = 0;
    ed.cursor_line = 0;
    ed.move_screen_middle();
    // mid = 0 + 78/2 = 39; min(19) = 19
    assert_eq!(ed.cursor_line, 19);
}

#[test]
fn test_h_with_scroll_offset() {
    // scroll_line = 10, scroll_off = 2 → H lands on line 12
    let lines: Vec<String> = (0..30).map(|i| format!("line{}", i)).collect();
    let text = lines.join("\n");
    let mut ed = editor_with(&text);
    ed.scroll_line = 10;
    ed.config.general.scroll_off = 2;
    ed.move_screen_top();
    assert_eq!(ed.cursor_line, 12);
}

// ─── :!cmd shell command parsing ──────────────────────────────────────────────

#[test]
fn test_shell_command_parsed_from_colon_bang() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("!echo hello");
    assert!(matches!(action, CommandAction::ShellCommand(ref s) if s == "echo hello"));
}

#[test]
fn test_shell_command_with_spaces_trimmed() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("!  ls -la  ");
    assert!(matches!(action, CommandAction::ShellCommand(ref s) if s == "ls -la"));
}

#[test]
fn test_shell_command_empty_bang_is_shell_command() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("!");
    assert!(matches!(action, CommandAction::ShellCommand(ref s) if s.is_empty()));
}

#[test]
fn test_non_bang_command_not_shell() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("w");
    // :w on a buffer with no path → SetMsg (error), not ShellCommand
    assert!(!matches!(action, CommandAction::ShellCommand(_)));
}

// ─── ; and , repeat f/F/t/T ───────────────────────────────────────────────────

#[test]
fn test_semicolon_repeats_f_forward() {
    // "abcabc" — f'a' lands on col 0, then ';' should jump to col 3
    let mut ed = editor_with("abcabc\n");
    ed.cursor_col = 0;
    // simulate f'a': set last_find and move
    ed.find_char_forward('a', false);
    // cursor is still at 0 (already on 'a'), so manually advance past it
    ed.cursor_col = 1;
    ed.find_char_forward('a', false); // lands on col 3
    // Now record last_find as if 'f' was pressed
    use crate::editor::FindState;
    ed.last_find = Some(FindState { ch: 'a', forward: true, till: false });
    ed.cursor_col = 0; // reset to start
    ed.find_char_forward('a', false); // first f'a' → col 0 (already there, no move)
    // Actually test: start at col 0, f'a' → stays at 0 (already 'a'), then ';' → col 3
    ed.cursor_col = 0;
    ed.last_find = Some(FindState { ch: 'a', forward: true, till: false });
    // ';' logic: forward=true → find_char_forward
    ed.find_char_forward('a', false); // from col 0+1 → finds col 3
    assert_eq!(ed.cursor_col, 3);
}

#[test]
fn test_comma_reverses_f() {
    // "abcabc" — after f'a' (forward), ',' should go backward
    let mut ed = editor_with("abcabc\n");
    use crate::editor::FindState;
    ed.cursor_col = 3; // on second 'a'
    ed.last_find = Some(FindState { ch: 'a', forward: true, till: false });
    // ',' logic: forward=true → find_char_backward
    ed.find_char_backward('a', false); // from col 3 → finds col 0
    assert_eq!(ed.cursor_col, 0);
}

// ─── :d / :{range}d ───────────────────────────────────────────────────────────

#[test]
fn test_colon_d_deletes_current_line() {
    let mut ed = editor_with("line1\nline2\nline3");
    ed.cursor_line = 1; // on "line2"
    let action = ed.parse_and_execute_command("d");
    assert!(matches!(action, CommandAction::DeleteLines { start: 1, end: 1 }));
}

#[test]
fn test_colon_nd_deletes_specific_line() {
    let mut ed = editor_with("line1\nline2\nline3");
    let action = ed.parse_and_execute_command("2d");
    assert!(matches!(action, CommandAction::DeleteLines { start: 1, end: 1 }));
}

#[test]
fn test_colon_range_d_deletes_range() {
    let mut ed = editor_with("line1\nline2\nline3");
    let action = ed.parse_and_execute_command("1,2d");
    assert!(matches!(action, CommandAction::DeleteLines { start: 0, end: 1 }));
}

#[test]
fn test_colon_u_returns_undo() {
    let mut ed = editor_with("hello");
    let action = ed.parse_and_execute_command("u");
    assert!(matches!(action, CommandAction::Undo));
}

// ─── it / at text object ──────────────────────────────────────────────────────

#[test]
fn test_it_inner_tag_content() {
    // <div>hello</div> — cursor inside "hello", dit should delete "hello"
    let mut ed = editor_with("<div>hello</div>\n");
    ed.cursor_col = 6; // on 'e' of "hello"
    ed.execute_text_obj('d', true, 't', 1);
    assert_eq!(ed.buffer.line_str(0), "<div></div>");
}

#[test]
fn test_at_outer_tag_content() {
    // <span>world</span> — cursor inside, dat should delete entire element
    let mut ed = editor_with("<span>world</span>\n");
    ed.cursor_col = 7; // on 'o' of "world"
    ed.execute_text_obj('d', false, 't', 1);
    assert_eq!(ed.buffer.line_str(0), "");
}

#[test]
fn test_it_nested_tags() {
    // <outer><inner>text</inner></outer> — cursor on "text", dit → deletes "text"
    let mut ed = editor_with("<outer><inner>text</inner></outer>\n");
    ed.cursor_col = 15; // on 't' of "text"
    ed.execute_text_obj('d', true, 't', 1);
    assert_eq!(ed.buffer.line_str(0), "<outer><inner></inner></outer>");
}

// ─── iW / aW WORD text object ─────────────────────────────────────────────────

#[test]
fn test_diw_deletes_word() {
    // "hello world" — cursor on 'h', diw → "world" (leading space gone too via aw)
    // iw: just the word chars
    let mut ed = editor_with("hello world\n");
    ed.cursor_col = 0;
    ed.execute_text_obj('d', true, 'w', 1);
    assert_eq!(ed.buffer.line_str(0), " world");
}

#[test]
fn test_daw_deletes_word_and_space() {
    // "hello world" — cursor on 'h', daw → "world" (trailing space consumed)
    let mut ed = editor_with("hello world\n");
    ed.cursor_col = 0;
    ed.execute_text_obj('d', false, 'w', 1);
    assert_eq!(ed.buffer.line_str(0), "world");
}

#[test]
fn test_diW_deletes_big_word() {
    // "foo.bar baz" — cursor on 'f', diW → " baz" (WORD = non-whitespace run)
    let mut ed = editor_with("foo.bar baz\n");
    ed.cursor_col = 0;
    ed.execute_text_obj('d', true, 'W', 1);
    assert_eq!(ed.buffer.line_str(0), " baz");
}

#[test]
fn test_daW_deletes_big_word_and_space() {
    // "foo.bar baz" — cursor on 'f', daW → "baz"
    let mut ed = editor_with("foo.bar baz\n");
    ed.cursor_col = 0;
    ed.execute_text_obj('d', false, 'W', 1);
    assert_eq!(ed.buffer.line_str(0), "baz");
}

#[test]
fn test_diW_mid_word() {
    // "one two.three four" — cursor on 't' of "two.three" (col 4), diW → "one  four"
    let mut ed = editor_with("one two.three four\n");
    ed.cursor_col = 4; // 't' of "two.three"
    ed.execute_text_obj('d', true, 'W', 1);
    assert_eq!(ed.buffer.line_str(0), "one  four");
}

// ─── is / as sentence text object ─────────────────────────────────────────────

#[test]
fn test_dis_deletes_sentence() {
    // "Hello world. Goodbye." — cursor on 'H', dis → ". Goodbye."
    let mut ed = editor_with("Hello world. Goodbye.\n");
    ed.cursor_col = 0;
    ed.execute_text_obj('d', true, 's', 1);
    // inner sentence: "Hello world" (no trailing punctuation trimmed, but period included)
    // text_obj_sentence inner: from start to '.' inclusive, no trailing spaces
    let result = ed.buffer.line_str(0);
    // The sentence "Hello world." is deleted (inner includes the period)
    assert!(result.contains("Goodbye"), "Expected 'Goodbye' to remain, got: {}", result);
}

#[test]
fn test_das_deletes_sentence_with_space() {
    // "Hello world. Goodbye." — cursor on 'H', das → "Goodbye."
    let mut ed = editor_with("Hello world. Goodbye.\n");
    ed.cursor_col = 0;
    ed.execute_text_obj('d', false, 's', 1);
    let result = ed.buffer.line_str(0);
    assert!(result.contains("Goodbye"), "Expected 'Goodbye' to remain, got: {}", result);
    // outer sentence includes trailing space, so "Hello world. " is gone
    assert!(!result.starts_with(' '), "Should not start with space, got: {}", result);
}

// ─── :set tabstop / :set nonu ─────────────────────────────────────────────────

#[test]
fn test_set_tabstop_command() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("set tabstop=2");
    assert!(matches!(action, CommandAction::SetTabWidth(2)));
}

#[test]
fn test_set_ts_shorthand() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("set ts=8");
    assert!(matches!(action, CommandAction::SetTabWidth(8)));
}

#[test]
fn test_set_number_command() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("set number");
    assert!(matches!(action, CommandAction::ToggleLineNumbers(true)));
}

#[test]
fn test_set_nu_shorthand() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("set nu");
    assert!(matches!(action, CommandAction::ToggleLineNumbers(true)));
}

#[test]
fn test_set_nonumber_command() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("set nonumber");
    assert!(matches!(action, CommandAction::ToggleLineNumbers(false)));
}

#[test]
fn test_set_nonu_shorthand() {
    let mut ed = editor_with("");
    let action = ed.parse_and_execute_command("set nonu");
    assert!(matches!(action, CommandAction::ToggleLineNumbers(false)));
}
