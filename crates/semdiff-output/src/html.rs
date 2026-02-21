use askama::Template;
use dashmap::DashMap;
use semdiff_core::Reporter;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use xxhash_rust::xxh3::xxh3_128;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub struct HtmlReport {
    root: PathBuf,
    detail_dir: PathBuf,
    detail_dir_name: String,
    back_link: String,
    unchanged_entries: DashMap<String, HtmlReportEntry>,
    modified_entries: DashMap<String, HtmlReportEntry>,
    added_entries: DashMap<String, HtmlReportEntry>,
    deleted_entries: DashMap<String, HtmlReportEntry>,
}

impl HtmlReport {
    pub fn new(root: PathBuf) -> HtmlReport {
        let detail_dir_name = root
            .file_stem()
            .map(|name| format!("{}_details", name.to_string_lossy()))
            .unwrap_or_else(|| "details".to_string());
        let detail_dir = root.parent().unwrap_or_else(|| Path::new(".")).join(&detail_dir_name);
        let root_file_name = root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "index.html".to_string());
        let back_link = format!("../{}", root_file_name);
        HtmlReport {
            root,
            detail_dir,
            detail_dir_name,
            back_link,
            unchanged_entries: DashMap::new(),
            modified_entries: DashMap::new(),
            added_entries: DashMap::new(),
            deleted_entries: DashMap::new(),
        }
    }

    pub fn record_unchanged(
        &self,
        name: &str,
        compares: &'static str,
        preview_html: impl Template,
        detail_html: impl Template,
    ) -> Result<(), HtmlReportError> {
        let preview_html = preview_html.render()?;
        let detail_html = detail_html.render()?;
        let detail_file_name = Some(self.write_detail(name, HtmlEntryStatus::Unchanged, compares, &detail_html)?);
        self.insert_entry(
            HtmlEntryStatus::Unchanged,
            name,
            HtmlReportEntry::new(HtmlEntryStatus::Unchanged, compares, preview_html, detail_file_name),
        );
        Ok(())
    }

    pub fn record_modified(
        &self,
        name: &str,
        compares: &'static str,
        preview_html: impl Template,
        detail_html: impl Template,
    ) -> Result<(), HtmlReportError> {
        let preview_html = preview_html.render()?;
        let detail_html = detail_html.render()?;
        let detail_file_name = Some(self.write_detail(name, HtmlEntryStatus::Modified, compares, &detail_html)?);
        self.insert_entry(
            HtmlEntryStatus::Modified,
            name,
            HtmlReportEntry::new(HtmlEntryStatus::Modified, compares, preview_html, detail_file_name),
        );
        Ok(())
    }

    pub fn record_added(
        &self,
        name: &str,
        compares: &'static str,
        preview_html: impl Template,
        detail_html: impl Template,
    ) -> Result<(), HtmlReportError> {
        let preview_html = preview_html.render()?;
        let detail_html = detail_html.render()?;
        let detail_file_name = Some(self.write_detail(name, HtmlEntryStatus::Added, compares, &detail_html)?);
        self.insert_entry(
            HtmlEntryStatus::Added,
            name,
            HtmlReportEntry::new(HtmlEntryStatus::Added, compares, preview_html, detail_file_name),
        );
        Ok(())
    }

    pub fn record_deleted(
        &self,
        name: &str,
        compares: &'static str,
        preview_html: impl Template,
        detail_html: impl Template,
    ) -> Result<(), HtmlReportError> {
        let preview_html = preview_html.render()?;
        let detail_html = detail_html.render()?;
        let detail_file_name = Some(self.write_detail(name, HtmlEntryStatus::Deleted, compares, &detail_html)?);
        self.insert_entry(
            HtmlEntryStatus::Deleted,
            name,
            HtmlReportEntry::new(HtmlEntryStatus::Deleted, compares, preview_html, detail_file_name),
        );
        Ok(())
    }

    fn insert_entry(&self, status: HtmlEntryStatus, name: &str, entry: HtmlReportEntry) {
        let key = name.to_owned();
        let previous = match status {
            HtmlEntryStatus::Unchanged => self.unchanged_entries.insert(key, entry),
            HtmlEntryStatus::Modified => self.modified_entries.insert(key, entry),
            HtmlEntryStatus::Added => self.added_entries.insert(key, entry),
            HtmlEntryStatus::Deleted => self.deleted_entries.insert(key, entry),
        };
        assert!(previous.is_none());
    }

    fn make_detail_filename(name: &str) -> String {
        format!("{}.html", Self::make_detail_stem(name))
    }

    fn make_detail_stem(name: &str) -> String {
        let sanitized = Self::sanitize_segment(name);
        let hash = xxh3_128(name.as_bytes());
        format!("{}_{}", sanitized, hash)
    }

    fn sanitize_segment(value: &str) -> String {
        value
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                    ch
                } else {
                    '_'
                }
            })
            .collect()
    }

    pub fn write_detail_asset(
        &self,
        name: &str,
        label: &str,
        extension: &str,
        f: impl FnOnce(&mut BufWriter<File>) -> Result<(), HtmlReportError>,
    ) -> Result<String, HtmlReportError> {
        fs::create_dir_all(&self.detail_dir)?;
        let stem = Self::make_detail_stem(name);
        let label = Self::sanitize_segment(label);
        let extension = extension.trim_start_matches('.');
        let file_name = if extension.is_empty() {
            format!("{}_{}", stem, label)
        } else {
            format!("{}_{}.{}", stem, label, extension)
        };
        let file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(self.detail_dir.join(&file_name))?;
        let mut writer = BufWriter::new(file);
        f(&mut writer)?;
        writer.flush()?;
        Ok(file_name)
    }

    pub fn detail_asset_path(&self, file_name: &str) -> String {
        format!("{}/{}", self.detail_dir_name, file_name)
    }

    fn write_detail(
        &self,
        name: &str,
        status: HtmlEntryStatus,
        compares: &'static str,
        body_html: &str,
    ) -> Result<String, HtmlReportError> {
        fs::create_dir_all(&self.detail_dir)?;
        let file_name = Self::make_detail_filename(name);
        let template = DetailTemplate {
            name,
            status_label: status.label(),
            status_class: status.class(),
            compares,
            body_html,
            back_link: &self.back_link,
        };
        let rendered = template.render()?;
        fs::write(self.detail_dir.join(&file_name), rendered)?;
        Ok(file_name)
    }
}

