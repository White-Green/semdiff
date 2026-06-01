use crate::json_path::parser::integer_literal::IntegerLiteral;
use crate::json_path::parser::number_literal::{Number, NumberLiteral};
use crate::json_path::parser::string_literal::StringLiteral;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while, take_while1};
use nom::character::complete::char;
use nom::combinator::{all_consuming, map, opt, recognize, value};
use nom::error::ErrorKind;
use nom::multi::{many0, separated_list0};
use nom::sequence::{delimited, pair, preceded, terminated};
use nom::{Err, IResult, Parser};
use std::fmt;
use std::str::FromStr;

pub(crate) mod integer_literal;
pub(crate) mod number_literal;
pub(crate) mod string_literal;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonPath {
    pub(crate) segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Segment {
    Child(Vec<Selector>),
    Descendant(Vec<Selector>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Selector {
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
pub(crate) struct Query {
    pub(crate) root: QueryRoot,
    pub(crate) segments: Vec<Segment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryRoot {
    Root,
    Current,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LogicalExpr {
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
pub(crate) enum TestExpr {
    Query(Query),
    Function(FunctionExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Comparable {
    Literal(Literal),
    SingularQuery(SingularQuery),
    Function(FunctionExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SingularQuery {
    pub(crate) root: QueryRoot,
    pub(crate) segments: Vec<SingularSegment>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SingularSegment {
    Name(String),
    Index(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComparisonOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Literal {
    Number(Number),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FunctionExpr {
    pub(crate) name: String,
    pub(crate) arguments: Vec<FunctionArgument>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FunctionArgument {
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
            Err::Incomplete(needed) => ParseError(Err::Incomplete(needed)),
            Err::Error(error) => ParseError(Err::Error(nom::error::Error {
                input: error.input.to_owned(),
                code: error.code,
            })),
            Err::Failure(error) => ParseError(Err::Failure(nom::error::Error {
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
