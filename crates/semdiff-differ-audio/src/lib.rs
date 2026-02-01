use image::{Rgba, RgbaImage};
use memmap2::Mmap;
use mime::Mime;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::{Fft, FftPlanner};
use semdiff_core::{Diff, DiffCalculator, MayUnsupported};
use semdiff_tree_fs::FileLeaf;
use std::f32::consts::PI;
use std::fmt::{Debug, Formatter};
use std::io::{Cursor, ErrorKind};
use std::ops::Range;
use std::sync::{Arc, LazyLock};
use std::{convert, iter};
use symphonia::core::audio::{AudioBuffer, SignalSpec};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

pub mod report_html;
pub mod report_json;
pub mod report_summary;

const WAVEFORM_WIDTH: u32 = 1024;
const WAVEFORM_HEIGHT: u32 = 256;
const SPECTROGRAM_WIDTH: u32 = 1024;
const SPECTROGRAM_HEIGHT: u32 = 256;
const SPECTROGRAM_DATA_HEIGHT: usize = 1024;
const FFT_WINDOW_SIZE: usize = SPECTROGRAM_DATA_HEIGHT * 2;
const LOG_EPSILON: f32 = 1e-6;

pub struct AudioDiffReporter {
    spectrogram_analyzer: SpectrogramAnalyzer,
}

impl Default for AudioDiffReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDiffReporter {
    pub fn new() -> AudioDiffReporter {
        AudioDiffReporter {
            spectrogram_analyzer: SpectrogramAnalyzer::new(),
        }
    }

    fn build_audio_data(&self, kind: Mime, content: Arc<Mmap>) -> Result<AudioData, AudioDecodeError> {
        let decoded = self.spectrogram_analyzer.decode_audio(&kind, &content)?;
        let stat = AudioStat::from_one(&decoded);
        Ok(build_audio_data_from_decoded(kind, content, &decoded, &stat))
    }
}

#[derive(Debug)]
pub enum AudioDiffStatus {
    Equal(AudioDiffDetail),
    Different(AudioDiffDetail),
    Incomparable,
}

impl AudioDiffStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioDiffStatus::Equal(_) => "equal",
            AudioDiffStatus::Different(_) => "different",
            AudioDiffStatus::Incomparable => "incomparable",
        }
    }
}

#[derive(Debug)]
pub struct AudioDiff {
    status: AudioDiffStatus,
    expected: AudioData,
    actual: AudioData,
}

impl Diff for AudioDiff {
    fn equal(&self) -> bool {
        matches!(self.status, AudioDiffStatus::Equal(_))
    }
}

impl AudioDiff {
    pub fn status(&self) -> &AudioDiffStatus {
        &self.status
    }

    pub fn expected(&self) -> &AudioData {
        &self.expected
    }

    pub fn actual(&self) -> &AudioData {
        &self.actual
    }

    pub fn diff_detail(&self) -> Option<&AudioDiffDetail> {
        match &self.status {
            AudioDiffStatus::Equal(detail) | AudioDiffStatus::Different(detail) => Some(detail),
            AudioDiffStatus::Incomparable => None,
        }
    }
}

#[derive(Debug)]
pub struct AudioDiffDetail {
    spectrogram_diff: Vec<RgbaImage>,
    stat: AudioDiffStat,
}

impl AudioDiffDetail {
    pub fn spectrogram_diff(&self) -> &[RgbaImage] {
        &self.spectrogram_diff
    }

