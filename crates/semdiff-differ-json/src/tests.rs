use super::*;
use serde_json::json;
use std::fmt::Formatter;

#[test]
fn json_diff_marks_equal_value() {
    let expected = json!("same");
    let actual = json!("same");
    let diff = json_diff(&expected, &actual);
    assert_eq!(diff.len(), 1);
    assert!(matches!(diff[0].tag(), ChangeTag::Unchanged));
}

#[test]
fn json_diff_marks_changed_value() {
    let expected = json!(1);
    let actual = json!(2);
    let diff = json_diff(&expected, &actual);
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

    let diff = json_diff(&expected, &actual);

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
