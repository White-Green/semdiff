use crate::{TextDiff, TextDiffReporter, is_text_file};
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::summary::SummaryReport;
use semdiff_tree_fs::FileLeaf;
use std::convert;

impl<W> DetailReporter<TextDiff, FileLeaf, SummaryReport<W>> for TextDiffReporter {
    type Error = convert::Infallible;

    fn report_unchanged(
        &self,
        _name: &str,
        _diff: TextDiff,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_unchanged();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        _name: &str,
        _diff: TextDiff,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_modified();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        _name: &str,
        data: FileLeaf,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_text_file(&data.kind, &data.content) {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.increment_added();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        _name: &str,
        data: FileLeaf,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_text_file(&data.kind, &data.content) {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.increment_deleted();
        Ok(MayUnsupported::Ok(()))
    }
}
