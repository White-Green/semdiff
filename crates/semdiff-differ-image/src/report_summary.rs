use crate::{ImageDiff, ImageDiffReporter, image_format};
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::summary::SummaryReport;
use std::convert;

impl<W> DetailReporter<ImageDiff, FileLeaf, SummaryReport<W>> for ImageDiffReporter {
    type Error = convert::Infallible;

    fn report_unchanged(
        &self,
        _name: &str,
        _diff: &ImageDiff,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_unchanged();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        _name: &str,
        _diff: &ImageDiff,
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
        if image_format(&data.kind)
            .is_none_or(|format| image::load_from_memory_with_format(&data.content, format).is_err())
        {
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
        if image_format(&data.kind)
            .is_none_or(|format| image::load_from_memory_with_format(&data.content, format).is_err())
        {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.increment_deleted();
        Ok(MayUnsupported::Ok(()))
    }
}
