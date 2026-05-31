use crate::json_path::number_literal::Number;
use crate::json_path::parser::{
    Comparable, ComparisonOp, FunctionArgument, FunctionExpr, JsonPath, Literal, LogicalExpr, Query, QueryRoot,
    Segment, Selector, SingularQuery, SingularSegment, TestExpr,
};
use smallvec::SmallVec;
use std::cmp::Ordering;
use std::marker::PhantomData;

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

// is_emptyが欲しくなることは無さそう
#[allow(clippy::len_without_is_empty)]
pub trait JsonPathArray<'a> {
    type JsonPathValue: JsonPathValue<'a>;

    fn len(&self) -> usize;
    fn element(&self, index: usize) -> Option<Self::JsonPathValue>;
}

pub trait JsonPathObject<'a> {
    type JsonPathValue: JsonPathValue<'a>;
    type MemberIter: Iterator<Item = (&'a str, Self::JsonPathValue)>;

    fn len(&self) -> usize;
    fn member(&self, name: &str) -> Option<Self::JsonPathValue>;
    fn members(&self) -> Self::MemberIter;
}

#[derive(Debug, Clone, Copy)]
pub struct SerdeJsonPathArray<'a> {
    values: &'a [serde_json::Value],
}

#[derive(Debug, Clone, Copy)]
pub struct SerdeJsonPathObject<'a> {
    values: &'a serde_json::Map<String, serde_json::Value>,
}

pub struct SerdeJsonPathObjectMembers<'a> {
    values: serde_json::map::Iter<'a>,
}

impl<'a> Iterator for SerdeJsonPathObjectMembers<'a> {
    type Item = (&'a str, &'a serde_json::Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.values.next().map(|(name, value)| (name.as_str(), value))
    }
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
    type MemberIter = SerdeJsonPathObjectMembers<'a>;

    fn len(&self) -> usize {
        self.values.len()
    }

    fn member(&self, name: &str) -> Option<Self::JsonPathValue> {
        self.values.get(name)
    }

    fn members(&self) -> Self::MemberIter {
        SerdeJsonPathObjectMembers {
            values: self.values.iter(),
        }
    }
}

pub(crate) struct JsonPathMatcher<'a> {
    paths: &'a [JsonPath],
    descendant_active: Vec<ActiveSegments<'a>>,
}

pub(crate) struct JsonPathMatchState<'state, 'path, V> {
    root: V,
    current: V,
    descendant_active: &'state mut Vec<ActiveSegments<'path>>,
    descendant_start: usize,
    child_active: ActiveSegmentsVec<'path>,
    matched: bool,
}

type ActiveSegments<'path> = &'path [Segment];
type ActiveSegmentsVec<'path> = SmallVec<[ActiveSegments<'path>; 8]>;

