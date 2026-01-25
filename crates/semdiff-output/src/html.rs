use semdiff_core::Reporter;
use std::path::PathBuf;
use thiserror::Error;

pub struct HtmlReport {}

impl HtmlReport {
    pub fn new(_root: PathBuf) -> HtmlReport {
        HtmlReport {}
    }
}

#[derive(Debug, Error)]
pub enum HtmlReportError {}

impl Reporter for HtmlReport {
    type Error = HtmlReportError;

    fn start(&mut self) -> Result<(), Self::Error> {
        todo!()
    }

    fn finish(self) -> Result<(), Self::Error> {
        todo!()
    }
}
