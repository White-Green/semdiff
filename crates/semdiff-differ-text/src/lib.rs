use memmap2::Mmap;
use semdiff_core::{Diff, DiffCalculator};
use semdiff_tree_fs::FileLeaf;
use similar::TextDiffConfig;
use std::convert;

pub mod report_html;
pub mod report_json;
pub mod report_summary;

pub struct TextDiffReporter;

#[derive(Debug)]
pub struct TextDiff {
    equal: bool,
    expected: Mmap,
    actual: Mmap,
}

impl Diff for TextDiff {
    fn equal(&self) -> bool {
        self.equal
    }
}

impl TextDiff {
    pub fn expected(&self) -> &str {
        std::str::from_utf8(&self.expected).unwrap_or("")
    }

    pub fn actual(&self) -> &str {
        std::str::from_utf8(&self.actual).unwrap_or("")
    }

    pub fn diff(&self) -> similar::TextDiff<'_, '_, '_, [u8]> {
        TextDiffConfig::default()
            .algorithm(similar::Algorithm::Patience)
            .diff_lines(&*self.expected, &*self.actual)
    }
}

#[derive(Default)]
pub struct TextDiffCalculator;

impl DiffCalculator<FileLeaf> for TextDiffCalculator {
    type Error = convert::Infallible;
    type Diff = TextDiff;

    fn available(&self, expected: &FileLeaf, actual: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(str::from_utf8(&expected.content).is_ok() && str::from_utf8(&actual.content).is_ok())
    }

    fn diff(&self, _name: &str, expected: FileLeaf, actual: FileLeaf) -> Result<Self::Diff, Self::Error> {
        Ok(TextDiff {
            equal: <[u8]>::eq(&*expected.content, &*actual.content),
            expected: expected.content,
            actual: actual.content,
        })
    }
}
