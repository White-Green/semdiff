use crate::json_path::JsonPath;
use crate::json_path::eval::{JsonPathMatchState, JsonPathMatcher};
use mime::Mime;
use semdiff_core::fs::FileLeaf;
use semdiff_core::{Diff, DiffCalculator, MayUnsupported};
use serde_json::Value;
use similar::algorithms::DiffHook;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt::Display;
use std::{convert, fmt, mem};

pub mod json_path;
pub mod report_html;
pub mod report_json;
pub mod report_summary;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, Default)]
pub struct JsonDiffReporter;

#[derive(Debug)]
enum JsonDiffBody {
    Equal { body: String, ignored_lines: JsonDiffLines },
    Modified(JsonDiffLines),
}

#[derive(Debug)]
pub struct JsonDiff {
    body: JsonDiffBody,
}

impl Diff for JsonDiff {
    fn equal(&self) -> bool {
        matches!(self.body, JsonDiffBody::Equal { .. })
    }
}

impl JsonDiff {
    fn body(&self) -> &JsonDiffBody {
        &self.body
    }
}

#[derive(Debug, Clone)]
pub struct JsonDiffCalculator {
    ignore_object_key_order: bool,
    ignore_paths: Vec<JsonPath>,
}

impl Default for JsonDiffCalculator {
    fn default() -> Self {
        Self::new(false, Vec::new())
    }
}

impl JsonDiffCalculator {
    pub fn new(ignore_object_key_order: bool, ignore_paths: Vec<JsonPath>) -> Self {
        Self {
            ignore_object_key_order,
            ignore_paths,
        }
    }

    pub fn ignore_object_key_order(&self) -> bool {
        self.ignore_object_key_order
    }
}

impl DiffCalculator<FileLeaf> for JsonDiffCalculator {
    type Error = convert::Infallible;
    type Diff = JsonDiff;

    fn diff(
        &self,
        _name: &str,
        expected: FileLeaf,
        actual: FileLeaf,
    ) -> Result<MayUnsupported<Self::Diff>, Self::Error> {
        if !is_json_mime(&expected.kind) || !is_json_mime(&actual.kind) {
            return Ok(MayUnsupported::Unsupported);
        }
        let Ok(mut expected) = serde_json::from_slice::<Value>(&expected.content) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let Ok(mut actual) = serde_json::from_slice::<Value>(&actual.content) else {
            return Ok(MayUnsupported::Unsupported);
        };
        if self.ignore_object_key_order {
            expected.sort_all_objects();
            actual.sort_all_objects();
        }
        let diff = json_diff(&expected, &actual, &self.ignore_paths);
        let body = if diff.iter().all(JsonDiffLine::is_equal_for_result) {
            let ignored_lines = if diff.iter().any(JsonDiffLine::is_ignored) {
                diff
            } else {
                JsonDiffLines::default()
            };
            JsonDiffBody::Equal {
                body: serde_json::to_string_pretty(&expected).unwrap(),
                ignored_lines,
            }
        } else {
            JsonDiffBody::Modified(diff)
        };
        let result = JsonDiff { body };
        Ok(MayUnsupported::Ok(result))
    }
}

fn is_json_mime(kind: &Mime) -> bool {
    if kind == &mime::APPLICATION_JSON {
        return true;
    }
    if kind.type_() == mime::APPLICATION
        && let Some(suffix) = kind.subtype().as_str().strip_suffix("+json")
    {
        return !suffix.is_empty();
    }
    kind.essence_str() == "text/json"
}

