use crate::{BinaryDiff, BinaryDiffReporter};
use semdiff_core::DetailReporter;
use semdiff_output::json::JsonReport;
use semdiff_tree_fs::FileLeaf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BinaryDiffReportError {}

impl<W> DetailReporter<BinaryDiff, FileLeaf, JsonReport<W>> for BinaryDiffReporter {
    type Error = BinaryDiffReportError;

    fn available(&self, _data: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn report_unchanged(&self, _name: &[String], _diff: BinaryDiff, _reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_modified(&self, _name: &[String], _diff: BinaryDiff, _reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_added(&self, _name: &[String], _data: FileLeaf, _reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_deleted(&self, _name: &[String], _data: FileLeaf, _reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        todo!()
    }
}
