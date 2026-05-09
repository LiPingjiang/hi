//! Tests for the rope Buffer: insert, delete, undo/redo, line ops.

use hi::buffer::Buffer;

// ── Basic line ops ────────────────────────────────────────────────────────────

#[test]
fn test_new_buffer_empty() {
    let buf = Buffer::new();
    assert_eq!(buf.line_str(0), "");
}

#[test]
fn test_insert_char_and_query() {
    let mut buf = Buffer::new();
    buf.insert_char(0, 'H');
    buf.insert_char(1, 'i');
    assert_eq!(buf.line_str(0), "Hi");
    assert_eq!(buf.line_len(0), 2);
}

#[test]
fn test_insert_str_multiline() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "foo\nbar\nbaz");
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.line_str(0), "foo");
    assert_eq!(buf.line_str(1), "bar");
    assert_eq!(buf.line_str(2), "baz");
}

#[test]
fn test_delete_range() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "hello world");
    buf.delete_range(5, 11);
    assert_eq!(buf.line_str(0), "hello");
}

#[test]
fn test_pos_to_char_multiline() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "abc\ndef");
    // line 1, col 2 → char index 6  (a=0,b=1,c=2,\n=3,d=4,e=5,f=6)
    let idx = buf.pos_to_char(1, 2);
    assert_eq!(idx, 6);
    assert_eq!(buf.char_to_line(idx), 1);
}

#[test]
fn test_delete_line() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "line0\nline1\nline2");
    let removed = buf.delete_line(1);
    assert!(removed.contains("line1"), "removed={:?}", removed);
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.line_str(0), "line0");
    assert_eq!(buf.line_str(1), "line2");
}

#[test]
fn test_insert_newline_with_indent() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "    hello");
    let char_idx = buf.pos_to_char(0, 4);
    let new_pos = buf.insert_newline(char_idx, "    ");
    assert_eq!(buf.line_count(), 2);
    let new_line = buf.char_to_line(new_pos);
    assert_eq!(new_line, 1);
}

// ── Undo / Redo ───────────────────────────────────────────────────────────────

#[test]
fn test_undo_basic() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "hello");
    assert_eq!(buf.line_str(0), "hello");
    buf.undo();
    assert_eq!(buf.line_str(0), "");
}

#[test]
fn test_redo_after_undo() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "world");
    buf.undo();
    buf.redo();
    assert_eq!(buf.line_str(0), "world");
}

#[test]
fn test_undo_stack_clears_on_new_edit() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "a");
    buf.undo();
    buf.insert_str(0, "b");
    buf.redo(); // redo should be a no-op
    assert_eq!(buf.line_str(0), "b");
}

#[test]
fn test_multiple_undo_steps() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "one");
    buf.insert_str(3, " two");
    buf.insert_str(7, " three");
    // undo once: remove " three"
    buf.undo();
    assert_eq!(buf.line_str(0), "one two");
    // undo again: remove " two"
    buf.undo();
    assert_eq!(buf.line_str(0), "one");
}

// ── begin_group ───────────────────────────────────────────────────────────────

#[test]
fn test_begin_group_merges_two_inserts() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "foo");      // entry 1 pushed
    buf.begin_group();             // next push merges into entry 1
    buf.insert_str(3, "bar");      // merged: entry 1.after = "foobar"
    // Single undo should revert to state before "foo" was inserted
    buf.undo();
    assert_eq!(buf.line_str(0), "");
}

// ── from_file / save ──────────────────────────────────────────────────────────

#[test]
fn test_from_file_roundtrip() {
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "line one").unwrap();
    writeln!(tmp, "line two").unwrap();
    tmp.flush().unwrap();

    let buf = Buffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.line_str(0), "line one");
    assert_eq!(buf.line_str(1), "line two");
    assert!(!buf.modified);
}

#[test]
fn test_save_writes_content() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();

    let mut buf = Buffer::from_path(path.clone()).unwrap();
    buf.insert_str(0, "saved content");
    buf.save().unwrap();

    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert_eq!(on_disk, "saved content");
    assert!(!buf.modified);
}

#[test]
fn test_indent_of_line() {
    let mut buf = Buffer::new();
    buf.insert_str(0, "    indented\nnot");
    assert_eq!(buf.indent_of_line(0), "    ");
    assert_eq!(buf.indent_of_line(1), "");
}
