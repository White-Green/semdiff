use crate::{BinaryDiff, BinaryDiffReporter};
use semdiff_core::DetailReporter;
use semdiff_output::summary::SummaryReport;
use semdiff_tree_fs::FileLeaf;
use std::convert;

impl<W> DetailReporter<BinaryDiff, FileLeaf, SummaryReport<W>> for BinaryDiffReporter {
    type Error = convert::Infallible;

    fn available(&self, _data: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn report_unchanged(&self, _name: &str, _diff: BinaryDiff, reporter: &SummaryReport<W>) -> Result<(), Self::Error> {
        reporter.increment_unchanged();
        Ok(())
    }

    fn report_modified(&self, _name: &str, _diff: BinaryDiff, reporter: &SummaryReport<W>) -> Result<(), Self::Error> {
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
