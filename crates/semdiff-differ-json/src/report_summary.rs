use crate::{JsonDiff, JsonDiffReporter, is_json_mime, try_into_json};
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::summary::SummaryReport;
use std::convert;

impl<W> DetailReporter<JsonDiff, FileLeaf, SummaryReport<W>> for JsonDiffReporter {
    type Error = convert::Infallible;

    fn report_unchanged(
        &self,
        _name: &str,
        _diff: &JsonDiff,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_unchanged();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        _name: &str,
        _diff: &JsonDiff,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_modified();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        _name: &str,
        data: &FileLeaf,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_json_mime(&data.kind) {
            return Ok(MayUnsupported::Unsupported);
        }
        if try_into_json(&data.content).is_none() {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.increment_added();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        _name: &str,
        data: &FileLeaf,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_json_mime(&data.kind) {
            return Ok(MayUnsupported::Unsupported);
        }
        if try_into_json(&data.content).is_none() {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.increment_deleted();
        Ok(MayUnsupported::Ok(()))
    }
}
