use std::cmp::Ordering;
use std::error::Error;
use std::mem;
use thiserror::Error;

#[derive(Debug)]
pub enum TraversalNode<Node, Leaf> {
    Node(Node),
    Leaf(Leaf),
}

impl<Node, Leaf> PartialEq for TraversalNode<Node, Leaf>
where
    Node: NodeTraverse,
    Leaf: LeafTraverse,
{
    fn eq(&self, other: &Self) -> bool {
        if mem::discriminant(self) != mem::discriminant(other) {
            return false;
        }
        match (self, other) {
            (TraversalNode::Node(a), TraversalNode::Node(b)) => a.name() == b.name(),
            (TraversalNode::Leaf(a), TraversalNode::Leaf(b)) => a.name() == b.name(),
            _ => unreachable!(),
        }
    }
}

impl<Node, Leaf> Eq for TraversalNode<Node, Leaf>
where
    Node: NodeTraverse,
    Leaf: LeafTraverse,
{
}

impl<Node, Leaf> PartialOrd for TraversalNode<Node, Leaf>
where
    Node: NodeTraverse,
    Leaf: LeafTraverse,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<Node, Leaf> Ord for TraversalNode<Node, Leaf>
where
    Node: NodeTraverse,
    Leaf: LeafTraverse,
{
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (TraversalNode::Node(_), TraversalNode::Leaf(_)) => Ordering::Less,
            (TraversalNode::Leaf(_), TraversalNode::Node(_)) => Ordering::Greater,
            (TraversalNode::Node(a), TraversalNode::Node(b)) => a.name().cmp(b.name()),
            (TraversalNode::Leaf(a), TraversalNode::Leaf(b)) => a.name().cmp(b.name()),
        }
    }
}

pub trait LeafTraverse {
    fn name(&self) -> &str;
}

pub trait NodeTraverse: Sized {
    type Leaf: LeafTraverse;
    fn name(&self) -> &str;
    type TraverseError: Error + Send + 'static;
    fn children(&mut self) -> impl Iterator<Item = Result<TraversalNode<Self, Self::Leaf>, Self::TraverseError>>;
}

pub trait Diff {
    fn equal(&self) -> bool;
}

pub trait DiffCalculator<T> {
    type Error: Error + Send + 'static;
    type Diff: Diff + Send;
    fn available(&self, expected: &T, actual: &T) -> Result<bool, Self::Error>;
    fn diff(&self, name: &[String], expected: T, actual: T) -> Result<Self::Diff, Self::Error>;
}

pub trait DetailReporter<Diff, T, Reporter> {
    type Error: Error + Send + 'static;
    fn available(&self, data: &T) -> Result<bool, Self::Error>;
    fn report_equal(&self, name: &[String], diff: Diff, reporter: &Reporter) -> Result<(), Self::Error>;
    fn report_diff(&self, name: &[String], diff: Diff, reporter: &Reporter) -> Result<(), Self::Error>;
    fn report_added(&self, name: &[String], data: T, reporter: &Reporter) -> Result<(), Self::Error>;
    fn report_deleted(&self, name: &[String], data: T, reporter: &Reporter) -> Result<(), Self::Error>;
}

#[doc(hidden)]
mod __sealed {
    pub trait Sealed {}
}

pub trait DiffReport<T, Reporter>: __sealed::Sealed {
    fn available(&self, expected: Option<&T>, actual: Option<&T>) -> Result<bool, Box<dyn Error + Send>>;
    fn diff(&self, name: &[String], expected: T, actual: T, reporter: &Reporter) -> Result<(), Box<dyn Error + Send>>;
    fn added(&self, name: &[String], data: T, reporter: &Reporter) -> Result<(), Box<dyn Error + Send>>;
    fn deleted(&self, name: &[String], data: T, reporter: &Reporter) -> Result<(), Box<dyn Error + Send>>;
}

#[derive(Debug)]
pub struct DiffAndReport<DiffCalculator, DetailReporter> {
    diff: DiffCalculator,
    report: DetailReporter,
}

impl<DiffCalculator, DetailReporter> __sealed::Sealed for DiffAndReport<DiffCalculator, DetailReporter> {}

impl<D, R, T, Reporter> DiffReport<T, Reporter> for DiffAndReport<D, R>
where
    D: DiffCalculator<T> + Sync,
    R: DetailReporter<D::Diff, T, Reporter> + Sync,
    T: Send,
    Reporter: Sync,
{
    fn available(&self, expected: Option<&T>, actual: Option<&T>) -> Result<bool, Box<dyn Error + Send>> {
        match (expected, actual) {
            (Some(expected), Some(actual)) => self
                .diff
                .available(expected, actual)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>),
            (Some(data), None) | (None, Some(data)) => self
                .report
                .available(data)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>),
            (None, None) => unreachable!(),
        }
    }

    fn diff(&self, name: &[String], expected: T, actual: T, reporter: &Reporter) -> Result<(), Box<dyn Error + Send>> {
        let diff = self
            .diff
            .diff(name, expected, actual)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        if diff.equal() {
            self.report
                .report_equal(name, diff, reporter)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        } else {
            self.report
                .report_diff(name, diff, reporter)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        }
        Ok(())
    }

    fn added(&self, name: &[String], data: T, reporter: &Reporter) -> Result<(), Box<dyn Error + Send>> {
        self.report
            .report_added(name, data, reporter)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    }

    fn deleted(&self, name: &[String], data: T, reporter: &Reporter) -> Result<(), Box<dyn Error + Send>> {
        self.report
            .report_deleted(name, data, reporter)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    }
}