    pub fn stat(&self) -> &AudioDiffStat {
        &self.stat
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AudioDiffStat {
    pub spectrogram_diff_rate: f64,
    pub shift_samples: i32,
    pub lufs_diff_db: f32,
}

#[derive(Debug)]
pub struct AudioData {
    mime: Mime,
    sample_rate: u32,
    channels: u16,
    duration_seconds: f32,
    waveform: Vec<RgbaImage>,
    spectrogram: Vec<RgbaImage>,
    content: Arc<Mmap>,
}

impl AudioData {
    pub fn mime(&self) -> &Mime {
        &self.mime
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn duration_seconds(&self) -> f32 {
        self.duration_seconds
    }

    pub fn waveform(&self) -> &[RgbaImage] {
        &self.waveform
    }

    pub fn spectrogram(&self) -> &[RgbaImage] {
        &self.spectrogram
    }

    pub fn content(&self) -> &[u8] {
        &self.content
    }
}

#[derive(Debug, Error)]
pub enum AudioDecodeError {
    #[error("symphonia error: {0}")]
    Symphonia(#[from] SymphoniaError),
    #[error("no default audio track")]
    NoDefaultTrack,
    #[error("missing sample rate")]
    MissingSampleRate,
}

#[derive(Default)]
pub struct AudioDiffCalculator {
    shift_tolerance_seconds: f32,
    lufs_tolerance_db: f32,
    spectral_tolerance: f32,
    spectrogram_diff_rate_tolerance: f64,
    spectrogram_analyzer: SpectrogramAnalyzer,
}

impl Debug for AudioDiffCalculator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioDiffCalculator")
            .field("shift_tolerance_seconds", &self.shift_tolerance_seconds)
            .field("lufs_tolerance_db", &self.lufs_tolerance_db)
            .field("spectral_tolerance", &self.spectral_tolerance)
            .field("spectrogram_diff_rate_tolerance", &self.spectrogram_diff_rate_tolerance)
            .finish()
    }
}

impl DiffCalculator<FileLeaf> for AudioDiffCalculator {
    type Error = convert::Infallible;
    type Diff = AudioDiff;

    fn diff(
        &self,
        _name: &str,
        expected: FileLeaf,
        actual: FileLeaf,
    ) -> Result<MayUnsupported<Self::Diff>, Self::Error> {
        if !is_audio_kind(&expected.kind) || !is_audio_kind(&actual.kind) {
            return Ok(MayUnsupported::Unsupported);
        }
        let Ok(expected_decoded) = self
            .spectrogram_analyzer
            .decode_audio(&expected.kind, expected.content.as_ref())
        else {
            return Ok(MayUnsupported::Unsupported);
        };
        let Ok(actual_decoded) = self
            .spectrogram_analyzer
            .decode_audio(&actual.kind, actual.content.as_ref())
        else {
            return Ok(MayUnsupported::Unsupported);
        };
        let stat_decoded = AudioStat::from_pair(&expected_decoded, &actual_decoded);
        let expected_data =
            build_audio_data_from_decoded(expected.kind, expected.content, &expected_decoded, &stat_decoded);
        let actual_data = build_audio_data_from_decoded(actual.kind, actual.content, &actual_decoded, &stat_decoded);
        if (expected_decoded.sample_rate, expected_decoded.channels)
            != (actual_decoded.sample_rate, actual_decoded.channels)
        {
            return Ok(MayUnsupported::Ok(AudioDiff {
                status: AudioDiffStatus::Incomparable,
                expected: expected_data,
                actual: actual_data,
            }));
        }
        let sample_rate = expected_decoded.sample_rate;
        let max_shift_samples = (self.shift_tolerance_seconds * sample_rate as f32).round() as i32;
        let (aligned_expected, aligned_actual, shift_samples) =
            align_samples(expected_decoded.samples, actual_decoded.samples, max_shift_samples);

        let expected_spectrogram = aligned_expected
            .iter()
            .map(|channel| self.spectrogram_analyzer.compute(channel))
            .collect::<Vec<_>>();
        let actual_spectrogram = aligned_actual
            .iter()
            .map(|channel| self.spectrogram_analyzer.compute(channel))
            .collect::<Vec<_>>();

        let (spectrogram_diff, spectrogram_diff_rate) =
            self.build_diff_images(&expected_spectrogram, &actual_spectrogram);

        let lufs_diff_db = summarize_channel_metrics(&aligned_expected, &aligned_actual);

        let detail = AudioDiffDetail {
            spectrogram_diff,
            stat: AudioDiffStat {
                spectrogram_diff_rate,
                shift_samples,
                lufs_diff_db,
            },
        };

        let equal =
            lufs_diff_db <= self.lufs_tolerance_db && spectrogram_diff_rate <= self.spectrogram_diff_rate_tolerance;
        let status = if equal {
            AudioDiffStatus::Equal(detail)
        } else {
            AudioDiffStatus::Different(detail)
        };

        Ok(MayUnsupported::Ok(AudioDiff {
            status,
            expected: expected_data,
            actual: actual_data,
        }))
    }
}

#[derive(Debug)]
struct AudioStat {
    signal_max: f32,
    spectrogram_min: f32,
    spectrogram_max: f32,
    duration: f32,
}

impl AudioStat {
    fn from_one(decoded: &AudioDecoded) -> AudioStat {
        let signal_max = decoded
            .samples
            .iter()
            .flatten()
            .copied()
            .map(f32::abs)
            .fold(0.0, f32::max);
        let (spectrogram_min, spectrogram_max) = decoded
            .spectrograms
            .iter()
            .flatten()
            .flatten()
            .copied()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), v| {
                (v.min(min), v.max(max))
            });
        let duration = decoded.duration_seconds;
        AudioStat {
            signal_max,
            spectrogram_min,
            spectrogram_max,
            duration,
        }
    }

