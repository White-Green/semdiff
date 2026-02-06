use crate::{BinaryDiff, BinaryDiffReporter};
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::json::JsonReport;
use serde::Serialize;
use std::convert;

const COMPARES_NAME: &str = "binary";

impl<W> DetailReporter<BinaryDiff, FileLeaf, JsonReport<W>> for BinaryDiffReporter {
    type Error = convert::Infallible;

    fn report_unchanged(
        &self,
        name: &str,
        diff: &BinaryDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let report = SingleReport {
            size: diff.expected().len(),
        };
        reporter.record_unchanged(name, COMPARES_NAME, report);
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        name: &str,
        diff: &BinaryDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let stat = BinaryDiff::stat(&diff.changes());
        let expected_size = diff.expected.len();
        let actual_size = diff.actual.len();
        let report = ModifiedReport {
            expected_size,
            actual_size,
            added: stat.added,
            deleted: stat.deleted,
        };
        reporter.record_modified(name, COMPARES_NAME, report);
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        name: &str,
        data: &FileLeaf,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let report = SingleReport {
            size: data.content.len(),
        };
        reporter.record_added(name, COMPARES_NAME, report);
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        name: &str,
        data: &FileLeaf,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let report = SingleReport {
            size: data.content.len(),
        };
        reporter.record_deleted(name, COMPARES_NAME, report);
        Ok(MayUnsupported::Ok(()))
    }
}

#[derive(Serialize)]
struct ModifiedReport {
    expected_size: usize,
    actual_size: usize,
    added: usize,
    deleted: usize,
}

#[derive(Serialize)]
struct SingleReport {
    size: usize,
}
