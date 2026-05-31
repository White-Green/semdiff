use crate::json_path::integer_literal::IntegerLiteral;
use crate::json_path::number_literal::{Number, NumberLiteral};
use crate::json_path::string_literal::StringLiteral;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while, take_while1};
use nom::character::complete::char;
use nom::combinator::{all_consuming, map, opt, recognize, value};
use nom::error::ErrorKind;
use nom::multi::{many0, separated_list0};
use nom::sequence::{delimited, pair, preceded, terminated};
use nom::{Err, IResult, Parser};
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

mod integer_literal;
mod number_literal;
mod string_literal;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonPath {
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq)]
enum Segment {
    Child(Vec<Selector>),
    Descendant(Vec<Selector>),
}

#[derive(Debug, Clone, PartialEq)]
enum Selector {
    Name(String),
    Wildcard,
    Slice {
        start: Option<i64>,
        end: Option<i64>,
        step: Option<i64>,
    },
    Index(i64),
    Filter(LogicalExpr),
}

#[derive(Debug, Clone, PartialEq)]
struct Query {
    root: QueryRoot,
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryRoot {
    Root,
    Current,
}

#[derive(Debug, Clone, PartialEq)]
enum LogicalExpr {
    Or(Box<LogicalExpr>, Box<LogicalExpr>),
    And(Box<LogicalExpr>, Box<LogicalExpr>),
    Not(Box<LogicalExpr>),
    Paren(Box<LogicalExpr>),
    Comparison {
        left: Comparable,
        op: ComparisonOp,
        right: Comparable,
    },
    Test(TestExpr),
}

#[derive(Debug, Clone, PartialEq)]
enum TestExpr {
    Query(Query),
    Function(FunctionExpr),
}

#[derive(Debug, Clone, PartialEq)]
enum Comparable {
    Literal(Literal),
    SingularQuery(SingularQuery),
    Function(FunctionExpr),
}

#[derive(Debug, Clone, PartialEq)]
struct SingularQuery {
    root: QueryRoot,
    segments: Vec<SingularSegment>,
}

#[derive(Debug, Clone, PartialEq)]
enum SingularSegment {
    Name(String),
    Index(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComparisonOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
enum Literal {
    Number(Number),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
struct FunctionExpr {
    name: String,
    arguments: Vec<FunctionArgument>,
}

#[derive(Debug, Clone, PartialEq)]
enum FunctionArgument {
    Literal(Literal),
    Query(Query),
    LogicalExpr(LogicalExpr),
    Function(FunctionExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(nom::Err<nom::error::Error<String>>);

impl ParseError {
    pub fn as_nom_error(&self) -> &nom::Err<nom::error::Error<String>> {
        &self.0
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::error::Error for ParseError {}

impl From<nom::Err<nom::error::Error<&str>>> for ParseError {
    fn from(value: nom::Err<nom::error::Error<&str>>) -> Self {
        match value {
            nom::Err::Incomplete(needed) => ParseError(nom::Err::Incomplete(needed)),
            nom::Err::Error(error) => ParseError(nom::Err::Error(nom::error::Error {
                input: error.input.to_owned(),
                code: error.code,
            })),
            nom::Err::Failure(error) => ParseError(nom::Err::Failure(nom::error::Error {
                input: error.input.to_owned(),
                code: error.code,
            })),
        }
    }
}

impl FromStr for JsonPath {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        parse(input)
    }
}

pub trait JsonPathValue<'a>: Copy + Sized {
    type ArrayType: 'a + JsonPathArray<'a, JsonPathValue = Self>;
    type ObjectType: 'a + JsonPathObject<'a, JsonPathValue = Self>;

    fn as_array(&self) -> Option<Self::ArrayType>;
    fn as_object(&self) -> Option<Self::ObjectType>;
    fn as_integer(&self) -> Option<i64>;
    fn as_float(&self) -> Option<f64>;
    fn as_bool(&self) -> Option<bool>;
    fn as_str(&self) -> Option<&'a str>;
    fn is_null(&self) -> bool;
}

pub trait JsonPathArray<'a> {
    type JsonPathValue: JsonPathValue<'a>;

    fn len(&self) -> usize;
    fn element(&self, index: usize) -> Option<Self::JsonPathValue>;
}

pub trait JsonPathObject<'a> {
    type JsonPathValue: JsonPathValue<'a>;

    fn member(&self, name: &str) -> Option<Self::JsonPathValue>;
}

#[derive(Debug, Clone, Copy)]
pub struct SerdeJsonPathArray<'a> {
    values: &'a [serde_json::Value],
}

#[derive(Debug, Clone, Copy)]
pub struct SerdeJsonPathObject<'a> {
    values: &'a serde_json::Map<String, serde_json::Value>,
}

impl<'a> JsonPathValue<'a> for &'a serde_json::Value {
    type ArrayType = SerdeJsonPathArray<'a>;
    type ObjectType = SerdeJsonPathObject<'a>;

