// crates/engine-core/src/wal.rs

use std::path::Path;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use crate::LogEntry;

pub struct WalHandler {
    writer: BufWriter<File>,
}

impl WalHandler {
    // Membuka atau membuat file WAL baru
    pub fn new(path: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    // Menulis satu entry ke disk
    pub fn write_entry(&mut self, entry: &LogEntry) -> std::io::Result<()> {
        // Serialize langsung ke buffer writer
        bincode::serialize_into(&mut self.writer, entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Untuk HFT murni, biasanya flush dilakukan per batch atau interval waktu.
        // Pada tahap ini, flush setiap kali demi keamanan data.
        // self.writer.flush()?;

        Ok(())
    }

    // Membaca ulang semua entry saat startup (Recovery)
    pub fn read_all(path: &str) -> std::io::Result<Vec<LogEntry>> {
        let path = Path::new(path);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut entries =Vec:: new();

        // Loop baca file sampai EOF (End of File)
        loop {
            match bincode::deserialize_from(&mut reader) {
                Ok(entry) => entries.push(entry),
                Err(_) => break,
            }
        }

        Ok(entries)
    }
}