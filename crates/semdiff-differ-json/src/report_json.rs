use crate::{JsonDiff, JsonDiffReporter, is_json_mime, try_into_json};
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::json::JsonReport;
use semdiff_tree_fs::FileLeaf;
use std::convert;

const COMPARES_NAME: &str = "json";

impl<W> DetailReporter<JsonDiff, FileLeaf, JsonReport<W>> for JsonDiffReporter {
    type Error = convert::Infallible;

    fn report_unchanged(
        &self,
        name: &str,
        _diff: JsonDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.record_unchanged(name, COMPARES_NAME, ());
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        name: &str,
        _diff: JsonDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.record_modified(name, COMPARES_NAME, ());
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        name: &str,
        data: FileLeaf,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_json_mime(&data.kind) {
            return Ok(MayUnsupported::Unsupported);
        }
        if try_into_json(&data.content).is_none() {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.record_added(name, COMPARES_NAME, ());
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        name: &str,
        data: FileLeaf,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_json_mime(&data.kind) {
            return Ok(MayUnsupported::Unsupported);
        }
        if try_into_json(&data.content).is_none() {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.record_deleted(name, COMPARES_NAME, ());
        Ok(MayUnsupported::Ok(()))
    }
}
