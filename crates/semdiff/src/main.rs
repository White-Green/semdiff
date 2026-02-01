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
    #[arg(long)]
    expected: PathBuf,
    #[arg(long)]
    actual: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    format: Option<String>,
}

enum OutputKind {
    Html(PathBuf),
    JsonToFile(PathBuf),
    JsonToStdout,
    Summary,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let output_kind = output_target(cli.output.clone(), cli.format.as_deref());
    let expected = FsNode::new_root(cli.expected);
    let actual = FsNode::new_root(cli.actual);
    match output_kind {
        OutputKind::Html(path) => {
            let report = HtmlReport::new(path);
            let diff = construct_diff();
            semdiff_core::calc_diff(expected, actual, &diff, report)?;
        }
        OutputKind::JsonToFile(path) => {
            let report = JsonReport::new(File::create_new(path).expect(""));
            let diff = construct_diff();
            semdiff_core::calc_diff(expected, actual, &diff, report)?;
        }
        OutputKind::JsonToStdout => {
            let report = JsonReport::new(io::stdout());
            let diff = construct_diff();
            semdiff_core::calc_diff(expected, actual, &diff, report)?;
        }
        OutputKind::Summary => {
            let report = SummaryReport::new(io::stdout());
            let diff = construct_diff();
            semdiff_core::calc_diff(expected, actual, &diff, report)?;
        }
    }
    Ok(())
}

fn construct_diff<R: Sync>() -> Vec<Box<dyn DiffReport<FileLeaf, R>>>
where
    semdiff_differ_text::TextDiffReporter:
        DetailReporter<<semdiff_differ_text::TextDiffCalculator as DiffCalculator<FileLeaf>>::Diff, FileLeaf, R>,
    semdiff_differ_audio::AudioDiffReporter:
        DetailReporter<<semdiff_differ_audio::AudioDiffCalculator as DiffCalculator<FileLeaf>>::Diff, FileLeaf, R>,
    semdiff_differ_image::ImageDiffReporter:
        DetailReporter<<semdiff_differ_image::ImageDiffCalculator as DiffCalculator<FileLeaf>>::Diff, FileLeaf, R>,
    semdiff_differ_binary::BinaryDiffReporter:
        DetailReporter<<semdiff_differ_binary::BinaryDiffCalculator as DiffCalculator<FileLeaf>>::Diff, FileLeaf, R>,
{
    vec![
        Box::new(DiffAndReport::new(
            semdiff_differ_text::TextDiffCalculator,
            semdiff_differ_text::TextDiffReporter,
        )) as Box<dyn DiffReport<FileLeaf, R>>,
        Box::new(DiffAndReport::new(
            semdiff_differ_audio::AudioDiffCalculator::default(),
            semdiff_differ_audio::AudioDiffReporter::default(),
        )) as Box<dyn DiffReport<FileLeaf, R>>,
        Box::new(DiffAndReport::new(
            semdiff_differ_image::ImageDiffCalculator::default(),
            semdiff_differ_image::ImageDiffReporter,
        )) as Box<dyn DiffReport<FileLeaf, R>>,
        Box::new(DiffAndReport::new(
            semdiff_differ_binary::BinaryDiffCalculator,
            semdiff_differ_binary::BinaryDiffReporter,
        )) as Box<dyn DiffReport<FileLeaf, R>>,
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
