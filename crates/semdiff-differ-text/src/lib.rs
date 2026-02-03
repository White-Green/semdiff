use memmap2::Mmap;
use mime::Mime;
use semdiff_core::{Diff, DiffCalculator, MayUnsupported};
use semdiff_tree_fs::FileLeaf;
use similar::TextDiffConfig;
use std::convert;
use std::sync::Arc;

pub mod report_html;
pub mod report_json;
pub mod report_summary;

#[cfg(test)]
mod tests;

pub struct TextDiffReporter;

#[derive(Debug)]
pub struct TextDiff {
    equal: bool,
    expected: Arc<Mmap>,
    actual: Arc<Mmap>,
}

impl Diff for TextDiff {
    fn equal(&self) -> bool {
        self.equal
    }
}

impl TextDiff {
    fn diff(&self) -> similar::TextDiff<'_, '_, '_, [u8]> {
        text_diff_lines(&self.expected[..], &self.actual[..])
    }
}

fn text_diff_lines<'a>(expected: &'a [u8], actual: &'a [u8]) -> similar::TextDiff<'a, 'a, 'a, [u8]> {
    TextDiffConfig::default()
        .algorithm(similar::Algorithm::Patience)
        .diff_lines(expected, actual)
}

fn is_text_file(kind: &Mime, body: &[u8]) -> bool {
    if is_text_mime(kind) {
        return true;
    }
    if is_binary_mime(kind) {
        return false;
    }

    let Ok(text) = str::from_utf8(body) else {
        return false;
    };

    text.chars()
        .all(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
}

fn is_text_mime(kind: &Mime) -> bool {
    kind.type_() == mime::TEXT
        || matches!(
            kind.essence_str(),
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-javascript"
                | "application/x-www-form-urlencoded"
                | "application/yaml"
                | "application/x-yaml"
                | "application/toml"
        )
}

fn is_binary_mime(kind: &Mime) -> bool {
    kind == &mime::APPLICATION_OCTET_STREAM
        || kind.type_() == mime::IMAGE
        || kind.type_() == mime::AUDIO
        || kind.type_() == mime::VIDEO
        || matches!(
            kind.essence_str(),
            "application/pdf"
                | "application/zip"
                | "application/gzip"
                | "application/x-tar"
                | "application/x-7z-compressed"
                | "application/x-rar-compressed"
                | "application/x-bzip2"
        )
}

#[derive(Default)]
pub struct TextDiffCalculator;

impl DiffCalculator<FileLeaf> for TextDiffCalculator {
    type Error = convert::Infallible;
    type Diff = TextDiff;

    fn diff(
        &self,
        _name: &str,
        expected: FileLeaf,
        actual: FileLeaf,
    ) -> Result<MayUnsupported<Self::Diff>, Self::Error> {
        'available: {
            if is_text_mime(&expected.kind) && is_text_mime(&actual.kind) {
                break 'available;
            }
            if is_binary_mime(&expected.kind) || is_binary_mime(&actual.kind) {
                return Ok(MayUnsupported::Unsupported);
            }
            let Ok(expected) = str::from_utf8(&expected.content) else {
                return Ok(MayUnsupported::Unsupported);
            };
            let Ok(actual) = str::from_utf8(&actual.content) else {
                return Ok(MayUnsupported::Unsupported);
            };

            if expected
                .chars()
                .all(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
            {
                return Ok(MayUnsupported::Unsupported);
            }
            if actual
                .chars()
                .all(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
            {
                return Ok(MayUnsupported::Unsupported);
            }
        }
        Ok(MayUnsupported::Ok(TextDiff {
            equal: <[u8] as PartialEq<[u8]>>::eq(&expected.content, &actual.content),
            expected: expected.content,
            actual: actual.content,
        }))
    }
}