impl<'state, 'path, 'value, V> JsonPathMatchState<'state, 'path, V>
where
    V: JsonPathValue<'value>,
{
    pub(crate) fn is_match(&self) -> bool {
        self.matched
    }

    pub(crate) fn advance_name<'child>(&'child mut self, name: &str) -> Option<JsonPathMatchState<'child, 'path, V>> {
        let child = self.current.as_object()?.member(name)?;
        Some(self.advance(JsonPathMatchStep::Name(name), child))
    }

    pub(crate) fn advance_index<'child>(
        &'child mut self,
        index: usize,
    ) -> Option<JsonPathMatchState<'child, 'path, V>> {
        let child = self.current.as_array()?.element(index)?;
        Some(self.advance(JsonPathMatchStep::Index(index), child))
    }

    fn advance<'child>(
        &'child mut self,
        step: JsonPathMatchStep<'_>,
        child: V,
    ) -> JsonPathMatchState<'child, 'path, V> {
        let root = self.root;
        let parent_current = self.current;
        let descendant_start = self.descendant_active.len();
        let mut child_active = ActiveSegmentsVec::new();
        let mut descendant_active = ActiveSegmentsVec::new();
        let mut matched = false;
        for &segments in &self.child_active {
            let Some(Segment::Child(selectors)) = segments.first() else {
                continue;
            };
            if selectors
                .iter()
                .any(|selector| Self::selector_matches(root, parent_current, selector, step, child))
            {
                activate_segments(&segments[1..], &mut child_active, &mut descendant_active, &mut matched);
            }
        }
        for &segments in &self.descendant_active[..descendant_start] {
            let Some(Segment::Descendant(selectors)) = segments.first() else {
                continue;
            };
            if selectors
                .iter()
                .any(|selector| Self::selector_matches(root, parent_current, selector, step, child))
            {
                activate_segments(&segments[1..], &mut child_active, &mut descendant_active, &mut matched);
            }
        }

        let shared_descendant_active = &mut *self.descendant_active;
        for segments in descendant_active {
            if !shared_descendant_active.contains(&segments) {
                shared_descendant_active.push(segments);
            }
        }
        JsonPathMatchState {
            root,
            current: child,
            descendant_active: shared_descendant_active,
            descendant_start,
            child_active,
            matched,
        }
    }

    fn selector_matches(root: V, parent: V, selector: &Selector, step: JsonPathMatchStep<'_>, child: V) -> bool {
        match (selector, step) {
            (Selector::Name(name), JsonPathMatchStep::Name(step_name)) => name == step_name,
            (Selector::Wildcard, JsonPathMatchStep::Name(_) | JsonPathMatchStep::Index(_)) => true,
            (Selector::Index(index), JsonPathMatchStep::Index(array_index)) => {
                Self::index_matches(parent, *index, array_index)
            }
            (Selector::Slice { start, end, step }, JsonPathMatchStep::Index(array_index)) => {
                Self::slice_matches(parent, *start, *end, *step, array_index)
            }
            (Selector::Filter(expr), JsonPathMatchStep::Name(_) | JsonPathMatchStep::Index(_)) => {
                FilterEvaluator::new(root, child).eval_logical(expr)
            }
            _ => false,
        }
    }

    fn index_matches(parent: V, index: i64, array_index: usize) -> bool {
        let Some(array) = parent.as_array() else {
            return false;
        };
        normalize_index(array.len(), index).is_some_and(|index| index == array_index)
    }

    fn slice_matches(parent: V, start: Option<i64>, end: Option<i64>, step: Option<i64>, array_index: usize) -> bool {
        let Some(array) = parent.as_array() else {
            return false;
        };
        slice_contains(array.len(), start, end, step, array_index)
    }
}

impl<'state, 'path, V> Drop for JsonPathMatchState<'state, 'path, V> {
    fn drop(&mut self) {
        self.descendant_active.truncate(self.descendant_start);
    }
}

#[derive(Debug, Clone, Copy)]
enum JsonPathMatchStep<'a> {
    Name(&'a str),
    Index(usize),
}

enum ComparableValue<V> {
    Nothing,
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Node(V),
}

enum FunctionResult<V> {
    Value(ComparableValue<V>),
    Logical(bool),
}

struct FilterEvaluator<'value, V>
where
    V: JsonPathValue<'value>,
{
    root: V,
    current: V,
    _marker: PhantomData<&'value ()>,
}