    fn as_array(&self) -> Option<Self::ArrayType> {
        match *self {
            serde_json::Value::Array(values) => Some(SerdeJsonPathArray { values }),
            _ => None,
        }
    }

    fn as_object(&self) -> Option<Self::ObjectType> {
        match *self {
            serde_json::Value::Object(values) => Some(SerdeJsonPathObject { values }),
            _ => None,
        }
    }

    fn as_integer(&self) -> Option<i64> {
        match *self {
            serde_json::Value::Number(value) => value.as_i64(),
            _ => None,
        }
    }

    fn as_float(&self) -> Option<f64> {
        match *self {
            serde_json::Value::Number(value) => value.as_f64(),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match *self {
            serde_json::Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&'a str> {
        match *self {
            serde_json::Value::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    fn is_null(&self) -> bool {
        matches!(*self, serde_json::Value::Null)
    }
}

impl<'a> JsonPathArray<'a> for SerdeJsonPathArray<'a> {
    type JsonPathValue = &'a serde_json::Value;

    fn len(&self) -> usize {
        self.values.len()
    }

    fn element(&self, index: usize) -> Option<Self::JsonPathValue> {
        self.values.get(index)
    }
}

impl<'a> JsonPathObject<'a> for SerdeJsonPathObject<'a> {
    type JsonPathValue = &'a serde_json::Value;

    fn member(&self, name: &str) -> Option<Self::JsonPathValue> {
        self.values.get(name)
    }
}

pub(crate) struct JsonPathMatcher<'a> {
    paths: &'a [JsonPath],
    active: Vec<ActiveState>,
}

pub(crate) struct JsonPathMatchState<'state, 'value, V>
where
    V: JsonPathValue<'value>,
{
    paths: &'state [JsonPath],
    root: V,
    current: V,
    active: &'state mut Vec<ActiveState>,
    start: usize,
    end: usize,
    matched: bool,
    _marker: PhantomData<&'value ()>,
}

impl<'state, 'value, V> JsonPathMatchState<'state, 'value, V>
where
    V: JsonPathValue<'value>,
{
    pub(crate) fn is_match(&self) -> bool {
        self.matched
    }

    pub(crate) fn advance_name<'child>(&'child mut self, name: &str) -> Option<JsonPathMatchState<'child, 'value, V>> {
        let child = self.current.as_object()?.member(name)?;
        Some(self.advance(JsonPathMatchStep::Name(name), child))
    }

    pub(crate) fn advance_index<'child>(
        &'child mut self,
        index: usize,
    ) -> Option<JsonPathMatchState<'child, 'value, V>> {
        let child = self.current.as_array()?.element(index)?;
        Some(self.advance(JsonPathMatchStep::Index(index), child))
    }

