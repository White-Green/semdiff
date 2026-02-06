use crate::{ImageDiff, ImageDiffReporter, image_format};
use image::ImageError;
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::json::JsonReport;
use serde::Serialize;
use thiserror::Error;

const COMPARES_NAME: &str = "image";

#[derive(Debug, Error)]
pub enum ImageJsonReportError {
    #[error("image decode error: {0}")]
    ImageDecode(#[from] ImageError),
}

impl<W> DetailReporter<ImageDiff, FileLeaf, JsonReport<W>> for ImageDiffReporter {
    type Error = ImageJsonReportError;

    fn report_unchanged(
        &self,
        name: &str,
        _diff: &ImageDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.record_unchanged(name, COMPARES_NAME, ());
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        name: &str,
        diff: &ImageDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let report = ModifiedReport {
            expected_width: diff.expected().width,
            expected_height: diff.expected().height,
            actual_width: diff.actual().width,
            actual_height: diff.actual().height,
            diff_pixels: diff.diff_stat().diff_pixels,
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
        let Some(format) = image_format(&data.kind) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let image = image::load_from_memory_with_format(&data.content, format)?;
        let report = SingleReport {
            width: image.width(),
            height: image.height(),
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
        let Some(format) = image_format(&data.kind) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let image = image::load_from_memory_with_format(&data.content, format)?;
        let report = SingleReport {
            width: image.width(),
            height: image.height(),
        };
        reporter.record_deleted(name, COMPARES_NAME, report);
        Ok(MayUnsupported::Ok(()))
    }
}

#[derive(Serialize)]
struct ModifiedReport {
    expected_width: u32,
    expected_height: u32,
    actual_width: u32,
    actual_height: u32,
    diff_pixels: u64,
}

#[derive(Serialize)]
struct SingleReport {
    width: u32,
    height: u32,
}
