use super::*;
use serde_json::json;
use std::fmt::Formatter;

#[test]
fn json_diff_marks_equal_value() {
    let expected = json!("same");
    let actual = json!("same");
    let diff = json_diff(&expected, &actual, &[]);
    assert_eq!(diff.len(), 1);
    assert!(matches!(diff[0].tag(), ChangeTag::Unchanged));
}

#[test]
fn json_diff_marks_changed_value() {
    let expected = json!(1);
    let actual = json!(2);
    let diff = json_diff(&expected, &actual, &[]);
    assert_eq!(diff.len(), 2);
    assert!(matches!(diff[0].tag(), ChangeTag::Deleted));
    assert!(matches!(diff[1].tag(), ChangeTag::Added));
}

#[test]
fn json_diff_handles_nested_structures() {
    let expected = json!({
        "a": 1,
        "b": [
            1,
            2,
            {
                "c": 3
            }
        ],
        "obj": {
            "x": true,
            "y": null
        }
    });
    let actual = json!({
        "a": 1,
        "b": [
            2,
            1,
            {
                "c": 4
            },
            3
        ],
        "obj": {
            "x": false,
            "z": "new"
        },
        "extra": [
            1,
            2
        ]
    });

    let diff = json_diff(&expected, &actual, &[]);

    let mut expected_rendered = String::new();
    let mut actual_rendered = String::new();

    struct IndentFormat(usize);
    impl Display for IndentFormat {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            for _ in 0..self.0 {
                write!(f, "  ")?;
            }
            Ok(())
        }
    }

    use std::fmt::Write;

    for d in diff {
        match d.state {
            JsonDiffLineState::Unchanged { expected, actual } => {
                writeln!(expected_rendered, "  {}{}", IndentFormat(d.indent), expected).unwrap();
                writeln!(actual_rendered, "  {}{}", IndentFormat(d.indent), actual).unwrap();
            }
            JsonDiffLineState::Ignored { expected, actual } => {
                if let Some(expected) = expected {
                    writeln!(expected_rendered, "  {}{}", IndentFormat(d.indent), expected).unwrap();
                } else {
                    writeln!(expected_rendered).unwrap();
                }
                if let Some(actual) = actual {
                    writeln!(actual_rendered, "  {}{}", IndentFormat(d.indent), actual).unwrap();
                } else {
                    writeln!(actual_rendered).unwrap();
                }
            }
            JsonDiffLineState::Added(actual) => {
                writeln!(expected_rendered).unwrap();
                writeln!(actual_rendered, "+ {}{}", IndentFormat(d.indent), actual).unwrap();
            }
            JsonDiffLineState::Deleted(expected) => {
                writeln!(expected_rendered, "- {}{}", IndentFormat(d.indent), expected).unwrap();
                writeln!(actual_rendered).unwrap();
            }
        }
    }

    assert_eq!(
        expected_rendered,
        r#"  {
    "a": 1,
    "b": [
-     1,
      2,

      {
-       "c": 3

      }

    ],
    "obj": {
-     "x": true,


-     "y": null
    }




  }
"#
    );
    assert_eq!(
        actual_rendered,
        r#"  {
    "a": 1,
    "b": [

      2,
+     1,
      {

+       "c": 4
      },
+     3
    ],
    "obj": {

+     "x": false,
+     "z": "new"

    },
+   "extra": [
+     1,
+     2
+   ]
  }
"#
    );
}

#[test]
fn json_diff_treats_ignored_object_value_as_equal() {
    let expected = json!({
        "stable": true,
        "volatile": 1
    });
    let actual = json!({
        "stable": true,
        "volatile": 2
    });
    let ignore_paths = vec!["$.volatile".parse::<JsonPath>().unwrap()];

    let diff = json_diff(&expected, &actual, &ignore_paths);

    assert!(diff.iter().all(JsonDiffLine::is_equal_for_result));
    let ignored = diff.iter().find(|line| line.is_ignored()).unwrap();
    assert_eq!(ignored.display_expected().to_string(), "  \"volatile\": 1");
    assert_eq!(ignored.display_actual().to_string(), "  \"volatile\": 2");
}

#[test]
fn json_diff_treats_ignored_array_index_as_equal() {
    let expected = json!([1, 2]);
    let actual = json!([3, 2]);
    let ignore_paths = vec!["$[0]".parse::<JsonPath>().unwrap()];

    let diff = json_diff(&expected, &actual, &ignore_paths);

    assert!(diff.iter().all(JsonDiffLine::is_equal_for_result));
    let ignored = diff.iter().find(|line| line.is_ignored()).unwrap();
    assert_eq!(ignored.display_expected().to_string(), "  1,");
    assert_eq!(ignored.display_actual().to_string(), "  3,");
}

#[test]
fn json_diff_does_not_pair_negative_index_ignore_with_non_ignored_array_element() {
    let expected = json!([1, 100, 2]);
    let actual = json!([1, 3, 200, 2]);
    let ignore_paths = vec!["$[-2]".parse::<JsonPath>().unwrap()];

    let diff = json_diff(&expected, &actual, &ignore_paths);

    assert!(diff.iter().any(JsonDiffLine::is_ignored));
    assert!(
        diff.iter()
            .any(|line| matches!(line.tag(), ChangeTag::Added) && line.display_actual().to_string() == "  3,")
    );
    let ignored = diff.iter().find(|line| line.is_ignored()).unwrap();
    assert_eq!(ignored.display_expected().to_string(), "  100,");
    assert_eq!(ignored.display_actual().to_string(), "  200,");
}

#[test]
fn json_diff_matches_ignored_array_elements_by_similarity_when_counts_differ() {
    let expected = json!([0, [1], [1, 2, 3], 9]);
    let actual = json!([0, [10, 20, 30], 9]);
    let ignore_paths = vec!["$[1:3]".parse::<JsonPath>().unwrap()];

    let diff = json_diff(&expected, &actual, &ignore_paths);
    let ignored_pairs = diff
        .iter()
        .filter(|line| line.is_ignored())
        .map(|line| (line.display_expected().to_string(), line.display_actual().to_string()))
        .collect::<Vec<_>>();

    assert!(
        ignored_pairs
            .iter()
            .any(|(expected, actual)| expected == "    1," && actual == "    10,")
    );
    assert!(
        ignored_pairs
            .iter()
            .any(|(expected, actual)| expected == "    1" && actual.is_empty())
    );
}

#[test]
fn json_diff_treats_ignored_root_as_equal() {
    let expected = json!({"a": 1});
    let actual = json!({"a": 2});
    let ignore_paths = vec!["$".parse::<JsonPath>().unwrap()];

    let diff = json_diff(&expected, &actual, &ignore_paths);

    assert!(diff.iter().all(JsonDiffLine::is_equal_for_result));
    assert!(diff.iter().any(JsonDiffLine::is_ignored));
}

#[test]
fn json_diff_keeps_non_ignored_changes_modified() {
    let expected = json!({
        "ignored": 1,
        "changed": 1
    });
    let actual = json!({
        "ignored": 2,
        "changed": 2
    });
    let ignore_paths = vec!["$.ignored".parse::<JsonPath>().unwrap()];

    let diff = json_diff(&expected, &actual, &ignore_paths);

    assert!(diff.iter().any(JsonDiffLine::is_ignored));
    assert!(
        diff.iter()
            .any(|line| matches!(line.tag(), ChangeTag::Added | ChangeTag::Deleted))
    );
}