    fn from_pair(expected: &AudioDecoded, actual: &AudioDecoded) -> AudioStat {
        let signal_max = expected
            .samples
            .iter()
            .chain(actual.samples.iter())
            .flatten()
            .copied()
            .map(f32::abs)
            .fold(0.0, f32::max);
        let (spectrogram_min, spectrogram_max) = expected
            .spectrograms
            .iter()
            .chain(actual.spectrograms.iter())
            .flatten()
            .flatten()
            .copied()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), v| {
                (v.min(min), v.max(max))
            });
        let duration = expected.duration_seconds.max(actual.duration_seconds);
        AudioStat {
            signal_max,
            spectrogram_min,
            spectrogram_max,
            duration,
        }
    }
}

impl AudioDiffCalculator {
    pub fn new(
        shift_tolerance_seconds: f32,
        lufs_tolerance_db: f32,
        spectral_tolerance: f32,
        spectrogram_diff_rate_tolerance: f64,
    ) -> Self {
        Self {
            shift_tolerance_seconds,
            lufs_tolerance_db,
            spectral_tolerance,
            spectrogram_diff_rate_tolerance,
            spectrogram_analyzer: SpectrogramAnalyzer::new(),
        }
    }

    fn build_diff_images(
        &self,
        expected: &[Vec<[f32; SPECTROGRAM_DATA_HEIGHT]>],
        actual: &[Vec<[f32; SPECTROGRAM_DATA_HEIGHT]>],
    ) -> (Vec<RgbaImage>, f64) {
        assert_eq!(expected.len(), actual.len());
        let mut diff_images = Vec::with_capacity(expected.len());
        let mut diff_rate_sum = 0.0;
        for (expected_frame, actual_frame) in expected.iter().zip(actual.iter()) {
            let (diff_image, diff_rate) = self.diff_spectrograms(expected_frame, actual_frame);
            diff_images.push(diff_image);
            diff_rate_sum += diff_rate;
        }
        (diff_images, diff_rate_sum / expected.len() as f64)
    }

