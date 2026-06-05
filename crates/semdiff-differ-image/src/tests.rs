use super::*;
use image::{Rgba, RgbaImage};
use semdiff_core::fs::FileLeaf;
use std::io::Cursor;
use std::sync::Arc;

#[test]
fn compare_counts_diff_pixels() {
    let calculator = ImageDiffCalculator::new(0.0, 0.0);
    let mut expected = RgbaImage::new(2, 2);
    let mut actual = RgbaImage::new(2, 2);
    expected.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
    actual.put_pixel(0, 0, Rgba([1, 0, 0, 0]));
    let (stat, diff_image) = calculator.compare(&expected, &actual);
    assert_eq!(stat.diff_pixels, 1);
    assert_eq!(stat.total_pixels, 4);
    assert!((stat.diff_ratio - 0.25).abs() < 1e-6);
    assert_eq!(diff_image.dimensions(), (2, 2));
}

#[test]
fn compare_counts_diff_pixels_with_alpha() {
    let calculator = ImageDiffCalculator::new(0.0, 0.0);
    let mut expected = RgbaImage::new(1, 1);
    let mut actual = RgbaImage::new(1, 1);
    expected.put_pixel(0, 0, Rgba([10, 20, 30, 0]));
    actual.put_pixel(0, 0, Rgba([10, 20, 30, 10]));
    let (stat, _diff_image) = calculator.compare(&expected, &actual);
    assert_eq!(stat.diff_pixels, 1);
    assert_eq!(stat.total_pixels, 1);
    assert!((stat.diff_ratio - 1.0).abs() < 1e-6);
}

#[test]
fn max_distance_option_treats_close_pixels_as_equal() {
    let mut expected = RgbaImage::new(1, 1);
    let mut actual = RgbaImage::new(1, 1);
    expected.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
    actual.put_pixel(0, 0, Rgba([11, 20, 30, 255]));

    let (strict_stat, _strict_diff_image) = ImageDiffCalculator::new(0.0, 0.0).compare(&expected, &actual);
    let (loose_stat, _loose_diff_image) = ImageDiffCalculator::new(1.0, 0.0).compare(&expected, &actual);

    assert_eq!(strict_stat.diff_pixels, 1);
    assert_eq!(loose_stat.diff_pixels, 0);
}

#[test]
fn max_diff_ratio_option_changes_equal_result() {
    let mut expected = RgbaImage::new(2, 2);
    let mut actual = RgbaImage::new(2, 2);
    expected.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
    actual.put_pixel(0, 0, Rgba([255, 255, 255, 255]));

    let strict = diff_images(ImageDiffCalculator::new(0.0, 0.0), &expected, &actual);
    let loose = diff_images(ImageDiffCalculator::new(0.0, 0.25), &expected, &actual);

    assert_eq!(strict.diff_stat().diff_pixels, 1);
    assert!(!strict.equal());
    assert_eq!(loose.diff_stat().diff_pixels, 1);
    assert!(loose.equal());
}

fn diff_images(calculator: ImageDiffCalculator, expected: &RgbaImage, actual: &RgbaImage) -> ImageDiff {
    let diff = calculator
        .diff(
            "image.png",
            file_leaf("expected.png", expected),
            file_leaf("actual.png", actual),
        )
        .unwrap();
    match diff {
        MayUnsupported::Ok(diff) => diff,
        MayUnsupported::Unsupported => panic!("PNG image should be supported"),
    }
}

fn file_leaf(name: &str, image: &RgbaImage) -> FileLeaf {
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    FileLeaf {
        name: name.to_owned(),
        kind: "image/png".parse().unwrap(),
        content: mmap_from_bytes(bytes.get_ref()),
    }
}

fn mmap_from_bytes(bytes: &[u8]) -> Arc<memmap2::Mmap> {
    let mut mmap = memmap2::MmapMut::map_anon(bytes.len()).unwrap();
    mmap.copy_from_slice(bytes);
    Arc::new(mmap.make_read_only().unwrap())
}
