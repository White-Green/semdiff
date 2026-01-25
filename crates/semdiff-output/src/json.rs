use dashmap::DashMap;
use semdiff_core::Reporter;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct JsonReport<W> {
    writer: W,
    unchanged: AtomicUsize,
    modified: AtomicUsize,
    added: AtomicUsize,
    deleted: AtomicUsize,
    entries: DashMap<String, JsonReportEntry>,
}

impl<W> JsonReport<W> {
    pub fn new(writer: W) -> JsonReport<W> {
        JsonReport {
            writer,
            unchanged: AtomicUsize::new(0),
            modified: AtomicUsize::new(0),
            added: AtomicUsize::new(0),
            deleted: AtomicUsize::new(0),
            entries: DashMap::new(),
        }
    }

    pub fn record_unchanged(
        &self,
        name: &[String],
        compares: &'static str,
        additional: impl Into<BTreeMap<String, Value>>,
    ) {
        self.unchanged.fetch_add(1, Ordering::Relaxed);
        self.insert_entry(
            name,
            JsonReportEntry::new(JsonEntryStatus::Unchanged, compares, additional.into()),
        );
    }

    pub fn record_modified(
        &self,
        name: &[String],
        compares: &'static str,
        additional: impl Into<BTreeMap<String, Value>>,
    ) {
        self.modified.fetch_add(1, Ordering::Relaxed);
        self.insert_entry(
            name,
            JsonReportEntry::new(JsonEntryStatus::Modified, compares, additional.into()),
        );
    }

    pub fn record_added(
        &self,
        name: &[String],
        compares: &'static str,
        additional: impl Into<BTreeMap<String, Value>>,
    ) {
        self.added.fetch_add(1, Ordering::Relaxed);
        self.insert_entry(
            name,
            JsonReportEntry::new(JsonEntryStatus::Added, compares, additional.into()),
        );
    }

    pub fn record_deleted(
        &self,
        name: &[String],
        compares: &'static str,
        additional: impl Into<BTreeMap<String, Value>>,
    ) {
        self.deleted.fetch_add(1, Ordering::Relaxed);
        self.insert_entry(
            name,
            JsonReportEntry::new(JsonEntryStatus::Deleted, compares, additional.into()),
        );
    }

    fn insert_entry(&self, name: &[String], entry: JsonReportEntry) {
        let key = join_name(name);
        assert!(self.entries.insert(key, entry).is_none());
    }
}

#[derive(Serialize)]
struct JsonReportOutput {
    unchanged: usize,
    modified: usize,
    added: usize,
    deleted: usize,
    entries: BTreeMap<String, JsonReportEntry>,
}

#[derive(Serialize)]
struct JsonReportEntry {
    status: JsonEntryStatus,
    compares: &'static str,
    #[serde(flatten)]
    additional: BTreeMap<String, Value>,
}

impl JsonReportEntry {
    fn new(status: JsonEntryStatus, compares: &'static str, additional: BTreeMap<String, Value>) -> JsonReportEntry {
        JsonReportEntry {
            status,
            compares,
            additional,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum JsonEntryStatus {
    Unchanged,
    Modified,
    Added,
    Deleted,
}

fn join_name(name: &[String]) -> String {
    let Some((first, tail)) = name.split_first() else {
        return String::new();
    };
    tail.iter().fold(first.clone(), |mut acc, item| {
        acc.push('/');
        acc.push_str(&item);
        acc
    })
}

impl<W: Write> Reporter for JsonReport<W> {
    type Error = serde_json::Error;

    fn start(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn finish(self) -> Result<(), Self::Error> {
        let JsonReport {
            mut writer,
            unchanged,
            modified,
            added,
            deleted,
            entries,
        } = self;
        let output = JsonReportOutput {
            unchanged: unchanged.into_inner(),
            modified: modified.into_inner(),
            added: added.into_inner(),
            deleted: deleted.into_inner(),
            entries: BTreeMap::from_iter(entries),
        };
        serde_json::to_writer_pretty(&mut writer, &output)
    }
}
