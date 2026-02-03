use clap::Parser;
use semdiff_core::{DetailReporter, DiffAndReport, DiffCalculator, DiffReport};
use semdiff_output::html::HtmlReport;
use semdiff_output::json::JsonReport;
use semdiff_output::summary::SummaryReport;
use semdiff_tree_fs::{FileLeaf, FsNode};
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::PathBuf;

#[derive(Debug, clap::Parser)]
#[command(name = "semdiff", version, about = "Semantic diff tool")]
struct Cli {
    /// Path to the expected input file or directory.
    #[arg(long)]
    expected: PathBuf,
    /// Path to the actual input file or directory.
    #[arg(long)]
    actual: PathBuf,
    /// Output path for JSON/HTML reports; if omitted, prints a summary to stdout.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Output format: json or html. If omitted, inferred from --output extension or defaults to summary.
    #[arg(long)]
    format: Option<String>,
    /// Ignore object key order when comparing JSON.
    #[arg(long)]
    json_ignore_object_key_order: bool,
    /// Max OkLab+alpha distance to treat two image pixels as equal.
    #[arg(long, default_value_t = 0.0)]
    image_max_distance: f32,
    /// Max ratio of differing pixels to treat images as equal.
    #[arg(long, default_value_t = 0.0)]
    image_max_diff_ratio: f32,
    /// Max allowed temporal shift (seconds) when aligning audio.
    #[arg(long, default_value_t = 0.0)]
    audio_shift_tolerance_seconds: f32,
    /// Max LUFS difference in dB to treat audio as equal.
    #[arg(long, default_value_t = 0.0)]
    audio_lufs_tolerance_db: f32,
    /// Per-bin spectral magnitude tolerance for audio comparison.
    #[arg(long, default_value_t = 0.0)]
    audio_spectral_tolerance: f32,
    /// Max ratio of differing spectrogram bins to treat audio as equal.
    #[arg(long, default_value_t = 0.0)]
    audio_spectrogram_diff_rate_tolerance: f64,
}

#[derive(Debug, Clone, Copy)]
struct DiffConfig {
    json_ignore_object_key_order: bool,
    image_max_distance: f32,
    image_max_diff_ratio: f32,
    audio_shift_tolerance_seconds: f32,
    audio_lufs_tolerance_db: f32,
    audio_spectral_tolerance: f32,
    audio_spectrogram_diff_rate_tolerance: f64,
}

impl DiffConfig {
    fn from_cli(cli: &Cli) -> Self {
        Self {
            json_ignore_object_key_order: cli.json_ignore_object_key_order,
            image_max_distance: cli.image_max_distance,
            image_max_diff_ratio: cli.image_max_diff_ratio,
            audio_shift_tolerance_seconds: cli.audio_shift_tolerance_seconds,
            audio_lufs_tolerance_db: cli.audio_lufs_tolerance_db,
            audio_spectral_tolerance: cli.audio_spectral_tolerance,
            audio_spectrogram_diff_rate_tolerance: cli.audio_spectrogram_diff_rate_tolerance,
        }
    }
}

struct DiffCalculators {
    json: semdiff_differ_json::JsonDiffCalculator,
    text: semdiff_differ_text::TextDiffCalculator,
    audio: semdiff_differ_audio::AudioDiffCalculator,
    image: semdiff_differ_image::ImageDiffCalculator,
    binary: semdiff_differ_binary::BinaryDiffCalculator,
}

enum OutputKind {
    Html(PathBuf),
    JsonToFile(PathBuf),
    JsonToStdout,
    Summary,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let diff_config = DiffConfig::from_cli(&cli);
    let output_kind = output_target(cli.output.clone(), cli.format.as_deref());
    let expected = FsNode::new_root(cli.expected);
    let actual = FsNode::new_root(cli.actual);
    match output_kind {
        OutputKind::Html(path) => {
            let report = HtmlReport::new(path);
            let diff = construct_diff(&diff_config);
            semdiff_core::calc_diff(expected, actual, &diff, report)?;
        }
        OutputKind::JsonToFile(path) => {
            let report = JsonReport::new(File::create_new(path).expect(""));
            let diff = construct_diff(&diff_config);
            semdiff_core::calc_diff(expected, actual, &diff, report)?;
        }
        OutputKind::JsonToStdout => {
            let report = JsonReport::new(io::stdout());
            let diff = construct_diff(&diff_config);
            semdiff_core::calc_diff(expected, actual, &diff, report)?;
        }
        OutputKind::Summary => {
            let report = SummaryReport::new(io::stdout());
            let diff = construct_diff(&diff_config);
            semdiff_core::calc_diff(expected, actual, &diff, report)?;
        }
    }
    Ok(())
}

