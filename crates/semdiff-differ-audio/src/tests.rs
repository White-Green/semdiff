use super::*;

#[test]
fn spectrogram_log_bin_range_covers_full_range() {
    let first = spectrogram_log_bin_range(0);
    let last = spectrogram_log_bin_range(SPECTROGRAM_HEIGHT - 1);
    assert_eq!(first.start, 0);
    assert!(first.end > first.start);
    assert_eq!(last.end, SPECTROGRAM_DATA_HEIGHT);
    assert!(last.start < last.end);

    let mut prev = first;
    for y in 1..SPECTROGRAM_HEIGHT {
        let current = spectrogram_log_bin_range(y);
        assert!(prev.start <= current.start);
        assert!(prev.end <= current.end);
        prev = current;
    }
}

#[test]
fn diff_decoded_returns_incomparable_on_mismatched_format() {
    let calculator = AudioDiffCalculator::default();
    let expected = AudioDecoded {
        sample_rate: 44_100,
        channels: 1,
        duration_seconds: 0.0,
        samples: vec![vec![0.0]],
        spectrograms: vec![Vec::new()],
    };
    let actual = AudioDecoded {
        sample_rate: 48_000,
        channels: 1,
        duration_seconds: 0.0,
        samples: vec![vec![0.0]],
        spectrograms: vec![Vec::new()],
    };
    let status = calculator.diff_decoded(&expected, &actual);
    assert!(matches!(status, AudioDiffStatus::Incomparable));
}

#[test]
fn test_align_samples_no_shift() {
    let expected = vec![vec![0.0, 1.0, 2.0, 3.0]];
    let actual = vec![vec![0.0, 1.0, 2.0, 3.0]];
    let max_shift = 2;
    let (aligned_exp, aligned_act, shift) = align_samples(expected.clone(), actual.clone(), max_shift);
    assert_eq!(shift, 0);
    assert_eq!(aligned_exp, expected);
    assert_eq!(aligned_act, actual);
}

#[test]
fn test_align_samples_all_zeros() {
    let expected = vec![vec![0.0, 0.0, 0.0, 0.0]];
    let actual = vec![vec![0.0, 0.0, 0.0, 0.0]];
    let max_shift = 2;
    let (aligned_exp, aligned_act, shift) = align_samples(expected.clone(), actual.clone(), max_shift);
    assert_eq!(shift, 0);
    assert_eq!(aligned_exp, expected);
    assert_eq!(aligned_act, actual);
}

#[test]
fn test_align_samples_positive_shift() {
    let expected = vec![vec![1.0, 2.0, 3.0, 4.0]];
    let actual = vec![vec![0.0, 1.0, 2.0, 3.0, 4.0]];
    let max_shift = 2;
    let (aligned_exp, aligned_act, shift) = align_samples(expected.clone(), actual.clone(), max_shift);
    assert_eq!(shift, 1);
    assert_eq!(aligned_exp[0], vec![1.0, 2.0, 3.0, 4.0]);
    assert_eq!(aligned_act[0], vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn test_align_samples_negative_shift() {
    let expected = vec![vec![0.0, 1.0, 2.0, 3.0, 4.0]];
    let actual = vec![vec![1.0, 2.0, 3.0, 4.0]];
    let max_shift = 2;
    let (aligned_exp, aligned_act, shift) = align_samples(expected.clone(), actual.clone(), max_shift);
    assert_eq!(shift, -1);
    assert_eq!(aligned_exp[0], vec![1.0, 2.0, 3.0, 4.0]);
    assert_eq!(aligned_act[0], vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn test_align_samples_max_shift_limit() {
    let expected = vec![vec![1.0, 2.0, 3.0]];
    let actual = vec![vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0]];
    let max_shift = 2;
    let (aligned_exp, aligned_act, shift) = align_samples(expected.clone(), actual.clone(), max_shift);
    assert_eq!(shift, 2);
    assert_eq!(aligned_exp[0], vec![1.0, 2.0, 3.0]);
    assert_eq!(aligned_act[0], vec![0.0, 1.0, 2.0]);
}

#[test]
fn test_align_samples_empty_input() {
    let expected: Vec<Vec<f32>> = vec![vec![]];
    let actual: Vec<Vec<f32>> = vec![vec![]];
    let max_shift = 2;
    let (aligned_exp, aligned_act, shift) = align_samples(expected.clone(), actual.clone(), max_shift);
    assert_eq!(shift, 0);
    assert_eq!(aligned_exp[0], Vec::<f32>::new());
    assert_eq!(aligned_act[0], Vec::<f32>::new());
}

#[test]
fn test_align_samples_zero_max_shift() {
    let expected = vec![vec![1.0, 2.0, 3.0]];
    let actual = vec![vec![0.0, 1.0, 2.0, 3.0]];
    let max_shift = 0;
    let (aligned_exp, aligned_act, shift) = align_samples(expected.clone(), actual.clone(), max_shift);
    assert_eq!(shift, 0);
    assert_eq!(aligned_exp[0], vec![1.0, 2.0, 3.0]);
    assert_eq!(aligned_act[0], vec![0.0, 1.0, 2.0]);
}
