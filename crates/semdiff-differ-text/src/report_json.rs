use crate::{TextDiff, TextDiffReporter};
use semdiff_core::DetailReporter;
use semdiff_output::json::JsonReport;
use semdiff_tree_fs::FileLeaf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TextDiffReportError {}

impl<W> DetailReporter<TextDiff, FileLeaf, JsonReport<W>> for TextDiffReporter {
    type Error = TextDiffReportError;

    fn available(&self, _data: &FileLeaf) -> Result<bool, Self::Error> {
        todo!()
    }

    fn report_unchanged(&self, _name: &[String], _diff: TextDiff, _reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_modified(&self, _name: &[String], _diff: TextDiff, _reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_added(&self, _name: &[String], _data: FileLeaf, _reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_deleted(&self, _name: &[String], _data: FileLeaf, _reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        todo!()
    }
}
