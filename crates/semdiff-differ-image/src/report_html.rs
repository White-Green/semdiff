use crate::{ImageDiff, ImageDiffReporter, image_format};
use askama::Template;
use image::{ImageError, ImageFormat, RgbaImage};
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::html::{HtmlReport, HtmlReportError};
use thiserror::Error;

const COMPARES_NAME: &str = "image";

#[derive(Debug, Error)]
pub enum ImageDiffReportError {
    #[error("html report error: {0}")]
    HtmlReport(#[from] HtmlReportError),
    #[error("image decode error: {0}")]
    ImageDecode(#[from] ImageError),
}

#[derive(Template)]
#[template(path = "image_preview.html")]
struct ImagePreviewTemplate<'a> {
    body: ImagePreviewBody<'a>,
}

#[derive(Clone)]
struct ImagePreviewImage<'a> {
    src: &'a str,
    label: &'a str,
}

enum ImagePreviewBody<'a> {
    Modified { image: ImagePreviewImage<'a> },
    Single { image: ImagePreviewImage<'a> },
}

#[derive(Template)]
#[template(path = "image_detail.html")]
struct ImageDetailTemplate<'a> {
    detail: ImageDetailBody<'a>,
}

#[derive(Clone)]
struct ImageDetailImage<'a> {
    uri: &'a str,
    width: u32,
    height: u32,
}

enum ImageDetailBody<'a> {
    Diff {
        expected: ImageDetailImage<'a>,
        actual: ImageDetailImage<'a>,
        diff: ImageDetailImage<'a>,
    },
    Single {
        label: &'a str,
        image: ImageDetailImage<'a>,
    },
}

impl DetailReporter<ImageDiff, FileLeaf, HtmlReport> for ImageDiffReporter {
    type Error = ImageDiffReportError;

    fn report_unchanged(
        &self,
        name: &str,
        diff: ImageDiff,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let detail_image = write_image(reporter, name, "same", &diff.expected().data)?;
        let preview_html = ImagePreviewTemplate {
            body: ImagePreviewBody::Single {
                image: ImagePreviewImage {
                    src: &reporter.detail_asset_path(&detail_image),
                    label: "same",
                },
            },
        };
        let detail_html = ImageDetailTemplate {
            detail: ImageDetailBody::Single {
                label: "same",
                image: ImageDetailImage {
                    uri: &detail_image,
                    width: diff.expected().width,
                    height: diff.expected.height,
                },
            },
        };
        reporter.record_unchanged(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        name: &str,
        diff: ImageDiff,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let expected_image = write_image(reporter, name, "expected", &diff.expected().data)?;
        let actual_image = write_image(reporter, name, "actual", &diff.actual().data)?;
        let diff_image = diff.diff_image();
        let diff_image_file_name = write_image(reporter, name, "diff", diff_image)?;
        let diff_image = ImageDetailImage {
            uri: &diff_image_file_name,
            width: diff.diff_image.width(),
            height: diff.diff_image.height(),
        };
        let preview_image = ImagePreviewImage {
            src: &reporter.detail_asset_path(diff_image.uri),
            label: "diff",
        };
        let preview_html = ImagePreviewTemplate {
            body: ImagePreviewBody::Modified { image: preview_image },
        };
        let detail_html = ImageDetailTemplate {
            detail: ImageDetailBody::Diff {
                expected: ImageDetailImage {
                    uri: &expected_image,
                    width: diff.expected().width,
                    height: diff.expected().height,
                },
                actual: ImageDetailImage {
                    uri: &actual_image,
                    width: diff.actual().width,
                    height: diff.actual().height,
                },
                diff: diff_image,
            },
        };
        reporter.record_modified(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        name: &str,
        data: FileLeaf,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let Some(format) = image_format(&data.kind) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let image = image::load_from_memory_with_format(&data.content, format)?.into_rgba8();
        let width = image.width();
        let height = image.height();
        let image_path = write_image(reporter, name, "added", &image)?;
        let preview_html = ImagePreviewTemplate {
            body: ImagePreviewBody::Single {
                image: ImagePreviewImage {
                    src: &reporter.detail_asset_path(&image_path),
                    label: "added",
                },
            },
        };
        let detail_html = ImageDetailTemplate {
            detail: ImageDetailBody::Single {
                label: "added",
                image: ImageDetailImage {
                    uri: &image_path,
                    width,
                    height,
                },
            },
        };
        reporter.record_added(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        name: &str,
        data: FileLeaf,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let Some(format) = image_format(&data.kind) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let image = image::load_from_memory_with_format(&data.content, format)?.into_rgba8();
        let width = image.width();
        let height = image.height();
        let image_path = write_image(reporter, name, "deleted", &image)?;
        let preview_html = ImagePreviewTemplate {
            body: ImagePreviewBody::Single {
                image: ImagePreviewImage {
                    src: &reporter.detail_asset_path(&image_path),
                    label: "deleted",
                },
            },
        };
        let detail_html = ImageDetailTemplate {
            detail: ImageDetailBody::Single {
                label: "deleted",
                image: ImageDetailImage {
                    uri: &image_path,
                    width,
                    height,
                },
            },
        };
        reporter.record_deleted(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }
}

fn write_image(reporter: &HtmlReport, name: &str, label: &str, image: &RgbaImage) -> Result<String, HtmlReportError> {
    reporter.write_detail_asset(name, label, "png", |w| match image.write_to(w, ImageFormat::Png) {
        Ok(()) => Ok(()),
        Err(ImageError::IoError(err)) => Err(err.into()),
        Err(err) => panic!("Unexpected error writing diff image: {}", err),
    })
}