    fn diff_spectrograms(
        &self,
        expected: &[[f32; SPECTROGRAM_DATA_HEIGHT]],
        actual: &[[f32; SPECTROGRAM_DATA_HEIGHT]],
    ) -> (RgbaImage, f64) {
        let spectrogram_len = expected.len().max(actual.len());
        let mut diff_image = RgbaImage::from_pixel(SPECTROGRAM_WIDTH, SPECTROGRAM_HEIGHT, Rgba([0, 0, 0, 0]));
        let mut diff_count = 0usize;
        let mut total_count = 0usize;
        assert!(SPECTROGRAM_DATA_HEIGHT >= SPECTROGRAM_HEIGHT as usize);
        if spectrogram_len >= SPECTROGRAM_WIDTH as usize {
            for x in 0..SPECTROGRAM_WIDTH {
                let x_range = x as usize * spectrogram_len / SPECTROGRAM_WIDTH as usize
                    ..(x + 1) as usize * spectrogram_len / SPECTROGRAM_WIDTH as usize;
                for y in 0..SPECTROGRAM_HEIGHT {
                    let y_range = spectrogram_log_bin_range(y);

                    let mut diff_sum = 0usize;
                    for y in y_range.clone() {
                        for x in x_range.clone() {
                            let expected = expected.get(x).map(|x| x[y]);
                            let actual = actual.get(x).map(|x| x[y]);
                            let diff = (expected.unwrap_or(f32::INFINITY) - actual.unwrap_or(f32::NEG_INFINITY)).abs();
                            total_count += 1;
                            if diff > self.spectral_tolerance {
                                diff_sum += 1;
                                diff_count += 1;
                            }
                        }
                    }
                    diff_image.put_pixel(
                        x,
                        SPECTROGRAM_HEIGHT - y - 1,
                        Rgba([
                            255,
                            0,
                            0,
                            (diff_sum as f64 / (x_range.len() * y_range.len()) as f64 * 255.0) as u8,
                        ]),
                    );
                }
            }
        } else {
            for x in 0..spectrogram_len {
                let image_x_range = x as u32 * SPECTROGRAM_WIDTH / spectrogram_len as u32
                    ..(x + 1) as u32 * SPECTROGRAM_WIDTH / spectrogram_len as u32;
                for y in 0..SPECTROGRAM_HEIGHT {
                    let y_range = spectrogram_log_bin_range(y);
                    let mut diff_sum = 0usize;
                    for y in y_range.clone() {
                        let expected = expected.get(x).map(|x| x[y]);
                        let actual = actual.get(x).map(|x| x[y]);
                        let diff = (expected.unwrap_or(f32::INFINITY) - actual.unwrap_or(f32::NEG_INFINITY)).abs();
                        total_count += 1;
                        if diff > self.spectral_tolerance {
                            diff_sum += 1;
                            diff_count += 1;
                        }
                    }
                    let color = Rgba([255, 0, 0, (diff_sum as f64 / y_range.len() as f64 * 255.0) as u8]);
                    for x in image_x_range.clone() {
                        diff_image.put_pixel(x, SPECTROGRAM_HEIGHT - y - 1, color);
                    }
                }
            }
        }
        let diff_rate = if total_count == 0 {
            0.0
        } else {
            diff_count as f64 / total_count as f64
        };
        (diff_image, diff_rate)
    }
}

pub fn audio_extension(kind: &Mime) -> Option<&'static str> {
    match kind.essence_str() {
        "audio/mpeg" => Some("mp3"),
        "audio/wav" | "audio/x-wav" => Some("wav"),
        "audio/flac" => Some("flac"),
        "audio/ogg" | "application/ogg" => Some("ogg"),
        "audio/opus" => Some("opus"),
        "audio/webm" => Some("webm"),
        "audio/aac" => Some("aac"),
        "audio/mp4" | "video/mp4" => Some("m4a"),
        "audio/x-m4a" => Some("m4a"),
        _ => mime_guess::get_mime_extensions(kind).and_then(|exts| exts.first().copied()),
    }
}

fn is_audio_kind(kind: &Mime) -> bool {
    kind.type_() == mime::AUDIO || kind.type_() == mime::VIDEO
}

fn build_audio_data_from_decoded(
    mime: Mime,
    content: Arc<Mmap>,
    decoded: &AudioDecoded,
    stat: &AudioStat,
) -> AudioData {
    let waveform = render_waveforms(&decoded.samples, stat, decoded.sample_rate);
    let spectrogram = render_spectrograms(&decoded.spectrograms, stat, decoded.sample_rate);
    AudioData {
        mime,
        sample_rate: decoded.sample_rate,
        channels: decoded.channels,
        duration_seconds: decoded.duration_seconds,
        waveform,
        spectrogram,
        content,
    }
}

struct AudioDecoded {
    sample_rate: u32,
    channels: u16,
    duration_seconds: f32,
    samples: Vec<Vec<f32>>,
    spectrograms: Vec<Vec<[f32; SPECTROGRAM_DATA_HEIGHT]>>,
}

fn align_samples(
    mut expected: Vec<Vec<f32>>,
    mut actual: Vec<Vec<f32>>,
    max_shift_samples: i32,
) -> (Vec<Vec<f32>>, Vec<Vec<f32>>, i32) {
    assert_eq!(expected.len(), actual.len());
    let best_shift = (-max_shift_samples..=max_shift_samples)
        .map(|shift| {
            let score_sum = expected
                .iter()
                .zip(actual.iter())
                .map(|(expected_channel, actual_channel)| {
                    let (expected_slice, actual_slice) = overlap_slices(expected_channel, actual_channel, shift);
                    normalized_correlation(expected_slice, actual_slice)
                })
                .sum::<f32>();
            (shift, score_sum)
        })
        .min_by(|&(_, score1), &(_, score2)| score1.partial_cmp(&score2).unwrap())
        .map_or(0, |(shift, _)| shift);

    for (expected, actual) in expected.iter_mut().zip(actual.iter_mut()) {
        let (expected_range, actual_range) = overlap_range(expected.len(), actual.len(), best_shift);
        expected.drain(..expected_range.start.min(expected.len()));
        actual.drain(..actual_range.start.min(actual.len()));
    }

    (expected, actual, best_shift)
}

