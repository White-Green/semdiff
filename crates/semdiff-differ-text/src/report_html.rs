use crate::{TextDiff, TextDiffReporter};
use askama::Template;
use semdiff_core::DetailReporter;
use semdiff_output::html::{HtmlReport, HtmlReportError};
use semdiff_tree_fs::FileLeaf;
use similar::ChangeTag;
use thiserror::Error;

const COMPARES_NAME: &str = "text";

#[derive(Debug, Error)]
pub enum TextDiffReportError {
    #[error("html report error: {0}")]
    HtmlReport(#[from] HtmlReportError),
}

#[derive(Template)]
#[template(path = "text_preview.html")]
struct TextPreviewTemplate<'a> {
    body: TextPreviewBody<'a>,
}

enum TextPreviewBody<'a> {
    Unchanged {
        body: &'a str,
    },
    Modified {
        diff: &'a similar::TextDiff<'a, 'a, 'a, [u8]>,
    },
    Added {
        body: &'a str,
    },
    Deleted {
        body: &'a str,
    },
}

impl TextPreviewTemplate<'_> {
    fn is_equal(change: &similar::Change<&[u8]>) -> bool {
        matches!(change.tag(), ChangeTag::Equal)
    }
}

#[derive(Template)]
#[template(path = "text_detail.html")]
struct TextDetailTemplate<'a> {
    detail: TextDetailBody<'a>,
}

enum TextDetailBody<'a> {
    Diff {
        lines: &'a similar::TextDiff<'a, 'a, 'a, [u8]>,
    },
    Single {
        label: &'a str,
        body: &'a str,
    },
}

impl TextDetailBody<'_> {
    fn is_multicolumn(&self) -> bool {
        matches!(self, TextDetailBody::Diff { .. })
    }
}

impl DetailReporter<TextDiff, FileLeaf, HtmlReport> for TextDiffReporter {
    type Error = TextDiffReportError;

    fn available(&self, data: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(str::from_utf8(&data.content).is_ok())
    }

    fn report_unchanged(&self, name: &str, diff: TextDiff, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let preview_html = TextPreviewTemplate {
            body: TextPreviewBody::Unchanged { body: diff.expected() },
        };
        let detail_html = TextDetailTemplate {
            detail: TextDetailBody::Single {
                label: "same",
                body: diff.expected(),
            },
        };
        reporter.record_unchanged(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }

    fn report_modified(&self, name: &str, diff: TextDiff, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let diff_view = diff.diff();
        let preview_html = TextPreviewTemplate {
            body: TextPreviewBody::Modified { diff: &diff_view },
        };
        let detail_html = TextDetailTemplate {
            detail: TextDetailBody::Diff { lines: &diff_view },
        };
        reporter.record_modified(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }

    fn report_added(&self, name: &str, data: FileLeaf, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let actual_text = str::from_utf8(&data.content).expect("Invalid content");
        let preview_html = TextPreviewTemplate {
            body: TextPreviewBody::Added { body: actual_text },
        };
        let detail_html = TextDetailTemplate {
            detail: TextDetailBody::Single {
                label: "added",
                body: actual_text,
            },
        };
        reporter.record_added(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }

    fn report_deleted(&self, name: &str, data: FileLeaf, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let expected_text = str::from_utf8(&data.content).expect("Invalid content");
        let preview_html = TextPreviewTemplate {
            body: TextPreviewBody::Deleted { body: expected_text },
        };
        let detail_html = TextDetailTemplate {
            detail: TextDetailBody::Single {
                label: "deleted",
                body: expected_text,
            },
        };
        reporter.record_deleted(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }
}