fn try_into_json(content: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<Value>(content).ok()?;
    Some(serde_json::to_string_pretty(&value).unwrap())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeTag {
    Unchanged,
    Ignored,
    Added,
    Deleted,
}

#[derive(Debug)]
struct JsonDiffLine {
    indent: usize,
    state: JsonDiffLineState,
}

impl JsonDiffLine {
    fn tag(&self) -> ChangeTag {
        match self.state {
            JsonDiffLineState::Unchanged { .. } => ChangeTag::Unchanged,
            JsonDiffLineState::Ignored { .. } => ChangeTag::Ignored,
            JsonDiffLineState::Added(_) => ChangeTag::Added,
            JsonDiffLineState::Deleted(_) => ChangeTag::Deleted,
        }
    }

    fn is_equal_for_result(&self) -> bool {
        matches!(
            self.state,
            JsonDiffLineState::Unchanged { .. } | JsonDiffLineState::Ignored { .. }
        )
    }

    fn is_ignored(&self) -> bool {
        matches!(self.state, JsonDiffLineState::Ignored { .. })
    }

    fn has_expected(&self) -> bool {
        !matches!(
            self.state,
            JsonDiffLineState::Added(_) | JsonDiffLineState::Ignored { expected: None, .. }
        )
    }

    fn has_actual(&self) -> bool {
        !matches!(
            self.state,
            JsonDiffLineState::Deleted(_) | JsonDiffLineState::Ignored { actual: None, .. }
        )
    }

    fn preview_text(&self) -> &str {
        match &self.state {
            JsonDiffLineState::Unchanged { expected, .. } => expected,
            JsonDiffLineState::Ignored { expected, actual } => {
                expected.as_ref().or(actual.as_ref()).map_or("", String::as_str)
            }
            JsonDiffLineState::Added(actual) => actual,
            JsonDiffLineState::Deleted(expected) => expected,
        }
    }

    fn display_expected(&self) -> impl Display {
        fmt::from_fn(|f| {
            let expected = match &self.state {
                JsonDiffLineState::Unchanged { expected, .. } => expected,
                JsonDiffLineState::Ignored {
                    expected: Some(expected),
                    ..
                } => expected,
                JsonDiffLineState::Ignored { expected: None, .. } => return Ok(()),
                JsonDiffLineState::Added(_) => return Ok(()),
                JsonDiffLineState::Deleted(expected) => expected,
            };
            for _ in 0..self.indent {
                f.write_str("  ")?;
            }
            f.write_str(expected)?;
            Ok(())
        })
    }

    fn display_actual(&self) -> impl Display {
        fmt::from_fn(|f| {
            let actual = match &self.state {
                JsonDiffLineState::Unchanged { actual, .. } => actual,
                JsonDiffLineState::Ignored {
                    actual: Some(actual), ..
                } => actual,
                JsonDiffLineState::Ignored { actual: None, .. } => return Ok(()),
                JsonDiffLineState::Added(actual) => actual,
                JsonDiffLineState::Deleted(_) => return Ok(()),
            };
            for _ in 0..self.indent {
                f.write_str("  ")?;
            }
            f.write_str(actual)?;
            Ok(())
        })
    }
}

#[derive(Debug)]
enum JsonDiffLineState {
    Unchanged {
        expected: String,
        actual: String,
    },
    Ignored {
        expected: Option<String>,
        actual: Option<String>,
    },
    Added(String),
    Deleted(String),
}

#[derive(Debug, Default)]
struct JsonDiffLines {
    lines: Vec<JsonDiffLine>,
}

impl JsonDiffLines {
    fn writer(&mut self) -> JsonDiffLineWriter<'_> {
        JsonDiffLineWriter {
            lines: &mut self.lines,
            indent: 0,
        }
    }
}

impl std::ops::Deref for JsonDiffLines {
    type Target = [JsonDiffLine];

    fn deref(&self) -> &Self::Target {
        &self.lines
    }
}

impl IntoIterator for JsonDiffLines {
    type Item = JsonDiffLine;
    type IntoIter = std::vec::IntoIter<JsonDiffLine>;

    fn into_iter(self) -> Self::IntoIter {
        self.lines.into_iter()
    }
}

struct JsonDiffLineWriter<'a> {
    lines: &'a mut Vec<JsonDiffLine>,
    indent: usize,
}

#[derive(Clone, Copy)]
struct RenderedJson<'a> {
    body: &'a str,
    prefix: Option<&'a str>,
    trailing_comma: bool,
}

impl<'a> RenderedJson<'a> {
    fn lines(self) -> RenderedJsonLines<'a> {
        RenderedJsonLines {
            lines: self.body.lines().peekable(),
            prefix: self.prefix,
            trailing_comma: self.trailing_comma,
        }
    }
}

struct RenderedJsonLines<'a> {
    lines: std::iter::Peekable<std::str::Lines<'a>>,
    prefix: Option<&'a str>,
    trailing_comma: bool,
}

