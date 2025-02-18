use std::{fs, path::Path};

use serde_json::{json, Value};

use crate::{config::Config, constants::DEV_ENV_CFG_FILE, dev_env_config::{self, DevEnvConfig, EditorType}, kiln_errors::{KilnError, KilnResult}};

pub(super) fn handle_editor_includes(config: &Config, proj_dir: impl AsRef<Path>) -> KilnResult<()> {
    let local_dev_file = proj_dir.as_ref().join(DEV_ENV_CFG_FILE);

    if !local_dev_file.exists() {
        return Ok(());
    }

    let local_config = fs::read_to_string(local_dev_file)?;
    let local_config: DevEnvConfig = toml::from_str(&local_config)?;

    if local_config.editor.is_none() {
        return Ok(());
    }

    let mut include_dirs = vec![
        "${workspaceFolder}/XXX/**".replace("XXX", config.get_include_dir().trim_matches('/'))
    ];

    if let Some(deps) = config.dependnecy.as_ref() {
        for dep in deps {
            if let Some(s) = dep.get_include_dir()? {
                let s = s.to_str().unwrap().to_string();
                include_dirs.push(s);
            }
        }
    }
    
    set_include(&local_config, &include_dirs, proj_dir)?;

    Ok(())
}

fn set_include(dev_config: &DevEnvConfig, includes: &[String], proj_dir: impl AsRef<Path>) -> KilnResult<()> {

    let editor = match dev_config.editor {
        Some(e) => e,
        None => return Err(KilnError::new_unknown("Dev config file doesn't exist")),
    };
    match editor {
        EditorType::VsCode => {
            set_include_vscode(includes, proj_dir);
        }
        _ => {
            eprintln!("Support for `{:?}` is not yet supported", editor);
        }
    }

    Ok(())
}

/// Updates the `.vscode/c_cpp_properties.json` file to include the propor include paths
fn set_include_vscode(includes: &[String], proj_dir: impl AsRef<Path>) {
    let default_config = include_str!("../assets/vscode-default-properties.json");
    let dc_json: Value = serde_json::from_str(default_config).unwrap();

    let default_include = "${workspaceFolder}/include/**";
    let config_file = proj_dir.as_ref().join(".vscode/c_cpp_properties.json");

    // Read the current config, or use the default if missing.
    let config_str = fs::read_to_string(&config_file).unwrap_or_else(|_| default_config.to_string());
    let mut config: Value = serde_json::from_str(&config_str).unwrap();

    if let Some(configurations) = config.get_mut("configurations").and_then(|v| v.as_array_mut()) {
        for config_item in configurations.iter_mut() {
            // Retrieve existing include paths if any.
            let mut existing_paths: Vec<String> = if let Some(path_array) = config_item
                .get("includePath")
                .and_then(|v| v.as_array())
            {
                path_array.iter().filter_map(|v| v.as_str().map(String::from)).collect()
            } else {
                Vec::new()
            };

            // Ensure the default include is present.
            if !existing_paths.contains(&default_include.to_string()) {
                existing_paths.push(default_include.to_string());
            }

            // Add each new include if it isn't already there.
            for include in includes {
                if !existing_paths.contains(include) {
                    existing_paths.push(include.clone());
                }
            }

            // Update the configuration with the merged list.
            config_item["includePath"] = Value::Array(
                existing_paths.into_iter().map(Value::String).collect()
            );

            // Ensure "name" exists by copying it from the default if missing.
            if config_item.get("name").is_none() {
                if let Some(default_configs) = dc_json.get("configurations").and_then(|v| v.as_array()) {
                    if let Some(default_name) = default_configs.get(0)
                        .and_then(|v| v.get("name"))
                        .and_then(|v| v.as_str())
                    {
                        config_item["name"] = Value::String(default_name.to_string());
                    }
                }
            }
        }
    } else {
        // If "configurations" isn't present, copy the whole default.
        config["configurations"] = dc_json.get("configurations").unwrap().clone();
    }

    // Ensure the .vscode directory exists before writing.
    if let Some(parent) = config_file.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    fs::write(&config_file, serde_json::to_string_pretty(&config).unwrap()).unwrap();
}
