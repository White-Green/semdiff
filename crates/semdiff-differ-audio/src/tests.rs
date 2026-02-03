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
