use crate::{AudioDiff, AudioDiffReporter, audio_extension};
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::json::JsonReport;
use serde::Serialize;
use thiserror::Error;

const COMPARES_NAME: &str = "audio";

#[derive(Debug, Error)]
pub enum AudioJsonReportError {
    #[error("audio decode error: {0}")]
    AudioDecode(#[from] crate::AudioDecodeError),
}

impl<W> DetailReporter<AudioDiff, FileLeaf, JsonReport<W>> for AudioDiffReporter {
    type Error = AudioJsonReportError;

    fn report_unchanged(
        &self,
        name: &str,
        _diff: &AudioDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.record_unchanged(name, COMPARES_NAME, ());
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        name: &str,
        diff: &AudioDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let (spectrogram_diff_rate, shift_samples, lufs_diff_db) = if let Some(detail) = diff.diff_detail() {
            let stat = detail.stat();
            (
                Some(stat.spectrogram_diff_rate),
                Some(stat.shift_samples),
                Some(stat.lufs_diff_db),
            )
        } else {
            (None, None, None)
        };
        let report = ModifiedReport {
            status: diff.status().as_str().to_string(),
            expected_sample_rate: diff.expected().sample_rate(),
            expected_channels: diff.expected().channels(),
            expected_duration_seconds: diff.expected().duration_seconds(),
            actual_sample_rate: diff.actual().sample_rate(),
            actual_channels: diff.actual().channels(),
            actual_duration_seconds: diff.actual().duration_seconds(),
            spectrogram_diff_rate,
            shift_samples,
            lufs_diff_db,
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
        if audio_extension(&data.kind).is_none() {
            return Ok(MayUnsupported::Unsupported);
        }
        let Ok(decoded) = self.spectrogram_analyzer.decode_audio(&data.kind, &data.content) else {
            return Ok(MayUnsupported::Unsupported);
        };
        reporter.record_added(
            name,
            COMPARES_NAME,
            SingleReport {
                sample_rate: decoded.sample_rate,
                channels: decoded.channels,
                duration_seconds: decoded.duration_seconds,
            },
        );
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        name: &str,
        data: &FileLeaf,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if audio_extension(&data.kind).is_none() {
            return Ok(MayUnsupported::Unsupported);
        }
        let Ok(decoded) = self.spectrogram_analyzer.decode_audio(&data.kind, &data.content) else {
            return Ok(MayUnsupported::Unsupported);
        };
        reporter.record_deleted(
            name,
            COMPARES_NAME,
            SingleReport {
                sample_rate: decoded.sample_rate,
                channels: decoded.channels,
                duration_seconds: decoded.duration_seconds,
            },
        );
        Ok(MayUnsupported::Ok(()))
    }
}

#[derive(Serialize)]
struct ModifiedReport {
    status: String,
    expected_sample_rate: u32,
    expected_channels: u16,
    expected_duration_seconds: f32,
    actual_sample_rate: u32,
    actual_channels: u16,
    actual_duration_seconds: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    spectrogram_diff_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shift_samples: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lufs_diff_db: Option<f32>,
}

#[derive(Serialize)]
struct SingleReport {
    sample_rate: u32,
    channels: u16,
    duration_seconds: f32,
}
