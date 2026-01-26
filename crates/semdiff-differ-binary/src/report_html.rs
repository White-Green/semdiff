use crate::{BinaryDiff, BinaryDiffReporter};
use askama::Template;
use semdiff_core::DetailReporter;
use semdiff_output::html::{HtmlReport, HtmlReportError};
use semdiff_tree_fs::FileLeaf;
use thiserror::Error;

const COMPARES_NAME: &str = "binary";

#[derive(Debug, Error)]
pub enum BinaryDiffReportError {
    #[error("html report error: {0}")]
    HtmlReport(#[from] HtmlReportError),
}

#[derive(Template)]
#[template(path = "binary_preview.html")]
struct BinaryPreviewTemplate<'a> {
    message: &'a str,
}

#[derive(Template)]
#[template(path = "binary_detail.html")]
struct BinaryDetailTemplate;

impl DetailReporter<BinaryDiff, FileLeaf, HtmlReport> for BinaryDiffReporter {
    type Error = BinaryDiffReportError;

    fn available(&self, _data: &FileLeaf) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn report_unchanged(&self, name: &str, _diff: BinaryDiff, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let preview_html = BinaryPreviewTemplate {
            message: "バイナリは同一です。詳細表示は未実装です。",
        };
        let detail_html = BinaryDetailTemplate;
        reporter.record_unchanged(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }

    fn report_modified(&self, name: &str, _diff: BinaryDiff, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let preview_html = BinaryPreviewTemplate {
            message: "バイナリが変更されています。詳細表示は未実装です。",
        };
        let detail_html = BinaryDetailTemplate;
        reporter.record_modified(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }

    fn report_added(&self, name: &str, data: FileLeaf, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let message = format!(
            "バイナリが追加されました ({} bytes)。詳細表示は未実装です。",
            data.meta.size
        );
        let preview_html = BinaryPreviewTemplate { message: &message };
        let detail_html = BinaryDetailTemplate;
        reporter.record_added(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }

    fn report_deleted(&self, name: &str, data: FileLeaf, reporter: &HtmlReport) -> Result<(), Self::Error> {
        let message = format!(
            "バイナリが削除されました ({} bytes)。詳細表示は未実装です。",
            data.meta.size
        );
        let preview_html = BinaryPreviewTemplate { message: &message };
        let detail_html = BinaryDetailTemplate;
        reporter.record_deleted(name, COMPARES_NAME, preview_html, detail_html)?;
        Ok(())
    }
}
