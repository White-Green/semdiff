use image::{ImageError, ImageFormat, Rgba, RgbaImage};
use mime::Mime;
use semdiff_core::{Diff, DiffCalculator, MayUnsupported};
use semdiff_tree_fs::FileLeaf;
use thiserror::Error;

pub mod report_html;
pub mod report_json;
pub mod report_summary;

#[cfg(test)]
mod tests;

pub struct ImageDiffReporter;

#[derive(Debug)]
pub struct ImageDiff {
    equal: bool,
    expected: ImageData,
    actual: ImageData,
    diff_stat: ImageDiffStat,
    diff_image: RgbaImage,
}

#[derive(Debug, Clone)]
pub struct ImageData {
    pub mime: Mime,
    pub width: u32,
    pub height: u32,
    pub data: RgbaImage,
}

#[derive(Debug)]
pub struct ImageDiffStat {
    pub diff_pixels: u64,
}

impl Diff for ImageDiff {
    fn equal(&self) -> bool {
        self.equal
    }
}

impl ImageDiff {
    pub fn expected(&self) -> &ImageData {
        &self.expected
    }

    pub fn actual(&self) -> &ImageData {
        &self.actual
    }

    pub fn diff_stat(&self) -> &ImageDiffStat {
        &self.diff_stat
    }

    pub fn diff_image(&self) -> &RgbaImage {
        &self.diff_image
    }
}

#[derive(Debug, Error)]
pub enum ImageDiffError {
    #[error("image error: {0}")]
    Image(#[from] ImageError),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ImageDiffCalculator {
    max_channel_delta: u32,
}

impl ImageDiffCalculator {
    pub fn new(max_channel_delta: u32) -> Self {
        Self { max_channel_delta }
    }

    #[inline(always)]
    fn pixel_diff(&self, expected: Rgba<u8>, actual: Rgba<u8>) -> bool {
        expected
            .0
            .iter()
            .zip(actual.0.iter())
            .map(|(&e, &a)| e.abs_diff(a) as u32)
            .sum::<u32>()
            > self.max_channel_delta
    }

    fn compare(&self, expected: &RgbaImage, actual: &RgbaImage) -> (ImageDiffStat, RgbaImage) {
        let (expected_width, expected_height) = expected.dimensions();
        let (actual_width, actual_height) = actual.dimensions();
        let mut diff_pixels = 0u64;
        let mut diff_image = RgbaImage::new(expected_width.max(actual_width), expected_height.max(actual_height));
        const DIFF_PIXEL_COLOR: Rgba<u8> = Rgba([255, 0, 0, 180]);
        const SAME_PIXEL_COLOR: Rgba<u8> = Rgba([0, 0, 0, 0]);
        for y in 0..expected_height.min(actual_height) {
            for x in 0..expected_width.min(actual_width) {
                let expected_pixel = *expected.get_pixel(x, y);
                let actual_pixel = *actual.get_pixel(x, y);
                let diff_pixel = if self.pixel_diff(expected_pixel, actual_pixel) {
                    diff_pixels += 1;
                    DIFF_PIXEL_COLOR
                } else {
                    SAME_PIXEL_COLOR
                };
                diff_image.put_pixel(x, y, diff_pixel);
            }
        }
        for y in 0..expected_height.min(actual_height) {
            for x in expected_width.min(actual_width)..expected_width.max(actual_width) {
                diff_image.put_pixel(x, y, DIFF_PIXEL_COLOR);
            }
        }
        for y in expected_height.min(actual_height)..expected_height.max(actual_height) {
            for x in 0..expected_width.min(actual_width) {
                diff_image.put_pixel(x, y, DIFF_PIXEL_COLOR);
            }
        }
        (ImageDiffStat { diff_pixels }, diff_image)
    }
}

impl DiffCalculator<FileLeaf> for ImageDiffCalculator {
    type Error = ImageDiffError;
    type Diff = ImageDiff;

    fn diff(
        &self,
        _name: &str,
        expected: FileLeaf,
        actual: FileLeaf,
    ) -> Result<MayUnsupported<Self::Diff>, Self::Error> {
        let (Some(expected_format), Some(actual_format)) = (image_format(&expected.kind), image_format(&actual.kind))
        else {
            return Ok(MayUnsupported::Unsupported);
        };
        let expected_image = match image::load_from_memory_with_format(&expected.content, expected_format) {
            Ok(image) => image,
            Err(_) => return Ok(MayUnsupported::Unsupported),
        };
        let actual_image = match image::load_from_memory_with_format(&actual.content, actual_format) {
            Ok(image) => image,
            Err(_) => return Ok(MayUnsupported::Unsupported),
        };
        let expected_image = expected_image.into_rgba8();
        let actual_image = actual_image.into_rgba8();
        let (diff_stat, diff_image) = self.compare(&expected_image, &actual_image);
        let expected_data = ImageData {
            mime: expected.kind,
            width: expected_image.width(),
            height: expected_image.height(),
            data: expected_image,
        };
        let actual_data = ImageData {
            mime: actual.kind,
            width: actual_image.width(),
            height: actual_image.height(),
            data: actual_image,
        };
        let equal = diff_stat.diff_pixels == 0
            && expected_data.width == actual_data.width
            && expected_data.height == actual_data.height;
        Ok(MayUnsupported::Ok(ImageDiff {
            equal,
            expected: expected_data,
            actual: actual_data,
            diff_stat,
            diff_image,
        }))
    }
}

fn image_format(mime: &Mime) -> Option<ImageFormat> {
    if mime.type_() != mime::IMAGE {
        return None;
    }
    let format = match mime.subtype().as_str() {
        "png" => ImageFormat::Png,
        "bmp" => ImageFormat::Bmp,
        "gif" => ImageFormat::Gif,
        "jpeg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::WebP,
        "avif" => ImageFormat::Avif,
        _ => return None,
    };
    Some(format)
}
