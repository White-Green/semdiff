use crate::{BinaryDiff, BinaryDiffReporter};
use askama::Template;
use semdiff_core::DetailReporter;
use semdiff_output::html::{HtmlReport, HtmlReportError};
use semdiff_tree_fs::FileLeaf;
use similar::ChangeTag;
use similar::utils::TextDiffRemapper;
use std::fmt;
use std::fmt::Display;
use thiserror::Error;

const COMPARES_NAME: &str = "binary";

#[derive(Debug, Error)]
pub enum BinaryDiffReportError {
    #[error("html report error: {0}")]
    HtmlReport(#[from] HtmlReportError),
}

#[derive(Template)]
#[template(path = "binary_preview.html")]
struct BinaryPreviewTemplate {
    body: BinaryPreviewBody,
}

enum BinaryPreviewBody {
    Modified {
        expected_size: usize,
        actual_size: usize,
        added_bytes: usize,
        deleted_bytes: usize,
    },
    Single {
        size: usize,
    },
}

#[derive(Template)]
#[template(path = "binary_detail.html")]
struct BinaryDetailTemplate<'a> {
    detail: BinaryDetailBody<'a>,
}

enum BinaryDetailBody<'a> {
    Diff {
        expected: &'a [u8],
        actual: &'a [u8],
        diff: &'a similar::TextDiff<'a, 'a, 'a, [u8]>,
    },
    Single {
        label: &'a str,
        body: &'a [u8],
    },
}

fn diff_iter<'a>(
    diff: &'a similar::TextDiff<[u8]>,
    expected: &'a [u8],
    actual: &'a [u8],
) -> impl Iterator<Item = (ChangeTag, &'a [u8])> {
    let remapper = TextDiffRemapper::from_text_diff(diff, expected, actual);
    diff.ops().iter().flat_map(move |x| remapper.iter_slices(x))
}

fn format_line(line: &[u8]) -> impl Display + '_ {
    fmt::from_fn(|f| {
        let Some((first, tail)) = line.split_first() else {
            return Ok(());
        };
        write!(f, "{:02X}", first)?;
        for byte in tail {
            write!(f, " {:02X}", byte)?;
        }
        Ok(())
    })
}

struct IncrementUsize {
    value: usize,
}

impl IncrementUsize {
    fn new() -> IncrementUsize {
        IncrementUsize { value: 0 }
    }

    fn incr(&mut self, value: usize) -> usize {
        let old = self.value;
        self.value += value;
        old
    }
}

impl BinaryDetailBody<'_> {
    fn is_multicolumn(&self) -> bool {
        matches!(self, BinaryDetailBody::Diff { .. })
    }
}

impl DetailReporter<BinaryDiff, FileLeaf, HtmlReport> for BinaryDiffReporter {
    type Error = BinaryDiffReportError;

    fn available(&self, _data: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn report_unchanged(&self, name: &str, diff: BinaryDiff, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let preview_html = BinaryPreviewTemplate {
            body: BinaryPreviewBody::Single {
                size: diff.expected().len(),
            },
        };
        let detail_html = BinaryDetailTemplate {
            detail: BinaryDetailBody::Single {
                label: "same",
                body: diff.expected(),
            },
        };
        reporter.record_unchanged(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }

    fn report_modified(&self, name: &str, diff: BinaryDiff, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let diff_changes = diff.changes();
        let stat = BinaryDiff::stat(&diff_changes);
        let preview_html = BinaryPreviewTemplate {
            body: BinaryPreviewBody::Modified {
                expected_size: diff.expected().len(),
                actual_size: diff.actual().len(),
                added_bytes: stat.added,
                deleted_bytes: stat.deleted,
            },
        };
        let detail_html = BinaryDetailTemplate {
            detail: BinaryDetailBody::Diff {
                expected: diff.expected(),
                actual: diff.actual(),
                diff: &diff_changes,
            },
        };
        reporter.record_modified(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }

    fn report_added(&self, name: &str, data: FileLeaf, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let preview_html = BinaryPreviewTemplate {
            body: BinaryPreviewBody::Single {
                size: data.content.len(),
            },
        };
        let detail_html = BinaryDetailTemplate {
            detail: BinaryDetailBody::Single {
                label: "added",
                body: &data.content,
            },
        };
        reporter.record_added(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }

    fn report_deleted(&self, name: &str, data: FileLeaf, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let preview_html = BinaryPreviewTemplate {
            body: BinaryPreviewBody::Single {
                size: data.content.len(),
            },
        };
        let detail_html = BinaryDetailTemplate {
            detail: BinaryDetailBody::Single {
                label: "deleted",
                body: &data.content,
            },
        };
        reporter.record_deleted(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }
}
