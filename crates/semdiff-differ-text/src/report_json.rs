use crate::{TextDiff, TextDiffReporter, is_text_file};
use semdiff_core::fs::FileLeaf;
use semdiff_core::{DetailReporter, MayUnsupported};
use semdiff_output::json::JsonReport;
use serde::Serialize;
use similar::ChangeTag;
use std::convert;

const COMPARES_NAME: &str = "text";

impl<W> DetailReporter<TextDiff, FileLeaf, JsonReport<W>> for TextDiffReporter {
    type Error = convert::Infallible;

    fn report_unchanged(
        &self,
        name: &str,
        _diff: TextDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        reporter.record_unchanged(name, COMPARES_NAME, ());
        Ok(MayUnsupported::Ok(()))
    }

    fn report_modified(
        &self,
        name: &str,
        diff: TextDiff,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        let s = diff
            .diff()
            .iter_all_changes()
            .fold(S::default(), |S { added, deleted }, change| match change.tag() {
                ChangeTag::Equal => S { added, deleted },
                ChangeTag::Delete => S {
                    added,
                    deleted: deleted + 1,
                },
                ChangeTag::Insert => S {
                    added: added + 1,
                    deleted,
                },
            });
        #[derive(Debug, Default, Serialize)]
        struct S {
            added: usize,
            deleted: usize,
        }
        reporter.record_modified(name, COMPARES_NAME, s);
        Ok(MayUnsupported::Ok(()))
    }

    fn report_added(
        &self,
        name: &str,
        data: FileLeaf,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_text_file(&data.kind, &data.content) {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.record_added(name, COMPARES_NAME, ());
        Ok(MayUnsupported::Ok(()))
    }

    fn report_deleted(
        &self,
        name: &str,
        data: FileLeaf,
        reporter: &JsonReport<W>,
    ) -> Result<MayUnsupported<()>, Self::Error> {
        if !is_text_file(&data.kind, &data.content) {
            return Ok(MayUnsupported::Unsupported);
        }
        reporter.record_deleted(name, COMPARES_NAME, ());
        Ok(MayUnsupported::Ok(()))
    }
}