impl<'a> Iterator for RenderedJsonLines<'a> {
    type Item = RenderedJsonLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.lines.next()?;
        Some(RenderedJsonLine {
            prefix: self.prefix.take(),
            line,
            trailing_comma: self.trailing_comma && self.lines.peek().is_none(),
        })
    }
}

struct RenderedJsonLine<'a> {
    prefix: Option<&'a str>,
    line: &'a str,
    trailing_comma: bool,
}

impl Display for RenderedJsonLine<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(prefix) = self.prefix {
            f.write_str(prefix)?;
        }
        f.write_str(self.line)?;
        if self.trailing_comma {
            f.write_str(",")?;
        }
        Ok(())
    }
}

struct ClosingLine {
    delimiter: char,
    trailing_comma: bool,
}

impl Display for ClosingLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.delimiter)?;
        if self.trailing_comma {
            f.write_str(",")?;
        }
        Ok(())
    }
}

struct MemberContainerStart<'a> {
    quoted_key: &'a str,
    delimiter: char,
}

impl Display for MemberContainerStart<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.quoted_key)?;
        f.write_str(": ")?;
        write!(f, "{}", self.delimiter)?;
        Ok(())
    }
}

impl JsonDiffLineWriter<'_> {
    fn indent(&mut self) -> JsonDiffLineWriter<'_> {
        JsonDiffLineWriter {
            lines: &mut *self.lines,
            indent: self.indent + 1,
        }
    }

    fn unchanged_display(&mut self, expected: impl Display, actual: impl Display) {
        self.lines.push(JsonDiffLine {
            indent: self.indent,
            state: JsonDiffLineState::Unchanged {
                expected: expected.to_string(),
                actual: actual.to_string(),
            },
        });
    }

    fn unchanged_same(&mut self, line: &'static str) {
        self.lines.push(JsonDiffLine {
            indent: self.indent,
            state: JsonDiffLineState::Unchanged {
                expected: line.to_owned(),
                actual: line.to_owned(),
            },
        });
    }

    fn unchanged_same_display(&mut self, line: impl Display) {
        let line = line.to_string();
        self.lines.push(JsonDiffLine {
            indent: self.indent,
            state: JsonDiffLineState::Unchanged {
                expected: line.clone(),
                actual: line,
            },
        });
    }

    fn unchanged_value(&mut self, value: &Value, expected_trailing_comma: bool, actual_trailing_comma: bool) {
        let body = serde_json::to_string_pretty(value).unwrap();
        self.unchanged_rendered_pair(
            RenderedJson {
                body: &body,
                prefix: None,
                trailing_comma: expected_trailing_comma,
            },
            RenderedJson {
                body: &body,
                prefix: None,
                trailing_comma: actual_trailing_comma,
            },
        );
    }

    fn unchanged_member(
        &mut self,
        key: &str,
        value: &Value,
        expected_trailing_comma: bool,
        actual_trailing_comma: bool,
    ) {
        let body = serde_json::to_string_pretty(value).unwrap();
        let prefix = Self::member_prefix(key);
        self.unchanged_rendered_pair(
            RenderedJson {
                body: &body,
                prefix: Some(&prefix),
                trailing_comma: expected_trailing_comma,
            },
            RenderedJson {
                body: &body,
                prefix: Some(&prefix),
                trailing_comma: actual_trailing_comma,
            },
        );
    }

    fn added_value(&mut self, value: &Value, trailing_comma: bool) {
        let body = serde_json::to_string_pretty(value).unwrap();
        self.added_rendered(RenderedJson {
            body: &body,
            prefix: None,
            trailing_comma,
        });
    }

    fn added_member(&mut self, key: &str, value: &Value, trailing_comma: bool) {
        let body = serde_json::to_string_pretty(value).unwrap();
        let prefix = Self::member_prefix(key);
        self.added_rendered(RenderedJson {
            body: &body,
            prefix: Some(&prefix),
            trailing_comma,
        });
    }

    fn deleted_value(&mut self, value: &Value, trailing_comma: bool) {
        let body = serde_json::to_string_pretty(value).unwrap();
        self.deleted_rendered(RenderedJson {
            body: &body,
            prefix: None,
            trailing_comma,
        });
    }

    fn deleted_member(&mut self, key: &str, value: &Value, trailing_comma: bool) {
        let body = serde_json::to_string_pretty(value).unwrap();
        let prefix = Self::member_prefix(key);
        self.deleted_rendered(RenderedJson {
            body: &body,
            prefix: Some(&prefix),
            trailing_comma,
        });
    }

    fn ignored_value(&mut self, expected: Option<(&Value, bool)>, actual: Option<(&Value, bool)>) {
        let expected =
            expected.map(|(value, trailing_comma)| (serde_json::to_string_pretty(value).unwrap(), trailing_comma));
        let actual =
            actual.map(|(value, trailing_comma)| (serde_json::to_string_pretty(value).unwrap(), trailing_comma));
        self.ignored_rendered_pair(
            expected.as_ref().map(|(body, trailing_comma)| RenderedJson {
                body,
                prefix: None,
                trailing_comma: *trailing_comma,
            }),
            actual.as_ref().map(|(body, trailing_comma)| RenderedJson {
                body,
                prefix: None,
                trailing_comma: *trailing_comma,
            }),
        );
    }

    fn ignored_member(&mut self, expected: Option<(&str, &Value, bool)>, actual: Option<(&str, &Value, bool)>) {
        let expected = expected.map(|(key, value, trailing_comma)| {
            (
                serde_json::to_string_pretty(value).unwrap(),
                Self::member_prefix(key),
                trailing_comma,
            )
        });
        let actual = actual.map(|(key, value, trailing_comma)| {
            (
                serde_json::to_string_pretty(value).unwrap(),
                Self::member_prefix(key),
                trailing_comma,
            )
        });
        self.ignored_rendered_pair(
            expected.as_ref().map(|(body, prefix, trailing_comma)| RenderedJson {
                body,
                prefix: Some(prefix),
                trailing_comma: *trailing_comma,
            }),
            actual.as_ref().map(|(body, prefix, trailing_comma)| RenderedJson {
                body,
                prefix: Some(prefix),
                trailing_comma: *trailing_comma,
            }),
        );
    }

    fn unchanged_rendered_pair(&mut self, expected: RenderedJson<'_>, actual: RenderedJson<'_>) {
        for (expected, actual) in expected.lines().zip(actual.lines()) {
            self.unchanged_display(expected, actual);
        }
    }

    fn added_rendered(&mut self, rendered: RenderedJson<'_>) {
        for line in rendered.lines() {
            self.lines.push(JsonDiffLine {
                indent: self.indent,
                state: JsonDiffLineState::Added(line.to_string()),
            });
        }
    }

    fn deleted_rendered(&mut self, rendered: RenderedJson<'_>) {
        for line in rendered.lines() {
            self.lines.push(JsonDiffLine {
                indent: self.indent,
                state: JsonDiffLineState::Deleted(line.to_string()),
            });
        }
    }

    fn ignored_rendered_pair(&mut self, expected: Option<RenderedJson<'_>>, actual: Option<RenderedJson<'_>>) {
        let mut expected_lines = expected.map(RenderedJson::lines);
        let mut actual_lines = actual.map(RenderedJson::lines);
        loop {
            let expected_line = expected_lines.as_mut().and_then(Iterator::next);
            let actual_line = actual_lines.as_mut().and_then(Iterator::next);
            if expected_line.is_none() && actual_line.is_none() {
                break;
            }
            self.lines.push(JsonDiffLine {
                indent: self.indent,
                state: JsonDiffLineState::Ignored {
                    expected: expected_line.map(|line| line.to_string()),
                    actual: actual_line.map(|line| line.to_string()),
                },
            });
        }
    }

    fn member_prefix(key: &str) -> String {
        let mut key = serde_json::to_string(key).unwrap();
        key.push_str(": ");
        key
    }
}

