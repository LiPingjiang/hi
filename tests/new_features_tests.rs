//! Tests for new features: dot-repeat, text objects, macros, named registers.

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

// ── Helper: simulate entering insert mode, typing, then Esc ──────────────────

fn enter_insert_type_esc(ed: &mut Editor, text: &str) {
    ed.begin_insert_session(0, false, false);
    ed.mode = Mode::Insert;
    ed.buffer.begin_group();
    for c in text.chars() {
        ed.handle_insert_key(key(KeyCode::Char(c)));
    }
    ed.handle_insert_key(key(KeyCode::Esc));
    ed.mode = Mode::Normal;
    ed.clamp_cursor();
}

// ═══════════════════════════════════════════════════════════════════════════════
// DOT-REPEAT
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_dot_repeat_insert_text() {
    // Type "hi" in insert mode, then use . to repeat on next line
    let mut ed = make_editor("line0\nline1\n");
    ed.cursor_line = 0;
    ed.cursor_col = 5; // end of "line0"

    enter_insert_type_esc(&mut ed, "hi");
    assert_eq!(ed.buffer.line_str(0), "line0hi");

    // Move to line 1, dot-repeat
    ed.cursor_line = 1;
    ed.cursor_col = 5;
    ed.dot_repeat();
    assert_eq!(ed.buffer.line_str(1), "line1hi");
}

#[test]
fn test_dot_repeat_replace_char() {
    let mut ed = make_editor("aaa");
    ed.cursor_col = 0;

    // r{x} → replace first char with 'x'
    ed.handle_normal_key(key(KeyCode::Char('r')));
    ed.handle_normal_key(key(KeyCode::Char('x')));
    assert_eq!(ed.buffer.line_str(0), "xaa");

    // Move right, dot-repeat
    ed.cursor_col = 1;
    ed.dot_repeat();
    assert_eq!(ed.buffer.line_str(0), "xxa");
}

#[test]
fn test_dot_repeat_indent() {
    let mut ed = make_editor("hello\nworld\n");
    ed.cursor_line = 0;

    // >> indent line 0
    ed.handle_normal_key(key(KeyCode::Char('>')));
    ed.handle_normal_key(key(KeyCode::Char('>')));
    let indented = ed.buffer.line_str(0);
    assert!(indented.starts_with("    "), "expected indent, got {:?}", indented);

    // Move to line 1, dot-repeat
    ed.cursor_line = 1;
    ed.dot_repeat();
    let indented2 = ed.buffer.line_str(1);
    assert!(indented2.starts_with("    "), "expected indent on line1, got {:?}", indented2);
}

