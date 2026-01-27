use crate::{TextDiff, TextDiffReporter, is_text_file};
use semdiff_core::DetailReporter;
use semdiff_output::summary::SummaryReport;
use semdiff_tree_fs::FileLeaf;
use std::convert;

impl<W> DetailReporter<TextDiff, FileLeaf, SummaryReport<W>> for TextDiffReporter {
    type Error = convert::Infallible;

    fn available(&self, data: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(is_text_file(&data.kind, &data.content))
    }

    fn report_unchanged(&self, _name: &str, _diff: TextDiff, reporter: &SummaryReport<W>) -> Result<(), Self::Error> {
        reporter.increment_unchanged();
        Ok(())
    }

    fn report_modified(&self, _name: &str, _diff: TextDiff, reporter: &SummaryReport<W>) -> Result<(), Self::Error> {
        reporter.increment_modified();
        Ok(())
    }

    fn report_added(&self, _name: &str, _data: FileLeaf, reporter: &SummaryReport<W>) -> Result<(), Self::Error> {
        reporter.increment_added();
        Ok(())
    }

    fn report_deleted(&self, _name: &str, _data: FileLeaf, reporter: &SummaryReport<W>) -> Result<(), Self::Error> {
        reporter.increment_deleted();
        Ok(())
    }
}
