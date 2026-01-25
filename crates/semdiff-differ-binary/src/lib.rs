use semdiff_core::{Diff, DiffCalculator};
use semdiff_tree_fs::FileLeaf;
use std::convert;

pub mod report_html;
pub mod report_json;
pub mod report_summary;

pub struct BinaryDiffReporter;

#[derive(Clone, Debug)]
pub struct BinaryDiff {
    equal: bool,
}

impl Diff for BinaryDiff {
    fn equal(&self) -> bool {
        self.equal
    }
}

#[derive(Default)]
pub struct BinaryDiffCalculator;

impl DiffCalculator<FileLeaf> for BinaryDiffCalculator {
    type Error = convert::Infallible;
    type Diff = BinaryDiff;

    fn available(&self, _expected: &FileLeaf, _actual: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn diff(&self, _name: &[String], expected: FileLeaf, actual: FileLeaf) -> Result<Self::Diff, Self::Error> {
        Ok(BinaryDiff {
            equal: <[u8]>::eq(&*expected.content, &*actual.content),
        })
    }
}
