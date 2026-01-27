use memmap2::Mmap;
use semdiff_core::{Diff, DiffCalculator};
use semdiff_tree_fs::FileLeaf;
use similar::{ChangeTag, TextDiffConfig};
use std::convert;

pub mod report_html;
pub mod report_json;
pub mod report_summary;

pub struct BinaryDiffReporter;

#[derive(Debug)]
pub struct BinaryDiff {
    equal: bool,
    expected: Mmap,
    actual: Mmap,
}

impl Diff for BinaryDiff {
    fn equal(&self) -> bool {
        self.equal
    }
}

impl BinaryDiff {
    fn expected(&self) -> &[u8] {
        &self.expected
    }

    fn actual(&self) -> &[u8] {
        &self.actual
    }

    fn changes(&self) -> similar::TextDiff<'_, '_, '_, [u8]> {
        TextDiffConfig::default()
            .algorithm(similar::Algorithm::Patience)
            .diff_chars(&*self.expected, &*self.actual)
    }

    fn stat<'a>(changes: &'a similar::TextDiff<'a, 'a, 'a, [u8]>) -> ChangeStat {
        changes
            .iter_all_changes()
            .fold(ChangeStat::default(), |stat, change| match change.tag() {
                ChangeTag::Delete => stat.deleted(),
                ChangeTag::Insert => stat.added(),
                ChangeTag::Equal => stat,
            })
    }
}

#[derive(Default)]
struct ChangeStat {
    added: usize,
    deleted: usize,
}

impl ChangeStat {
    fn added(self) -> ChangeStat {
        ChangeStat {
            added: self.added + 1,
            deleted: self.deleted,
        }
    }

    fn deleted(self) -> ChangeStat {
        ChangeStat {
            added: self.added,
            deleted: self.deleted + 1,
        }
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

    fn diff(&self, _name: &str, expected: FileLeaf, actual: FileLeaf) -> Result<Self::Diff, Self::Error> {
        Ok(BinaryDiff {
            equal: <[u8]>::eq(&*expected.content, &*actual.content),
            expected: expected.content,
            actual: actual.content,
        })
    }
}