impl<'value, V> FilterEvaluator<'value, V>
where
    V: JsonPathValue<'value>,
{
    fn new(root: V, current: V) -> Self {
        Self {
            root,
            current,
            _marker: PhantomData,
        }
    }

    fn eval_logical(&self, expr: &LogicalExpr) -> bool {
        match expr {
            LogicalExpr::Or(left, right) => self.eval_logical(left) || self.eval_logical(right),
            LogicalExpr::And(left, right) => self.eval_logical(left) && self.eval_logical(right),
            LogicalExpr::Not(expr) => !self.eval_logical(expr),
            LogicalExpr::Paren(expr) => self.eval_logical(expr),
            LogicalExpr::Comparison { left, op, right } => self.eval_comparison(left, *op, right),
            LogicalExpr::Test(expr) => self.eval_test(expr),
        }
    }

    fn eval_test(&self, expr: &TestExpr) -> bool {
        match expr {
            TestExpr::Query(query) => !self.eval_query(query).is_empty(),
            TestExpr::Function(function) => match self.eval_function(function) {
                FunctionResult::Logical(value) => value,
                FunctionResult::Value(_) => panic!("ValueType function result cannot be used as a test expression"),
            },
        }
    }

    fn eval_comparison(&self, left: &Comparable, op: ComparisonOp, right: &Comparable) -> bool {
        let left = self.eval_comparable(left);
        let right = self.eval_comparable(right);
        match op {
            ComparisonOp::Eq => Self::values_equal(&left, &right),
            ComparisonOp::Ne => !Self::values_equal(&left, &right),
            ComparisonOp::Lt => Self::values_lt(&left, &right),
            ComparisonOp::Le => Self::values_lt(&left, &right) || Self::values_equal(&left, &right),
            ComparisonOp::Gt => Self::values_lt(&right, &left),
            ComparisonOp::Ge => Self::values_lt(&right, &left) || Self::values_equal(&left, &right),
        }
    }

    fn eval_comparable(&self, comparable: &Comparable) -> ComparableValue<V> {
        match comparable {
            Comparable::Literal(literal) => Self::literal_to_value(literal),
            Comparable::SingularQuery(query) => self
                .eval_singular_query(query)
                .map(Self::node_to_value)
                .unwrap_or(ComparableValue::Nothing),
            Comparable::Function(function) => match self.eval_function(function) {
                FunctionResult::Value(value) => value,
                FunctionResult::Logical(value) => ComparableValue::Bool(value),
            },
        }
    }

    fn eval_query(&self, query: &Query) -> Vec<V> {
        let root = match query.root {
            QueryRoot::Root => self.root,
            QueryRoot::Current => self.current,
        };
        let mut nodes = vec![root];
        for segment in &query.segments {
            nodes = self.eval_segment(&nodes, segment);
        }
        nodes
    }

    fn eval_segment(&self, inputs: &[V], segment: &Segment) -> Vec<V> {
        let mut result = Vec::new();
        for &node in inputs {
            match segment {
                Segment::Child(selectors) => self.apply_selectors(node, selectors, &mut result),
                Segment::Descendant(selectors) => self.apply_descendant_selectors(node, selectors, &mut result),
            }
        }
        result
    }

    fn apply_descendant_selectors(&self, node: V, selectors: &[Selector], result: &mut Vec<V>) {
        self.apply_selectors(node, selectors, result);
        if let Some(array) = node.as_array() {
            for index in 0..array.len() {
                if let Some(child) = array.element(index) {
                    self.apply_descendant_selectors(child, selectors, result);
                }
            }
        }
        if let Some(object) = node.as_object() {
            for (_, child) in object.members() {
                self.apply_descendant_selectors(child, selectors, result);
            }
        }
    }

    fn apply_selectors(&self, node: V, selectors: &[Selector], result: &mut Vec<V>) {
        for selector in selectors {
            self.apply_selector(node, selector, result);
        }
    }

    fn apply_selector(&self, node: V, selector: &Selector, result: &mut Vec<V>) {
        match selector {
            Selector::Name(name) => {
                if let Some(object) = node.as_object()
                    && let Some(value) = object.member(name)
                {
                    result.push(value);
                }
            }
            Selector::Wildcard => {
                if let Some(array) = node.as_array() {
                    for index in 0..array.len() {
                        if let Some(value) = array.element(index) {
                            result.push(value);
                        }
                    }
                }
                if let Some(object) = node.as_object() {
                    for (_, value) in object.members() {
                        result.push(value);
                    }
                }
            }
            Selector::Index(index) => {
                if let Some(array) = node.as_array()
                    && let Some(index) = normalize_index(array.len(), *index)
                    && let Some(value) = array.element(index)
                {
                    result.push(value);
                }
            }
            Selector::Slice { start, end, step } => {
                if let Some(array) = node.as_array() {
                    for index in slice_indices(array.len(), *start, *end, *step) {
                        if let Some(value) = array.element(index) {
                            result.push(value);
                        }
                    }
                }
            }
            Selector::Filter(expr) => {
                if let Some(array) = node.as_array() {
                    for index in 0..array.len() {
                        if let Some(value) = array.element(index)
                            && FilterEvaluator::new(self.root, value).eval_logical(expr)
                        {
                            result.push(value);
                        }
                    }
                }
                if let Some(object) = node.as_object() {
                    for (_, value) in object.members() {
                        if FilterEvaluator::new(self.root, value).eval_logical(expr) {
                            result.push(value);
                        }
                    }
                }
            }
        }
    }

    fn eval_singular_query(&self, query: &SingularQuery) -> Option<V> {
        let mut value = match query.root {
            QueryRoot::Root => self.root,
            QueryRoot::Current => self.current,
        };
        for segment in &query.segments {
            value = match segment {
                SingularSegment::Name(name) => value.as_object()?.member(name)?,
                SingularSegment::Index(index) => {
                    let array = value.as_array()?;
                    let index = normalize_index(array.len(), *index)?;
                    array.element(index)?
                }
            };
        }
        Some(value)
    }

    fn eval_function(&self, function: &FunctionExpr) -> FunctionResult<V> {
        match function.name.as_str() {
            "length" => FunctionResult::Value(self.eval_length_function(function)),
            "count" => FunctionResult::Value(self.eval_count_function(function)),
            "value" => FunctionResult::Value(self.eval_value_function(function)),
            "match" => FunctionResult::Logical(self.eval_regex_function(function, RegexMode::Full)),
            "search" => FunctionResult::Logical(self.eval_regex_function(function, RegexMode::Partial)),
            _ => unimplemented!("unsupported JSONPath function: {}", function.name),
        }
    }

    fn eval_length_function(&self, function: &FunctionExpr) -> ComparableValue<V> {
        let argument = self.single_argument(function);
        match self.eval_value_argument(argument) {
            ComparableValue::String(value) => ComparableValue::Number(len_to_number(value.chars().count())),
            ComparableValue::Node(value) => {
                if let Some(array) = value.as_array() {
                    ComparableValue::Number(len_to_number(array.len()))
                } else if let Some(object) = value.as_object() {
                    ComparableValue::Number(len_to_number(object.len()))
                } else {
                    ComparableValue::Nothing
                }
            }
            _ => ComparableValue::Nothing,
        }
    }

    fn eval_count_function(&self, function: &FunctionExpr) -> ComparableValue<V> {
        let argument = self.single_argument(function);
        ComparableValue::Number(len_to_number(self.eval_nodes_argument(argument).len()))
    }

    fn eval_value_function(&self, function: &FunctionExpr) -> ComparableValue<V> {
        let argument = self.single_argument(function);
        let nodes = self.eval_nodes_argument(argument);
        match nodes.as_slice() {
            [value] => Self::node_to_value(*value),
            _ => ComparableValue::Nothing,
        }
    }

    fn eval_regex_function(&self, function: &FunctionExpr, mode: RegexMode) -> bool {
        let [value_argument, regex_argument] = function.arguments.as_slice() else {
            panic!(
                "JSONPath function {} expects exactly two arguments, got {}",
                function.name,
                function.arguments.len()
            );
        };
        let ComparableValue::String(value) = self.eval_value_argument(value_argument) else {
            return false;
        };
        let ComparableValue::String(regex) = self.eval_value_argument(regex_argument) else {
            panic!("JSONPath function {} expects a string regexp argument", function.name);
        };
        let regex = regex_lite::Regex::new(&regex)
            .unwrap_or_else(|error| panic!("invalid JSONPath regexp {:?}: {}", regex, error));
        match mode {
            RegexMode::Full => regex
                .find(&value)
                .is_some_and(|matched| matched.start() == 0 && matched.end() == value.len()),
            RegexMode::Partial => regex.is_match(&value),
        }
    }

    fn single_argument<'expr>(&self, function: &'expr FunctionExpr) -> &'expr FunctionArgument {
        match function.arguments.as_slice() {
            [argument] => argument,
            _ => panic!(
                "JSONPath function {} expects exactly one argument, got {}",
                function.name,
                function.arguments.len()
            ),
        }
    }

    fn eval_value_argument(&self, argument: &FunctionArgument) -> ComparableValue<V> {
        match argument {
            FunctionArgument::Literal(literal) => Self::literal_to_value(literal),
            FunctionArgument::Query(query) => self.eval_query_as_value(query),
            FunctionArgument::LogicalExpr(LogicalExpr::Test(TestExpr::Query(query))) => self.eval_query_as_value(query),
            FunctionArgument::LogicalExpr(_) => {
                panic!("logical expression cannot be used as a ValueType function argument")
            }
            FunctionArgument::Function(function) => match self.eval_function(function) {
                FunctionResult::Value(value) => value,
                FunctionResult::Logical(_) => panic!("LogicalType function result cannot be used as ValueType"),
            },
        }
    }

    fn eval_nodes_argument(&self, argument: &FunctionArgument) -> Vec<V> {
        match argument {
            FunctionArgument::Query(query) => self.eval_query(query),
            FunctionArgument::LogicalExpr(LogicalExpr::Test(TestExpr::Query(query))) => self.eval_query(query),
            FunctionArgument::Function(function) => match self.eval_function(function) {
                FunctionResult::Value(_) => panic!("ValueType function result cannot be used as NodesType"),
                FunctionResult::Logical(_) => panic!("LogicalType function result cannot be used as NodesType"),
            },
            FunctionArgument::Literal(_) | FunctionArgument::LogicalExpr(_) => {
                panic!("argument cannot be used as a NodesType function argument")
            }
        }
    }

    fn eval_query_as_value(&self, query: &Query) -> ComparableValue<V> {
        if !Self::query_is_singular(query) {
            panic!("non-singular query cannot be used as a ValueType function argument");
        }
        let nodes = self.eval_query(query);
        match nodes.as_slice() {
            [] => ComparableValue::Nothing,
            [value] => Self::node_to_value(*value),
            _ => unreachable!("syntactically singular query returned multiple nodes"),
        }
    }

    fn query_is_singular(query: &Query) -> bool {
        query.segments.iter().all(|segment| match segment {
            Segment::Child(selectors) => matches!(selectors.as_slice(), [Selector::Name(_) | Selector::Index(_)]),
            Segment::Descendant(_) => false,
        })
    }

    fn literal_to_value(literal: &Literal) -> ComparableValue<V> {
        match literal {
            Literal::Number(value) => ComparableValue::Number(*value),
            Literal::String(value) => ComparableValue::String(value.clone()),
            Literal::Bool(value) => ComparableValue::Bool(*value),
            Literal::Null => ComparableValue::Null,
        }
    }

    fn node_to_value(value: V) -> ComparableValue<V> {
        if value.is_null() {
            ComparableValue::Null
        } else if let Some(value) = value.as_bool() {
            ComparableValue::Bool(value)
        } else if let Some(value) = value.as_integer() {
            ComparableValue::Number(Number::Int(value))
        } else if let Some(value) = value.as_float() {
            ComparableValue::Number(Number::Float(value))
        } else if let Some(value) = value.as_str() {
            ComparableValue::String(value.to_owned())
        } else {
            ComparableValue::Node(value)
        }
    }

    fn values_equal(left: &ComparableValue<V>, right: &ComparableValue<V>) -> bool {
        match (left, right) {
            (ComparableValue::Nothing, ComparableValue::Nothing) => true,
            (ComparableValue::Null, ComparableValue::Null) => true,
            (ComparableValue::Bool(left), ComparableValue::Bool(right)) => left == right,
            (ComparableValue::Number(left), ComparableValue::Number(right)) => numbers_equal(*left, *right),
            (ComparableValue::String(left), ComparableValue::String(right)) => left == right,
            (ComparableValue::Node(left), ComparableValue::Node(right)) => Self::json_values_equal(*left, *right),
            _ => false,
        }
    }

    fn values_lt(left: &ComparableValue<V>, right: &ComparableValue<V>) -> bool {
        match (left, right) {
            (ComparableValue::Number(left), ComparableValue::Number(right)) => numbers_lt(*left, *right),
            (ComparableValue::String(left), ComparableValue::String(right)) => left < right,
            _ => false,
        }
    }

    fn json_values_equal(left: V, right: V) -> bool {
        if left.is_null() || right.is_null() {
            return left.is_null() && right.is_null();
        }
        if let (Some(left), Some(right)) = (left.as_bool(), right.as_bool()) {
            return left == right;
        }
        if let (Some(left), Some(right)) = (left.as_str(), right.as_str()) {
            return left == right;
        }
        if let (Some(left), Some(right)) = (value_number(left), value_number(right)) {
            return numbers_equal(left, right);
        }
        if let (Some(left), Some(right)) = (left.as_array(), right.as_array()) {
            if left.len() != right.len() {
                return false;
            }
            for index in 0..left.len() {
                let Some(left) = left.element(index) else {
                    return false;
                };
                let Some(right) = right.element(index) else {
                    return false;
                };
                if !Self::json_values_equal(left, right) {
                    return false;
                }
            }
            return true;
        }
        if let (Some(left), Some(right)) = (left.as_object(), right.as_object()) {
            if left.len() != right.len() {
                return false;
            }
            for (name, left) in left.members() {
                let Some(right) = right.member(name) else {
                    return false;
                };
                if !Self::json_values_equal(left, right) {
                    return false;
                }
            }
            return true;
        }
        false
    }
}

