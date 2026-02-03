use super::*;

#[test]
fn binary_change_stat_counts_added_deleted() {
    let expected = b"abc";
    let actual = b"adc";
    let changes = binary_diff_changes(expected, actual);
    let stat = binary_change_stat(&changes);
    assert_eq!(stat.added, 1);
    assert_eq!(stat.deleted, 1);
}
