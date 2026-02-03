use super::*;
use image::{Rgba, RgbaImage};

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