enum RegexMode {
    Full,
    Partial,
}

fn normalize_index(len: usize, index: i64) -> Option<usize> {
    let normalized = if index < 0 { len as i64 + index } else { index };
    (normalized >= 0)
        .then_some(normalized)
        .and_then(|index| usize::try_from(index).ok())
        .filter(|&index| index < len)
}

fn slice_indices(len: usize, start: Option<i64>, end: Option<i64>, step: Option<i64>) -> Vec<usize> {
    let step = step.unwrap_or(1);
    if step == 0 {
        return Vec::new();
    }
    if step > 0 {
        (0..len)
            .filter(|&index| slice_contains(len, start, end, Some(step), index))
            .collect()
    } else {
        (0..len)
            .rev()
            .filter(|&index| slice_contains(len, start, end, Some(step), index))
            .collect()
    }
}

fn slice_contains(len: usize, start: Option<i64>, end: Option<i64>, step: Option<i64>, array_index: usize) -> bool {
    let len = len as i64;
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

fn len_to_number(len: usize) -> Number {
    i64::try_from(len)
        .map(Number::Int)
        .unwrap_or_else(|_| Number::Float(len as f64))
}

fn value_number<'value, V>(value: V) -> Option<Number>
where
    V: JsonPathValue<'value>,
{
    if let Some(value) = value.as_integer() {
        Some(Number::Int(value))
    } else {
        value.as_float().map(Number::Float)
    }
}

fn numbers_equal(left: Number, right: Number) -> bool {
    match (left, right) {
        (Number::Int(left), Number::Int(right)) => left == right,
        _ => number_as_f64(left) == number_as_f64(right),
    }
}

fn numbers_lt(left: Number, right: Number) -> bool {
    match (left, right) {
        (Number::Int(left), Number::Int(right)) => left < right,
        _ => number_as_f64(left)
            .partial_cmp(&number_as_f64(right))
            .is_some_and(|ordering| ordering == Ordering::Less),
    }
}

fn number_as_f64(value: Number) -> f64 {
    match value {
        Number::Int(value) => value as f64,
        Number::Float(value) => value,
    }
}

impl<'a> JsonPathMatcher<'a> {
    pub(crate) fn new(paths: &'a [JsonPath]) -> Self {
        Self {
            paths,
            descendant_active: Vec::new(),
        }
    }

    pub(crate) fn root_state<'state, 'value, V>(&'state mut self, root: V) -> JsonPathMatchState<'state, 'a, V>
    where
        V: JsonPathValue<'value>,
    {
        self.descendant_active.clear();
        let mut state = JsonPathMatchState {
            root,
            current: root,
            descendant_active: &mut self.descendant_active,
            descendant_start: 0,
            child_active: ActiveSegmentsVec::new(),
            matched: false,
        };
        for path in self.paths {
            let mut descendant_active = ActiveSegmentsVec::new();
            activate_segments(
                path.segments.as_slice(),
                &mut state.child_active,
                &mut descendant_active,
                &mut state.matched,
            );
            for segments in descendant_active {
                if !state.descendant_active.contains(&segments) {
                    state.descendant_active.push(segments);
                }
            }
        }
        state
    }
}

fn activate_segments<'path>(
    segments: ActiveSegments<'path>,
    child_active: &mut ActiveSegmentsVec<'path>,
    descendant_active: &mut ActiveSegmentsVec<'path>,
    matched: &mut bool,
) {
    match segments.first() {
        None => *matched = true,
        Some(Segment::Child(_)) => push_unique(child_active, segments),
        Some(Segment::Descendant(_)) => push_unique(descendant_active, segments),
    }
}

fn push_unique<'path>(values: &mut ActiveSegmentsVec<'path>, segments: ActiveSegments<'path>) {
    if !values.contains(&segments) {
        values.push(segments);
    }
}
