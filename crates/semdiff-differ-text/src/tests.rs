use super::*;
use similar::ChangeTag;

#[test]
fn text_diff_lines_counts_line_changes() {
    let expected = b"line1\nline2\n";
    let actual = b"line1\nline3\n";
    let diff = text_diff_lines(expected, actual);
    let mut added = 0;
    let mut deleted = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => deleted += 1,
            ChangeTag::Equal => {}
        }
    }
    assert_eq!(added, 1);
    assert_eq!(deleted, 1);
}