fn summarize_channel_metrics(expected: &[Vec<f32>], actual: &[Vec<f32>]) -> f32 {
    let channel_count = expected.len().min(actual.len());
    if channel_count == 0 {
        return f32::INFINITY;
    }
    let mut max_lufs_diff = 0.0f32;
    for channel_index in 0..channel_count {
        let expected_channel = &expected[channel_index];
        let actual_channel = &actual[channel_index];
        if expected_channel.is_empty() || actual_channel.is_empty() {
            continue;
        }
        let expected_lufs = loudness_db(expected_channel);
        let actual_lufs = loudness_db(actual_channel);
        max_lufs_diff = max_lufs_diff.max((expected_lufs - actual_lufs).abs());
    }
    max_lufs_diff
}

fn render_waveforms(samples: &[Vec<f32>], stat: &AudioStat, sample_rate: u32) -> Vec<RgbaImage> {
    samples
        .iter()
        .map(|channel| render_waveform(channel, stat, sample_rate))
        .collect()
}

fn overlap_range(expected_len: usize, actual_len: usize, shift: i32) -> (Range<usize>, Range<usize>) {
    if shift >= 0 {
        let shift = shift as usize;
        let len = expected_len.min(actual_len.saturating_sub(shift));
        (0..len, shift..shift + len)
    } else {
        let shift = (-shift) as usize;
        let len = actual_len.min(expected_len.saturating_sub(shift));
        (shift..shift + len, 0..len)
    }
}

fn overlap_slices<'a>(expected: &'a [f32], actual: &'a [f32], shift: i32) -> (&'a [f32], &'a [f32]) {
    if shift >= 0 {
        let shift = shift as usize;
        let len = expected.len().min(actual.len().saturating_sub(shift));
        (&expected[..len], &actual[shift..shift + len])
    } else {
        let shift = (-shift) as usize;
        let len = actual.len().min(expected.len().saturating_sub(shift));
        (&expected[shift..shift + len], &actual[..len])
    }
}

fn normalized_correlation(expected: &[f32], actual: &[f32]) -> f32 {
    assert_eq!(expected.len(), actual.len());
    let mut dot = 0.0f32;
    let mut expected_power = 0.0f32;
    let mut actual_power = 0.0f32;
    for (&e, &a) in expected.iter().zip(actual.iter()) {
        dot += e * a;
        expected_power += e * e;
        actual_power += a * a;
    }
    let denom = (expected_power.sqrt() * actual_power.sqrt()).max(LOG_EPSILON);
    dot / denom
}

fn loudness_db(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -100.0;
    }
    let power = samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    let rms = power.sqrt();
    20.0 * rms.max(LOG_EPSILON).log10()
}

fn render_waveform(samples: &[f32], stat: &AudioStat, sample_rate: u32) -> RgbaImage {
    const WAVEFORM_COLOR: Rgba<u8> = Rgba([0, 255, 0, 255]);
    let clip = (stat.signal_max * 1.2).clamp(LOG_EPSILON, 1.0);
    let mut image = RgbaImage::from_pixel(WAVEFORM_WIDTH, WAVEFORM_HEIGHT, Rgba([0, 0, 0, 0]));
    if stat.duration <= 0.0 || sample_rate == 0 {
        return image;
    }
    let to_y = |value: f32| {
        let normalized = (value + clip) / (2.0 * clip);
        ((normalized * WAVEFORM_HEIGHT as f32).round() as u32).min(WAVEFORM_HEIGHT - 1)
    };
    let duration = stat.duration;
    for x in 0..WAVEFORM_WIDTH {
        let start_time = x as f32 * duration / WAVEFORM_WIDTH as f32;
        let end_time = (x + 1) as f32 * duration / WAVEFORM_WIDTH as f32;
        let start = (start_time * sample_rate as f32).floor() as usize;
        let end = (end_time * sample_rate as f32).ceil() as usize;
        let start = start.min(samples.len());
        let end = end.min(samples.len());
        if end <= start {
            continue;
        }
        let (min, max) = samples[start..end]
            .iter()
            .fold((1.0f32, -1.0f32), |(min, max), &s| (min.min(s), max.max(s)));
        let y_min = to_y(min);
        let y_max = to_y(max);
        for y in y_min..=y_max {
            image.put_pixel(x, WAVEFORM_HEIGHT - y - 1, WAVEFORM_COLOR);
        }
    }
    image
}

