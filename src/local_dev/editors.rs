use crate::{
    config::Config,
    constants::DEV_ENV_CFG_FILE,
    local_dev::dev_env_config::{DevEnvConfig, EditorType},
};

use serde_json::Value;
use serde_yaml::{Mapping, Value as YmlValue};
use std::{fs, path::Path};
use anyhow::{Result, anyhow};

pub fn handle_editor_includes(config: &Config, proj_dir: impl AsRef<Path>) -> Result<()> {
    let local_dev_file = proj_dir.as_ref().join(DEV_ENV_CFG_FILE);

    if !local_dev_file.exists() {
        return Ok(());
    }

    let local_config = fs::read_to_string(local_dev_file)?;
    let local_config: DevEnvConfig = toml::from_str(&local_config)?;

    if local_config.editor.is_none() {
        return Ok(());
    }

    let mut include_dirs = vec![];

    for include_dir in &config.project.include_dirs {
        let s =  "${workspaceFolder}/XXX/**".replace("XXX", include_dir);
        include_dirs.push(s);
    }

    if let Some(deps) = config.dependency.as_ref() {
        for ingot in deps {
            let ingot_path = ingot.get_global_path()
                .join("build");

            let ingot_path = ingot_path.to_str()
                .unwrap()
                .to_string();

            include_dirs.push(ingot_path);
        }
    }

    set_include(&local_config, &include_dirs, proj_dir)?;

    Ok(())
}

fn set_include(
    dev_config: &DevEnvConfig,
    includes: &[String],
    proj_dir: impl AsRef<Path>,
) -> Result<()> {
    let editor = match dev_config.editor {
        Some(e) => e,
        None => return Err(anyhow!("Dev config file doesn't exist")),
    };
    match editor {
        EditorType::VsCode => {
            set_include_vscode(includes, proj_dir);
        }
        EditorType::Helix | EditorType::Zed | EditorType::NeoVim => {
            set_include_clangd(includes, proj_dir)?;
        }
        _ => {
            let msg = format!("Support for `{:?}` is not yet supported", editor);
            return Err(anyhow!(msg));
        }
    }

    Ok(())
}

/// Updates the `.vscode/c_cpp_properties.json` file to include the propor include paths
fn set_include_vscode(includes: &[String], proj_dir: impl AsRef<Path>) {
    let default_config = include_str!("../../assets/vscode-default-properties.json");
    let dc_json: Value = serde_json::from_str(default_config).unwrap();

    let default_include = "${workspaceFolder}/include/**";
    let config_file = proj_dir.as_ref().join(".vscode/c_cpp_properties.json");

    // Read the current config, or use the default if missing.
    let config_str =
        fs::read_to_string(&config_file).unwrap_or_else(|_| default_config.to_string());
    let mut config: Value = serde_json::from_str(&config_str).unwrap();

    if let Some(configurations) = config
        .get_mut("configurations")
        .and_then(|v| v.as_array_mut())
    {
        for config_item in configurations.iter_mut() {
            // Retrieve existing include paths if any.
            let mut existing_paths: Vec<String> = if let Some(path_array) =
                config_item.get("includePath").and_then(|v| v.as_array())
            {
                path_array
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
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
            config_item["includePath"] =
                Value::Array(existing_paths.into_iter().map(Value::String).collect());

            // Ensure "name" exists by copying it from the default if missing.
            if config_item.get("name").is_none() {
                if let Some(default_configs) =
                    dc_json.get("configurations").and_then(|v| v.as_array())
                {
                    if let Some(default_name) = default_configs
                        .get(0)
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

// Sets the propor include paths in `.clangd`
fn set_include_clangd(includes: &[String], proj_dir: impl AsRef<Path>) -> Result<()> {
    let config_file = proj_dir.as_ref().join(".clangd");

    // Read the existing .clangd file or start with an empty mapping.
    let mut config: YmlValue = if config_file.exists() {
        let config_str = fs::read_to_string(&config_file)?;
        serde_yaml::from_str(&config_str).unwrap_or(YmlValue::Mapping(Mapping::new()))
    } else {
        YmlValue::Mapping(Mapping::new())
    };

    // Ensure that the top-level is a mapping.
    let config_map = config.as_mapping_mut().ok_or_else(|| {
        anyhow!("Invalid .clangd file structure: expected a mapping")
    })?;

    // Get or create the "CompileFlags" mapping.
    let compile_flags = config_map
        .entry(YmlValue::String("CompileFlags".into()))
        .or_insert_with(|| YmlValue::Mapping(Mapping::new()));

    let compile_flags_map = compile_flags.as_mapping_mut().ok_or_else(|| {
        anyhow!(
            "Invalid .clangd file structure: expected CompileFlags to be a mapping",
        )
    })?;

    // Get or create the "Add" key as a sequence.
    let add = compile_flags_map
        .entry(YmlValue::String("Add".into()))
        .or_insert_with(|| YmlValue::Sequence(vec![]));

    let add_seq = add.as_sequence_mut().ok_or_else(|| {
        anyhow!("Invalid .clangd file structure: expected Add to be a sequence")
    })?;

    // Define the default include flag.
    let default_include = "${workspaceFolder}/include/**";
    let default_flag = format!("-I{}", default_include);

    // Ensure the default flag is present.
    if !add_seq.iter().any(|v| v.as_str() == Some(&default_flag)) {
        add_seq.push(YmlValue::String(default_flag));
    }

    // For each provided include directory, add a clangd include flag (-I<path>) if not already present.
    for include in includes {
        let flag = format!("-I{}", include);
        if !add_seq.iter().any(|v| v.as_str() == Some(&flag)) {
            add_seq.push(YmlValue::String(flag));
        }
    }

    // Ensure the parent directory exists.
    if let Some(parent) = config_file.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write the updated YAML configuration back to the .clangd file.
    let yaml_str = serde_yaml::to_string(&config).unwrap();
    fs::write(&config_file, yaml_str)?;

    Ok(())
}
