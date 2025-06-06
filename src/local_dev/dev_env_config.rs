use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, EnumIter)]
pub enum EditorType {
    VsCode,
    NeoVim,
    Zed,
    VisualStudio,
    Helix,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DevEnvConfig {
    pub editor: Option<EditorType>,
}