fn build_diff_calculators(config: &DiffConfig) -> DiffCalculators {
    DiffCalculators {
        json: semdiff_differ_json::JsonDiffCalculator::new(config.json_ignore_object_key_order),
        text: semdiff_differ_text::TextDiffCalculator,
        audio: semdiff_differ_audio::AudioDiffCalculator::new(
            config.audio_shift_tolerance_seconds,
            config.audio_lufs_tolerance_db,
            config.audio_spectral_tolerance,
            config.audio_spectrogram_diff_rate_tolerance,
        ),
        image: semdiff_differ_image::ImageDiffCalculator::new(config.image_max_distance, config.image_max_diff_ratio),
        binary: semdiff_differ_binary::BinaryDiffCalculator,
    }
}

fn construct_diff<R: Sync>(config: &DiffConfig) -> Vec<Box<dyn DiffReport<FileLeaf, R>>>
where
    semdiff_differ_text::TextDiffReporter:
        DetailReporter<<semdiff_differ_text::TextDiffCalculator as DiffCalculator<FileLeaf>>::Diff, FileLeaf, R>,
    semdiff_differ_json::JsonDiffReporter:
        DetailReporter<<semdiff_differ_json::JsonDiffCalculator as DiffCalculator<FileLeaf>>::Diff, FileLeaf, R>,
    semdiff_differ_audio::AudioDiffReporter:
        DetailReporter<<semdiff_differ_audio::AudioDiffCalculator as DiffCalculator<FileLeaf>>::Diff, FileLeaf, R>,
    semdiff_differ_image::ImageDiffReporter:
        DetailReporter<<semdiff_differ_image::ImageDiffCalculator as DiffCalculator<FileLeaf>>::Diff, FileLeaf, R>,
    semdiff_differ_binary::BinaryDiffReporter:
        DetailReporter<<semdiff_differ_binary::BinaryDiffCalculator as DiffCalculator<FileLeaf>>::Diff, FileLeaf, R>,
{
    let DiffCalculators {
        json,
        text,
        audio,
        image,
        binary,
    } = build_diff_calculators(config);
    vec![
        Box::new(DiffAndReport::new(json, semdiff_differ_json::JsonDiffReporter)) as Box<dyn DiffReport<FileLeaf, R>>,
        Box::new(DiffAndReport::new(text, semdiff_differ_text::TextDiffReporter)) as Box<dyn DiffReport<FileLeaf, R>>,
        Box::new(DiffAndReport::new(
            audio,
            semdiff_differ_audio::AudioDiffReporter::default(),
        )) as Box<dyn DiffReport<FileLeaf, R>>,
        Box::new(DiffAndReport::new(image, semdiff_differ_image::ImageDiffReporter))
            as Box<dyn DiffReport<FileLeaf, R>>,
        Box::new(DiffAndReport::new(binary, semdiff_differ_binary::BinaryDiffReporter))
            as Box<dyn DiffReport<FileLeaf, R>>,
    ]
}

fn output_target(output: Option<PathBuf>, format: Option<&str>) -> OutputKind {
    match format {
        Some("json") => output.map_or(OutputKind::JsonToStdout, OutputKind::JsonToFile),
        Some("html") => OutputKind::Html(output.expect("Output path required for HTML format")),
        Some(fmt) => panic!("Unsupported output format: {fmt}"),
        None => {
            if let Some(output_path) = output {
                match output_path.extension().and_then(OsStr::to_str) {
                    Some("json") => OutputKind::JsonToFile(output_path),
                    Some("html") => OutputKind::Html(output_path),
                    Some(ext) => panic!("Unsupported output extension: {ext}"),
                    None => panic!("Unsupported output extension"),
                }
            } else {
                OutputKind::Summary
            }
        }
    }
}
