use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Serialize, Default)]
pub struct ProcessingReport {
    pub report_version: u32,
    pub tool_version: String,
    pub input: InputSummary,
    pub result: ResultSummary,
    pub events: EventCounters,
}

#[derive(Debug, Serialize, Default)]
pub struct InputSummary {
    pub path: String,
    pub format: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Default)]
pub struct ResultSummary {
    pub flows_read: u64,
    pub flows_emitted: u64,
    pub paths_in_spec: u64,
}

#[derive(Debug, Serialize, Default)]
pub struct EventCounters {
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub cap_fired: HashMap<String, u64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub rejected: HashMap<String, u64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub parse_error: HashMap<String, u64>,
}

impl ProcessingReport {
    pub fn new() -> Self {
        Self {
            report_version: 1,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        }
    }

    pub fn write_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("tmp");
        let file = std::fs::File::create(&tmp_path)?;
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self).map_err(std::io::Error::other)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }
}
