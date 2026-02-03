use super::*;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct TestLeaf {
    name: String,
    value: i32,
}

impl TestLeaf {
    fn new(name: &str, value: i32) -> Self {
        Self {
            name: name.to_owned(),
            value,
        }
    }
}

impl LeafTraverse for TestLeaf {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
struct TestNode {
    name: String,
    children: Vec<TestChild>,
}

#[derive(Debug, Clone)]
enum TestChild {
    Node(TestNode),
    Leaf(TestLeaf),
}

impl TestNode {
    fn new(name: &str, children: Vec<TestChild>) -> Self {
        Self {
            name: name.to_owned(),
            children,
        }
    }
}

impl NodeTraverse for TestNode {
    type Leaf = TestLeaf;
    type TraverseError = Infallible;

    fn name(&self) -> &str {
        &self.name
    }

    fn children(
        &mut self,
    ) -> Result<impl Iterator<Item = Result<TraversalNode<Self, Self::Leaf>, Self::TraverseError>>, Self::TraverseError>
    {
        let mut children = Vec::with_capacity(self.children.len());
        for child in &self.children {
            match child {
                TestChild::Node(node) => children.push(TraversalNode::Node(node.clone())),
                TestChild::Leaf(leaf) => children.push(TraversalNode::Leaf(leaf.clone())),
            }
        }
        Ok(children.into_iter().map(Ok))
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum ReportEvent {
    Start,
    Finish,
    Unchanged(String),
    Modified(String),
    Added(String),
    Deleted(String),
}

#[derive(Clone, Default)]
struct TestReporter {
    events: Arc<Mutex<Vec<ReportEvent>>>,
}

impl Reporter for TestReporter {
    type Error = Infallible;

    fn start(&mut self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push(ReportEvent::Start);
        Ok(())
    }

    fn finish(self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push(ReportEvent::Finish);
        Ok(())
    }
}

#[derive(Clone, Default)]
struct TestDetailReporter {
    events: Arc<Mutex<Vec<ReportEvent>>>,
}

impl DetailReporter<TestDiff, TestLeaf, TestReporter> for TestDetailReporter {
    type Error = Infallible;

    fn report_unchanged(
        &self,
        name: &str,
        _diff: TestDiff,
        _reporter: &TestReporter,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        self.events
            .lock()
            .unwrap()
            .push(ReportEvent::Unchanged(name.to_owned()));
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        name: &str,
        _diff: TestDiff,
        _reporter: &TestReporter,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        self.events.lock().unwrap().push(ReportEvent::Modified(name.to_owned()));
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        name: &str,
        _data: TestLeaf,
        _reporter: &TestReporter,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        self.events.lock().unwrap().push(ReportEvent::Added(name.to_owned()));
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        name: &str,
        _data: TestLeaf,
        _reporter: &TestReporter,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        self.events.lock().unwrap().push(ReportEvent::Deleted(name.to_owned()));
        Ok(MayUnsupported::Ok(()))
    }
}

#[derive(Debug)]
struct TestDiff {
    equal: bool,
}

impl Diff for TestDiff {
    fn equal(&self) -> bool {
        self.equal
    }
}

#[derive(Debug)]
struct TestDiffCalculator;

impl DiffCalculator<TestLeaf> for TestDiffCalculator {
    type Error = Infallible;
    type Diff = TestDiff;

    fn diff(
        &self,
        _name: &str,
        expected: TestLeaf,
        actual: TestLeaf,
    ) -> Result<MayUnsupported<Self::Diff>, Self::Error> {
        Ok(MayUnsupported::Ok(TestDiff {
            equal: expected.value == actual.value,
        }))
    }
}

#[test]
fn traversal_node_ordering_and_eq() {
    let node_a = TraversalNode::Node(TestNode::new("a", vec![]));
    let node_a2 = TraversalNode::Node(TestNode::new("a", vec![]));
    let node_b = TraversalNode::Node(TestNode::new("b", vec![]));
    let leaf_a = TraversalNode::Leaf(TestLeaf::new("a", 1));
    let leaf_b = TraversalNode::Leaf(TestLeaf::new("b", 1));

    assert_eq!(node_a, node_a2);
    assert_ne!(node_a, node_b);
    assert_ne!(node_a, leaf_a);
    assert!(node_a < leaf_a);
    assert!(leaf_a < leaf_b);
}

#[test]
fn calc_diff_reports_expected_events() {
    let expected = TestNode::new(
        "root",
        vec![TestChild::Node(TestNode::new(
            "dir",
            vec![
                TestChild::Leaf(TestLeaf::new("same", 1)),
                TestChild::Leaf(TestLeaf::new("changed", 1)),
                TestChild::Leaf(TestLeaf::new("deleted", 1)),
            ],
        ))],
    );
    let actual = TestNode::new(
        "root",
        vec![TestChild::Node(TestNode::new(
            "dir",
            vec![
                TestChild::Leaf(TestLeaf::new("same", 1)),
                TestChild::Leaf(TestLeaf::new("changed", 2)),
                TestChild::Leaf(TestLeaf::new("added", 3)),
            ],
        ))],
    );

    let events = Arc::new(Mutex::new(Vec::new()));
    let reporter = TestReporter {
        events: Arc::clone(&events),
    };
    let diff = DiffAndReport::new(
        TestDiffCalculator,
        TestDetailReporter {
            events: Arc::clone(&events),
        },
    );

    let result = calc_diff(expected, actual, &[Box::new(diff)], reporter);
    assert!(result.is_ok());

    let events = events.lock().unwrap().clone();
    assert_eq!(
        events,
        vec![
            ReportEvent::Start,
            ReportEvent::Added("dir/added".to_owned()),
            ReportEvent::Modified("dir/changed".to_owned()),
            ReportEvent::Deleted("dir/deleted".to_owned()),
            ReportEvent::Unchanged("dir/same".to_owned()),
            ReportEvent::Finish,
        ]
    );
}

#[test]
fn calc_diff_reports_expected_events_with_mixed_children_order() {
    let expected = TestNode::new(
        "root",
        vec![
            TestChild::Leaf(TestLeaf::new("root-leaf", 1)),
            TestChild::Node(TestNode::new(
                "dir",
                vec![
                    TestChild::Leaf(TestLeaf::new("same", 1)),
                    TestChild::Leaf(TestLeaf::new("changed", 1)),
                ],
            )),
            TestChild::Leaf(TestLeaf::new("removed", 1)),
        ],
    );
    let actual = TestNode::new(
        "root",
        vec![
            TestChild::Node(TestNode::new(
                "dir",
                vec![
                    TestChild::Leaf(TestLeaf::new("changed", 2)),
                    TestChild::Leaf(TestLeaf::new("same", 1)),
                    TestChild::Leaf(TestLeaf::new("added", 3)),
                ],
            )),
            TestChild::Leaf(TestLeaf::new("root-leaf", 1)),
            TestChild::Leaf(TestLeaf::new("added-root", 5)),
        ],
    );

    let events = Arc::new(Mutex::new(Vec::new()));
    let reporter = TestReporter {
        events: Arc::clone(&events),
    };
    let diff = DiffAndReport::new(
        TestDiffCalculator,
        TestDetailReporter {
            events: Arc::clone(&events),
        },
    );

    let result = calc_diff(expected, actual, &[Box::new(diff)], reporter);
    assert!(result.is_ok());

    let events = events.lock().unwrap().clone();
    assert_eq!(
        events,
        vec![
            ReportEvent::Start,
            ReportEvent::Added("dir/added".to_owned()),
            ReportEvent::Modified("dir/changed".to_owned()),
            ReportEvent::Unchanged("dir/same".to_owned()),
            ReportEvent::Added("added-root".to_owned()),
            ReportEvent::Deleted("removed".to_owned()),
            ReportEvent::Unchanged("root-leaf".to_owned()),
            ReportEvent::Finish,
        ]
    );
}

#[test]
fn calc_diff_deletes_missing_node_children_in_mixed_order() {
    let expected = TestNode::new(
        "root",
        vec![
            TestChild::Leaf(TestLeaf::new("root-leaf", 1)),
            TestChild::Node(TestNode::new(
                "dir",
                vec![
                    TestChild::Leaf(TestLeaf::new("a", 1)),
                    TestChild::Node(TestNode::new("sub", vec![TestChild::Leaf(TestLeaf::new("b", 1))])),
                ],
            )),
        ],
    );
    let actual = TestNode::new("root", vec![TestChild::Leaf(TestLeaf::new("root-leaf", 1))]);

    let events = Arc::new(Mutex::new(Vec::new()));
    let reporter = TestReporter {
        events: Arc::clone(&events),
    };
    let diff = DiffAndReport::new(
        TestDiffCalculator,
        TestDetailReporter {
            events: Arc::clone(&events),
        },
    );

    let result = calc_diff(expected, actual, &[Box::new(diff)], reporter);
    assert!(result.is_ok());

    let events = events.lock().unwrap().clone();
    assert_eq!(
        events,
        vec![
            ReportEvent::Start,
            ReportEvent::Deleted("dir/a".to_owned()),
            ReportEvent::Deleted("dir/sub/b".to_owned()),
            ReportEvent::Unchanged("root-leaf".to_owned()),
            ReportEvent::Finish,
        ]
    );
}