fn json_diff(expected: &Value, actual: &Value, ignore_paths: &[JsonPath]) -> JsonDiffLines {
    fn json_array_diff<'stack, 'path, 'value>(
        expected: &'value [Value],
        actual: &'value [Value],
        expected_state: &mut JsonPathMatchState<'stack, 'path, &'value Value>,
        actual_state: &mut JsonPathMatchState<'stack, 'path, &'value Value>,
        writer: &mut JsonDiffLineWriter<'_>,
    ) {
        let mut hook = ArrayDiffHook {
            expected,
            actual,
            expected_state,
            actual_state,
            writer,
        };
        similar::algorithms::patience::diff(
            &mut similar::algorithms::Replace::new(&mut hook),
            expected,
            0..expected.len(),
            actual,
            0..actual.len(),
        )
        .unwrap();

        struct ArrayDiffHook<'hook, 'value, 'stack, 'path, 'lines> {
            expected: &'value [Value],
            actual: &'value [Value],
            expected_state: &'hook mut JsonPathMatchState<'stack, 'path, &'value Value>,
            actual_state: &'hook mut JsonPathMatchState<'stack, 'path, &'value Value>,
            writer: &'hook mut JsonDiffLineWriter<'lines>,
        }

        impl DiffHook for ArrayDiffHook<'_, '_, '_, '_, '_> {
            type Error = convert::Infallible;

            fn equal(&mut self, old_index: usize, new_index: usize, len: usize) -> Result<(), Self::Error> {
                for (expected_index, actual_index) in (old_index..).zip(new_index..).take(len) {
                    let need_extra_comma_expected = expected_index < self.expected.len() - 1;
                    let need_extra_comma_actual = actual_index < self.actual.len() - 1;
                    let v = &self.expected[expected_index];
                    self.writer
                        .unchanged_value(v, need_extra_comma_expected, need_extra_comma_actual);
                }
                Ok(())
            }

            fn delete(&mut self, old_index: usize, old_len: usize, _new_index: usize) -> Result<(), Self::Error> {
                for i in (old_index..).take(old_len) {
                    let need_extra_comma = i < self.expected.len() - 1;
                    let v = &self.expected[i];
                    let expected_state = self.expected_state.advance_index(i).unwrap();
                    if expected_state.is_match() {
                        self.writer.ignored_value(Some((v, need_extra_comma)), None);
                        continue;
                    }
                    self.writer.deleted_value(v, need_extra_comma);
                }
                Ok(())
            }

            fn insert(&mut self, _old_index: usize, new_index: usize, new_len: usize) -> Result<(), Self::Error> {
                for i in (new_index..).take(new_len) {
                    let need_extra_comma = i < self.actual.len() - 1;
                    let v = &self.actual[i];
                    let actual_state = self.actual_state.advance_index(i).unwrap();
                    if actual_state.is_match() {
                        self.writer.ignored_value(None, Some((v, need_extra_comma)));
                        continue;
                    }
                    self.writer.added_value(v, need_extra_comma);
                }
                Ok(())
            }

            fn replace(
                &mut self,
                old_index: usize,
                old_len: usize,
                new_index: usize,
                new_len: usize,
            ) -> Result<(), Self::Error> {
                fn value_match_score(expected: &Value, actual: &Value) -> usize {
                    if expected == actual {
                        0
                    } else {
                        match (expected, actual) {
                            (Value::Array(expected), Value::Array(actual)) => {
                                10 + expected.len().abs_diff(actual.len())
                            }
                            (Value::Object(expected), Value::Object(actual)) => {
                                10 + expected.keys().filter(|k| !actual.contains_key(k.as_str())).count()
                                    + actual.keys().filter(|k| !expected.contains_key(k.as_str())).count()
                            }
                            _ if mem::discriminant::<Value>(expected) == mem::discriminant::<Value>(actual) => 100,
                            _ => 1000,
                        }
                    }
                }

                let mut expected_to_actual = vec![None::<usize>; old_len];
                let mut actual_to_expected = vec![None::<usize>; new_len];

                let mut expected_ignored = Vec::with_capacity(old_len);
                for expected_index in 0..old_len {
                    let expected_state = self.expected_state.advance_index(old_index + expected_index).unwrap();
                    expected_ignored.push(expected_state.is_match());
                }
                let mut actual_ignored = Vec::with_capacity(new_len);
                for actual_index in 0..new_len {
                    let actual_state = self.actual_state.advance_index(new_index + actual_index).unwrap();
                    actual_ignored.push(actual_state.is_match());
                }

                let mut q = BinaryHeap::new();
                for (expected_index, expected) in self.expected[old_index..][..old_len].iter().enumerate() {
                    for (actual_index, actual) in self.actual[new_index..][..new_len].iter().enumerate() {
                        q.push((
                            Reverse(value_match_score(expected, actual)),
                            expected_ignored[expected_index] == actual_ignored[actual_index],
                            Reverse(expected_index.abs_diff(actual_index)),
                            expected_index,
                            actual_index,
                        ));
                    }
                }
                while let Some((_, _, _, expected_index, actual_index)) = q.pop() {
                    if expected_to_actual[expected_index].is_some() || actual_to_expected[actual_index].is_some() {
                        continue;
                    }
                    let requirements = [
                        expected_to_actual[..expected_index]
                            .iter()
                            .rev()
                            .find_map(Option::as_ref)
                            .is_none_or(|&left| left < actual_index),
                        expected_to_actual[expected_index..]
                            .iter()
                            .find_map(Option::as_ref)
                            .is_none_or(|&right| actual_index < right),
                        actual_to_expected[..actual_index]
                            .iter()
                            .rev()
                            .find_map(Option::as_ref)
                            .is_none_or(|&left| left < expected_index),
                        actual_to_expected[actual_index..]
                            .iter()
                            .find_map(Option::as_ref)
                            .is_none_or(|&right| expected_index < right),
                    ];
                    if requirements.into_iter().all(convert::identity) {
                        expected_to_actual[expected_index] = Some(actual_index);
                        actual_to_expected[actual_index] = Some(expected_index);
                    }
                }
                let mut expected_to_actual = expected_to_actual.into_iter().enumerate().peekable();
                let mut actual_to_expected = actual_to_expected.into_iter().enumerate().peekable();
                let expected_index_base = old_index;
                let actual_index_base = new_index;
                loop {
                    while let Some((expected_index, _)) =
                        expected_to_actual.next_if(|(_, actual_index)| actual_index.is_none())
                    {
                        self.delete(expected_index_base + expected_index, 1, 0)?;
                    }
                    while let Some((actual_index, _)) =
                        actual_to_expected.next_if(|(_, expected_index)| expected_index.is_none())
                    {
                        self.insert(0, actual_index_base + actual_index, 1)?;
                    }
                    match (expected_to_actual.next(), actual_to_expected.next()) {
                        (None, None) => break,
                        (Some((expected_index, Some(actual_index_))), Some((actual_index, Some(expected_index_)))) => {
                            assert_eq!(expected_index, expected_index_);
                            assert_eq!(actual_index, actual_index_);
                            let expected_index = expected_index_base + expected_index;
                            let actual_index = actual_index_base + actual_index;
                            let need_extra_comma_expected = expected_index < self.expected.len() - 1;
                            let need_extra_comma_actual = actual_index < self.actual.len() - 1;
                            let expected_value = &self.expected[expected_index];
                            let actual_value = &self.actual[actual_index];
                            let mut expected_state = self.expected_state.advance_index(expected_index).unwrap();
                            let mut actual_state = self.actual_state.advance_index(actual_index).unwrap();
                            if expected_state.is_match() || actual_state.is_match() {
                                self.writer.ignored_value(
                                    Some((expected_value, need_extra_comma_expected)),
                                    Some((actual_value, need_extra_comma_actual)),
                                );
                                continue;
                            }
                            if expected_value == actual_value {
                                self.writer.unchanged_value(
                                    expected_value,
                                    need_extra_comma_expected,
                                    need_extra_comma_actual,
                                );
                                continue;
                            }
                            match (expected_value, actual_value) {
                                (Value::Array(expected), Value::Array(actual)) => {
                                    self.writer.unchanged_same("[");
                                    let mut result = self.writer.indent();
                                    json_array_diff(
                                        expected,
                                        actual,
                                        &mut expected_state,
                                        &mut actual_state,
                                        &mut result,
                                    );
                                    self.writer.unchanged_display(
                                        ClosingLine {
                                            delimiter: ']',
                                            trailing_comma: need_extra_comma_expected,
                                        },
                                        ClosingLine {
                                            delimiter: ']',
                                            trailing_comma: need_extra_comma_actual,
                                        },
                                    );
                                }
                                (Value::Object(expected), Value::Object(actual)) => {
                                    self.writer.unchanged_same("{");
                                    let mut result = self.writer.indent();
                                    json_object_diff(
                                        expected,
                                        actual,
                                        &mut expected_state,
                                        &mut actual_state,
                                        &mut result,
                                    );
                                    self.writer.unchanged_display(
                                        ClosingLine {
                                            delimiter: '}',
                                            trailing_comma: need_extra_comma_expected,
                                        },
                                        ClosingLine {
                                            delimiter: '}',
                                            trailing_comma: need_extra_comma_actual,
                                        },
                                    );
                                }
                                _ => {
                                    drop(expected_state);
                                    drop(actual_state);
                                    self.delete(expected_index, 1, 0)?;
                                    self.insert(0, actual_index, 1)?;
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                Ok(())
            }
        }
    }
    fn json_object_diff<'stack, 'path, 'value>(
        expected: &'value serde_json::Map<String, Value>,
        actual: &'value serde_json::Map<String, Value>,
        expected_state: &mut JsonPathMatchState<'stack, 'path, &'value Value>,
        actual_state: &mut JsonPathMatchState<'stack, 'path, &'value Value>,
        writer: &mut JsonDiffLineWriter<'_>,
    ) {
        let expected_keys = expected.keys().collect::<Vec<_>>();
        let actual_keys = actual.keys().collect::<Vec<_>>();
        let mut hook = ObjectDiffHook {
            expected,
            actual,
            expected_keys: &expected_keys,
            actual_keys: &actual_keys,
            expected_state,
            actual_state,
            writer,
        };
        similar::algorithms::patience::diff(
            &mut hook,
            &expected_keys,
            0..expected_keys.len(),
            &actual_keys,
            0..actual_keys.len(),
        )
        .unwrap();

        struct ObjectDiffHook<'hook, 'value, 'stack, 'path, 'lines> {
            expected: &'value serde_json::Map<String, Value>,
            actual: &'value serde_json::Map<String, Value>,
            expected_keys: &'hook [&'value String],
            actual_keys: &'hook [&'value String],
            expected_state: &'hook mut JsonPathMatchState<'stack, 'path, &'value Value>,
            actual_state: &'hook mut JsonPathMatchState<'stack, 'path, &'value Value>,
            writer: &'hook mut JsonDiffLineWriter<'lines>,
        }

        impl DiffHook for ObjectDiffHook<'_, '_, '_, '_, '_> {
            type Error = convert::Infallible;

            fn equal(&mut self, old_index: usize, new_index: usize, len: usize) -> Result<(), Self::Error> {
                assert_eq!(len, 1);
                let need_extra_comma_expected = old_index < self.expected_keys.len() - 1;
                let need_extra_comma_actual = new_index < self.actual_keys.len() - 1;
                let k = self.expected_keys[old_index];
                let expected_v = self.expected.get(k).unwrap();
                let actual_v = self.actual.get(k).unwrap();
                let mut expected_state = self.expected_state.advance_name(k).unwrap();
                let mut actual_state = self.actual_state.advance_name(k).unwrap();
                if (expected_state.is_match() || actual_state.is_match()) && expected_v != actual_v {
                    self.writer.ignored_member(
                        Some((k, expected_v, need_extra_comma_expected)),
                        Some((k, actual_v, need_extra_comma_actual)),
                    );
                    return Ok(());
                }
                match (expected_v, actual_v) {
                    (expected @ Value::Null, actual @ Value::Null)
                    | (expected @ Value::Bool(_), actual @ Value::Bool(_))
                    | (expected @ Value::Number(_), actual @ Value::Number(_))
                    | (expected @ Value::String(_), actual @ Value::String(_))
                        if expected == actual =>
                    {
                        self.writer
                            .unchanged_member(k, expected, need_extra_comma_expected, need_extra_comma_actual);
                    }
                    (Value::Array(expected), Value::Array(actual)) => {
                        let quoted_key = serde_json::to_string(k).unwrap();
                        self.writer.unchanged_same_display(MemberContainerStart {
                            quoted_key: &quoted_key,
                            delimiter: '[',
                        });
                        let mut result = self.writer.indent();
                        json_array_diff(expected, actual, &mut expected_state, &mut actual_state, &mut result);
                        self.writer.unchanged_display(
                            ClosingLine {
                                delimiter: ']',
                                trailing_comma: need_extra_comma_expected,
                            },
                            ClosingLine {
                                delimiter: ']',
                                trailing_comma: need_extra_comma_actual,
                            },
                        );
                    }
                    (Value::Object(expected), Value::Object(actual)) => {
                        let quoted_key = serde_json::to_string(k).unwrap();
                        self.writer.unchanged_same_display(MemberContainerStart {
                            quoted_key: &quoted_key,
                            delimiter: '{',
                        });
                        let mut result = self.writer.indent();
                        json_object_diff(expected, actual, &mut expected_state, &mut actual_state, &mut result);
                        self.writer.unchanged_display(
                            ClosingLine {
                                delimiter: '}',
                                trailing_comma: need_extra_comma_expected,
                            },
                            ClosingLine {
                                delimiter: '}',
                                trailing_comma: need_extra_comma_actual,
                            },
                        );
                    }
                    _ => {
                        drop(expected_state);
                        drop(actual_state);
                        self.delete(old_index, 1, 0)?;
                        self.insert(0, new_index, 1)?;
                    }
                }
                Ok(())
            }

            fn delete(&mut self, old_index: usize, old_len: usize, _new_index: usize) -> Result<(), Self::Error> {
                assert_eq!(old_len, 1);
                let need_extra_comma = old_index < self.expected.len() - 1;
                let k = self.expected_keys[old_index];
                let v = self.expected.get(k).unwrap();
                let expected_state = self.expected_state.advance_name(k).unwrap();
                if expected_state.is_match() {
                    self.writer.ignored_member(Some((k, v, need_extra_comma)), None);
                    return Ok(());
                }
                self.writer.deleted_member(k, v, need_extra_comma);
                Ok(())
            }

            fn insert(&mut self, _old_index: usize, new_index: usize, new_len: usize) -> Result<(), Self::Error> {
                assert_eq!(new_len, 1);
                let need_extra_comma = new_index < self.actual.len() - 1;
                let k = self.actual_keys[new_index];
                let v = self.actual.get(k).unwrap();
                let actual_state = self.actual_state.advance_name(k).unwrap();
                if actual_state.is_match() {
                    self.writer.ignored_member(None, Some((k, v, need_extra_comma)));
                    return Ok(());
                }
                self.writer.added_member(k, v, need_extra_comma);
                Ok(())
            }

            fn replace(
                &mut self,
                _old_index: usize,
                _old_len: usize,
                _new_index: usize,
                _new_len: usize,
            ) -> Result<(), Self::Error> {
                unreachable!()
            }
        }
    }
    let mut expected_matcher = JsonPathMatcher::new(ignore_paths);
    let mut actual_matcher = JsonPathMatcher::new(ignore_paths);
    let mut expected_state = expected_matcher.root_state(expected);
    let mut actual_state = actual_matcher.root_state(actual);
    let mut result = JsonDiffLines::default();
    let mut writer = result.writer();
    if (expected_state.is_match() || actual_state.is_match()) && expected != actual {
        writer.ignored_value(Some((expected, false)), Some((actual, false)));
    } else {
        match (expected, actual) {
            (expected @ Value::Null, actual @ Value::Null)
            | (expected @ Value::Bool(_), actual @ Value::Bool(_))
            | (expected @ Value::Number(_), actual @ Value::Number(_))
            | (expected @ Value::String(_), actual @ Value::String(_)) => {
                if expected == actual {
                    writer.unchanged_value(expected, false, false);
                } else {
                    writer.deleted_value(expected, false);
                    writer.added_value(actual, false);
                }
            }
            (Value::Array(expected), Value::Array(actual)) => {
                writer.unchanged_same("[");
                let mut child = writer.indent();
                json_array_diff(expected, actual, &mut expected_state, &mut actual_state, &mut child);
                writer.unchanged_same("]");
            }
            (Value::Object(expected), Value::Object(actual)) => {
                writer.unchanged_same("{");
                let mut child = writer.indent();
                json_object_diff(expected, actual, &mut expected_state, &mut actual_state, &mut child);
                writer.unchanged_same("}");
            }
            (expected, actual) => {
                writer.deleted_value(expected, false);
                writer.added_value(actual, false);
            }
        }
    }
    result
}
