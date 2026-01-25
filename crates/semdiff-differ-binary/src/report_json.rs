use crate::{BinaryDiff, BinaryDiffReporter};
use semdiff_core::DetailReporter;
use semdiff_output::json::JsonReport;
use semdiff_tree_fs::FileLeaf;
use std::convert;

const COMPARES_NAME: &'static str = "binary";

impl<W> DetailReporter<BinaryDiff, FileLeaf, JsonReport<W>> for BinaryDiffReporter {
    type Error = convert::Infallible;

    fn available(&self, _data: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn report_unchanged(&self, name: &str, _diff: BinaryDiff, reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        reporter.record_unchanged(name, COMPARES_NAME, []);
        Ok(())
    }

    fn report_modified(&self, name: &str, _diff: BinaryDiff, reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        reporter.record_modified(name, COMPARES_NAME, []);
        Ok(())
    }

    fn report_added(&self, name: &str, _data: FileLeaf, reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        reporter.record_added(name, COMPARES_NAME, []);
        Ok(())
    }

    fn report_deleted(&self, name: &str, _data: FileLeaf, reporter: &JsonReport<W>) -> Result<(), Self::Error> {
        reporter.record_deleted(name, COMPARES_NAME, []);
        Ok(())
    }
}
