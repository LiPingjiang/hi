//! Tests for mode handlers: Normal keys, Insert keys, Command parsing, Visual ops.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hi::buffer::Buffer;
use hi::config::Config;
use hi::editor::Editor;
use hi::mode::Mode;
use hi::mode::normal::NormalAction;
use hi::mode::insert::InsertAction;

fn make_editor(content: &str) -> Editor {
    let mut ed = Editor::new(Config::default(), 80, 24);
    if !content.is_empty() {
        ed.buffer.insert_str(0, content);
    }
    ed
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

// ── Normal mode: mode transitions ────────────────────────────────────────────

#[test]
fn test_normal_i_enters_insert() {
    let mut ed = make_editor("hello");
    let action = ed.handle_normal_key(key(KeyCode::Char('i')));
    assert!(matches!(action, NormalAction::EnterInsert { col_offset: 0 }));
}

#[test]
fn test_normal_a_enters_insert_offset1() {
    let mut ed = make_editor("hello");
    let action = ed.handle_normal_key(key(KeyCode::Char('a')));
    assert!(matches!(action, NormalAction::EnterInsert { col_offset: 1 }));
}

#[test]
fn test_normal_v_enters_visual_char() {
    let mut ed = make_editor("hello");
    let action = ed.handle_normal_key(key(KeyCode::Char('v')));
    assert!(matches!(action, NormalAction::EnterVisual { kind: hi::mode::VisualKind::Char }));
}

#[test]
fn test_normal_V_enters_visual_line() {
    let mut ed = make_editor("hello");
    let action = ed.handle_normal_key(key(KeyCode::Char('V')));
    assert!(matches!(action, NormalAction::EnterVisual { kind: hi::mode::VisualKind::Line }));
}

#[test]
fn test_normal_colon_enters_command() {
    let mut ed = make_editor("hello");
    let action = ed.handle_normal_key(key(KeyCode::Char(':')));
    assert!(matches!(action, NormalAction::EnterCommand));
}

#[test]
fn test_normal_slash_enters_search() {
    let mut ed = make_editor("hello");
    let action = ed.handle_normal_key(key(KeyCode::Char('/')));
    assert!(matches!(action, NormalAction::EnterSearch));
}

#[test]
fn test_normal_question_enters_ai() {
    let mut ed = make_editor("hello");
    let action = ed.handle_normal_key(key(KeyCode::Char('?')));
    assert!(matches!(action, NormalAction::EnterAi));
}

// ── Normal mode: cursor movement ─────────────────────────────────────────────

#[test]
fn test_normal_hjkl_moves_cursor() {
    let mut ed = make_editor("abc\ndef");
    // move right
    ed.handle_normal_key(key(KeyCode::Char('l')));
    assert_eq!(ed.cursor_col, 1);
    // move down
    ed.handle_normal_key(key(KeyCode::Char('j')));
    assert_eq!(ed.cursor_line, 1);
    // move left
    ed.handle_normal_key(key(KeyCode::Char('h')));
    assert_eq!(ed.cursor_col, 0);
    // move up
    ed.handle_normal_key(key(KeyCode::Char('k')));
    assert_eq!(ed.cursor_line, 0);
}

#[test]
fn test_normal_digit_prefix() {
    let mut ed = make_editor("aaaaaaa\nbbbbbb");
    // Press '3' then 'l' → move right 3
    ed.handle_normal_key(key(KeyCode::Char('3')));
    ed.handle_normal_key(key(KeyCode::Char('l')));
    assert_eq!(ed.cursor_col, 3);
}

// ── Normal mode: editing ops ──────────────────────────────────────────────────

#[test]
fn test_normal_x_deletes_char() {
    let mut ed = make_editor("hello");
    ed.cursor_col = 0;
    ed.handle_normal_key(key(KeyCode::Char('x')));
    assert_eq!(ed.buffer.line_str(0), "ello");
}

#[test]
fn test_normal_dd_deletes_line() {
    let mut ed = make_editor("line0\nline1\nline2");
    ed.cursor_line = 1;
    // First key 'd'
    ed.handle_normal_key(key(KeyCode::Char('d')));
    // Second key 'd'
    ed.handle_normal_key(key(KeyCode::Char('d')));
    assert_eq!(ed.buffer.line_count(), 2);
    assert_eq!(ed.buffer.line_str(1), "line2");
}

#[test]
fn test_normal_yy_then_p_pastes() {
    let mut ed = make_editor("alpha\nbeta");
    ed.cursor_line = 0;
    // yy
    ed.handle_normal_key(key(KeyCode::Char('y')));
    ed.handle_normal_key(key(KeyCode::Char('y')));
    assert!(ed.buffer.register.contains("alpha"));
    assert!(ed.buffer.register_linewise);
    // p — paste after line 0
    ed.handle_normal_key(key(KeyCode::Char('p')));
    assert_eq!(ed.buffer.line_count(), 3);
    assert_eq!(ed.buffer.line_str(1), "alpha");
}

#[test]
fn test_normal_u_undo() {
    let mut ed = make_editor("");
    ed.buffer.insert_str(0, "something");
    ed.handle_normal_key(key(KeyCode::Char('u')));
    assert_eq!(ed.buffer.line_str(0), "");
}

#[test]
fn test_normal_ctrl_r_redo() {
    let mut ed = make_editor("");
    ed.buffer.insert_str(0, "redo me");
    ed.handle_normal_key(key(KeyCode::Char('u')));
    ed.handle_normal_key(ctrl('r'));
    assert_eq!(ed.buffer.line_str(0), "redo me");
}

// ── Normal mode: gg / G ──────────────────────────────────────────────────────

#[test]
fn test_normal_gg_goes_to_top() {
    let mut ed = make_editor("a\nb\nc");
    ed.cursor_line = 2;
    ed.handle_normal_key(key(KeyCode::Char('g')));
    ed.handle_normal_key(key(KeyCode::Char('g')));
    assert_eq!(ed.cursor_line, 0);
}

#[test]
fn test_normal_G_goes_to_bottom() {
    let mut ed = make_editor("a\nb\nc");
    ed.handle_normal_key(key(KeyCode::Char('G')));
    assert_eq!(ed.cursor_line, 2);
}

// ── Insert mode ───────────────────────────────────────────────────────────────

#[test]
fn test_insert_char_types() {
    let mut ed = make_editor("hi");
    ed.mode = Mode::Insert;
    ed.cursor_col = 2;
    ed.handle_insert_key(key(KeyCode::Char('!')));
    assert_eq!(ed.buffer.line_str(0), "hi!");
    assert_eq!(ed.cursor_col, 3);
}

#[test]
fn test_insert_backspace_deletes() {
    let mut ed = make_editor("hello");
    ed.mode = Mode::Insert;
    ed.cursor_col = 5;
    ed.handle_insert_key(key(KeyCode::Backspace));
    assert_eq!(ed.buffer.line_str(0), "hell");
    assert_eq!(ed.cursor_col, 4);
}

#[test]
fn test_insert_enter_splits_line() {
    let mut ed = make_editor("helloworld");
    ed.mode = Mode::Insert;
    ed.cursor_col = 5;
    ed.handle_insert_key(key(KeyCode::Enter));
    assert_eq!(ed.buffer.line_count(), 2);
    assert_eq!(ed.buffer.line_str(0), "hello");
    assert_eq!(ed.buffer.line_str(1), "world");
}

#[test]
fn test_insert_esc_returns_to_normal() {
    let mut ed = make_editor("hi");
    ed.mode = Mode::Insert;
    ed.cursor_col = 2;
    let action = ed.handle_insert_key(key(KeyCode::Esc));
    assert!(matches!(action, InsertAction::ExitToNormal));
    // Cursor should have moved left by 1
    assert_eq!(ed.cursor_col, 1);
}

#[test]
fn test_insert_tab_expands() {
    let mut ed = make_editor("hello");
    ed.mode = Mode::Insert;
    ed.cursor_col = 5;
    ed.handle_insert_key(key(KeyCode::Tab));
    // Default tab_width = 4, expand_tab = true
    let line = ed.buffer.line_str(0);
    assert!(line.ends_with("    "), "expected 4 spaces, got {:?}", line);
}

#[test]
fn test_insert_ctrl_w_deletes_word() {
    let mut ed = make_editor("hello world");
    ed.mode = Mode::Insert;
    ed.cursor_col = 11; // end of "hello world"
    ed.handle_insert_key(ctrl('w'));
    assert_eq!(ed.buffer.line_str(0), "hello ");
}

#[test]
fn test_insert_ctrl_u_deletes_to_bol() {
    let mut ed = make_editor("    hello");
    ed.mode = Mode::Insert;
    ed.cursor_col = 9;
    ed.handle_insert_key(ctrl('u'));
    assert_eq!(ed.buffer.line_str(0), "");
    assert_eq!(ed.cursor_col, 0);
}

// ── Command parsing ───────────────────────────────────────────────────────────

#[test]
fn test_command_w_saves() {
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "").unwrap();
    tmp.flush().unwrap();
    let path = tmp.path().to_path_buf();

    let mut ed = make_editor("save me");
    ed.buffer.path = Some(path.clone());
    let mut input = "w".to_string();
    let action = ed.handle_command_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut input,
    );
    // Should return SetMsg (file written) or SaveAndQuit
    assert!(matches!(action,
        hi::mode::command::CommandAction::SetMsg(_)
    ));
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert_eq!(on_disk, "save me");
}

