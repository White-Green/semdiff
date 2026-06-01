use crate::json_path::eval::JsonPathMatcher;
use crate::json_path::parser::number_literal::Number;
use crate::json_path::parser::{
    Comparable, ComparisonOp, FunctionArgument, FunctionExpr, Literal, LogicalExpr, Query, QueryRoot, Segment,
    Selector, SingularQuery, SingularSegment, TestExpr,
};
use crate::json_path::{JsonPath, parse};
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

fn assert_parse(input: &str, segments: Vec<Segment>) {
    assert_eq!(parse(input).unwrap(), JsonPath { segments });
}

fn child(selector: Selector) -> Segment {
    Segment::Child(vec![selector])
}

fn child_selectors(selectors: Vec<Selector>) -> Segment {
    Segment::Child(selectors)
}

fn descendant(selector: Selector) -> Segment {
    Segment::Descendant(vec![selector])
}

fn descendant_selectors(selectors: Vec<Selector>) -> Segment {
    Segment::Descendant(selectors)
}

fn name(value: &str) -> Selector {
    Selector::Name(value.to_owned())
}

fn wildcard() -> Selector {
    Selector::Wildcard
}

fn index(value: i64) -> Selector {
    Selector::Index(value)
}

fn slice(start: Option<i64>, end: Option<i64>, step: Option<i64>) -> Selector {
    Selector::Slice { start, end, step }
}

fn filter(expr: LogicalExpr) -> Selector {
    Selector::Filter(expr)
}

fn query(root: QueryRoot, segments: Vec<Segment>) -> Query {
    Query { root, segments }
}

fn test_query(root: QueryRoot, segments: Vec<Segment>) -> LogicalExpr {
    LogicalExpr::Test(TestExpr::Query(query(root, segments)))
}

fn singular(root: QueryRoot, segments: Vec<SingularSegment>) -> Comparable {
    Comparable::SingularQuery(SingularQuery { root, segments })
}

fn singular_name(value: &str) -> SingularSegment {
    SingularSegment::Name(value.to_owned())
}

fn comparison(left: Comparable, op: ComparisonOp, right: Comparable) -> LogicalExpr {
    LogicalExpr::Comparison { left, op, right }
}

fn or(left: LogicalExpr, right: LogicalExpr) -> LogicalExpr {
    LogicalExpr::Or(Box::new(left), Box::new(right))
}

fn and(left: LogicalExpr, right: LogicalExpr) -> LogicalExpr {
    LogicalExpr::And(Box::new(left), Box::new(right))
}

fn paren(expr: LogicalExpr) -> LogicalExpr {
    LogicalExpr::Paren(Box::new(expr))
}

fn int(value: i64) -> Comparable {
    Comparable::Literal(Literal::Number(Number::Int(value)))
}

fn float(value: f64) -> Comparable {
    Comparable::Literal(Literal::Number(Number::Float(value)))
}

fn string(value: &str) -> Comparable {
    Comparable::Literal(Literal::String(value.to_owned()))
}

fn bool_literal(value: bool) -> Comparable {
    Comparable::Literal(Literal::Bool(value))
}

fn null() -> Comparable {
    Comparable::Literal(Literal::Null)
}

fn function_expr(name: &str, arguments: Vec<FunctionArgument>) -> FunctionExpr {
    FunctionExpr {
        name: name.to_owned(),
        arguments,
    }
}

fn function(name: &str, arguments: Vec<FunctionArgument>) -> Comparable {
    Comparable::Function(function_expr(name, arguments))
}

fn function_test(name: &str, arguments: Vec<FunctionArgument>) -> LogicalExpr {
    LogicalExpr::Test(TestExpr::Function(function_expr(name, arguments)))
}

fn query_arg(root: QueryRoot, segments: Vec<Segment>) -> FunctionArgument {
    FunctionArgument::LogicalExpr(test_query(root, segments))
}

fn logical_arg(expr: LogicalExpr) -> FunctionArgument {
    FunctionArgument::LogicalExpr(expr)
}

fn literal_arg(literal: Literal) -> FunctionArgument {
    FunctionArgument::Literal(literal)
}

fn function_arg(name: &str, arguments: Vec<FunctionArgument>) -> FunctionArgument {
    FunctionArgument::Function(function_expr(name, arguments))
}

fn int_literal(value: i64) -> Literal {
    Literal::Number(Number::Int(value))
}

fn string_literal(value: &str) -> Literal {
    Literal::String(value.to_owned())
}