struct HtmlReportEntry {
    status: HtmlEntryStatus,
    compares: &'static str,
    preview_html: String,
    detail_file_name: Option<String>,
}

impl HtmlReportEntry {
    fn new(
        status: HtmlEntryStatus,
        compares: &'static str,
        preview_html: String,
        detail_file_name: Option<String>,
    ) -> HtmlReportEntry {
        HtmlReportEntry {
            status,
            compares,
            preview_html,
            detail_file_name,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HtmlEntryStatus {
    Unchanged,
    Modified,
    Added,
    Deleted,
}

impl HtmlEntryStatus {
    fn label(self) -> &'static str {
        match self {
            HtmlEntryStatus::Unchanged => "unchanged",
            HtmlEntryStatus::Modified => "modified",
            HtmlEntryStatus::Added => "added",
            HtmlEntryStatus::Deleted => "deleted",
        }
    }

    fn class(self) -> &'static str {
        match self {
            HtmlEntryStatus::Unchanged => "unchanged",
            HtmlEntryStatus::Modified => "modified",
            HtmlEntryStatus::Added => "added",
            HtmlEntryStatus::Deleted => "deleted",
        }
    }
}

#[derive(Debug, Error)]
pub enum HtmlReportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("template error: {0}")]
    Template(#[from] askama::Error),
}

#[derive(Template)]
#[template(path = "report_root.html")]
struct RootTemplate<'a> {
    total: usize,
    unchanged: usize,
    modified: usize,
    added: usize,
    deleted: usize,
    entry_groups: &'a [HtmlEntryGroup],
}

struct HtmlEntryGroup {
    status_label: &'static str,
    status_class: &'static str,
    entries: Vec<HtmlEntryView>,
}

#[derive(Template)]
#[template(path = "report_detail.html")]
struct DetailTemplate<'a> {
    name: &'a str,
    status_label: &'a str,
    status_class: &'a str,
    compares: &'a str,
    body_html: &'a str,
    back_link: &'a str,
}

struct HtmlEntryView {
    name: String,
    status_label: &'static str,
    status_class: &'static str,
    compares: &'static str,
    preview_html: String,
    detail_link: String,
}

impl Reporter for HtmlReport {
    type Error = HtmlReportError;

    fn start(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn finish(self) -> Result<(), Self::Error> {
        let HtmlReport {
            root,
            detail_dir_name,
            unchanged_entries,
            modified_entries,
            added_entries,
            deleted_entries,
            ..
        } = self;
        let unchanged_count = unchanged_entries.len();
        let modified_count = modified_entries.len();
        let added_count = added_entries.len();
        let deleted_count = deleted_entries.len();
        let status_order = [
            (HtmlEntryStatus::Modified, modified_entries),
            (HtmlEntryStatus::Deleted, deleted_entries),
            (HtmlEntryStatus::Added, added_entries),
            (HtmlEntryStatus::Unchanged, unchanged_entries),
        ];
        let mut entry_groups = Vec::with_capacity(status_order.len());
        for (status, entries) in status_order {
            let sorted_entries = BTreeMap::from_iter(entries);
            let mut group_entries = Vec::new();
            for (name, entry) in sorted_entries {
                let detail_link = entry
                    .detail_file_name
                    .as_ref()
                    .map(|file_name| format!("{}/{}", detail_dir_name, file_name))
                    .unwrap_or_default();
                group_entries.push(HtmlEntryView {
                    name,
                    status_label: entry.status.label(),
                    status_class: entry.status.class(),
                    compares: entry.compares,
                    preview_html: entry.preview_html.clone(),
                    detail_link,
                });
            }
            entry_groups.push(HtmlEntryGroup {
                status_label: status.label(),
                status_class: status.class(),
                entries: group_entries,
            });
        }

        let template = RootTemplate {
            total: unchanged_count + modified_count + added_count + deleted_count,
            unchanged: unchanged_count,
            modified: modified_count,
            added: added_count,
            deleted: deleted_count,
            entry_groups: &entry_groups,
        };
        let rendered = template.render()?;
        fs::write(root, rendered)?;
        Ok(())
    }
}