#[test]
fn test_command_goto_line() {
    let mut ed = make_editor("a\nb\nc\nd");
    let mut input = "3".to_string();
    let action = ed.handle_command_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut input,
    );
    assert!(matches!(action, hi::mode::command::CommandAction::GoToLine(3)));
}

#[test]
fn test_command_q_quits() {
    let mut ed = make_editor("");
    let mut input = "q".to_string();
    let action = ed.handle_command_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut input,
    );
    assert!(matches!(action, hi::mode::command::CommandAction::Quit { force: false }));
}

#[test]
fn test_command_q_bang_force_quits() {
    let mut ed = make_editor("unsaved");
    ed.buffer.modified = true;
    let mut input = "q!".to_string();
    let action = ed.handle_command_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut input,
    );
    assert!(matches!(action, hi::mode::command::CommandAction::Quit { force: true }));
}

#[test]
fn test_command_noh_clears_search() {
    let mut ed = make_editor("foo bar");
    ed.run_search("foo", false);
    assert!(ed.search_highlight);
    let mut input = "noh".to_string();
    ed.handle_command_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut input,
    );
    assert!(!ed.search_highlight);
}

#[test]
fn test_command_substitution_current_line() {
    use hi::mode::command::CommandAction;
    let mut ed = make_editor("foo bar foo");
    let mut input = "s/foo/baz/".to_string();
    let action = ed.handle_command_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut input,
    );
    assert!(matches!(action, CommandAction::RunSubstitution { .. }));
}

