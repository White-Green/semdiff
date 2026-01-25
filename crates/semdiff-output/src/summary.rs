use semdiff_core::Reporter;
use std::io;
use std::io::Write;
use std::sync::atomic;
use std::sync::atomic::AtomicUsize;

pub struct SummaryReport<W> {
    writer: W,
    unchanged: AtomicUsize,
    modified: AtomicUsize,
    added: AtomicUsize,
    deleted: AtomicUsize,
}

impl<W> SummaryReport<W> {
    pub fn new(writer: W) -> SummaryReport<W> {
        SummaryReport {
            writer,
            unchanged: AtomicUsize::new(0),
            modified: AtomicUsize::new(0),
            added: AtomicUsize::new(0),
            deleted: AtomicUsize::new(0),
        }
    }

    pub fn increment_unchanged(&self) {
        self.unchanged.fetch_add(1, atomic::Ordering::Relaxed);
    }

    pub fn increment_modified(&self) {
        self.modified.fetch_add(1, atomic::Ordering::Relaxed);
    }

    pub fn increment_added(&self) {
        self.added.fetch_add(1, atomic::Ordering::Relaxed);
    }

    pub fn increment_deleted(&self) {
        self.deleted.fetch_add(1, atomic::Ordering::Relaxed);
    }
}

impl<W: Write> Reporter for SummaryReport<W> {
    type Error = io::Error;

    fn start(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn finish(self) -> Result<(), Self::Error> {
        let SummaryReport {
            mut writer,
            unchanged,
            modified,
            added,
            deleted,
        } = self;
        let unchanged = unchanged.into_inner();
        let modified = modified.into_inner();
        let added = added.into_inner();
        let deleted = deleted.into_inner();

        writeln!(
            writer,
            r#"Summary Report
Unchanged: {}
Modified:  {}
Added:     {}
Deleted:   {}"#,
            unchanged, modified, added, deleted
        )
    }
}
