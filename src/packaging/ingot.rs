use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

use crate::config::KilnIngot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngotMetadata {
    pub metadata: Metadata,
}

impl IngotMetadata {
    pub fn to(&self, path: impl AsRef<Path>) -> Result<()> {
        let toml_str = toml::to_string_pretty(self)?;
        fs::write(path, toml_str)?;
        Ok(())
    }

    pub fn from(path: impl AsRef<Path>) -> Result<Self> {
        let s = fs::read_to_string(path)?;
        let c: Self = toml::from_str(&s)?;
        Ok(c)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    // Some ingots will only have code, and some may only have precompiled static libraries. 
    // These fields tell us which is which. 
    pub source_support: bool,
    pub staticlib_support: bool,
    pub sys_libs: Vec<String>,
    pub ingot_deps: Vec<KilnIngot>,
}

