use semdiff_core::Reporter;
use std::io::Write;
use thiserror::Error;

pub struct JsonReport<W> {
    writer: W,
}

impl<W: Write> JsonReport<W> {
    pub fn new(writer: W) -> JsonReport<W> {
        JsonReport { writer }
    }
}

#[derive(Debug, Error)]
pub enum JsonReportError {}

impl<W: Write> Reporter for JsonReport<W> {
    type Error = JsonReportError;

    fn start(&mut self) -> Result<(), Self::Error> {
        todo!()
    }

    fn finish(self) -> Result<(), Self::Error> {
        todo!()
    }
}