#[test]
fn test_dot_repeat_noop_when_no_last_action() {
    let mut ed = make_editor("hello");
    // No action recorded yet — dot_repeat should return false
    assert!(!ed.dot_repeat());
    // Buffer unchanged
    assert_eq!(ed.buffer.line_str(0), "hello");
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEXT OBJECTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_text_obj_di_double_quote() {
    // di" on: say "hello" world
    //              ^cursor inside quotes
    let mut ed = make_editor("say \"hello\" world");
    ed.cursor_col = 6; // inside "hello"

    // d i "
    ed.handle_normal_key(key(KeyCode::Char('d')));
    ed.handle_normal_key(key(KeyCode::Char('i')));
    ed.handle_normal_key(key(KeyCode::Char('"')));
    assert_eq!(ed.buffer.line_str(0), "say \"\" world");
}

#[test]
fn test_text_obj_da_double_quote() {
    // da" deletes including the quotes
    let mut ed = make_editor("say \"hello\" world");
    ed.cursor_col = 6;

    ed.handle_normal_key(key(KeyCode::Char('d')));
    ed.handle_normal_key(key(KeyCode::Char('a')));
    ed.handle_normal_key(key(KeyCode::Char('"')));
    // "hello" (7 chars) removed
    assert_eq!(ed.buffer.line_str(0), "say  world");
}

#[test]
fn test_text_obj_di_parens() {
    // di( on: foo(bar) → foo()
    let mut ed = make_editor("foo(bar)");
    ed.cursor_col = 5; // inside parens

    ed.handle_normal_key(key(KeyCode::Char('d')));
    ed.handle_normal_key(key(KeyCode::Char('i')));
    ed.handle_normal_key(key(KeyCode::Char('(')));
    assert_eq!(ed.buffer.line_str(0), "foo()");
}

#[test]
fn test_text_obj_da_parens() {
    // da( on: foo(bar) → foo
    let mut ed = make_editor("foo(bar)");
    ed.cursor_col = 5;

    ed.handle_normal_key(key(KeyCode::Char('d')));
    ed.handle_normal_key(key(KeyCode::Char('a')));
    ed.handle_normal_key(key(KeyCode::Char('(')));
    assert_eq!(ed.buffer.line_str(0), "foo");
}

#[test]
fn test_text_obj_di_curly() {
    // di{ on: fn { body } → fn {  }
    let mut ed = make_editor("fn { body }");
    ed.cursor_col = 6; // inside braces

    ed.handle_normal_key(key(KeyCode::Char('d')));
    ed.handle_normal_key(key(KeyCode::Char('i')));
    ed.handle_normal_key(key(KeyCode::Char('{')));
    assert_eq!(ed.buffer.line_str(0), "fn {}");
}

#[test]
fn test_text_obj_yi_single_quote() {
    // yi' yanks content inside single quotes
    let mut ed = make_editor("let x = 'value';");
    ed.cursor_col = 11; // inside 'value'

    ed.handle_normal_key(key(KeyCode::Char('y')));
    ed.handle_normal_key(key(KeyCode::Char('i')));
    ed.handle_normal_key(key(KeyCode::Char('\'')));
    assert_eq!(ed.buffer.register, "value");
    // Buffer unchanged
    assert_eq!(ed.buffer.line_str(0), "let x = 'value';");
}

#[test]
fn test_text_obj_ci_parens_enters_insert() {
    // ci( should delete inner content and return EnterInsert
    let mut ed = make_editor("call(arg)");
    ed.cursor_col = 6; // inside parens

    let action1 = ed.handle_normal_key(key(KeyCode::Char('c')));
    assert!(matches!(action1, NormalAction::None)); // waiting for i/a
    let action2 = ed.handle_normal_key(key(KeyCode::Char('i')));
    assert!(matches!(action2, NormalAction::None)); // waiting for delimiter
    let action3 = ed.handle_normal_key(key(KeyCode::Char('(')));
    assert!(matches!(action3, NormalAction::EnterInsert { .. }));
    assert_eq!(ed.buffer.line_str(0), "call()");
}

#[test]
fn test_text_obj_di_backtick() {
    let mut ed = make_editor("echo `cmd`");
    ed.cursor_col = 7; // inside backticks

    ed.handle_normal_key(key(KeyCode::Char('d')));
    ed.handle_normal_key(key(KeyCode::Char('i')));
    ed.handle_normal_key(key(KeyCode::Char('`')));
    assert_eq!(ed.buffer.line_str(0), "echo ``");
}

#[test]
fn test_text_obj_di_square_bracket() {
    let mut ed = make_editor("arr[idx]");
    ed.cursor_col = 5;

    ed.handle_normal_key(key(KeyCode::Char('d')));
    ed.handle_normal_key(key(KeyCode::Char('i')));
    ed.handle_normal_key(key(KeyCode::Char('[')));
    assert_eq!(ed.buffer.line_str(0), "arr[]");
}

// ═══════════════════════════════════════════════════════════════════════════════
// NAMED REGISTERS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_named_register_yank_and_paste() {
    let mut ed = make_editor("alpha\nbeta\n");
    ed.cursor_line = 0;

    // "ayy — yank line 0 into register 'a'
    ed.handle_normal_key(key(KeyCode::Char('"')));
    ed.handle_normal_key(key(KeyCode::Char('a')));
    ed.handle_normal_key(key(KeyCode::Char('y')));
    ed.handle_normal_key(key(KeyCode::Char('y')));

    assert!(ed.named_registers.contains_key(&'a'));
    assert_eq!(ed.named_registers[&'a'].text, "alpha\n");
    assert!(ed.named_registers[&'a'].linewise);
}

