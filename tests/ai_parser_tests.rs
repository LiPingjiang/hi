//! Tests for the AI plan parser and step applicator.

use hi::ai::parser::{parse_plan, apply_steps, EditStep};
use hi::buffer::Buffer;

// ── Parse tests ───────────────────────────────────────────────────────────────

#[test]
fn test_parse_insert() {
    let raw = "1. INSERT line 2: hello world";
    let steps = parse_plan(raw);
    assert_eq!(steps.len(), 1);
    assert!(matches!(&steps[0], EditStep::Insert { line: 2, text } if text == "hello world"));
}

#[test]
fn test_parse_delete() {
    let raw = "1. DELETE line 5";
    let steps = parse_plan(raw);
    assert_eq!(steps.len(), 1);
    assert!(matches!(&steps[0], EditStep::Delete { line: 5 }));
}

#[test]
fn test_parse_replace() {
    let raw = "1. REPLACE line 0: new content";
    let steps = parse_plan(raw);
    assert_eq!(steps.len(), 1);
    assert!(matches!(&steps[0], EditStep::Replace { line: 0, text } if text == "new content"));
}

#[test]
fn test_parse_replace_range() {
    let raw = "1. REPLACE range 3-5: first line\\nsecond line";
    let steps = parse_plan(raw);
    assert_eq!(steps.len(), 1);
    match &steps[0] {
        EditStep::ReplaceRange { start: 3, end: 5, text } => {
            assert!(text.contains('\n'), "unescape should turn \\n into newline");
        }
        other => panic!("unexpected step: {:?}", other),
    }
}

#[test]
fn test_parse_message() {
    let raw = "1. MESSAGE: This file looks great";
    let steps = parse_plan(raw);
    assert_eq!(steps.len(), 1);
    assert!(matches!(&steps[0], EditStep::Message(m) if m.contains("great")));
}

#[test]
fn test_parse_multiple_steps() {
    let raw = "\
1. INSERT line 0: # Header
2. DELETE line 2
3. REPLACE line 4: updated
4. MESSAGE: Done
";
    let steps = parse_plan(raw);
    assert_eq!(steps.len(), 4);
}

#[test]
fn test_parse_skips_garbage() {
    let raw = "Some random prose that matches nothing\nINSERT line 1: real step\nmore prose";
    let steps = parse_plan(raw);
    // "INSERT line 1: real step" should be parsed (the leading "I" will be checked)
    // only the line starting with known keyword gets parsed
    assert_eq!(steps.len(), 1, "expected 1 real step, got {}: {:?}", steps.len(), steps.iter().map(|s| format!("{:?}", s)).collect::<Vec<_>>());
}

// ── Apply tests ───────────────────────────────────────────────────────────────

#[test]
fn test_apply_insert() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "line0\nline2");
    let steps = vec![EditStep::Insert { line: 1, text: "line1".to_string() }];
    apply_steps(&mut buf, &steps).unwrap();
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.line_str(1), "line1");
    assert_eq!(buf.line_str(2), "line2");
}

#[test]
fn test_apply_delete() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "line0\nline1\nline2");
    let steps = vec![EditStep::Delete { line: 1 }];
    apply_steps(&mut buf, &steps).unwrap();
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.line_str(1), "line2");
}

#[test]
fn test_apply_replace() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "old content\nkeep this");
    let steps = vec![EditStep::Replace { line: 0, text: "new content".to_string() }];
    apply_steps(&mut buf, &steps).unwrap();
    assert_eq!(buf.line_str(0), "new content");
    assert_eq!(buf.line_str(1), "keep this");
}

#[test]
fn test_apply_replace_range() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "a\nb\nc\nd");
    let steps = vec![EditStep::ReplaceRange {
        start: 1,
        end: 2,
        text: "B\nC".to_string(),
    }];
    apply_steps(&mut buf, &steps).unwrap();
    assert_eq!(buf.line_str(0), "a");
    assert!(buf.line_str(1) == "B" || buf.line_str(1).contains('B'),
        "line 1 = {:?}", buf.line_str(1));
}

#[test]
fn test_apply_message_no_edit() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "unchanged");
    let steps = vec![EditStep::Message("advisory".to_string())];
    apply_steps(&mut buf, &steps).unwrap();
    assert_eq!(buf.line_str(0), "unchanged");
}

#[test]
fn test_apply_multiple_steps_in_order() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "line0\nline1\nline2");
    // Replace line2, delete line1 — applied in reverse order so indices stay valid
    let steps = vec![
        EditStep::Replace { line: 2, text: "replaced".to_string() },
        EditStep::Delete  { line: 1 },
    ];
    apply_steps(&mut buf, &steps).unwrap();
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.line_str(0), "line0");
    assert_eq!(buf.line_str(1), "replaced");
}
