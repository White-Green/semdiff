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
fn shift_tolerance_option_can_align_offset_audio() {
    let mut expected_samples = vec![0.0; 32];
    expected_samples[10] = 1.0;
    let mut actual_samples = vec![0.0; 33];
    actual_samples[11] = 1.0;
    let expected = mono_decoded(expected_samples);
    let actual = mono_decoded(actual_samples);

    let strict = AudioDiffCalculator::new(0.0, 1.0, 0.0, 0.0).diff_decoded(&expected, &actual);
    let strict_detail = expect_different(&strict);
    assert_eq!(strict_detail.stat().shift_samples, 0);
    assert!(strict_detail.stat().spectrogram_diff_rate > 0.0);

    let shifted = AudioDiffCalculator::new(0.1, 1.0, 0.0, 0.0).diff_decoded(&expected, &actual);
    let shifted_detail = expect_equal(&shifted);
    assert_eq!(shifted_detail.stat().shift_samples, 1);
    assert_eq!(shifted_detail.stat().spectrogram_diff_rate, 0.0);
}

#[test]
fn lufs_tolerance_option_changes_equal_result() {
    let expected = mono_decoded(vec![0.5; 32]);
    let actual = mono_decoded(vec![0.25; 32]);

    let strict = AudioDiffCalculator::new(0.0, 0.0, 100.0, 0.0).diff_decoded(&expected, &actual);
    let strict_detail = expect_different(&strict);
    assert!(strict_detail.stat().lufs_diff_db > 6.0);
    assert_eq!(strict_detail.stat().spectrogram_diff_rate, 0.0);

    let loose = AudioDiffCalculator::new(0.0, 7.0, 100.0, 0.0).diff_decoded(&expected, &actual);
    let loose_detail = expect_equal(&loose);
    assert!(loose_detail.stat().lufs_diff_db > 6.0);
}

#[test]
fn spectral_tolerance_option_changes_equal_result() {
    let expected = mono_decoded(vec![0.5; 32]);
    let actual = mono_decoded(vec![0.25; 32]);

    let strict = AudioDiffCalculator::new(0.0, 100.0, 0.0, 0.0).diff_decoded(&expected, &actual);
    let strict_detail = expect_different(&strict);
    assert!(strict_detail.stat().spectrogram_diff_rate > 0.0);

    let loose = AudioDiffCalculator::new(0.0, 100.0, 100.0, 0.0).diff_decoded(&expected, &actual);
    let loose_detail = expect_equal(&loose);
    assert_eq!(loose_detail.stat().spectrogram_diff_rate, 0.0);
}

#[test]
fn spectrogram_diff_rate_tolerance_option_changes_equal_result() {
    let expected = mono_decoded(vec![0.5; 32]);
    let actual = mono_decoded(vec![0.25; 32]);

    let strict = AudioDiffCalculator::new(0.0, 100.0, 0.0, 0.0).diff_decoded(&expected, &actual);
    let strict_detail = expect_different(&strict);
    assert!(strict_detail.stat().spectrogram_diff_rate > 0.0);

    let loose = AudioDiffCalculator::new(0.0, 100.0, 0.0, 1.0).diff_decoded(&expected, &actual);
    let loose_detail = expect_equal(&loose);
    assert!(loose_detail.stat().spectrogram_diff_rate > 0.0);
}

fn mono_decoded(samples: Vec<f32>) -> AudioDecoded {
    AudioDecoded {
        sample_rate: 10,
        channels: 1,
        duration_seconds: samples.len() as f32 / 10.0,
        samples: vec![samples],
        spectrograms: vec![Vec::new()],
    }
}

fn expect_equal(status: &AudioDiffStatus) -> &AudioDiffDetail {
    match status {
        AudioDiffStatus::Equal(detail) => detail,
        status => panic!("expected equal audio diff, got {}", status.as_str()),
    }
}

fn expect_different(status: &AudioDiffStatus) -> &AudioDiffDetail {
    match status {
        AudioDiffStatus::Different(detail) => detail,
        status => panic!("expected different audio diff, got {}", status.as_str()),
    }
}
