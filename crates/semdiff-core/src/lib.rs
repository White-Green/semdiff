use rayon::Scope;
use std::cmp::Ordering;
use std::error::Error;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use thiserror::Error;

pub mod fs;

#[cfg(test)]
mod tests;

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
    type Leaf: LeafTraverse + Clone;
    type TraverseError: Error + Send + 'static;
    fn name(&self) -> &str;
    #[allow(clippy::type_complexity)]
    fn children(
        &mut self,
    ) -> Result<impl Iterator<Item = Result<TraversalNode<Self, Self::Leaf>, Self::TraverseError>>, Self::TraverseError>;
}

pub trait Diff {
    fn equal(&self) -> bool;
}

#[derive(Debug)]
pub enum MayUnsupported<T> {
    Ok(T),
    Unsupported,
}

pub trait DiffCalculator<T> {
    type Error: Error + Send + 'static;
    type Diff: Diff + Send;
    fn diff(&self, name: &str, expected: T, actual: T) -> Result<MayUnsupported<Self::Diff>, Self::Error>;
}

pub trait DetailReporter<Diff, T, Reporter> {
    type Error: Error + Send + 'static;
    fn report_unchanged(&self, name: &str, diff: Diff, reporter: &Reporter) -> Result<MayUnsupported<()>, Self::Error>;
    fn report_modified(&self, name: &str, diff: Diff, reporter: &Reporter) -> Result<MayUnsupported<()>, Self::Error>;
    fn report_added(&self, name: &str, data: T, reporter: &Reporter) -> Result<MayUnsupported<()>, Self::Error>;
    fn report_deleted(&self, name: &str, data: T, reporter: &Reporter) -> Result<MayUnsupported<()>, Self::Error>;
}

#[doc(hidden)]
mod __sealed {
    pub trait Sealed {}
}

pub trait DiffReport<T, Reporter>: __sealed::Sealed + Sync {
    fn diff(
        &self,
        name: &str,
        expected: T,
        actual: T,
        reporter: &Reporter,
    ) -> Result<MayUnsupported<()>, Box<dyn Error + Send>>;
    fn added(&self, name: &str, data: T, reporter: &Reporter) -> Result<MayUnsupported<()>, Box<dyn Error + Send>>;
    fn deleted(&self, name: &str, data: T, reporter: &Reporter) -> Result<MayUnsupported<()>, Box<dyn Error + Send>>;
}

#[derive(Debug)]
pub struct DiffAndReport<DiffCalculator, DetailReporter> {
    diff: DiffCalculator,
    report: DetailReporter,
}

impl<DiffCalculator, DetailReporter> DiffAndReport<DiffCalculator, DetailReporter> {
    pub fn new(diff: DiffCalculator, report: DetailReporter) -> Self {
        Self { diff, report }
    }
}

impl<DiffCalculator, DetailReporter> __sealed::Sealed for DiffAndReport<DiffCalculator, DetailReporter> {}

impl<D, R, T, Reporter> DiffReport<T, Reporter> for DiffAndReport<D, R>
where
    D: DiffCalculator<T> + Sync,
    R: DetailReporter<D::Diff, T, Reporter> + Sync,
    T: Send,
    Reporter: Sync,
{
    fn diff(
        &self,
        name: &str,
        expected: T,
        actual: T,
        reporter: &Reporter,
    ) -> Result<MayUnsupported<()>, Box<dyn Error + Send>> {
        let diff = self
            .diff
            .diff(name, expected, actual)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        let MayUnsupported::Ok(diff) = diff else {
            return Ok(MayUnsupported::Unsupported);
        };
        if diff.equal() {
            self.report
                .report_unchanged(name, diff, reporter)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        } else {
            self.report
                .report_modified(name, diff, reporter)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        }
    }

    fn added(&self, name: &str, data: T, reporter: &Reporter) -> Result<MayUnsupported<()>, Box<dyn Error + Send>> {
        self.report
            .report_added(name, data, reporter)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    }

    fn deleted(&self, name: &str, data: T, reporter: &Reporter) -> Result<MayUnsupported<()>, Box<dyn Error + Send>> {
        self.report
            .report_deleted(name, data, reporter)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    }
}

pub trait Reporter {
    type Error: Error + Send + 'static;
    fn start(&mut self) -> Result<(), Self::Error>;
    fn finish(self) -> Result<(), Self::Error>;
}