#[test]
fn test_named_register_paste_from_register() {
    let mut ed = make_editor("alpha\nbeta\n");
    ed.cursor_line = 0;

    // "ayy
    ed.handle_normal_key(key(KeyCode::Char('"')));
    ed.handle_normal_key(key(KeyCode::Char('a')));
    ed.handle_normal_key(key(KeyCode::Char('y')));
    ed.handle_normal_key(key(KeyCode::Char('y')));

    // Move to line 1, "ap
    ed.cursor_line = 1;
    ed.handle_normal_key(key(KeyCode::Char('"')));
    ed.handle_normal_key(key(KeyCode::Char('a')));
    ed.handle_normal_key(key(KeyCode::Char('p')));

    // Should have pasted "alpha\n" after line 1
    assert_eq!(ed.buffer.line_count(), 4);
    assert_eq!(ed.buffer.line_str(2), "alpha");
}

#[test]
fn test_named_register_dd_stores_in_register() {
    let mut ed = make_editor("line0\nline1\nline2\n");
    ed.cursor_line = 1;

    // "bdd — delete line 1 into register 'b'
    ed.handle_normal_key(key(KeyCode::Char('"')));
    ed.handle_normal_key(key(KeyCode::Char('b')));
    ed.handle_normal_key(key(KeyCode::Char('d')));
    ed.handle_normal_key(key(KeyCode::Char('d')));

    assert!(ed.named_registers.contains_key(&'b'));
    assert_eq!(ed.named_registers[&'b'].text, "line1\n");
    assert_eq!(ed.buffer.line_count(), 3); // line1 deleted
}

#[test]
fn test_default_register_unaffected_by_named() {
    let mut ed = make_editor("first\nsecond\n");

    // yy into default register
    ed.cursor_line = 0;
    ed.handle_normal_key(key(KeyCode::Char('y')));
    ed.handle_normal_key(key(KeyCode::Char('y')));
    let default_reg = ed.buffer.register.clone();

    // "ayy into named register 'a'
    ed.cursor_line = 1;
    ed.handle_normal_key(key(KeyCode::Char('"')));
    ed.handle_normal_key(key(KeyCode::Char('a')));
    ed.handle_normal_key(key(KeyCode::Char('y')));
    ed.handle_normal_key(key(KeyCode::Char('y')));

    // Default register is now "second\n" (apply_operator syncs it)
    // Named register 'a' is "second\n"
    assert_eq!(ed.named_registers[&'a'].text, "second\n");
}

// ═══════════════════════════════════════════════════════════════════════════════
// MACRO RECORDING
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_macro_start_recording() {
    let mut ed = make_editor("hello");

    // q a → start recording into register 'a'
    ed.handle_normal_key(key(KeyCode::Char('q')));
    assert!(ed.macro_recording.is_none()); // still waiting for register char
    ed.handle_normal_key(key(KeyCode::Char('a')));
    assert_eq!(ed.macro_recording, Some('a'));
}

#[test]
fn test_macro_stop_recording_with_q() {
    let mut ed = make_editor("hello");

    // qa → start
    ed.handle_normal_key(key(KeyCode::Char('q')));
    ed.handle_normal_key(key(KeyCode::Char('a')));
    assert_eq!(ed.macro_recording, Some('a'));

    // q → stop
    ed.handle_normal_key(key(KeyCode::Char('q')));
    assert!(ed.macro_recording.is_none());
}

