use std::{fs, path::Path};

use serde_json::{json, Value};

use crate::{dev_env_config::{self, DevEnvConfig, EditorType}, kiln_errors::{KilnError, KilnResult}};



pub(super) fn add_include(dev_config: &DevEnvConfig, includes: &[String], proj_dir: impl AsRef<Path>) -> KilnResult<()> {

    let editor = match dev_config.editor {
        Some(e) => e,
        None => return Err(KilnError::new_unknown("Dev config file doesn't exist")),
    };
    match editor {
        EditorType::VsCode => {

        }
        _ => {

        }
    }

    Ok(())
}


fn add_include_vscode(includes: &[String], proj_dir: impl AsRef<Path>) {
    let default_config = include_str!("../assets/vscode-default-properties.json");
    let default_include = "${workspaceFolder}/include/**";

    let config_file = proj_dir.as_ref().join(".vscode/c_cpp_properties.json");

    let config = fs::read_to_string(&config_file).unwrap_or(default_config.to_string());

    let mut config: Value = serde_json::from_str(&config).unwrap();

    if let Some(c) = config.get_mut("configurations") {
        
    } else {
        let configurations = json!(r#"{"name": "Default",
        "includePath": [
            "${workspaceFolder}/**"
        ]}"#);

        config["configurations"] = configurations;
    }


}