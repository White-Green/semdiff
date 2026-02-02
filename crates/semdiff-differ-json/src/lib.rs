use mime::Mime;
use semdiff_core::{Diff, DiffCalculator, MayUnsupported};
use semdiff_tree_fs::FileLeaf;
use serde_json::Value;
use similar::algorithms::DiffHook;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt::Display;
use std::{convert, fmt};

pub mod report_html;
pub mod report_json;
pub mod report_summary;

#[derive(Debug, Clone, Copy, Default)]
pub struct JsonDiffReporter;

#[derive(Debug)]
enum JsonDiffBody {
    Equal(String),
    Modified(Vec<JsonDiffLine>),
}

#[derive(Debug)]
pub struct JsonDiff {
    body: JsonDiffBody,
}

impl Diff for JsonDiff {
    fn equal(&self) -> bool {
        matches!(self.body, JsonDiffBody::Equal(_))
    }
}

impl JsonDiff {
    fn body(&self) -> &JsonDiffBody {
        &self.body
    }
}

#[derive(Debug, Clone, Copy)]
pub struct JsonDiffCalculator {
    ignore_object_key_order: bool,
}

impl Default for JsonDiffCalculator {
    fn default() -> Self {
        Self::new(false)
    }
}

impl JsonDiffCalculator {
    pub fn new(ignore_object_key_order: bool) -> Self {
        Self {
            ignore_object_key_order,
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
        let diff = json_diff(&expected, &actual);
        let body = if diff
            .iter()
            .all(|d| matches!(d.state, JsonDiffLineState::Unchanged { .. }))
        {
            JsonDiffBody::Equal(serde_json::to_string_pretty(&expected).unwrap())
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
            JsonDiffLineState::Added(_) => ChangeTag::Added,
            JsonDiffLineState::Deleted(_) => ChangeTag::Deleted,
        }
    }

    fn display_expected(&self) -> impl Display {
        fmt::from_fn(|f| {
            let expected = match &self.state {
                JsonDiffLineState::Unchanged { expected, .. } => expected,
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

    fn unchanged(indent: usize, expected: String, actual: String) -> JsonDiffLine {
        JsonDiffLine {
            indent,
            state: JsonDiffLineState::Unchanged { expected, actual },
        }
    }

    fn added(indent: usize, actual: String) -> JsonDiffLine {
        JsonDiffLine {
            indent,
            state: JsonDiffLineState::Added(actual),
        }
    }

    fn deleted(indent: usize, expected: String) -> JsonDiffLine {
        JsonDiffLine {
            indent,
            state: JsonDiffLineState::Deleted(expected),
        }
    }
}

#[derive(Debug)]
enum JsonDiffLineState {
    Unchanged { expected: String, actual: String },
    Added(String),
    Deleted(String),
}

fn json_diff(expected: &Value, actual: &Value) -> Vec<JsonDiffLine> {
    fn json_array_diff(expected: &[Value], actual: &[Value], indent: usize, result: &mut Vec<JsonDiffLine>) {
        let mut hook = ArrayDiffHook {
            indent,
            expected,
            actual,
            result,
        };
        similar::algorithms::patience::diff(
            &mut similar::algorithms::Replace::new(&mut hook),
            expected,
            0..expected.len(),
            actual,
            0..actual.len(),
        )
        .unwrap();

        struct ArrayDiffHook<'a> {
            indent: usize,
            expected: &'a [Value],
            actual: &'a [Value],
            result: &'a mut Vec<JsonDiffLine>,
        }

        impl DiffHook for ArrayDiffHook<'_> {
            type Error = convert::Infallible;

            fn equal(&mut self, old_index: usize, new_index: usize, len: usize) -> Result<(), Self::Error> {
                for (expected_index, actual_index) in (old_index..).zip(new_index..).take(len) {
                    let need_extra_comma_expected = expected_index < self.expected.len() - 1;
                    let need_extra_comma_actual = actual_index < self.actual.len() - 1;
                    let v = &self.expected[expected_index];
                    let v = serde_json::to_string_pretty(v).unwrap();
                    let mut lines = v.lines();
                    let last_line = lines.next_back().unwrap();
                    for line in lines {
                        self.result
                            .push(JsonDiffLine::unchanged(self.indent, line.to_owned(), line.to_owned()));
                    }
                    self.result.push(JsonDiffLine::unchanged(
                        self.indent,
                        format!("{}{}", last_line, if need_extra_comma_expected { "," } else { "" }),
                        format!("{}{}", last_line, if need_extra_comma_actual { "," } else { "" }),
                    ));
                }
                Ok(())
            }

            fn delete(&mut self, old_index: usize, old_len: usize, _new_index: usize) -> Result<(), Self::Error> {
                for i in (old_index..).take(old_len) {
                    let need_extra_comma = i < self.expected.len() - 1;
                    let v = &self.expected[i];
                    let v = serde_json::to_string_pretty(v).unwrap();
                    let mut lines = v.lines();
                    let last_line = lines.next_back().unwrap();
                    for line in lines {
                        self.result.push(JsonDiffLine::deleted(self.indent, line.to_owned()));
                    }
                    self.result.push(JsonDiffLine::deleted(
                        self.indent,
                        format!("{}{}", last_line, if need_extra_comma { "," } else { "" }),
                    ));
                }
                Ok(())
            }

            fn insert(&mut self, _old_index: usize, new_index: usize, new_len: usize) -> Result<(), Self::Error> {
                for i in (new_index..).take(new_len) {
                    let need_extra_comma = i < self.actual.len() - 1;
                    let v = &self.actual[i];
                    let v = serde_json::to_string_pretty(v).unwrap();
                    let mut lines = v.lines();
                    let last_line = lines.next_back().unwrap();
                    for line in lines {
                        self.result.push(JsonDiffLine::added(self.indent, line.to_owned()));
                    }
                    self.result.push(JsonDiffLine::added(
                        self.indent,
                        format!("{}{}", last_line, if need_extra_comma { "," } else { "" }),
                    ));
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
                let mut q = BinaryHeap::new();
                for (expected_index, expected) in self.expected[old_index..][..old_len].iter().enumerate() {
                    for (actual_index, actual) in self.actual[new_index..][..new_len].iter().enumerate() {
                        let index_diff = Reverse(expected_index.abs_diff(actual_index));
                        match (expected, actual) {
                            (Value::Array(expected), Value::Array(actual)) => q.push((
                                Reverse(expected.len().abs_diff(actual.len())),
                                index_diff,
                                expected_index,
                                actual_index,
                            )),
                            (Value::Object(expected), Value::Object(actual)) => q.push((
                                Reverse(
                                    expected.keys().filter(|k| !actual.contains_key(k.as_str())).count()
                                        + actual.keys().filter(|k| !expected.contains_key(k.as_str())).count(),
                                ),
                                index_diff,
                                expected_index,
                                actual_index,
                            )),
                            _ => {}
                        }
                    }
                }
                let mut expected_to_actual = vec![None::<usize>; old_len];
                let mut actual_to_expected = vec![None::<usize>; new_len];
                while let Some((_, _, expected_index, actual_index)) = q.pop() {
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
                            .is_none_or(|&right| right < actual_index),
                        actual_to_expected[..actual_index]
                            .iter()
                            .rev()
                            .find_map(Option::as_ref)
                            .is_none_or(|&left| left < expected_index),
                        actual_to_expected[actual_index..]
                            .iter()
                            .find_map(Option::as_ref)
                            .is_none_or(|&right| right < expected_index),
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
                            match (&self.expected[expected_index], &self.actual[actual_index]) {
                                (Value::Array(expected), Value::Array(actual)) => {
                                    self.result.push(JsonDiffLine::unchanged(
                                        self.indent,
                                        "[".to_owned(),
                                        "[".to_owned(),
                                    ));
                                    json_array_diff(expected, actual, self.indent + 1, self.result);
                                    self.result.push(JsonDiffLine::unchanged(
                                        self.indent,
                                        format!("]{}", if need_extra_comma_expected { "," } else { "" }),
                                        format!("]{}", if need_extra_comma_actual { "," } else { "" }),
                                    ));
                                }
                                (Value::Object(expected), Value::Object(actual)) => {
                                    self.result.push(JsonDiffLine::unchanged(
                                        self.indent,
                                        "{".to_owned(),
                                        "{".to_owned(),
                                    ));
                                    json_object_diff(expected, actual, self.indent + 1, self.result);
                                    self.result.push(JsonDiffLine::unchanged(
                                        self.indent,
                                        format!("}}{}", if need_extra_comma_expected { "," } else { "" }),
                                        format!("}}{}", if need_extra_comma_actual { "," } else { "" }),
                                    ));
                                }
                                _ => unreachable!(),
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                Ok(())
            }
        }
    }
    fn json_object_diff(
        expected: &serde_json::Map<String, Value>,
        actual: &serde_json::Map<String, Value>,
        indent: usize,
        result: &mut Vec<JsonDiffLine>,
    ) {
        let expected_keys = expected.keys().collect::<Vec<_>>();
        let actual_keys = actual.keys().collect::<Vec<_>>();
        let mut hook = ObjectDiffHook {
            indent,
            expected,
            actual,
            expected_keys: &expected_keys,
            actual_keys: &actual_keys,
            result,
        };
        similar::algorithms::patience::diff(
            &mut hook,
            &expected_keys,
            0..expected_keys.len(),
            &actual_keys,
            0..actual_keys.len(),
        )
        .unwrap();

        struct ObjectDiffHook<'a> {
            indent: usize,
            expected: &'a serde_json::Map<String, Value>,
            actual: &'a serde_json::Map<String, Value>,
            expected_keys: &'a [&'a String],
            actual_keys: &'a [&'a String],
            result: &'a mut Vec<JsonDiffLine>,
        }

        impl DiffHook for ObjectDiffHook<'_> {
            type Error = convert::Infallible;

            fn equal(&mut self, old_index: usize, new_index: usize, len: usize) -> Result<(), Self::Error> {
                assert_eq!(len, 1);
                let need_extra_comma_expected = old_index < self.expected_keys.len() - 1;
                let need_extra_comma_actual = new_index < self.actual_keys.len() - 1;
                let k = self.expected_keys[old_index];
                let expected_v = self.expected.get(k).unwrap();
                let actual_v = self.actual.get(k).unwrap();
                match dbg!(expected_v, actual_v) {
                    (expected @ Value::Null, actual @ Value::Null)
                    | (expected @ Value::Bool(_), actual @ Value::Bool(_))
                    | (expected @ Value::Number(_), actual @ Value::Number(_))
                    | (expected @ Value::String(_), actual @ Value::String(_))
                        if expected == actual =>
                    {
                        let k = serde_json::to_string(k).unwrap();
                        let v = serde_json::to_string(expected).unwrap();
                        self.result.push(JsonDiffLine::unchanged(
                            self.indent,
                            format!("{k}: {v}{}", if need_extra_comma_expected { "," } else { "" }),
                            format!("{k}: {v}{}", if need_extra_comma_actual { "," } else { "" }),
                        ));
                    }
                    (Value::Array(expected), Value::Array(actual)) => {
                        let k = serde_json::to_string(k).unwrap();
                        self.result.push(JsonDiffLine::unchanged(
                            self.indent,
                            format!("{k}: ["),
                            format!("{k}: ["),
                        ));
                        json_array_diff(expected, actual, self.indent + 1, self.result);
                        self.result.push(JsonDiffLine::unchanged(
                            self.indent,
                            format!("]{}", if need_extra_comma_expected { "," } else { "" }),
                            format!("]{}", if need_extra_comma_actual { "," } else { "" }),
                        ));
                    }
                    (Value::Object(expected), Value::Object(actual)) => {
                        let k = serde_json::to_string(k).unwrap();
                        self.result.push(JsonDiffLine::unchanged(
                            self.indent,
                            format!("{k}: {{"),
                            format!("{k}: {{"),
                        ));
                        json_object_diff(expected, actual, self.indent + 1, self.result);
                        self.result.push(JsonDiffLine::unchanged(
                            self.indent,
                            format!("}}{}", if need_extra_comma_expected { "," } else { "" }),
                            format!("}}{}", if need_extra_comma_actual { "," } else { "" }),
                        ));
                    }
                    _ => {
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
                if let Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) = v {
                    self.result.push(JsonDiffLine::deleted(
                        self.indent,
                        format!(
                            "{}: {}{}",
                            serde_json::to_string(k).unwrap(),
                            serde_json::to_string(v).unwrap(),
                            if need_extra_comma { "," } else { "" }
                        ),
                    ));
                    return Ok(());
                }
                let v = serde_json::to_string_pretty(v).unwrap();
                let mut lines = v.lines().peekable();
                let first_line = lines.next().unwrap();
                self.result.push(JsonDiffLine::deleted(
                    self.indent,
                    format!("{}: {}", serde_json::to_string(k).unwrap(), first_line),
                ));
                while let Some(line) = lines.next() {
                    if lines.peek().is_none() && need_extra_comma {
                        self.result
                            .push(JsonDiffLine::deleted(self.indent, format!("{},", line)));
                    } else {
                        self.result.push(JsonDiffLine::deleted(self.indent, line.to_owned()));
                    }
                }
                Ok(())
            }

            fn insert(&mut self, _old_index: usize, new_index: usize, new_len: usize) -> Result<(), Self::Error> {
                assert_eq!(new_len, 1);
                let need_extra_comma = new_index < self.actual.len() - 1;
                let k = self.actual_keys[new_index];
                let v = self.actual.get(k).unwrap();
                if let Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) = v {
                    self.result.push(JsonDiffLine::added(
                        self.indent,
                        format!(
                            "{}: {}{}",
                            serde_json::to_string(k).unwrap(),
                            serde_json::to_string(v).unwrap(),
                            if need_extra_comma { "," } else { "" }
                        ),
                    ));
                    return Ok(());
                }
                let v = serde_json::to_string_pretty(v).unwrap();
                let mut lines = v.lines().peekable();
                let first_line = lines.next().unwrap();
                self.result.push(JsonDiffLine::added(
                    self.indent,
                    format!("{}: {}", serde_json::to_string(k).unwrap(), first_line),
                ));
                while let Some(line) = lines.next() {
                    if lines.peek().is_none() && need_extra_comma {
                        self.result.push(JsonDiffLine::added(self.indent, format!("{},", line)));
                    } else {
                        self.result.push(JsonDiffLine::added(self.indent, line.to_owned()));
                    }
                }
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
    let mut result = Vec::new();
    match (expected, actual) {
        (expected @ Value::Null, actual @ Value::Null)
        | (expected @ Value::Bool(_), actual @ Value::Bool(_))
        | (expected @ Value::Number(_), actual @ Value::Number(_))
        | (expected @ Value::String(_), actual @ Value::String(_)) => {
            if expected == actual {
                result.push(JsonDiffLine::unchanged(0, expected.to_string(), actual.to_string()));
            } else {
                result.push(JsonDiffLine::deleted(0, expected.to_string()));
                result.push(JsonDiffLine::added(0, actual.to_string()));
            }
        }
        (Value::Array(expected), Value::Array(actual)) => {
            result.push(JsonDiffLine::unchanged(0, "[".to_owned(), "[".to_owned()));
            json_array_diff(expected, actual, 1, &mut result);
            result.push(JsonDiffLine::unchanged(0, "]".to_owned(), "]".to_owned()));
        }
        (Value::Object(expected), Value::Object(actual)) => {
            result.push(JsonDiffLine::unchanged(0, "{".to_owned(), "{".to_owned()));
            json_object_diff(expected, actual, 1, &mut result);
            result.push(JsonDiffLine::unchanged(0, "}".to_owned(), "}".to_owned()));
        }
        (expected, actual) => {
            for line in serde_json::to_string_pretty(expected).unwrap().lines() {
                result.push(JsonDiffLine::deleted(0, line.to_owned()));
            }
            for line in serde_json::to_string_pretty(actual).unwrap().lines() {
                result.push(JsonDiffLine::added(0, line.to_owned()));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_diff() {
        let expected = json! {{
            "id": 1,
            "profile": {
                "first": "Taro",
                "last": "Yamada"
            },
            "scores": [
                10,
                20,
                30
            ]
        }};
        let actual = json! {{
            "profile": {
                "last": "Yamada",
                "first": "Taro"
            },
            "scores": [
                10,
                20,
                40
            ],
            "id": 1
        }};
        json_diff(&expected, &actual);
    }
}
