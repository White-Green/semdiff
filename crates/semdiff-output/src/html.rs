use askama::Template;
use dashmap::DashMap;
use semdiff_core::Reporter;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;

pub struct HtmlReport {
    root: PathBuf,
    detail_dir: PathBuf,
    detail_dir_name: String,
    back_link: String,
    unchanged: AtomicUsize,
    modified: AtomicUsize,
    added: AtomicUsize,
    deleted: AtomicUsize,
    entries: DashMap<String, HtmlReportEntry>,
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
            unchanged: AtomicUsize::new(0),
            modified: AtomicUsize::new(0),
            added: AtomicUsize::new(0),
            deleted: AtomicUsize::new(0),
            entries: DashMap::new(),
        }
    }

    pub fn record_unchanged(
        &self,
        name: &str,
        compares: &'static str,
        preview_html: impl Template,
        detail_html: impl Template,
    ) -> Result<(), HtmlReportError> {
        self.unchanged.fetch_add(1, Ordering::Relaxed);
        let preview_html = preview_html.render()?;
        let detail_html = detail_html.render()?;
        let detail_file_name = Some(self.write_detail(name, HtmlEntryStatus::Unchanged, compares, &detail_html)?);
        self.insert_entry(
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
        self.modified.fetch_add(1, Ordering::Relaxed);
        let preview_html = preview_html.render()?;
        let detail_html = detail_html.render()?;
        let detail_file_name = Some(self.write_detail(name, HtmlEntryStatus::Modified, compares, &detail_html)?);
        self.insert_entry(
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
        self.added.fetch_add(1, Ordering::Relaxed);
        let preview_html = preview_html.render()?;
        let detail_html = detail_html.render()?;
        let detail_file_name = Some(self.write_detail(name, HtmlEntryStatus::Added, compares, &detail_html)?);
        self.insert_entry(
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
        self.deleted.fetch_add(1, Ordering::Relaxed);
        let preview_html = preview_html.render()?;
        let detail_html = detail_html.render()?;
        let detail_file_name = Some(self.write_detail(name, HtmlEntryStatus::Deleted, compares, &detail_html)?);
        self.insert_entry(
            name,
            HtmlReportEntry::new(HtmlEntryStatus::Deleted, compares, preview_html, detail_file_name),
        );
        Ok(())
    }

    fn insert_entry(&self, name: &str, entry: HtmlReportEntry) {
        let key = name.to_owned();
        assert!(self.entries.insert(key, entry).is_none());
    }

    fn make_detail_filename(name: &str) -> String {
        let sanitized: String = name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        let hash = hasher.finish();
        format!("{}_{}.html", sanitized, hash)
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

#[derive(Clone, Copy)]
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
    unchanged: usize,
    modified: usize,
    added: usize,
    deleted: usize,
    entries: &'a [HtmlEntryView],
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
            unchanged,
            modified,
            added,
            deleted,
            entries,
            ..
        } = self;
        let sorted_entries: BTreeMap<String, HtmlReportEntry> = BTreeMap::from_iter(entries);
        let mut views = Vec::with_capacity(sorted_entries.len());

        for (name, entry) in sorted_entries {
            let detail_link = entry
                .detail_file_name
                .as_ref()
                .map(|file_name| format!("{}/{}", detail_dir_name, file_name))
                .unwrap_or_default();
            views.push(HtmlEntryView {
                name,
                status_label: entry.status.label(),
                status_class: entry.status.class(),
                compares: entry.compares,
                preview_html: entry.preview_html,
                detail_link,
            });
        }

        let template = RootTemplate {
            unchanged: unchanged.into_inner(),
            modified: modified.into_inner(),
            added: added.into_inner(),
            deleted: deleted.into_inner(),
            entries: &views,
        };
        let rendered = template.render()?;
        fs::write(root, rendered)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlReport;
    use askama::Template;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Template)]
    #[template(source = "preview", ext = "html")]
    struct PreviewTemplate;

    #[derive(Template)]
    #[template(source = "<div>detail</div>", ext = "html")]
    struct DetailTemplate;

    #[test]
    fn writes_detail_before_finish() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!("semdiff_html_report_test_{suffix}"));
        fs::create_dir_all(&base_dir).expect("failed to create temp dir");

        let root = base_dir.join("report.html");
        let report = HtmlReport::new(root.clone());

        report
            .record_modified("dir/file.txt", "text", PreviewTemplate, DetailTemplate)
            .expect("failed to record detail");

        let detail_dir = base_dir.join("report_details");
        let detail_file = detail_dir.join(HtmlReport::make_detail_filename("dir/file.txt"));
        assert!(detail_file.exists(), "detail file should be written before finish");

        fs::remove_dir_all(&base_dir).expect("failed to remove temp dir");
    }
}
