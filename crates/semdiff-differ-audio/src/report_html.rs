use crate::{AudioData, AudioDiff, AudioDiffReporter, audio_extension};
use askama::Template;
use image::{ImageError, ImageFormat, Rgba, RgbaImage};
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::html::{HtmlReport, HtmlReportError};
use std::io::Write;
use thiserror::Error;

const COMPARES_NAME: &str = "audio";

#[derive(Debug, Error)]
pub enum AudioDiffReportError {
    #[error("html report error: {0}")]
    HtmlReport(#[from] HtmlReportError),
    #[error("image encode error: {0}")]
    ImageEncode(#[from] ImageError),
}

#[derive(Template)]
#[template(path = "audio_preview.html")]
struct AudioPreviewTemplate {
    body: AudioPreviewBody,
}

#[derive(Clone)]
struct AudioPreviewImage {
    src: String,
    label: String,
    kind: String,
    width: u32,
    height: u32,
}

struct PreviewImageFile {
    path: String,
    width: u32,
    height: u32,
}

enum AudioPreviewBody {
    Modified {
        images: Vec<AudioPreviewImage>,
        audio_src: String,
    },
    Single {
        images: Vec<AudioPreviewImage>,
        audio_src: String,
    },
}

#[derive(Template)]
#[template(path = "audio_detail.html")]
struct AudioDetailTemplate {
    detail: AudioDetailBody,
}

#[derive(Clone)]
struct AudioDetailImage {
    uri: String,
    width: u32,
    height: u32,
}

#[derive(Clone)]
struct AudioDetailData {
    label: String,
    audio_src: String,
    waveforms: Vec<AudioDetailImage>,
    spectrograms: Vec<AudioDetailImage>,
    sample_rate: u32,
    channels: u16,
    duration_seconds: f32,
}

enum AudioDetailBody {
    Diff {
        expected: AudioDetailData,
        actual: AudioDetailData,
        spectrogram_diff: Vec<AudioDetailImage>,
    },
    Single {
        data: AudioDetailData,
    },
}

impl DetailReporter<AudioDiff, FileLeaf, HtmlReport> for AudioDiffReporter {
    type Error = AudioDiffReportError;

    fn report_unchanged(
        &self,
        name: &str,
        diff: &AudioDiff,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let expected = diff.expected();
        let Some(extension) = audio_extension(expected.mime()) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let audio_file = write_audio(reporter, name, "same", extension, expected.content())?;
        let waveform_files = write_channel_images(reporter, name, "same_waveform", expected.waveform())?;
        let spectrogram_files = write_channel_images(reporter, name, "same_spectrogram", expected.spectrogram())?;
        let detail_data = build_detail_data("same", expected, &audio_file, &waveform_files, &spectrogram_files);
        let preview_image = write_preview_image(reporter, name, "preview_waveform", expected.waveform())?;
        let preview_images = preview_image
            .as_ref()
            .map(|file| build_preview_images(reporter, std::slice::from_ref(file), "waveform"))
            .unwrap_or_default();
        let preview_html = AudioPreviewTemplate {
            body: AudioPreviewBody::Single {
                images: preview_images,
                audio_src: reporter.detail_asset_path(&audio_file),
            },
        };
        let detail_html = AudioDetailTemplate {
            detail: AudioDetailBody::Single { data: detail_data },
        };
        reporter.record_unchanged(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        name: &str,
        diff: &AudioDiff,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let expected = diff.expected();
        let actual = diff.actual();
        let Some(expected_ext) = audio_extension(expected.mime()) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let Some(actual_ext) = audio_extension(actual.mime()) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let expected_audio = write_audio(reporter, name, "expected", expected_ext, expected.content())?;
        let actual_audio = write_audio(reporter, name, "actual", actual_ext, actual.content())?;
        let expected_waveforms = write_channel_images(reporter, name, "expected_waveform", expected.waveform())?;
        let actual_waveforms = write_channel_images(reporter, name, "actual_waveform", actual.waveform())?;
        let expected_spectrograms =
            write_channel_images(reporter, name, "expected_spectrogram", expected.spectrogram())?;
        let actual_spectrograms = write_channel_images(reporter, name, "actual_spectrogram", actual.spectrogram())?;
        let spectrogram_diff_detail = if let Some(detail) = diff.diff_detail() {
            let spectrogram_diffs =
                write_channel_images(reporter, name, "spectrogram_diff", detail.spectrogram_diff())?;

            build_detail_images(&spectrogram_diffs, detail.spectrogram_diff())
        } else {
            Vec::new()
        };

        let (preview_image, preview_label) = if let Some(detail) = diff.diff_detail() {
            (
                write_preview_image(reporter, name, "preview_spectrogram_diff", detail.spectrogram_diff())?,
                "spectrogram diff",
            )
        } else {
            (
                write_preview_image(reporter, name, "preview_waveform", actual.waveform())?,
                "waveform",
            )
        };
        let preview_images = preview_image
            .as_ref()
            .map(|file| build_preview_images(reporter, std::slice::from_ref(file), preview_label))
            .unwrap_or_default();
        let preview_html = AudioPreviewTemplate {
            body: AudioPreviewBody::Modified {
                images: preview_images,
                audio_src: reporter.detail_asset_path(&actual_audio),
            },
        };
        let detail_html = AudioDetailTemplate {
            detail: AudioDetailBody::Diff {
                expected: build_detail_data(
                    "expected",
                    expected,
                    &expected_audio,
                    &expected_waveforms,
                    &expected_spectrograms,
                ),
                actual: build_detail_data("actual", actual, &actual_audio, &actual_waveforms, &actual_spectrograms),
                spectrogram_diff: spectrogram_diff_detail,
            },
        };
        reporter.record_modified(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        name: &str,
        data: &FileLeaf,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let Some(extension) = audio_extension(&data.kind) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let Ok(audio_data) = self.build_audio_data(data.kind.clone(), data.content.clone()) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let audio_file = write_audio(reporter, name, "added", extension, audio_data.content())?;
        let waveform_files = write_channel_images(reporter, name, "added_waveform", audio_data.waveform())?;
        let spectrogram_files = write_channel_images(reporter, name, "added_spectrogram", audio_data.spectrogram())?;
        let preview_image = write_preview_image(reporter, name, "preview_waveform", audio_data.waveform())?;
        let preview_images = preview_image
            .as_ref()
            .map(|file| build_preview_images(reporter, std::slice::from_ref(file), "waveform"))
            .unwrap_or_default();
        let preview_html = AudioPreviewTemplate {
            body: AudioPreviewBody::Single {
                images: preview_images,
                audio_src: reporter.detail_asset_path(&audio_file),
            },
        };
        let detail_html = AudioDetailTemplate {
            detail: AudioDetailBody::Single {
                data: build_detail_data("added", &audio_data, &audio_file, &waveform_files, &spectrogram_files),
            },
        };
        reporter.record_added(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        name: &str,
        data: &FileLeaf,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let Some(extension) = audio_extension(&data.kind) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let Ok(audio_data) = self.build_audio_data(data.kind.clone(), data.content.clone()) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let audio_file = write_audio(reporter, name, "deleted", extension, audio_data.content())?;
        let waveform_files = write_channel_images(reporter, name, "deleted_waveform", audio_data.waveform())?;
        let spectrogram_files = write_channel_images(reporter, name, "deleted_spectrogram", audio_data.spectrogram())?;
        let preview_image = write_preview_image(reporter, name, "preview_waveform", audio_data.waveform())?;
        let preview_images = preview_image
            .as_ref()
            .map(|file| build_preview_images(reporter, std::slice::from_ref(file), "waveform"))
            .unwrap_or_default();
        let preview_html = AudioPreviewTemplate {
            body: AudioPreviewBody::Single {
                images: preview_images,
                audio_src: reporter.detail_asset_path(&audio_file),
            },
        };
        let detail_html = AudioDetailTemplate {
            detail: AudioDetailBody::Single {
                data: build_detail_data("deleted", &audio_data, &audio_file, &waveform_files, &spectrogram_files),
            },
        };
        reporter.record_deleted(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }
}

fn build_detail_data(
    label: &str,
    data: &AudioData,
    audio_uri: &str,
    waveform_uris: &[String],
    spectrogram_uris: &[String],
) -> AudioDetailData {
    AudioDetailData {
        label: label.to_string(),
        audio_src: audio_uri.to_string(),
        waveforms: build_detail_images(waveform_uris, data.waveform()),
        spectrograms: build_detail_images(spectrogram_uris, data.spectrogram()),
        sample_rate: data.sample_rate(),
        channels: data.channels(),
        duration_seconds: data.duration_seconds(),
    }
}

fn build_preview_images(
    reporter: &HtmlReport,
    image_files: &[PreviewImageFile],
    label_prefix: &str,
) -> Vec<AudioPreviewImage> {
    let kind = label_prefix.replace(' ', "-");
    image_files
        .iter()
        .enumerate()
        .map(|(index, file)| AudioPreviewImage {
            src: reporter.detail_asset_path(&file.path),
            label: format!("{label_prefix} ch{}", index + 1),
            kind: kind.clone(),
            width: file.width,
            height: file.height,
        })
        .collect()
}

fn write_preview_image(
    reporter: &HtmlReport,
    name: &str,
    label: &str,
    images: &[RgbaImage],
) -> Result<Option<PreviewImageFile>, HtmlReportError> {
    let Some(merged) = merge_channel_images(images) else {
        return Ok(None);
    };
    let width = merged.width();
    let height = merged.height();
    let path = write_image(reporter, name, label, &merged)?;
    Ok(Some(PreviewImageFile { path, width, height }))
}

fn merge_channel_images(images: &[RgbaImage]) -> Option<RgbaImage> {
    let first = images.first()?;
    let width = first.width();
    let height = first.height();
    let mut merged = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 0]));
    for image in images {
        for y in 0..height {
            for x in 0..width {
                let pixel = image.get_pixel(x, y);
                let current = merged.get_pixel(x, y);
                merged.put_pixel(
                    x,
                    y,
                    Rgba([
                        current[0].max(pixel[0]),
                        current[1].max(pixel[1]),
                        current[2].max(pixel[2]),
                        current[3].max(pixel[3]),
                    ]),
                );
            }
        }
    }
    Some(merged)
}

fn build_detail_images(image_files: &[String], images: &[RgbaImage]) -> Vec<AudioDetailImage> {
    let count = image_files.len().min(images.len());
    (0..count)
        .map(|index| AudioDetailImage {
            uri: image_files[index].clone(),
            width: images[index].width(),
            height: images[index].height(),
        })
        .collect()
}

fn write_channel_images(
    reporter: &HtmlReport,
    name: &str,
    label_prefix: &str,
    images: &[RgbaImage],
) -> Result<Vec<String>, HtmlReportError> {
    let mut files = Vec::with_capacity(images.len());
    for (index, image) in images.iter().enumerate() {
        let label = format!("{label_prefix}_ch{}", index + 1);
        files.push(write_image(reporter, name, &label, image)?);
    }
    Ok(files)
}

fn write_image(reporter: &HtmlReport, name: &str, label: &str, image: &RgbaImage) -> Result<String, HtmlReportError> {
    reporter.write_detail_asset(name, label, "png", |w| match image.write_to(w, ImageFormat::Png) {
        Ok(()) => Ok(()),
        Err(ImageError::IoError(err)) => Err(err.into()),
        Err(err) => panic!("Unexpected error writing audio image: {}", err),
    })
}

fn write_audio(
    reporter: &HtmlReport,
    name: &str,
    label: &str,
    extension: &str,
    content: &[u8],
) -> Result<String, HtmlReportError> {
    reporter.write_detail_asset(name, label, extension, |w| {
        w.write_all(content)?;
        Ok(())
    })
}