#[test]
fn test_command_set_number() {
    use hi::mode::command::CommandAction;
    let mut ed = make_editor("");
    let mut input = "set nu".to_string();
    let action = ed.handle_command_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut input,
    );
    assert!(matches!(action, CommandAction::ToggleLineNumbers(true)));
}

#[test]
fn test_command_history_up_down() {
    let mut ed = make_editor("");
    // Add some history by submitting commands
    let cmds = ["noh", "set nu", "3"];
    for cmd in &cmds {
        let mut input = cmd.to_string();
        ed.handle_command_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut input);
    }
    // Now simulate pressing Up in command mode
    let mut input = String::new();
    ed.handle_command_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &mut input);
    assert!(!input.is_empty(), "Up should recall last command, got empty");
}

// ── AI mode input ─────────────────────────────────────────────────────────────

#[test]
fn test_ai_input_accumulates() {
    use hi::mode::ai::{handle_ai_input_key, AiInputAction};
    let mut input = String::new();
    handle_ai_input_key(&mut input, key(KeyCode::Char('h')));
    handle_ai_input_key(&mut input, key(KeyCode::Char('i')));
    assert_eq!(input, "hi");
}

#[test]
fn test_ai_input_submit_on_enter() {
    use hi::mode::ai::{handle_ai_input_key, AiInputAction};
    let mut input = "hello".to_string();
    let action = handle_ai_input_key(&mut input, key(KeyCode::Enter));
    assert!(matches!(action, AiInputAction::Submit(s) if s == "hello"));
    assert!(input.is_empty());
}

#[test]
fn test_ai_input_cancel_on_esc() {
    use hi::mode::ai::{handle_ai_input_key, AiInputAction};
    let mut input = "partial".to_string();
    let action = handle_ai_input_key(&mut input, key(KeyCode::Esc));
    assert!(matches!(action, AiInputAction::Cancel));
    assert!(input.is_empty());
}

#[test]
fn test_ai_input_backspace() {
    use hi::mode::ai::{handle_ai_input_key, AiInputAction};
    let mut input = "abc".to_string();
    handle_ai_input_key(&mut input, key(KeyCode::Backspace));
    assert_eq!(input, "ab");
}