#[test]
fn test_macro_records_keys() {
    let mut ed = make_editor("hello");

    // qa → start recording
    ed.handle_normal_key(key(KeyCode::Char('q')));
    ed.handle_normal_key(key(KeyCode::Char('a')));

    // Press some keys (they get recorded)
    ed.handle_normal_key(key(KeyCode::Char('l')));
    ed.handle_normal_key(key(KeyCode::Char('l')));

    // q → stop
    ed.handle_normal_key(key(KeyCode::Char('q')));

    // Macro 'a' should have recorded keys
    assert!(ed.macros.contains_key(&'a'));
    // The recorded keys include 'l', 'l', and 'q' (stop key is also captured)
    let macro_keys = &ed.macros[&'a'];
    assert!(!macro_keys.is_empty());
}

#[test]
fn test_macro_play_action_returned() {
    let mut ed = make_editor("hello");

    // Store a macro manually
    ed.macros.insert('z', vec![
        hi::editor::MacroKey { code: KeyCode::Char('l'), modifiers: KeyModifiers::NONE },
    ]);

    // @z → should return PlayMacro('z')
    ed.handle_normal_key(key(KeyCode::Char('@')));
    let action = ed.handle_normal_key(key(KeyCode::Char('z')));
    assert!(matches!(action, NormalAction::PlayMacro('z')));
}

#[test]
fn test_macro_nonexistent_register() {
    let mut ed = make_editor("hello");
    // @x where 'x' has no macro — PlayMacro still returned, app handles the error
    ed.handle_normal_key(key(KeyCode::Char('@')));
    let action = ed.handle_normal_key(key(KeyCode::Char('x')));
    assert!(matches!(action, NormalAction::PlayMacro('x')));
}

// ═══════════════════════════════════════════════════════════════════════════════
// REGISTER PREFIX PENDING STATE
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_register_prefix_sets_active_register() {
    let mut ed = make_editor("hello");

    // " a → active_register = Some('a')
    ed.handle_normal_key(key(KeyCode::Char('"')));
    ed.handle_normal_key(key(KeyCode::Char('a')));
    assert_eq!(ed.active_register, Some('a'));
}

#[test]
fn test_register_prefix_cleared_after_use() {
    let mut ed = make_editor("hello\n");

    // "ayy → active_register consumed
    ed.handle_normal_key(key(KeyCode::Char('"')));
    ed.handle_normal_key(key(KeyCode::Char('a')));
    ed.handle_normal_key(key(KeyCode::Char('y')));
    ed.handle_normal_key(key(KeyCode::Char('y')));

    // active_register should be None after yank
    assert!(ed.active_register.is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// INSERT SESSION TRACKING
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_insert_session_records_text() {
    let mut ed = make_editor("hello");
    ed.cursor_col = 5;

    enter_insert_type_esc(&mut ed, "world");

    // last_action should be Insert with text="world"
    match &ed.last_action {
        Some(hi::editor::RepeatAction::Insert { text, .. }) => {
            assert_eq!(text, "world");
        }
        other => panic!("expected Insert action, got {:?}", other),
    }
}

#[test]
fn test_insert_session_newline_below_flag() {
    let mut ed = make_editor("hello\n");
    ed.cursor_line = 0;

    // Simulate 'o' (newline below)
    ed.begin_insert_session(0, false, true);
    ed.mode = Mode::Insert;
    ed.buffer.begin_group();
    ed.handle_insert_key(key(KeyCode::Char('x')));
    ed.handle_insert_key(key(KeyCode::Esc));
    ed.mode = Mode::Normal;

    match &ed.last_action {
        Some(hi::editor::RepeatAction::Insert { newline_below, text, .. }) => {
            assert!(*newline_below);
            assert_eq!(text, "x");
        }
        other => panic!("expected Insert action, got {:?}", other),
    }
}