    fn advance<'child>(
        &'child mut self,
        step: JsonPathMatchStep<'_>,
        child: V,
    ) -> JsonPathMatchState<'child, 'value, V> {
        let paths = self.paths;
        let root = self.root;
        let parent_current = self.current;
        let parent_start = self.start;
        let parent_end = self.end;
        let start = self.active.len();
        let active = &mut *self.active;
        let mut next = JsonPathMatchState {
            paths,
            root,
            current: child,
            active,
            start,
            end: start,
            matched: false,
            _marker: PhantomData,
        };
        for active_index in parent_start..parent_end {
            let active = next.active[active_index];
            let Some(segment) = next.paths[active.path_index].segments.get(active.segment_index) else {
                continue;
            };
            match segment {
                Segment::Child(selectors) => {
                    if selectors
                        .iter()
                        .any(|selector| Self::selector_matches(parent_current, selector, step))
                    {
                        next.push_state(active.path_index, active.segment_index + 1);
                    }
                }
                Segment::Descendant(selectors) => {
                    next.push_state(active.path_index, active.segment_index);
                    if selectors
                        .iter()
                        .any(|selector| Self::selector_matches(parent_current, selector, step))
                    {
                        next.push_state(active.path_index, active.segment_index + 1);
                    }
                }
            }
        }
        next
    }

    fn push_state(&mut self, path_index: usize, segment_index: usize) {
        if segment_index == self.paths[path_index].segments.len() {
            self.matched = true;
            return;
        }
        let active = ActiveState {
            path_index,
            segment_index,
        };
        if !self.active[self.start..self.end].contains(&active) {
            self.active.push(active);
            self.end += 1;
        }
    }

    fn selector_matches(parent: V, selector: &Selector, step: JsonPathMatchStep<'_>) -> bool {
        match (selector, step) {
            (Selector::Name(name), JsonPathMatchStep::Name(step_name)) => name == step_name,
            (Selector::Wildcard, JsonPathMatchStep::Name(_) | JsonPathMatchStep::Index(_)) => true,
            (Selector::Index(index), JsonPathMatchStep::Index(array_index)) => {
                Self::index_matches(parent, *index, array_index)
            }
            (Selector::Slice { start, end, step }, JsonPathMatchStep::Index(array_index)) => {
                Self::slice_matches(parent, *start, *end, *step, array_index)
            }
            _ => false,
        }
    }

    fn index_matches(parent: V, index: i64, array_index: usize) -> bool {
        let Some(array) = parent.as_array() else {
            return false;
        };
        let array_len = array.len();
        let normalized = if index < 0 { array_len as i64 + index } else { index };
        normalized >= 0 && usize::try_from(normalized).is_ok_and(|index| index == array_index)
    }

    fn slice_matches(parent: V, start: Option<i64>, end: Option<i64>, step: Option<i64>, array_index: usize) -> bool {
        let Some(array) = parent.as_array() else {
            return false;
        };
        let array_len = array.len();
        let len = array_len as i64;
        let index = array_index as i64;
        let step = step.unwrap_or(1);
        if step == 0 {
            return false;
        }
        let normalize = |value: i64| if value < 0 { len + value } else { value };
        if step > 0 {
            let start = start.map(normalize).unwrap_or(0).clamp(0, len);
            let end = end.map(normalize).unwrap_or(len).clamp(0, len);
            index >= start && index < end && (index - start) % step == 0
        } else {
            let start = start.map(normalize).unwrap_or(len - 1).clamp(-1, len - 1);
            let end = end.map(normalize).unwrap_or(-1).clamp(-1, len - 1);
            index <= start && index > end && (start - index) % -step == 0
        }
    }
}

