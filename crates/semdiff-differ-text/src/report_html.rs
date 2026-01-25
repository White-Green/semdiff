use crate::{TextDiff, TextDiffReporter};
use semdiff_core::DetailReporter;
use semdiff_output::html::HtmlReport;
use semdiff_tree_fs::FileLeaf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TextDiffReportError {}

impl DetailReporter<TextDiff, FileLeaf, HtmlReport> for TextDiffReporter {
    type Error = TextDiffReportError;

    fn available(&self, _data: &FileLeaf) -> Result<bool, Self::Error> {
        todo!()
    }

    fn report_unchanged(&self, _name: &[String], _diff: TextDiff, _reporter: &HtmlReport) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_modified(&self, _name: &[String], _diff: TextDiff, _reporter: &HtmlReport) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_added(&self, _name: &[String], _data: FileLeaf, _reporter: &HtmlReport) -> Result<(), Self::Error> {
        todo!()
    }

    fn report_deleted(&self, _name: &[String], _data: FileLeaf, _reporter: &HtmlReport) -> Result<(), Self::Error> {
        todo!()
    }
}
