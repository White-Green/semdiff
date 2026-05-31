pub(crate) mod eval;
mod integer_literal;
mod number_literal;
mod parser;
mod string_literal;

pub use parser::{JsonPath, ParseError, parse};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_path::eval::JsonPathMatcher;
    use crate::json_path::parser::{LogicalExpr, Segment, Selector};
    use nom::error::ErrorKind;
    use serde_json::{Value, json};

    fn matching_named_array_indices(root: &Value, path: &str, name: &str) -> Vec<bool> {
        let paths = vec![path.parse::<JsonPath>().unwrap()];
        let mut matcher = JsonPathMatcher::new(&paths);
        let mut root_state = matcher.root_state(root);
        let mut array_state = root_state.advance_name(name).unwrap();
        let array_len = root.as_object().unwrap()[name].as_array().unwrap().len();
        let mut matches = Vec::with_capacity(array_len);
        for index in 0..array_len {
            let state = array_state.advance_index(index).unwrap();
            matches.push(state.is_match());
            drop(state);
        }
        matches
    }

    fn matching_root_array_indices(root: &Value, path: &str) -> Vec<bool> {
        let paths = vec![path.parse::<JsonPath>().unwrap()];
        let mut matcher = JsonPathMatcher::new(&paths);
        let mut root_state = matcher.root_state(root);
        let array_len = root.as_array().unwrap().len();
        let mut matches = Vec::with_capacity(array_len);
        for index in 0..array_len {
            let state = root_state.advance_index(index).unwrap();
            matches.push(state.is_match());
            drop(state);
        }
        matches
    }

    #[test]
    fn test_parse() {
        assert!(parse("$['store']['book'][0]['title']").is_ok());
        assert!(parse("$.store.book[0].title").is_ok());
        assert!(parse("$.store.book[?@.price < 10].title").is_ok());
        assert!(parse("$.store.book[*].author").is_ok());
        assert!(parse("$..author").is_ok());
        assert!(parse("$.store.*").is_ok());
        assert!(parse("$.store..price").is_ok());
        assert!(parse("$..book[2]").is_ok());
        assert!(parse("$..book[2].author").is_ok());
        assert!(parse("$..book[2].publisher").is_ok());
        assert!(parse("$..book[-1]").is_ok());
        assert!(parse("$..book[0,1]").is_ok());
        assert!(parse("$..book[:2]").is_ok());
        assert!(parse("$..book[?@.isbn]").is_ok());
        assert!(parse("$..book[?@.price<10]").is_ok());
        assert!(parse("$..*").is_ok());
        assert!(parse("$").is_ok());
        assert!(parse("$.o['j j']").is_ok());
        assert!(parse("$.o['j j']['k.k']").is_ok());
        assert!(parse("$.o[\"j j\"][\"k.k\"]").is_ok());
        assert!(parse("$[\"'\"][\"@\"]").is_ok());
        assert!(parse("$[*]").is_ok());
        assert!(parse("$.o[*]").is_ok());
        assert!(parse("$.o[*, *]").is_ok());
        assert!(parse("$.a[*]").is_ok());
        assert!(parse("$[1]").is_ok());
        assert!(parse("$[-2]").is_ok());
        assert!(parse("$[1:3]").is_ok());
        assert!(parse("$[5:]").is_ok());
        assert!(parse("$[1:5:2]").is_ok());
        assert!(parse("$[5:1:-2]").is_ok());
        assert!(parse("$[::-1]").is_ok());
        assert!(parse("$.a[?@.b == 'kilo']").is_ok());
        assert!(parse("$.a[?(@.b == 'kilo')]").is_ok());
        assert!(parse("$.a[?@>3.5]").is_ok());
        assert!(parse("$.a[?@.b]").is_ok());
        assert!(parse("$[?@.*]").is_ok());
        assert!(parse("$[?@[?@.b]]").is_ok());
        assert!(parse("$.o[?@<3, ?@<3]").is_ok());
        assert!(parse("$.a[?@<2 || @.b == \"k\"]").is_ok());
        assert!(parse("$.a[?match(@.b, \"[jk]\")]").is_ok());
        assert!(parse("$.a[?search(@.b, \"[jk]\")]").is_ok());
        assert!(parse("$.o[?@>1 && @<4]").is_ok());
        assert!(parse("$.o[?@.u || @.x]").is_ok());
        assert!(parse("$.a[?@.b == $.x]").is_ok());
        assert!(parse("$.a[?@ == @]").is_ok());
        assert!(parse("$[?length(@) < 3]").is_ok());
        assert!(parse("$[?length(@.*) < 3]").is_ok());
        assert!(parse("$[?count(@.*) == 1]").is_ok());
        assert!(parse("$[?count(1) == 1]").is_ok());
        assert!(parse("$[?count(foo(@.*)) == 1]").is_ok());
        assert!(parse("$[?match(@.timezone, 'Europe/.*')]").is_ok());
        assert!(parse("$[?match(@.timezone, 'Europe/.*') == true]").is_ok());
        assert!(parse("$[?value(@..color) == \"red\"]").is_ok());
        assert!(parse("$[?value(@..color)]").is_ok());
        assert!(parse("$[?bar(@.a)]").is_ok());
        assert!(parse("$[?bnl(@.*)]").is_ok());
        assert!(parse("$[?blt(1==1)]").is_ok());
        assert!(parse("$[?blt(1)]").is_ok());
        assert!(parse("$[?bal(1)]").is_ok());
        assert!(parse("$[0, 3]").is_ok());
        assert!(parse("$[0:2, 5]").is_ok());
        assert!(parse("$[0, 0]").is_ok());
        assert!(parse("$..j").is_ok());
        assert!(parse("$..[0]").is_ok());
        assert!(parse("$..[*]").is_ok());
        assert!(parse("$..*").is_ok());
        assert!(parse("$..o").is_ok());
        assert!(parse("$.o..[*, *]").is_ok());
        assert!(parse("$.a..[0, 1]").is_ok());
        assert!(parse("$.a").is_ok());
        assert!(parse("$.a[0]").is_ok());
        assert!(parse("$.a.d").is_ok());
        assert!(parse("$.b[0]").is_ok());
        assert!(parse("$.b[*]").is_ok());
        assert!(parse("$.b[?@]").is_ok());
        assert!(parse("$.b[?@==null]").is_ok());
        assert!(parse("$.c[?@.d==null]").is_ok());
        assert!(parse("$.null").is_ok());
        assert!(parse("$.a").is_ok());
        assert!(parse("$[1]").is_ok());
        assert!(parse("$[-3]").is_ok());
        assert!(parse("$.a.b[1:2]").is_ok());
        assert!(parse("$[\"\\u000B\"]").is_ok());
        assert!(parse("$[\"\\u0061\"]").is_ok());
    }

    #[test]
    fn test_parse_ast() {
        assert_eq!(
            parse("$.store.book[0].title").unwrap(),
            JsonPath {
                segments: vec![
                    Segment::Child(vec![Selector::Name("store".to_owned())]),
                    Segment::Child(vec![Selector::Name("book".to_owned())]),
                    Segment::Child(vec![Selector::Index(0)]),
                    Segment::Child(vec![Selector::Name("title".to_owned())]),
                ],
            }
        );

        assert_eq!(
            parse("$[1:5:2]").unwrap(),
            JsonPath {
                segments: vec![Segment::Child(vec![Selector::Slice {
                    start: Some(1),
                    end: Some(5),
                    step: Some(2),
                }])],
            }
        );

        let path = parse("$.a[?@<2 || @.b == \"k\"]").unwrap();
        assert!(matches!(
            path.segments.as_slice(),
            [Segment::Child(_), Segment::Child(selectors)]
                if matches!(selectors.as_slice(), [Selector::Filter(LogicalExpr::Or(_, _))])
        ));
    }

    #[test]
    fn json_path_matcher_matches_structural_selectors() {
        let root = json!({
            "items": [
                { "id": 1, "name": "first" },
                { "id": 2, "name": "second" },
                { "id": 3, "name": "third" }
            ]
        });
        let paths = vec![
            "$.items[1].name".parse::<JsonPath>().unwrap(),
            "$.items[*].id".parse::<JsonPath>().unwrap(),
            "$..name".parse::<JsonPath>().unwrap(),
            "$.items[1:3]".parse::<JsonPath>().unwrap(),
        ];
        let mut matcher = JsonPathMatcher::new(&paths);
        let mut root_state = matcher.root_state(&root);
        let mut items_state = root_state.advance_name("items").unwrap();

        let mut item_1_state = items_state.advance_index(1).unwrap();
        let name_state = item_1_state.advance_name("name").unwrap();
        assert!(name_state.is_match());
        drop(name_state);
        drop(item_1_state);

        let mut item_0_state = items_state.advance_index(0).unwrap();
        let id_state = item_0_state.advance_name("id").unwrap();
        assert!(id_state.is_match());
        drop(id_state);
        assert!(!item_0_state.is_match());
        drop(item_0_state);

        let item_2_state = items_state.advance_index(2).unwrap();
        assert!(item_2_state.is_match());
        drop(item_2_state);
        drop(items_state);
        assert!(root_state.advance_name("missing").is_none());
    }

    #[test]
    fn json_path_matcher_does_not_leak_child_candidates_to_siblings() {
        let root = json!({
            "a": { "b": 1 },
            "c": { "b": 2 }
        });
        let paths = vec!["$.a.b".parse::<JsonPath>().unwrap()];
        let mut matcher = JsonPathMatcher::new(&paths);
        let mut root_state = matcher.root_state(&root);

        let mut a_state = root_state.advance_name("a").unwrap();
        let b_state = a_state.advance_name("b").unwrap();
        assert!(b_state.is_match());
        drop(b_state);
        drop(a_state);

        let mut c_state = root_state.advance_name("c").unwrap();
        let b_state = c_state.advance_name("b").unwrap();
        assert!(!b_state.is_match());
    }

    #[test]
    fn json_path_matcher_matches_descendant_child_suffixes() {
        let root = json!({
            "outer": {
                "target": { "value": 1 }
            }
        });
        let paths = vec!["$..target.value".parse::<JsonPath>().unwrap()];
        let mut matcher = JsonPathMatcher::new(&paths);
        let mut root_state = matcher.root_state(&root);
        let mut outer_state = root_state.advance_name("outer").unwrap();
        assert!(!outer_state.is_match());

        let mut target_state = outer_state.advance_name("target").unwrap();
        assert!(!target_state.is_match());

        let value_state = target_state.advance_name("value").unwrap();
        assert!(value_state.is_match());
    }

    #[test]
    fn json_path_matcher_does_not_leak_descendant_candidates_to_siblings() {
        let root = json!({
            "a": {
                "deep": { "target": 1 }
            },
            "b": {
                "target": 2
            }
        });
        let paths = vec!["$.a..target".parse::<JsonPath>().unwrap()];
        let mut matcher = JsonPathMatcher::new(&paths);
        let mut root_state = matcher.root_state(&root);

        let mut a_state = root_state.advance_name("a").unwrap();
        let mut deep_state = a_state.advance_name("deep").unwrap();
        let target_state = deep_state.advance_name("target").unwrap();
        assert!(target_state.is_match());
        drop(target_state);
        drop(deep_state);
        drop(a_state);

        let mut b_state = root_state.advance_name("b").unwrap();
        let target_state = b_state.advance_name("target").unwrap();
        assert!(!target_state.is_match());
    }

    #[test]
    fn json_path_matcher_matches_nested_descendant_suffixes() {
        let root = json!({
            "container": {
                "child": { "target": 1 }
            },
            "other": {
                "target": 2
            }
        });
        let paths = vec!["$..container..target".parse::<JsonPath>().unwrap()];
        let mut matcher = JsonPathMatcher::new(&paths);
        let mut root_state = matcher.root_state(&root);

        let mut container_state = root_state.advance_name("container").unwrap();
        assert!(!container_state.is_match());
        let mut child_state = container_state.advance_name("child").unwrap();
        let target_state = child_state.advance_name("target").unwrap();
        assert!(target_state.is_match());
        drop(target_state);
        drop(child_state);
        drop(container_state);

        let mut other_state = root_state.advance_name("other").unwrap();
        let target_state = other_state.advance_name("target").unwrap();
        assert!(!target_state.is_match());
    }

    #[test]
    fn json_path_matcher_matches_filter_selectors() {
        let root = json!({
            "x": "k",
            "a": [
                { "b": "k", "n": 1 },
                { "b": "m", "n": 2 },
                { "n": 3 },
                4
            ]
        });

        assert_eq!(
            matching_named_array_indices(&root, "$.a[?@.b]", "a"),
            vec![true, true, false, false]
        );
        assert_eq!(
            matching_named_array_indices(&root, "$.a[?@.b == $.x]", "a"),
            vec![true, false, false, false]
        );
        assert_eq!(
            matching_named_array_indices(&root, "$.a[?@.n >= 2]", "a"),
            vec![false, true, true, false]
        );
        assert_eq!(
            matching_named_array_indices(&root, "$.a[?@.n < 2 || @.b == \"m\"]", "a"),
            vec![true, true, false, false]
        );
        assert_eq!(
            matching_named_array_indices(&root, "$.a[?@ < 5]", "a"),
            vec![false, false, false, true]
        );
        assert_eq!(
            matching_named_array_indices(&root, "$.a[?@.missing == $.missing]", "a"),
            vec![true, true, true, true]
        );
    }

    #[test]
    fn json_path_matcher_matches_filter_functions() {
        let root = json!([
            { "tags": ["a"] },
            { "tags": ["a", "b"] },
            { "tags": [] },
            { "color": "red" },
            { "nested": { "color": "red" } },
            { "nested": { "color": "blue" } }
        ]);

        assert_eq!(
            matching_root_array_indices(&root, "$[?length(@.tags) < 2]"),
            vec![true, false, true, false, false, false]
        );
        assert_eq!(
            matching_root_array_indices(&root, "$[?count(@.tags[*]) == 1]"),
            vec![true, false, false, false, false, false]
        );
        assert_eq!(
            matching_root_array_indices(&root, "$[?value(@..color) == \"red\"]"),
            vec![false, false, false, true, true, false]
        );
    }

    #[test]
    fn json_path_matcher_matches_regex_functions() {
        let root = json!({
            "a": [
                { "b": "j" },
                { "b": "kilo" },
                { "b": "m" },
                { "b": 1 },
                {}
            ]
        });

        assert_eq!(
            matching_named_array_indices(&root, "$.a[?match(@.b, \"[jk]\")]", "a"),
            vec![true, false, false, false, false]
        );
        assert_eq!(
            matching_named_array_indices(&root, "$.a[?search(@.b, \"[jk]\")]", "a"),
            vec![true, true, false, false, false]
        );
        assert_eq!(
            matching_named_array_indices(&root, "$.a[?match(@.b, \"[jk]\") == true]", "a"),
            vec![true, false, false, false, false]
        );
    }

    #[test]
    fn json_path_matcher_matches_object_filter_selectors() {
        let root = json!({
            "o": {
                "one": 1,
                "two": 2,
                "four": 4,
                "obj": { "u": true },
                "none": {}
            }
        });
        let paths = vec![
            "$.o[?@ > 1 && @ < 4]".parse::<JsonPath>().unwrap(),
            "$.o[?@.u || @.x]".parse::<JsonPath>().unwrap(),
        ];
        let mut matcher = JsonPathMatcher::new(&paths);
        let mut root_state = matcher.root_state(&root);
        let mut object_state = root_state.advance_name("o").unwrap();

        let one_state = object_state.advance_name("one").unwrap();
        assert!(!one_state.is_match());
        drop(one_state);

        let two_state = object_state.advance_name("two").unwrap();
        assert!(two_state.is_match());
        drop(two_state);

        let four_state = object_state.advance_name("four").unwrap();
        assert!(!four_state.is_match());
        drop(four_state);

        let obj_state = object_state.advance_name("obj").unwrap();
        assert!(obj_state.is_match());
        drop(obj_state);

        let none_state = object_state.advance_name("none").unwrap();
        assert!(!none_state.is_match());
    }

    #[test]
    fn json_path_from_str_preserves_nom_error_details() {
        let error = "$#".parse::<JsonPath>().unwrap_err();

        assert_eq!(
            error.as_nom_error(),
            &nom::Err::Error(nom::error::Error {
                input: "#".to_owned(),
                code: ErrorKind::Eof,
            })
        );
        assert_eq!(error.to_string(), r##"Error(Error { input: "#", code: Eof })"##);
    }
}