#[derive(Debug, Error)]
pub enum CalcDiffError<TraverseError, ReporterError> {
    #[error("{0}")]
    TraverseError(#[source] TraverseError),
    #[error("{0}")]
    ReporterError(#[source] ReporterError),
    #[error("{0}")]
    DiffError(#[source] Box<dyn Error + Send>),
    #[error("No diff report matched")]
    NoDiffReportMatched,
}

pub fn calc_diff<N, R>(
    expected: N,
    actual: N,
    diff: &[Box<dyn DiffReport<N::Leaf, R>>],
    mut reporter: R,
) -> Result<(), CalcDiffError<N::TraverseError, R::Error>>
where
    N: NodeTraverse + Send,
    N::Leaf: Send,
    R: Reporter + Sync,
{
    reporter.start().map_err(CalcDiffError::ReporterError)?;
    let errors = Mutex::new(None);
    rayon::scope(|scope| {
        if let Err(error) = calc_diff_inner::<N, R, R::Error>(
            &mut String::new(),
            Some(expected),
            Some(actual),
            diff,
            &reporter,
            scope,
            &errors,
        ) {
            record_error(&errors, error);
        }
    });
    if let Some(error) = errors.lock().unwrap().take() {
        return Err(error);
    }
    reporter.finish().map_err(CalcDiffError::ReporterError)?;
    Ok(())
}

fn calc_diff_inner<'scope, N, R, RE>(
    name: &mut String,
    expected: Option<N>,
    actual: Option<N>,
    diff: &'scope [Box<dyn DiffReport<N::Leaf, R>>],
    reporter: &'scope R,
    scope: &Scope<'scope>,
    errors: &'scope Mutex<Option<CalcDiffError<N::TraverseError, RE>>>,
) -> Result<(), CalcDiffError<N::TraverseError, RE>>
where
    N: NodeTraverse,
    N::Leaf: Send,
    R: Reporter + Sync,
    RE: Send + 'scope,
{
    match (expected, actual) {
        (Some(mut expected), Some(mut actual)) => {
            let mut expected = expected
                .children()
                .map_err(CalcDiffError::TraverseError)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CalcDiffError::TraverseError)?;
            let mut actual = actual
                .children()
                .map_err(CalcDiffError::TraverseError)?
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
                            let mut name = AppendedName::new(name, expected.name());
                            calc_diff_inner(&mut name, Some(expected), Some(actual), diff, reporter, scope, errors)?;
                        }
                        (TraversalNode::Leaf(expected), TraversalNode::Leaf(actual)) => {
                            let name = AppendedName::new(name, expected.name());
                            let name = name.clone();
                            spawn_task(scope, errors, move || {
                                run_diff::<N, R, RE>(diff, reporter, &name, &expected, &actual)
                            });
                        }
                        _ => unreachable!(),
                    },
                    (Some(expected), None) => match expected {
                        TraversalNode::Node(node) => {
                            let mut name = AppendedName::new(name, node.name());
                            calc_diff_inner(&mut name, Some(node), None, diff, reporter, scope, errors)?;
                        }
                        TraversalNode::Leaf(leaf) => {
                            let name = AppendedName::new(name, leaf.name());
                            let name = name.clone();
                            spawn_task(scope, errors, move || {
                                run_deleted::<N, R, RE>(diff, reporter, &name, &leaf)
                            });
                        }
                    },
                    (None, Some(actual)) => match actual {
                        TraversalNode::Node(node) => {
                            let mut name = AppendedName::new(name, node.name());
                            calc_diff_inner(&mut name, None, Some(node), diff, reporter, scope, errors)?;
                        }
                        TraversalNode::Leaf(leaf) => {
                            let name = AppendedName::new(name, leaf.name());
                            let name = name.clone();
                            spawn_task(scope, errors, move || {
                                run_added::<N, R, RE>(diff, reporter, &name, &leaf)
                            });
                        }
                    },
                }
            }
        }
        (Some(mut expected), None) => {
            for result in expected.children().map_err(CalcDiffError::TraverseError)? {
                let node = result.map_err(CalcDiffError::TraverseError)?;
                match node {
                    TraversalNode::Node(node) => {
                        let mut name = AppendedName::new(name, node.name());
                        calc_diff_inner(&mut name, Some(node), None, diff, reporter, scope, errors)?;
                    }
                    TraversalNode::Leaf(leaf) => {
                        let name = AppendedName::new(name, leaf.name());
                        let name = name.clone();
                        spawn_task(scope, errors, move || {
                            run_deleted::<N, R, RE>(diff, reporter, &name, &leaf)
                        });
                    }
                }
            }
        }
        (None, Some(mut actual)) => {
            for result in actual.children().map_err(CalcDiffError::TraverseError)? {
                let node = result.map_err(CalcDiffError::TraverseError)?;
                match node {
                    TraversalNode::Node(node) => {
                        let mut name = AppendedName::new(name, node.name());
                        calc_diff_inner(&mut name, Some(node), None, diff, reporter, scope, errors)?;
                    }
                    TraversalNode::Leaf(leaf) => {
                        let name = AppendedName::new(name, leaf.name());
                        let name = name.clone();
                        spawn_task(scope, errors, move || {
                            run_added::<N, R, RE>(diff, reporter, &name, &leaf)
                        });
                    }
                }
            }
        }
        (None, None) => {}
    }
    Ok(())
}