#[test]
fn test_parse() {
    assert_parse(
        "$['store']['book'][0]['title']",
        vec![
            child(name("store")),
            child(name("book")),
            child(index(0)),
            child(name("title")),
        ],
    );
    assert_parse(
        "$.store.book[0].title",
        vec![
            child(name("store")),
            child(name("book")),
            child(index(0)),
            child(name("title")),
        ],
    );
    assert_parse(
        "$.store.book[?@.price < 10].title",
        vec![
            child(name("store")),
            child(name("book")),
            child(filter(comparison(
                singular(QueryRoot::Current, vec![singular_name("price")]),
                ComparisonOp::Lt,
                int(10),
            ))),
            child(name("title")),
        ],
    );
    assert_parse(
        "$.store.book[*].author",
        vec![
            child(name("store")),
            child(name("book")),
            child(wildcard()),
            child(name("author")),
        ],
    );
    assert_parse("$..author", vec![descendant(name("author"))]);
    assert_parse("$.store.*", vec![child(name("store")), child(wildcard())]);
    assert_parse("$.store..price", vec![child(name("store")), descendant(name("price"))]);
    assert_parse("$..book[2]", vec![descendant(name("book")), child(index(2))]);
    assert_parse(
        "$..book[2].author",
        vec![descendant(name("book")), child(index(2)), child(name("author"))],
    );
    assert_parse(
        "$..book[2].publisher",
        vec![descendant(name("book")), child(index(2)), child(name("publisher"))],
    );
    assert_parse("$..book[-1]", vec![descendant(name("book")), child(index(-1))]);
    assert_parse(
        "$..book[0,1]",
        vec![descendant(name("book")), child_selectors(vec![index(0), index(1)])],
    );
    assert_parse(
        "$..book[:2]",
        vec![descendant(name("book")), child(slice(None, Some(2), None))],
    );
    assert_parse(
        "$..book[?@.isbn]",
        vec![
            descendant(name("book")),
            child(filter(test_query(QueryRoot::Current, vec![child(name("isbn"))]))),
        ],
    );
    assert_parse(
        "$..book[?@.price<10]",
        vec![
            descendant(name("book")),
            child(filter(comparison(
                singular(QueryRoot::Current, vec![singular_name("price")]),
                ComparisonOp::Lt,
                int(10),
            ))),
        ],
    );
    assert_parse("$..*", vec![descendant(wildcard())]);
    assert_parse("$", vec![]);
    assert_parse("$.o['j j']", vec![child(name("o")), child(name("j j"))]);
    assert_parse(
        "$.o['j j']['k.k']",
        vec![child(name("o")), child(name("j j")), child(name("k.k"))],
    );
    assert_parse(
        "$.o[\"j j\"][\"k.k\"]",
        vec![child(name("o")), child(name("j j")), child(name("k.k"))],
    );
    assert_parse("$[\"'\"][\"@\"]", vec![child(name("'")), child(name("@"))]);
    assert_parse("$[*]", vec![child(wildcard())]);
    assert_parse("$.o[*]", vec![child(name("o")), child(wildcard())]);
    assert_parse(
        "$.o[*, *]",
        vec![child(name("o")), child_selectors(vec![wildcard(), wildcard()])],
    );
    assert_parse("$.a[*]", vec![child(name("a")), child(wildcard())]);
    assert_parse("$[1]", vec![child(index(1))]);
    assert_parse("$[-2]", vec![child(index(-2))]);
    assert_parse("$[1:3]", vec![child(slice(Some(1), Some(3), None))]);
    assert_parse("$[5:]", vec![child(slice(Some(5), None, None))]);
    assert_parse("$[1:5:2]", vec![child(slice(Some(1), Some(5), Some(2)))]);
    assert_parse("$[5:1:-2]", vec![child(slice(Some(5), Some(1), Some(-2)))]);
    assert_parse("$[::-1]", vec![child(slice(None, None, Some(-1)))]);
    assert_parse(
        "$.a[?@.b == 'kilo']",
        vec![
            child(name("a")),
            child(filter(comparison(
                singular(QueryRoot::Current, vec![singular_name("b")]),
                ComparisonOp::Eq,
                string("kilo"),
            ))),
        ],
    );
    assert_parse(
        "$.a[?(@.b == 'kilo')]",
        vec![
            child(name("a")),
            child(filter(paren(comparison(
                singular(QueryRoot::Current, vec![singular_name("b")]),
                ComparisonOp::Eq,
                string("kilo"),
            )))),
        ],
    );
    assert_parse(
        "$.a[?@>3.5]",
        vec![
            child(name("a")),
            child(filter(comparison(
                singular(QueryRoot::Current, vec![]),
                ComparisonOp::Gt,
                float(3.5),
            ))),
        ],
    );
    assert_parse(
        "$.a[?@.b]",
        vec![
            child(name("a")),
            child(filter(test_query(QueryRoot::Current, vec![child(name("b"))]))),
        ],
    );
    assert_parse(
        "$[?@.*]",
        vec![child(filter(test_query(QueryRoot::Current, vec![child(wildcard())])))],
    );
    assert_parse(
        "$[?@[?@.b]]",
        vec![child(filter(test_query(
            QueryRoot::Current,
            vec![child(filter(test_query(QueryRoot::Current, vec![child(name("b"))])))],
        )))],
    );
    assert_parse(
        "$.o[?@<3, ?@<3]",
        vec![
            child(name("o")),
            child_selectors(vec![
                filter(comparison(
                    singular(QueryRoot::Current, vec![]),
                    ComparisonOp::Lt,
                    int(3),
                )),
                filter(comparison(
                    singular(QueryRoot::Current, vec![]),
                    ComparisonOp::Lt,
                    int(3),
                )),
            ]),
        ],
    );
    assert_parse(
        "$.a[?@<2 || @.b == \"k\"]",
        vec![
            child(name("a")),
            child(filter(or(
                comparison(singular(QueryRoot::Current, vec![]), ComparisonOp::Lt, int(2)),
                comparison(
                    singular(QueryRoot::Current, vec![singular_name("b")]),
                    ComparisonOp::Eq,
                    string("k"),
                ),
            ))),
        ],
    );
    assert_parse(
        "$.a[?match(@.b, \"[jk]\")]",
        vec![
            child(name("a")),
            child(filter(function_test(
                "match",
                vec![
                    query_arg(QueryRoot::Current, vec![child(name("b"))]),
                    literal_arg(string_literal("[jk]")),
                ],
            ))),
        ],
    );
    assert_parse(
        "$.a[?search(@.b, \"[jk]\")]",
        vec![
            child(name("a")),
            child(filter(function_test(
                "search",
                vec![
                    query_arg(QueryRoot::Current, vec![child(name("b"))]),
                    literal_arg(string_literal("[jk]")),
                ],
            ))),
        ],
    );
    assert_parse(
        "$.o[?@>1 && @<4]",
        vec![
            child(name("o")),
            child(filter(and(
                comparison(singular(QueryRoot::Current, vec![]), ComparisonOp::Gt, int(1)),
                comparison(singular(QueryRoot::Current, vec![]), ComparisonOp::Lt, int(4)),
            ))),
        ],
    );
    assert_parse(
        "$.o[?@.u || @.x]",
        vec![
            child(name("o")),
            child(filter(or(
                test_query(QueryRoot::Current, vec![child(name("u"))]),
                test_query(QueryRoot::Current, vec![child(name("x"))]),
            ))),
        ],
    );
    assert_parse(
        "$.a[?@.b == $.x]",
        vec![
            child(name("a")),
            child(filter(comparison(
                singular(QueryRoot::Current, vec![singular_name("b")]),
                ComparisonOp::Eq,
                singular(QueryRoot::Root, vec![singular_name("x")]),
            ))),
        ],
    );
    assert_parse(
        "$.a[?@ == @]",
        vec![
            child(name("a")),
            child(filter(comparison(
                singular(QueryRoot::Current, vec![]),
                ComparisonOp::Eq,
                singular(QueryRoot::Current, vec![]),
            ))),
        ],
    );
    assert_parse(
        "$[?length(@) < 3]",
        vec![child(filter(comparison(
            function("length", vec![query_arg(QueryRoot::Current, vec![])]),
            ComparisonOp::Lt,
            int(3),
        )))],
    );
    assert_parse(
        "$[?length(@.*) < 3]",
        vec![child(filter(comparison(
            function("length", vec![query_arg(QueryRoot::Current, vec![child(wildcard())])]),
            ComparisonOp::Lt,
            int(3),
        )))],
    );
    assert_parse(
        "$[?count(@.*) == 1]",
        vec![child(filter(comparison(
            function("count", vec![query_arg(QueryRoot::Current, vec![child(wildcard())])]),
            ComparisonOp::Eq,
            int(1),
        )))],
    );
    assert_parse(
        "$[?count(1) == 1]",
        vec![child(filter(comparison(
            function("count", vec![literal_arg(int_literal(1))]),
            ComparisonOp::Eq,
            int(1),
        )))],
    );
    assert_parse(
        "$[?count(foo(@.*)) == 1]",
        vec![child(filter(comparison(
            function(
                "count",
                vec![function_arg(
                    "foo",
                    vec![query_arg(QueryRoot::Current, vec![child(wildcard())])],
                )],
            ),
            ComparisonOp::Eq,
            int(1),
        )))],
    );
    assert_parse(
        "$[?match(@.timezone, 'Europe/.*')]",
        vec![child(filter(function_test(
            "match",
            vec![
                query_arg(QueryRoot::Current, vec![child(name("timezone"))]),
                literal_arg(string_literal("Europe/.*")),
            ],
        )))],
    );
    assert_parse(
        "$[?match(@.timezone, 'Europe/.*') == true]",
        vec![child(filter(comparison(
            function(
                "match",
                vec![
                    query_arg(QueryRoot::Current, vec![child(name("timezone"))]),
                    literal_arg(string_literal("Europe/.*")),
                ],
            ),
            ComparisonOp::Eq,
            bool_literal(true),
        )))],
    );
    assert_parse(
        "$[?value(@..color) == \"red\"]",
        vec![child(filter(comparison(
            function(
                "value",
                vec![query_arg(QueryRoot::Current, vec![descendant(name("color"))])],
            ),
            ComparisonOp::Eq,
            string("red"),
        )))],
    );
    assert_parse(
        "$[?value(@..color)]",
        vec![child(filter(function_test(
            "value",
            vec![query_arg(QueryRoot::Current, vec![descendant(name("color"))])],
        )))],
    );
    assert_parse(
        "$[?bar(@.a)]",
        vec![child(filter(function_test(
            "bar",
            vec![query_arg(QueryRoot::Current, vec![child(name("a"))])],
        )))],
    );
    assert_parse(
        "$[?bnl(@.*)]",
        vec![child(filter(function_test(
            "bnl",
            vec![query_arg(QueryRoot::Current, vec![child(wildcard())])],
        )))],
    );
    assert_parse(
        "$[?blt(1==1)]",
        vec![child(filter(function_test(
            "blt",
            vec![logical_arg(comparison(int(1), ComparisonOp::Eq, int(1)))],
        )))],
    );
    assert_parse(
        "$[?blt(1)]",
        vec![child(filter(function_test("blt", vec![literal_arg(int_literal(1))])))],
    );
    assert_parse(
        "$[?bal(1)]",
        vec![child(filter(function_test("bal", vec![literal_arg(int_literal(1))])))],
    );
    assert_parse("$[0, 3]", vec![child_selectors(vec![index(0), index(3)])]);
    assert_parse(
        "$[0:2, 5]",
        vec![child_selectors(vec![slice(Some(0), Some(2), None), index(5)])],
    );
    assert_parse("$[0, 0]", vec![child_selectors(vec![index(0), index(0)])]);
    assert_parse("$..j", vec![descendant(name("j"))]);
    assert_parse("$..[0]", vec![descendant(index(0))]);
    assert_parse("$..[*]", vec![descendant(wildcard())]);
    assert_parse("$..*", vec![descendant(wildcard())]);
    assert_parse("$..o", vec![descendant(name("o"))]);
    assert_parse(
        "$.o..[*, *]",
        vec![child(name("o")), descendant_selectors(vec![wildcard(), wildcard()])],
    );
    assert_parse(
        "$.a..[0, 1]",
        vec![child(name("a")), descendant_selectors(vec![index(0), index(1)])],
    );
    assert_parse("$.a", vec![child(name("a"))]);
    assert_parse("$.a[0]", vec![child(name("a")), child(index(0))]);
    assert_parse("$.a.d", vec![child(name("a")), child(name("d"))]);
    assert_parse("$.b[0]", vec![child(name("b")), child(index(0))]);
    assert_parse("$.b[*]", vec![child(name("b")), child(wildcard())]);
    assert_parse(
        "$.b[?@]",
        vec![child(name("b")), child(filter(test_query(QueryRoot::Current, vec![])))],
    );
    assert_parse(
        "$.b[?@==null]",
        vec![
            child(name("b")),
            child(filter(comparison(
                singular(QueryRoot::Current, vec![]),
                ComparisonOp::Eq,
                null(),
            ))),
        ],
    );
    assert_parse(
        "$.c[?@.d==null]",
        vec![
            child(name("c")),
            child(filter(comparison(
                singular(QueryRoot::Current, vec![singular_name("d")]),
                ComparisonOp::Eq,
                null(),
            ))),
        ],
    );
    assert_parse("$.null", vec![child(name("null"))]);
    assert_parse("$.a", vec![child(name("a"))]);
    assert_parse("$[1]", vec![child(index(1))]);
    assert_parse("$[-3]", vec![child(index(-3))]);
    assert_parse(
        "$.a.b[1:2]",
        vec![child(name("a")), child(name("b")), child(slice(Some(1), Some(2), None))],
    );
    assert_parse("$[\"\\u000B\"]", vec![child(name("\u{000B}"))]);
    assert_parse("$[\"\\u0061\"]", vec![child(name("a"))]);
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
