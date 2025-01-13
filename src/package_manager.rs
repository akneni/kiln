use crate::config::{self, Config, Dependnecy};
use crate::constants::{CONFIG_FILE, PACKAGE_CONFIG_FILE, PACKAGE_DIR};
use crate::kiln_package::{self, KilnPackageConfig};
use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::io::Write;
use std::path::Path;
use std::{fs, path::PathBuf, time::Duration};

use flate2::read::GzDecoder;
use reqwest;
use serde::{Deserialize, Serialize};
use tar::Archive;
use tempfile::TempDir;

use anyhow;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PkgError {
    // =============== Crate Errors ===============
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Networking error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Serialization error: {0}")]
    Toml(#[from] toml::ser::Error),

    #[error("Async runtime error: {0}")]
    Tokio(#[from] tokio::task::JoinError),

    #[error("Unknown error: {0}")]
    Anyhow(#[from] anyhow::Error),

    // =============== Custom errors ===============
    #[error("User error: {0}")]
    UsrErr(String),

    #[error("Dir not Found error: {0}")]
    PkgAmbiguous(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Tag {
    pub name: String, // The name of the tag is actually the version
    pub zipball_url: String,
    pub tarball_url: String,
}

#[derive(Debug, Clone)]
pub(super) struct Package {
    pub owner: String,
    pub repo_name: String,
    pub tag: Tag,
}

impl Package {
    pub(super) fn get_global_path(&self) -> PathBuf {
        let package_dir = (*PACKAGE_DIR).clone();
        let package_name = format!("{}_{}_{}", &self.owner, &self.repo_name, &self.tag.name);
        package_dir.join(&package_name)
    }

    pub(super) fn get_kiln_cfg(&self) -> Result<Option<Config>, PkgError> {
        let cfg_file = self.get_global_path().join(CONFIG_FILE);
        if !cfg_file.exists() {
            return Ok(None);
        }

        let cfg = Config::from(&cfg_file)?;
        Ok(Some(cfg))
    }

    pub(super) fn get_include_dir(&self) -> Result<PathBuf, PkgError> {
        let p = self.get_global_path();

        let config_path = p.join(CONFIG_FILE);
        if config_path.exists() {
            let config = Config::from(&config_path)?;
            return Ok(Path::new(&config.get_include_dir()).to_path_buf());
        }

        let pgk_path = p.join(PACKAGE_CONFIG_FILE);
        if pgk_path.exists() {
            let pkg_cfg = KilnPackageConfig::from(&pgk_path)?;
            return Ok(Path::new(&pkg_cfg.metadata.include_dir).to_path_buf());
        }

        Err(PkgError::PkgAmbiguous("".to_string()))
    }

    pub(super) fn get_source_dir(&self) -> Result<PathBuf, PkgError> {
        let p = self.get_global_path();

        let config_path = p.join(CONFIG_FILE);
        if config_path.exists() {
            let config = Config::from(&config_path)?;
            return Ok(Path::new(&config.get_src_dir()).to_path_buf());
        }

        let pgk_path = p.join(PACKAGE_CONFIG_FILE);
        if pgk_path.exists() {
            let pkg_cfg = KilnPackageConfig::from(&pgk_path)?;
            return Ok(Path::new(&pkg_cfg.metadata.source_dir).to_path_buf());
        }

        Err(PkgError::PkgAmbiguous("".to_string()))
    }
}

pub(super) fn parse_github_uri(uri: &str) -> Result<(&str, &str), PkgError> {
    let mut uri = match uri.split_once("github.com/") {
        Some(s) => s.1,
        None => return Err(PkgError::UsrErr("Invalid GitHub uri".to_string())),
    };

    if uri.ends_with(".com") || uri.ends_with(".git") {
        uri = &uri[..uri.len() - 4];
    }

    let (owner, proj_name) = match uri.split_once("/") {
        Some(s) => s,
        None => return Err(PkgError::UsrErr("Invalid GitHub uri".to_string())),
    };

    Ok((owner, proj_name))
}

async fn find_tags(owner: &str, repo_name: &str) -> Result<Vec<Package>, PkgError> {
    let endpoint = format!("https://api.github.com/repos/{}/{}/tags", owner, repo_name);

    // println!("Endpoint: {}", endpoint);
    // std::process::exit(0);

    let res = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(4))
        .build()?
        .get(&endpoint)
        .header("User-Agent", "Kiln Build System")
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(PkgError::Unknown(format!(
            "Non 200 status code from github: {}",
            res.status().as_str()
        )));
    }

    let body = res.text().await?;

    let tags: Vec<Tag> = serde_json::from_str(&body)?;
    let mut packages = vec![];
    for t in tags {
        packages.push(Package {
            owner: owner.to_string(),
            repo_name: repo_name.to_string(),
            tag: t,
        });
    }

    Ok(packages)
}

/// Installs a package in the glocal cache. does NOT create a kiln-package.toml file
/// If the package already exists locally, it does nothing
async fn install_globally(package: &Package) -> Result<(), PkgError> {
    let package_dir = package.get_global_path();
    let package_name = format!(
        "{}_{}_{}",
        &package.owner, &package.repo_name, &package.tag.name
    );

    if package_dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(&package_dir)?;

    let res = reqwest::Client::new()
        .get(package.tag.tarball_url.clone())
        .header("User-Agent", "Kiln Build System")
        .send();

    let res = tokio::spawn(res);

    // Create a temporary directory
    let tmp = TempDir::new()?;
    let tmp_file = tmp.path().join(format!("{}.tar.gz", package_name));

    let res = res.await??;
    if !res.status().is_success() {
        let mut msg =
            format!("Github returned a non 200 status code when trying to download the tarball\n");
        msg.push_str(&format!("Status code: {}\n", res.status().as_u16()));
        msg.push_str(&format!(
            "Text: \n{}\n",
            res.text().await.unwrap_or("".to_string())
        ));

        return Err(PkgError::Unknown(msg));
    }

    let body = res.bytes().await?;
    let body: Vec<u8> = body.to_vec();

    fs::write(&tmp_file, &body)?;

    let tar_gz = fs::File::open(&tmp_file)?;
    let tar = GzDecoder::new(tar_gz);

    unpack_without_top_folder(tar, &package_dir)?;

    Ok(())
}

/// Generates the `kiln-package.toml' file
/// PRECONDITION: This package must be locally installed
/// Returns false if the package file could not be infered
fn generate_package_file(package: &Package) -> Result<(), PkgError> {
    let package_dir = package.get_global_path();

    let kiln_pkg_toml = package_dir.join(PACKAGE_CONFIG_FILE);
    if kiln_pkg_toml.exists() {
        return Ok(());
    }

    let kiln_config = package_dir.join(CONFIG_FILE);
    if kiln_config.exists() {
        let config = config::Config::from(&kiln_config)?;

        let pkg_cfg =
            kiln_package::KilnPackageConfig::new(config.get_include_dir(), config.get_src_dir());

        let pkg_cfg = toml::to_string_pretty(&pkg_cfg)?;
        fs::write(kiln_pkg_toml, pkg_cfg)?;
        return Ok(());
    }

    let include_dir = package_dir.join("include");
    let source_dir = package_dir.join("src");

    if include_dir.exists() && source_dir.exists() {
        let pkg_cfg =
            kiln_package::KilnPackageConfig::new("include".to_string(), "src".to_string());

        let pkg_cfg = toml::to_string_pretty(&pkg_cfg)?;
        fs::write(kiln_pkg_toml, pkg_cfg)?;
        return Ok(());
    }

    Err(PkgError::PkgAmbiguous(
        "Package does not exist.".to_string(),
    ))
}

/// Takes care of the entire installation process (High Level Function)
/// PRECONDITION: CWD must be in the root directory of a kiln project
/// This *will* take care of chained dependncies
pub(super) async fn resolve_adding_package(
    config: &mut config::Config,
    owner: &str,
    proj_name: &str,
    version: Option<&str>,
) -> Result<(), PkgError> {
    // TODO: Add a better error message by providing the link to see all the github repo's tags
    if let None = config.dependnecy {
        config.dependnecy = Some(vec![]);
    }

    let mut packages_added: HashSet<String> = HashSet::new();

    let mut deps = vec![[
        owner.to_string(),
        proj_name.to_string(),
        version.unwrap_or("").to_string(),
    ]];

    while deps.len() > 0 {
        let mut futures = vec![];

        for dep in &deps {
            let owner = dep[0].clone();
            let proj_name = dep[1].clone();
            let version = if dep[2] == "" {
                None
            } else {
                Some(dep[2].clone())
            };

            let repo_name = format!("https://github.com/{}/{}", owner, proj_name);
            if packages_added.contains(&repo_name) {
                continue;
            }
            packages_added.insert(repo_name);

            let f = add_package(owner, proj_name, version);
            let f = tokio::spawn(f);
            futures.push(f);
        }
        deps.clear();

        for f in futures {
            let (chain_deps, cfg) = f.await??;
            config.dependnecy.as_mut().unwrap().push(cfg);
            deps.extend(chain_deps);
        }
    }

    Ok(())
}

/// Takes care of the remote to global to local instalation process
/// This recursive helper function to [fn resolve_adding_package]
async fn add_package(
    owner: String,
    proj_name: String,
    version: Option<String>,
) -> Result<(Vec<[String; 3]>, Dependnecy), PkgError> {
    // TODO: Add a better error message by providing the link to see all the github repo's tags
    let repo_name = format!("https://github.com/{}/{}", owner, proj_name);

    let tags = find_tags(&owner, &proj_name).await?;
    if tags.len() == 0 {
        return Err(PkgError::UsrErr(format!(
            "No versions available for {}",
            repo_name
        )));
    }

    let mut pkg: &Package = &tags[0];
    if let Some(v) = version {
        let mut assigned = false;
        for t in &tags {
            if t.tag.name == v {
                pkg = t;
                assigned = true;
                break;
            }
        }
        if !assigned {
            return Err(PkgError::UsrErr(format!(
                "Version {} doesn't exist for {}",
                v, repo_name
            )));
        }
    } else {
        pkg = tags
            .iter()
            .max_by(|&a, &b| a.tag.name.cmp(&b.tag.name))
            .unwrap();
    }

    install_globally(pkg).await?;
    let res = generate_package_file(pkg);

    if let Err(PkgError::PkgAmbiguous(_)) = res {
        let mut stdin_buf = String::new();
        let include_dir: String;
        let source_dir: String;

        println!("Enter the path to the source code files in {} (all files other than .c, .cpp, and .cu will be ignored)", repo_name);
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut stdin_buf)?;
        source_dir = stdin_buf.trim().trim_matches('/').to_string();

        println!("Enter the path to the header files in {} (all files other than .h, .hpp, and .cuh will be ignored)", repo_name);
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut stdin_buf)?;
        include_dir = stdin_buf.trim().trim_matches('/').to_string();

        let pkg_cfg = KilnPackageConfig::new(include_dir, source_dir);
        let pkg_cfg_str = toml::to_string_pretty(&pkg_cfg)?;

        let out_path = pkg.get_global_path().join(PACKAGE_CONFIG_FILE);

        #[cfg(debug_assertions)]
        {
            println!("out_path: {:?}", out_path.to_str());
        }

        fs::write(out_path, pkg_cfg_str)?;
    } else if let Err(e) = res {
        return Err(e);
    }

    copy_deps_global_to_local(pkg, "header_files")?;
    copy_deps_global_to_local(pkg, "source_code")?;
    copy_deps_global_to_local(pkg, "shared_objects")?;
    copy_deps_global_to_local(pkg, "static_libraries")?;

    let include_dir = match pkg.get_include_dir() {
        Ok(r) => Some(r.to_str().unwrap().to_string()),
        Err(_) => None,
    };

    let source_dir = match pkg.get_source_dir() {
        Ok(r) => Some(r.to_str().unwrap().to_string()),
        Err(_) => None,
    };

    let kiln_toml_dep = Dependnecy {
        uri: repo_name.clone(),
        version: pkg.tag.name.clone(),
        include_dir,
        source_dir,
        shared_object_dir: None,
        static_lib_dir: None,
    };

    let mut chain_dep_ids = vec![];

    if let Some(cfg) = pkg.get_kiln_cfg()? {
        let chain_deps = cfg.dependnecy.as_ref().unwrap();
        for chain_dep in chain_deps {
            chain_dep_ids.push([
                owner.to_string(),
                proj_name.to_string(),
                chain_dep.version.clone(),
            ]);
        }
    }

    Ok((chain_dep_ids, kiln_toml_dep))
}

/// Copies all the files from their globally installed location to their local spot in the project
/// This function is Atomic: it will remove all files it if an error occurs.
fn copy_deps_global_to_local(pkg: &Package, dep_type: &str) -> Result<(), PkgError> {
    let file_ext = match dep_type {
        "source_code" => [".c", ".cpp", ".cu"].as_slice(),
        "header_files" => [".h", ".hpp", ".cuh"].as_slice(),
        "shared_objects" => [".so", ".dylib", ".dll"].as_slice(),
        "static_libraries" => [".a", ".lib"].as_slice(),
        _ => {
            eprintln!("[fn copy_files] Invalid dep type rceived: {}", dep_type);
            std::process::exit(1);
        }
    };

    let cwd = env::current_dir()?;

    let local_source_dir = cwd.join("dependencies").join(dep_type);
    if !local_source_dir.exists() {
        fs::create_dir_all(&local_source_dir)?;
    }

    match pkg.get_source_dir() {
        Ok(global_source_dir_relative) => {
            let mut global_source_dir = pkg.get_global_path();

            let gsdr_str = global_source_dir_relative.to_str().unwrap();

            if !matches!(gsdr_str.trim(), "" | "." | "./") {
                global_source_dir = global_source_dir.join(global_source_dir_relative);
            }

            #[cfg(debug_assertions)]
            if dep_type == "header_files" {
                dbg!(&global_source_dir);
                dbg!(&local_source_dir);
            }
            
            for f in global_source_dir.read_dir()? {
                let f = match f {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("\nIterating over directory failed");
                        dbg!(e);
                        eprintln!();
                        continue;
                    }
                };
                let tmp = f.file_name();
                let g_filename = match tmp.to_str() {
                    Some(s) => s,
                    None => {
                        #[cfg(debug_assertions)]
                        {
                            println!(
                                "Received `None` from OsStr.as_str() [fn resolve_adding_package]"
                            );
                        }
                        continue;
                    }
                };

                if !file_ext.iter().any(|&i| g_filename.ends_with(i)) {
                    continue;
                }

                let global_p = global_source_dir.join(g_filename);
                let new_p = local_source_dir.join(g_filename);
                if new_p.exists() {
                    eprintln!(
                        "File {} already exists. (Multiple packages have files with this same name)",
                        g_filename
                    );
                    eprintln!(
                        "Aborting import of https://githib.com/{}/{}",
                        pkg.owner, pkg.repo_name
                    );

                    // Remove all files we've added to prevent an invalid state
                    remove_deps_local(pkg, dep_type)?;

                    std::process::exit(1);
                }

                #[cfg(debug_assertions)] println!("copying {:?}", global_p);
                fs::copy(global_p, new_p)?;
            }
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            {
                eprintln!("Global source dir could not be found: {}", e);
            }
        }
    }

    Ok(())
}

fn remove_deps_local(pkg: &Package, dep_type: &str) -> Result<(), PkgError> {
    let cwd = env::current_dir()?;

    let global_source_dir = cwd.join("dependencies").join(dep_type);
    let file_names: Vec<std::ffi::OsString> =
        global_source_dir.iter().map(|i| i.to_os_string()).collect();

    if let Ok(local_source_dir) = pkg.get_source_dir() {
        for f in local_source_dir.iter() {
            let f = f.to_os_string();
            if file_names.contains(&f) {
                fs::remove_file(&f)?;
            }
        }
    }

    Ok(())
}

fn unpack_without_top_folder<R: std::io::Read>(reader: R, dst: &Path) -> Result<(), PkgError> {
    let mut archive = Archive::new(reader);
    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        // figure out the existing path inside the tar
        let old_path = entry.path()?.into_owned();

        // skip the first directory component
        let mut comps = old_path.components();
        comps.next(); // remove the top-level folder component

        // re-build a "stripped" path
        let new_path = comps.as_path();

        // join that onto your destination
        let out_path = dst.join(new_path);

        // create all intermediate directories
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // finally unpack
        entry.unpack(out_path)?;
    }
    Ok(())
}
