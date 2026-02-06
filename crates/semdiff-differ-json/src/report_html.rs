use crate::{ChangeTag, JsonDiff, JsonDiffBody, JsonDiffLine, JsonDiffReporter, is_json_mime, try_into_json};
use askama::Template;
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::html::{HtmlReport, HtmlReportError};
use thiserror::Error;

const COMPARES_NAME: &str = "json";

#[derive(Debug, Error)]
pub enum JsonDiffReportError {
    #[error("html report error: {0}")]
    HtmlReport(#[from] HtmlReportError),
}

#[derive(Template)]
#[template(path = "json_preview.html")]
struct JsonPreviewTemplate<'a> {
    body: JsonPreviewBody<'a>,
}

enum JsonPreviewBody<'a> {
    Unchanged { body: &'a str },
    Modified { lines: &'a [JsonDiffLine] },
    Added { body: &'a str },
    Deleted { body: &'a str },
}

impl JsonPreviewTemplate<'_> {
    fn is_equal(line: &&JsonDiffLine) -> bool {
        matches!(line.tag(), ChangeTag::Unchanged)
    }
}

#[derive(Template)]
#[template(path = "json_detail.html")]
struct JsonDetailTemplate<'a> {
    detail: JsonDetailBody<'a>,
}

enum JsonDetailBody<'a> {
    Diff { lines: &'a [JsonDiffLine] },
    Single { label: &'a str, body: &'a str },
}

impl JsonDetailBody<'_> {
    fn is_multicolumn(&self) -> bool {
        matches!(self, JsonDetailBody::Diff { .. })
    }
}

impl DetailReporter<JsonDiff, FileLeaf, HtmlReport> for JsonDiffReporter {
    type Error = JsonDiffReportError;

    fn report_unchanged(
        &self,
        name: &str,
        diff: &JsonDiff,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let JsonDiffBody::Equal(body) = diff.body() else {
            debug_assert!(false, "report_unchanged called with modified diff");
            return Ok(MayUnsupported::Ok(()));
        };
        let preview_html = JsonPreviewTemplate {
            body: JsonPreviewBody::Unchanged { body },
        };
        let detail_html = JsonDetailTemplate {
            detail: JsonDetailBody::Single { label: "same", body },
        };
        reporter.record_unchanged(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        name: &str,
        diff: &JsonDiff,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let JsonDiffBody::Modified(lines) = diff.body() else {
            debug_assert!(false, "report_modified called with equal diff");
            return Ok(MayUnsupported::Ok(()));
        };
        let preview_html = JsonPreviewTemplate {
            body: JsonPreviewBody::Modified { lines },
        };
        let detail_html = JsonDetailTemplate {
            detail: JsonDetailBody::Diff { lines },
        };
        reporter.record_modified(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        name: &str,
        data: &FileLeaf,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_json_mime(&data.kind) {
            return Ok(MayUnsupported::Unsupported);
        }
        let Some(body) = try_into_json(&data.content) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let preview_html = JsonPreviewTemplate {
            body: JsonPreviewBody::Added { body: &body },
        };
        let detail_html = JsonDetailTemplate {
            detail: JsonDetailBody::Single {
                label: "added",
                body: &body,
            },
        };
        reporter.record_added(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        name: &str,
        data: &FileLeaf,
        reporter: &HtmlReport,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_json_mime(&data.kind) {
            return Ok(MayUnsupported::Unsupported);
        }
        let Some(body) = try_into_json(&data.content) else {
            return Ok(MayUnsupported::Unsupported);
        };
        let preview_html = JsonPreviewTemplate {
            body: JsonPreviewBody::Deleted { body: &body },
        };
        let detail_html = JsonDetailTemplate {
            detail: JsonDetailBody::Single {
                label: "deleted",
                body: &body,
            },
        };
        reporter.record_deleted(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(MayUnsupported::Ok(()))
    }
}