fn render_spectrograms(
    spectrograms: &[Vec<[f32; SPECTROGRAM_DATA_HEIGHT]>],
    stat: &AudioStat,
    sample_rate: u32,
) -> Vec<RgbaImage> {
    spectrograms
        .iter()
        .map(|channel| render_spectrogram(channel, stat, sample_rate))
        .collect()
}

fn spectrogram_log_bin_range(y: u32) -> Range<usize> {
    static RANGES: LazyLock<[Range<usize>; SPECTROGRAM_HEIGHT as usize]> = LazyLock::new(|| {
        const B: f64 = 20.0;
        const A: f64 = SPECTROGRAM_DATA_HEIGHT as f64 / (B - 1.0);
        let mut ranges = [const { 0usize..0 }; SPECTROGRAM_HEIGHT as usize];
        let mut wrote = 0;
        for y in 0..SPECTROGRAM_DATA_HEIGHT {
            let p1 = f64::log(1.0 / A * y as f64 + 1.0, B);
            let p2 = f64::log(1.0 / A * (y + 1) as f64 + 1.0, B);
            let range = p1 * SPECTROGRAM_HEIGHT as f64..p2 * SPECTROGRAM_HEIGHT as f64;
            if range.end - range.start < 1.0 {
                break;
            }
            ranges[range.start.round() as usize..range.end.round() as usize].fill(y..y + 1);
            wrote = y + 1;
        }
        for (y, slot) in ranges
            .iter_mut()
            .enumerate()
            .take(SPECTROGRAM_HEIGHT as usize)
            .skip(wrote)
        {
            let range = (A * (f64::powf(B, y as f64 / SPECTROGRAM_HEIGHT as f64) - 1.0)).round() as usize
                ..(A * (f64::powf(B, (y + 1) as f64 / SPECTROGRAM_HEIGHT as f64) - 1.0)).round() as usize;
            *slot = range;
        }
        ranges
    });
    RANGES[y as usize].clone()
}

struct SpectrogramAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    window: Box<[f32]>,
}

