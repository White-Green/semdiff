use crate::{AudioDiff, AudioDiffReporter, audio_extension};
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::summary::SummaryReport;
use std::convert;

impl<W> DetailReporter<AudioDiff, FileLeaf, SummaryReport<W>> for AudioDiffReporter {
    type Error = convert::Infallible;

    fn report_unchanged(
        &self,
        _name: &str,
        _diff: &AudioDiff,
        reporter: &SummaryReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.increment_unchanged();
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        _name: &str,
        _diff: &AudioDiff,
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
        if audio_extension(&data.kind).is_none()
            || self
                .spectrogram_analyzer
                .decode_audio(&data.kind, &data.content)
                .is_err()
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
        if audio_extension(&data.kind).is_none()
            || self
                .spectrogram_analyzer
                .decode_audio(&data.kind, &data.content)
                .is_err()
        {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.increment_deleted();
        Ok(MayUnsupported::Ok(()))
    }
}
