use crate::{BinaryDiff, BinaryDiffReporter};
use semdiff_core::DetailReporter;
use semdiff_output::html::HtmlReport;
use semdiff_tree_fs::FileLeaf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BinaryDiffReportError {}

impl DetailReporter<BinaryDiff, FileLeaf, HtmlReport> for BinaryDiffReporter {
    type Error = BinaryDiffReportError;

    fn available(&self, _data: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn report_unchanged(&self, _name: &str, _diff: BinaryDiff, _reporter: &HtmlReport) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_modified(&self, _name: &str, _diff: BinaryDiff, _reporter: &HtmlReport) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_added(&self, _name: &str, _data: FileLeaf, _reporter: &HtmlReport) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_deleted(&self, _name: &str, _data: FileLeaf, _reporter: &HtmlReport) -> Result<(), Self::Error> {
        todo!()
    }
}
