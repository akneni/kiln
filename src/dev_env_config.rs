use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::kiln_errors::KilnResult;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, EnumIter)]
pub(super) enum EditorType {
    VsCode,
    NeoVim,
    Zed,
    VisualStudio,
    Helix,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct DevEnvConfig {
    pub editor: Option<EditorType>,
}

impl DevEnvConfig {
    pub(super) fn new(path: impl AsRef<Path>) -> KilnResult<Self> {
        let text = fs::read_to_string(path)?;

        let config: Self = toml::from_str(&text)?;
        Ok(config)
    }
}