fn record_error<TE, RE>(errors: &Mutex<Option<CalcDiffError<TE, RE>>>, error: CalcDiffError<TE, RE>) {
    let mut guard = errors.lock().unwrap();
    if guard.is_none() {
        *guard = Some(error);
    }
}

fn spawn_task<'scope, TE, RE>(
    scope: &Scope<'scope>,
    errors: &'scope Mutex<Option<CalcDiffError<TE, RE>>>,
    task: impl FnOnce() -> Result<(), CalcDiffError<TE, RE>> + Send + 'scope,
) where
    TE: Send + 'scope,
    RE: Send + 'scope,
{
    scope.spawn(move |_| {
        if let Err(error) = task() {
            record_error(errors, error);
        }
    });
}

struct AppendedName<'a> {
    original_len: usize,
    name: &'a mut String,
}

impl AppendedName<'_> {
    fn new<'a>(name: &'a mut String, segment: &str) -> AppendedName<'a> {
        let original_len = name.len();
        if !name.is_empty() {
            name.push('/');
        }
        name.push_str(segment);
        AppendedName { original_len, name }
    }
}

impl Deref for AppendedName<'_> {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        self.name
    }
}

impl DerefMut for AppendedName<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.name
    }
}

impl Drop for AppendedName<'_> {
    fn drop(&mut self) {
        self.name.truncate(self.original_len);
    }
}

fn run_diff<N, R, RE>(
    diff: &[Box<dyn DiffReport<N::Leaf, R>>],
    reporter: &R,
    name: &str,
    expected: &N::Leaf,
    actual: &N::Leaf,
) -> Result<(), CalcDiffError<N::TraverseError, RE>>
where
    N: NodeTraverse,
    N::Leaf: Clone,
    R: Reporter + Sync,
{
    for diff in diff {
        if let MayUnsupported::Ok(()) = diff
            .diff(name, expected.clone(), actual.clone(), reporter)
            .map_err(CalcDiffError::DiffError)?
        {
            return Ok(());
        }
    }
    Err(CalcDiffError::<N::TraverseError, RE>::NoDiffReportMatched)
}

fn run_added<N, R, RE>(
    diff: &[Box<dyn DiffReport<N::Leaf, R>>],
    reporter: &R,
    name: &str,
    actual: &N::Leaf,
) -> Result<(), CalcDiffError<N::TraverseError, RE>>
where
    N: NodeTraverse,
    N::Leaf: Clone,
    R: Reporter + Sync,
{
    for diff in diff {
        if let MayUnsupported::Ok(()) = diff
            .added(name, actual.clone(), reporter)
            .map_err(CalcDiffError::DiffError)?
        {
            return Ok(());
        }
    }
    Err(CalcDiffError::<N::TraverseError, RE>::NoDiffReportMatched)
}

fn run_deleted<N, R, RE>(
    diff: &[Box<dyn DiffReport<N::Leaf, R>>],
    reporter: &R,
    name: &str,
    expected: &N::Leaf,
) -> Result<(), CalcDiffError<N::TraverseError, RE>>
where
    N: NodeTraverse,
    N::Leaf: Clone,
    R: Reporter + Sync,
{
    for diff in diff {
        if let MayUnsupported::Ok(()) = diff
            .deleted(name, expected.clone(), reporter)
            .map_err(CalcDiffError::DiffError)?
        {
            return Ok(());
        }
    }
    Err(CalcDiffError::<N::TraverseError, RE>::NoDiffReportMatched)
}
