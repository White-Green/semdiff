use crate::{BinaryDiff, BinaryDiffReporter};
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::summary::SummaryReport;
use std::convert;

impl<W> DetailReporter<BinaryDiff, FileLeaf, SummaryReport<W>> for BinaryDiffReporter {
    type Error = convert::Infallible;

    fn report_unchanged(
        &self,
        _name: &str,
        _diff: BinaryDiff,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_unchanged();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        _name: &str,
        _diff: BinaryDiff,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_modified();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        _name: &str,
        _data: FileLeaf,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_added();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        _name: &str,
        _data: FileLeaf,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_deleted();
        Ok(MayUnsupported::Ok(()))
    }
}
