use memmap2::Mmap;
use mime::Mime;
use semdiff_core::{Diff, DiffCalculator};
use semdiff_tree_fs::FileLeaf;
use similar::TextDiffConfig;
use std::convert;

pub mod report_html;
pub mod report_json;
pub mod report_summary;

pub struct TextDiffReporter;

#[derive(Debug)]
pub struct TextDiff {
    equal: bool,
    expected: Mmap,
    actual: Mmap,
}

impl Diff for TextDiff {
    fn equal(&self) -> bool {
        self.equal
    }
}

impl TextDiff {
    pub fn diff(&self) -> similar::TextDiff<'_, '_, '_, [u8]> {
        TextDiffConfig::default()
            .algorithm(similar::Algorithm::Patience)
            .diff_lines(&*self.expected, &*self.actual)
    }
}

pub fn is_text_file(kind: &Mime, body: &[u8]) -> bool {
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

    fn available(&self, expected: &FileLeaf, actual: &FileLeaf) -> Result<bool, Self::Error> {
        if is_text_mime(&expected.kind) && is_text_mime(&actual.kind) {
            return Ok(true);
        }
        if is_binary_mime(&expected.kind) || is_binary_mime(&actual.kind) {
            return Ok(false);
        }
        let Ok(expected) = str::from_utf8(&expected.content) else {
            return Ok(false);
        };
        let Ok(actual) = str::from_utf8(&actual.content) else {
            return Ok(false);
        };

        Ok(expected
            .chars()
            .all(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
            && actual
                .chars()
                .all(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t')))
    }

    fn diff(&self, _name: &str, expected: FileLeaf, actual: FileLeaf) -> Result<Self::Diff, Self::Error> {
        Ok(TextDiff {
            equal: <[u8]>::eq(&*expected.content, &*actual.content),
            expected: expected.content,
            actual: actual.content,
        })
    }
}