impl Default for SpectrogramAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectrogramAnalyzer {
    fn new() -> SpectrogramAnalyzer {
        let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_WINDOW_SIZE);
        let window = (0..FFT_WINDOW_SIZE)
            .map(|i| (PI * i as f32 / (FFT_WINDOW_SIZE - 1) as f32).sin())
            .collect();
        SpectrogramAnalyzer { fft, window }
    }

    fn decode_audio(&self, mime: &Mime, content: &[u8]) -> Result<AudioDecoded, AudioDecodeError> {
        let mut hint = Hint::new();
        if let Some(extension) = audio_extension(mime) {
            hint.with_extension(extension);
        }

        let owned = content.to_vec();
        let mss = MediaSourceStream::new(Box::new(Cursor::new(owned)), Default::default());
        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let mut format = probed.format;
        let track = format.default_track().ok_or(AudioDecodeError::NoDefaultTrack)?;
        let track_id = track.id;
        let codec_params = track.codec_params.clone();
        let mut decoder = symphonia::default::get_codecs().make(&codec_params, &DecoderOptions::default())?;

        let mut samples = vec![Vec::new()];
        let mut signal_spec = if let Some(rate) = codec_params.sample_rate
            && let Some(channels) = codec_params.channels
        {
            Some(SignalSpec { rate, channels })
        } else {
            None
        };
        let mut sample_buf = None::<AudioBuffer<f32>>;
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(SymphoniaError::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err.into()),
            };
            if packet.track_id() != track_id {
                continue;
            }
            let decoded = decoder.decode(&packet)?;
            let spec = *decoded.spec();
            if signal_spec.is_none() {
                signal_spec = Some(spec);
            }
            let sample_buf = sample_buf.get_or_insert_with(|| AudioBuffer::<f32>::new(decoded.capacity() as u64, spec));
            decoded.convert(sample_buf);
            samples.resize_with(sample_buf.planes().planes().len(), Vec::new);
            for (plane, samples) in sample_buf.planes().planes().iter().zip(samples.iter_mut()) {
                samples.extend_from_slice(plane);
            }
        }

        let Some(signal_spec) = signal_spec else {
            return Err(AudioDecodeError::MissingSampleRate);
        };

        let max_len = samples.iter().map(|channel| channel.len()).max().unwrap_or(0);
        let duration_seconds = max_len as f32 / signal_spec.rate as f32;

        let spectrograms = samples.iter().map(|sample| self.compute(sample)).collect::<Vec<_>>();

        Ok(AudioDecoded {
            sample_rate: signal_spec.rate,
            channels: signal_spec.channels.count() as u16,
            duration_seconds,
            samples,
            spectrograms,
        })
    }

    fn compute(&self, samples: &[f32]) -> Vec<[f32; SPECTROGRAM_DATA_HEIGHT]> {
        let mut buffer =
            Box::<[Complex<f32>; FFT_WINDOW_SIZE]>::try_from(vec![Complex::zero(); FFT_WINDOW_SIZE]).unwrap();
        let mut scratch = vec![Complex::zero(); self.fft.get_inplace_scratch_len()];
        let mut result = Vec::with_capacity(samples.len() / (FFT_WINDOW_SIZE / 2));
        for i in 0.. {
            let Some(samples) = samples.get(i * (FFT_WINDOW_SIZE / 2)..) else {
                break;
            };
            buffer
                .iter_mut()
                .zip(
                    samples
                        .iter()
                        .copied()
                        .chain(iter::repeat(0.0))
                        .zip(self.window.iter().copied()),
                )
                .for_each(|(slot, (s, w))| *slot = Complex::from(s * w));
            self.fft.process_with_scratch(&mut *buffer, &mut scratch);
            result.push([0.0; SPECTROGRAM_DATA_HEIGHT]);
            result
                .last_mut()
                .unwrap()
                .iter_mut()
                .zip(buffer.iter().copied())
                .for_each(|(slot, b)| *slot = b.norm().log10());
        }
        result
    }
}

fn render_spectrogram(spectrogram: &[[f32; SPECTROGRAM_DATA_HEIGHT]], stat: &AudioStat, sample_rate: u32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(SPECTROGRAM_WIDTH, SPECTROGRAM_HEIGHT, Rgba([0, 0, 0, 0]));
    if spectrogram.is_empty() || stat.duration <= 0.0 || sample_rate == 0 {
        return image;
    }

    let value_range = (stat.spectrogram_max - stat.spectrogram_min).max(LOG_EPSILON);
    let map_value = |v: f32| (v - stat.spectrogram_min) / value_range;
    assert!(SPECTROGRAM_HEIGHT <= SPECTROGRAM_DATA_HEIGHT as u32);
    let duration = stat.duration;
    let hop_samples = (FFT_WINDOW_SIZE / 2) as f32;
    let frame_duration = hop_samples / sample_rate as f32;
    if frame_duration <= 0.0 {
        return image;
    }

    for x in 0..SPECTROGRAM_WIDTH {
        let start_time = x as f32 * duration / SPECTROGRAM_WIDTH as f32;
        let end_time = (x + 1) as f32 * duration / SPECTROGRAM_WIDTH as f32;
        let start = (start_time / frame_duration).floor() as usize;
        let end = (end_time / frame_duration).ceil() as usize;
        let start = start.min(spectrogram.len());
        let end = end.min(spectrogram.len());
        if end <= start {
            continue;
        }

        for y in 0..SPECTROGRAM_HEIGHT {
            let y_range = spectrogram_log_bin_range(y);
            let sum = spectrogram[start..end]
                .iter()
                .flat_map(|spec| spec[y_range.clone()].iter().copied())
                .sum::<f32>();
            let value = sum / ((end - start) * y_range.len()) as f32;
            let intensity = map_value(value);
            image.put_pixel(
                x,
                SPECTROGRAM_HEIGHT - y - 1,
                Rgba([0, 0, 255, (intensity * 255.0) as u8]),
            );
        }
    }
    image
}

#[cfg(test)]
mod tests {
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
}
