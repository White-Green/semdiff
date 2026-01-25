use semdiff_core::{Diff, DiffCalculator};
use semdiff_tree_fs::FileLeaf;
use std::convert;

pub mod report_html;
pub mod report_json;
pub mod report_summary;

pub struct TextDiffReporter;

#[derive(Clone, Debug)]
pub struct TextDiff {
    equal: bool,
}

impl Diff for TextDiff {
    fn equal(&self) -> bool {
        self.equal
    }
}

#[derive(Default)]
pub struct TextDiffCalculator;

impl DiffCalculator<FileLeaf> for TextDiffCalculator {
    type Error = convert::Infallible;
    type Diff = TextDiff;

    fn available(&self, expected: &FileLeaf, actual: &FileLeaf) -> Result<bool, Self::Error> {
        let expected_is_text = expected.kind.type_() == mime::TEXT;
        let actual_is_text = actual.kind.type_() == mime::TEXT;
        Ok(expected_is_text && actual_is_text)
    }

    fn diff(&self, _name: &[String], expected: FileLeaf, actual: FileLeaf) -> Result<Self::Diff, Self::Error> {
        Ok(TextDiff {
            equal: <[u8]>::eq(&*expected.content, &*actual.content),
        })
    }
}