impl<'state, 'value, V> Drop for JsonPathMatchState<'state, 'value, V>
where
    V: JsonPathValue<'value>,
{
    fn drop(&mut self) {
        self.active.truncate(self.start);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveState {
    path_index: usize,
    segment_index: usize,
}

#[derive(Debug, Clone, Copy)]
enum JsonPathMatchStep<'a> {
    Name(&'a str),
    Index(usize),
}

impl<'a> JsonPathMatcher<'a> {
    pub(crate) fn new(paths: &'a [JsonPath]) -> Self {
        Self {
            paths,
            active: Vec::new(),
        }
    }

    pub(crate) fn root_state<'state, 'value, V>(&'state mut self, root: V) -> JsonPathMatchState<'state, 'value, V>
    where
        V: JsonPathValue<'value>,
    {
        self.active.clear();
        let mut state = JsonPathMatchState {
            paths: self.paths,
            root,
            current: root,
            active: &mut self.active,
            start: 0,
            end: 0,
            matched: false,
            _marker: PhantomData,
        };
        for path_index in 0..self.paths.len() {
            state.push_state(path_index, 0);
        }
        state
    }
}

// JSONPath parser
// see: RFC 9535

pub fn parse(input: &str) -> Result<JsonPath, ParseError> {
    all_consuming(jsonpath_query)
        .parse(input)
        .map(|(_, query)| JsonPath {
            segments: query.segments,
        })
        .map_err(ParseError::from)
}

fn jsonpath_query(input: &str) -> IResult<&str, Query> {
    map(pair(char('$'), segments), |(_, segments)| Query {
        root: QueryRoot::Root,
        segments,
    })
    .parse(input)
}

fn filter_query(input: &str) -> IResult<&str, Query> {
    alt((
        map(pair(char('@'), segments), |(_, segments)| Query {
            root: QueryRoot::Current,
            segments,
        }),
        jsonpath_query,
    ))
    .parse(input)
}

fn segments(input: &str) -> IResult<&str, Vec<Segment>> {
    many0(preceded(s, segment)).parse(input)
}

fn segment(input: &str) -> IResult<&str, Segment> {
    alt((descendant_segment, child_segment)).parse(input)
}

fn child_segment(input: &str) -> IResult<&str, Segment> {
    alt((
        map(bracketed_selection, Segment::Child),
        map(
            preceded(
                char('.'),
                alt((
                    value(vec![Selector::Wildcard], char('*')),
                    map(member_name_shorthand, |name| vec![Selector::Name(name)]),
                )),
            ),
            Segment::Child,
        ),
    ))
    .parse(input)
}

fn descendant_segment(input: &str) -> IResult<&str, Segment> {
    map(
        preceded(
            tag(".."),
            alt((
                bracketed_selection,
                value(vec![Selector::Wildcard], char('*')),
                map(member_name_shorthand, |name| vec![Selector::Name(name)]),
            )),
        ),
        Segment::Descendant,
    )
    .parse(input)
}

fn bracketed_selection(input: &str) -> IResult<&str, Vec<Selector>> {
    delimited(
        terminated(char('['), s),
        separated_list0(delimited(s, char(','), s), selector),
        preceded(s, char(']')),
    )
    .parse(input)
}

fn selector(input: &str) -> IResult<&str, Selector> {
    alt((
        map(StringLiteral::new(), Selector::Name),
        value(Selector::Wildcard, char('*')),
        slice_selector,
        map(index_selector, Selector::Index),
        filter_selector,
    ))
    .parse(input)
}

fn filter_selector(input: &str) -> IResult<&str, Selector> {
    map(preceded(pair(char('?'), s), logical_expr), Selector::Filter).parse(input)
}

fn slice_selector(input: &str) -> IResult<&str, Selector> {
    map(
        (
            opt(terminated(index_selector, s)),
            char(':'),
            preceded(s, opt(terminated(index_selector, s))),
            opt(preceded(pair(char(':'), s), index_selector)),
        ),
        |(start, _, end, step)| Selector::Slice { start, end, step },
    )
    .parse(input)
}

fn index_selector(input: &str) -> IResult<&str, i64> {
    IntegerLiteral::new().parse(input)
}

fn logical_expr(input: &str) -> IResult<&str, LogicalExpr> {
    logical_or_expr(input)
}

fn logical_or_expr(input: &str) -> IResult<&str, LogicalExpr> {
    let (mut input, mut expr) = logical_and_expr(input)?;
    loop {
        match preceded((s, tag("||"), s), logical_and_expr).parse(input) {
            Ok((next, rhs)) => {
                expr = LogicalExpr::Or(Box::new(expr), Box::new(rhs));
                input = next;
            }
            Err(_) => return Ok((input, expr)),
        }
    }
}

fn logical_and_expr(input: &str) -> IResult<&str, LogicalExpr> {
    let (mut input, mut expr) = basic_expr(input)?;
    loop {
        match preceded((s, tag("&&"), s), basic_expr).parse(input) {
            Ok((next, rhs)) => {
                expr = LogicalExpr::And(Box::new(expr), Box::new(rhs));
                input = next;
            }
            Err(_) => return Ok((input, expr)),
        }
    }
}

fn basic_expr(input: &str) -> IResult<&str, LogicalExpr> {
    alt((paren_expr, comparison_expr, test_expr)).parse(input)
}

fn paren_expr(input: &str) -> IResult<&str, LogicalExpr> {
    map(
        pair(
            opt(terminated(logical_not_op, s)),
            delimited(char('('), delimited(s, logical_expr, s), char(')')),
        ),
        |(not, expr)| {
            let expr = LogicalExpr::Paren(Box::new(expr));
            if not.is_some() {
                LogicalExpr::Not(Box::new(expr))
            } else {
                expr
            }
        },
    )
    .parse(input)
}

fn test_expr(input: &str) -> IResult<&str, LogicalExpr> {
    map(
        pair(
            opt(terminated(logical_not_op, s)),
            alt((
                map(filter_query, TestExpr::Query),
                map(function_expr, TestExpr::Function),
            )),
        ),
        |(not, expr)| {
            let expr = LogicalExpr::Test(expr);
            if not.is_some() {
                LogicalExpr::Not(Box::new(expr))
            } else {
                expr
            }
        },
    )
    .parse(input)
}

fn logical_not_op(input: &str) -> IResult<&str, char> {
    char('!').parse(input)
}

fn comparison_expr(input: &str) -> IResult<&str, LogicalExpr> {
    map(
        (comparable, delimited(s, comparison_op, s), comparable),
        |(left, op, right)| LogicalExpr::Comparison { left, op, right },
    )
    .parse(input)
}

fn comparable(input: &str) -> IResult<&str, Comparable> {
    alt((
        map(literal, Comparable::Literal),
        map(singular_query, Comparable::SingularQuery),
        map(function_expr, Comparable::Function),
    ))
    .parse(input)
}

fn comparison_op(input: &str) -> IResult<&str, ComparisonOp> {
    alt((
        value(ComparisonOp::Eq, tag("==")),
        value(ComparisonOp::Ne, tag("!=")),
        value(ComparisonOp::Le, tag("<=")),
        value(ComparisonOp::Ge, tag(">=")),
        value(ComparisonOp::Lt, tag("<")),
        value(ComparisonOp::Gt, tag(">")),
    ))
    .parse(input)
}

fn singular_query(input: &str) -> IResult<&str, SingularQuery> {
    alt((
        map(pair(char('@'), singular_query_segments), |(_, segments)| {
            SingularQuery {
                root: QueryRoot::Current,
                segments,
            }
        }),
        map(pair(char('$'), singular_query_segments), |(_, segments)| {
            SingularQuery {
                root: QueryRoot::Root,
                segments,
            }
        }),
    ))
    .parse(input)
}

fn singular_query_segments(input: &str) -> IResult<&str, Vec<SingularSegment>> {
    many0(preceded(
        s,
        alt((
            map(name_segment, SingularSegment::Name),
            map(index_segment, SingularSegment::Index),
        )),
    ))
    .parse(input)
}

fn name_segment(input: &str) -> IResult<&str, String> {
    alt((
        delimited(char('['), delimited(s, StringLiteral::new(), s), char(']')),
        preceded(char('.'), member_name_shorthand),
    ))
    .parse(input)
}

fn index_segment(input: &str) -> IResult<&str, i64> {
    delimited(char('['), delimited(s, index_selector, s), char(']')).parse(input)
}

fn literal(input: &str) -> IResult<&str, Literal> {
    alt((
        map(NumberLiteral::new(), Literal::Number),
        map(StringLiteral::new(), Literal::String),
        value(Literal::Bool(true), tag("true")),
        value(Literal::Bool(false), tag("false")),
        value(Literal::Null, tag("null")),
    ))
    .parse(input)
}

fn function_expr(input: &str) -> IResult<&str, FunctionExpr> {
    map(
        pair(
            function_name,
            delimited(
                pair(char('('), s),
                separated_list0(delimited(s, char(','), s), function_argument),
                preceded(s, char(')')),
            ),
        ),
        |(name, arguments)| FunctionExpr { name, arguments },
    )
    .parse(input)
}

fn function_argument(input: &str) -> IResult<&str, FunctionArgument> {
    alt((
        map(function_expr, FunctionArgument::Function),
        map(logical_expr, FunctionArgument::LogicalExpr),
        map(filter_query, FunctionArgument::Query),
        map(literal, FunctionArgument::Literal),
    ))
    .parse(input)
}

fn function_name(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(
            take_while1(|c: char| c.is_ascii_lowercase()),
            take_while(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
        )),
        ToOwned::to_owned,
    )
    .parse(input)
}

fn member_name_shorthand(input: &str) -> IResult<&str, String> {
    let (input, first) = take_char_if(input, is_name_first)?;
    let (input, rest) = take_while(is_name_char).parse(input)?;
    let mut name = String::from(first);
    name.push_str(rest);
    Ok((input, name))
}

fn s(input: &str) -> IResult<&str, &str> {
    take_while(|c: char| matches!(c, ' ' | '\t' | '\r' | '\n')).parse(input)
}

fn take_char_if(input: &str, predicate: fn(char) -> bool) -> IResult<&str, char> {
    let Some(ch) = input.chars().next() else {
        return Err(Err::Error(nom::error::Error::new(input, ErrorKind::Char)));
    };
    if predicate(ch) {
        Ok((&input[ch.len_utf8()..], ch))
    } else {
        Err(Err::Error(nom::error::Error::new(input, ErrorKind::Char)))
    }
}

fn is_name_first(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || is_non_surrogate_non_ascii(ch)
}

fn is_name_char(ch: char) -> bool {
    is_name_first(ch) || ch.is_ascii_digit()
}

fn is_non_surrogate_non_ascii(ch: char) -> bool {
    matches!(ch as u32, 0x80..=0xD7FF | 0xE000..=0x10FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