#[derive(Debug, Error)]
pub enum CalcDiffError<TraverseError> {
    #[error("{0}")]
    TraverseError(#[source] TraverseError),
    #[error("{0}")]
    DiffError(#[source] Box<dyn Error + Send>),
    #[error("No diff report matched")]
    NoDiffReportMatched,
}

pub fn calc_diff<N, R>(
    expected: N,
    actual: N,
    diff: &[Box<dyn DiffReport<N::Leaf, R>>],
    reporter: R,
) -> Result<(), CalcDiffError<N::TraverseError>>
where
    N: NodeTraverse,
{
    return calc_diff_inner::<N, R>(&mut Vec::new(), Some(expected), Some(actual), diff, &reporter);
    fn calc_diff_inner<N, R>(
        name: &mut Vec<String>,
        expected: Option<N>,
        actual: Option<N>,
        diff: &[Box<dyn DiffReport<N::Leaf, R>>],
        reporter: &R,
    ) -> Result<(), CalcDiffError<N::TraverseError>>
    where
        N: NodeTraverse,
    {
        let get_diff_report =
            |expected: Option<&N::Leaf>, actual: Option<&N::Leaf>| -> Result<_, CalcDiffError<N::TraverseError>> {
                for diff in diff {
                    if diff.available(expected, actual).map_err(CalcDiffError::DiffError)? {
                        return Ok(diff);
                    }
                }
                Err(CalcDiffError::NoDiffReportMatched)
            };
        match (expected, actual) {
            (Some(mut expected), Some(mut actual)) => {
                let mut expected = expected
                    .children()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CalcDiffError::TraverseError)?;
                let mut actual = actual
                    .children()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CalcDiffError::TraverseError)?;
                expected.sort_unstable();
                actual.sort_unstable();
                let mut expected_iter = expected.into_iter().peekable();
                let mut actual_iter = actual.into_iter().peekable();

                loop {
                    let pair = match (expected_iter.peek(), actual_iter.peek()) {
                        (Some(expected), Some(actual)) => match expected.cmp(actual) {
                            Ordering::Less => (expected_iter.next(), None),
                            Ordering::Equal => (expected_iter.next(), actual_iter.next()),
                            Ordering::Greater => (None, actual_iter.next()),
                        },
                        (Some(_), None) => (expected_iter.next(), None),
                        (None, Some(_)) => (None, actual_iter.next()),
                        (None, None) => (None, None),
                    };
                    match pair {
                        (None, None) => break,
                        (Some(expected), Some(actual)) => match (expected, actual) {
                            (TraversalNode::Node(expected), TraversalNode::Node(actual)) => {
                                name.push(expected.name().to_string());
                                calc_diff_inner(name, Some(expected), Some(actual), diff, reporter)?;
                                name.pop();
                            }
                            (TraversalNode::Leaf(expected), TraversalNode::Leaf(actual)) => {
                                get_diff_report(Some(&expected), Some(&actual))?
                                    .diff(name, expected, actual, reporter)
                                    .map_err(CalcDiffError::DiffError)?;
                            }
                            _ => unreachable!(),
                        },
                        (Some(expected), None) => match expected {
                            TraversalNode::Node(node) => {
                                name.push(node.name().to_string());
                                calc_diff_inner(name, Some(node), None, diff, reporter)?;
                                name.pop();
                            }
                            TraversalNode::Leaf(leaf) => {
                                get_diff_report(Some(&leaf), None)?
                                    .deleted(name, leaf, reporter)
                                    .map_err(CalcDiffError::DiffError)?;
                            }
                        },
                        (None, Some(actual)) => match actual {
                            TraversalNode::Node(node) => {
                                name.push(node.name().to_string());
                                calc_diff_inner(name, None, Some(node), diff, reporter)?;
                                name.pop();
                            }
                            TraversalNode::Leaf(leaf) => {
                                get_diff_report(None, Some(&leaf))?
                                    .added(name, leaf, reporter)
                                    .map_err(CalcDiffError::DiffError)?;
                            }
                        },
                    }
                }
            }
            (Some(mut expected), None) => {
                for result in expected.children() {
                    let node = result.map_err(CalcDiffError::TraverseError)?;
                    match node {
                        TraversalNode::Node(node) => {
                            name.push(node.name().to_string());
                            calc_diff_inner(name, Some(node), None, diff, reporter)?;
                            name.pop();
                        }
                        TraversalNode::Leaf(leaf) => {
                            get_diff_report(Some(&leaf), None)?
                                .deleted(name, leaf, reporter)
                                .map_err(CalcDiffError::DiffError)?;
                        }
                    }
                }
            }
            (None, Some(mut actual)) => {
                for result in actual.children() {
                    let node = result.map_err(CalcDiffError::TraverseError)?;
                    match node {
                        TraversalNode::Node(node) => {
                            name.push(node.name().to_string());
                            calc_diff_inner(name, Some(node), None, diff, reporter)?;
                            name.pop();
                        }
                        TraversalNode::Leaf(leaf) => {
                            get_diff_report(None, Some(&leaf))?
                                .added(name, leaf, reporter)
                                .map_err(CalcDiffError::DiffError)?;
                        }
                    }
                }
            }
            (None, None) => {}
        }
        Ok(())
    }
}
