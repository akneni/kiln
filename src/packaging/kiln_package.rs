use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KilnPackageConfig {
    pub metadata: Metadata,
}

impl KilnPackageConfig {
    pub fn new(include_dir: String, source_dir: String) -> Self {
        let metadata = Metadata {
            include_dir,
            source_dir,
        };
        Self { metadata }
    }

    pub fn from(path: impl AsRef<Path>) -> Result<Self> {
        let s = fs::read_to_string(path)?;
        let c: Self = toml::from_str(&s)?;
        Ok(c)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub include_dir: String,
    pub source_dir: String,
}
