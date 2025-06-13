use crate::config::{self, Config, KilnIngot};
use crate::constants::{CONFIG_FILE, PACKAGE_CONFIG_FILE};
use crate::packaging::ingot::IngotMetadata;

use std::collections::HashSet;
use std::fmt::Debug;
use std::io::Write;
use std::path::Path;
use std::{fs, time::Duration};

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

    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String, // The name of the tag is actually the version
    pub zipball_url: String,
    pub tarball_url: String,
}

#[derive(Debug, Clone, Copy)]
pub enum DepType {
    SourceCode,
    HeaderFile,
    SharedObject,
    StaticLibrary,
}

impl From<&str> for DepType {
    fn from(value: &str) -> Self {
        match value {
            "source_code" => Self::SourceCode,
            "header_file" | "header_files" => Self::HeaderFile,
            "shared_object" | "shared_objects" => Self::SharedObject,
            "static_libraries" | "static_library" => Self::StaticLibrary,
            _ => unreachable!(),
        }
    }
}

impl Into<&str> for DepType {
    fn into(self) -> &'static str {
        match self {
            DepType::HeaderFile => "header_files",
            DepType::SourceCode => "source_code",
            DepType::SharedObject => "shared_objects",
            DepType::StaticLibrary => "static_libraries",
        }
    }
}

impl AsRef<Path> for DepType {
    fn as_ref(&self) -> &Path {
        let path: &str = (*self).into();
        Path::new(path)
    }
}

pub fn parse_github_uri(uri: &str) -> Result<(&str, &str), PkgError> {
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

async fn find_tags(owner: &str, repo_name: &str) -> Result<Vec<Tag>, PkgError> {
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
        packages.push(t);
    }

    Ok(packages)
}

/// Installs a package in the glocal cache. does NOT create a kiln-package.toml file
/// If the package already exists locally, it does nothing
async fn install_globally(package: &KilnIngot, tag: &Tag) -> Result<(), PkgError> {
    let package_dir = package.get_global_path();
    let tarball_tmp_name = format!(
        "{}_{}_{}",
        &package.owner(),
        &package.repo_name(),
        &package.version
    );

    if package_dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(&package_dir)?;

    let res = reqwest::Client::new()
        .get(tag.tarball_url.clone())
        .header("User-Agent", "Kiln Build System")
        .send();

    let res = tokio::spawn(res);

    // Create a temporary directory
    let tmp_dir = TempDir::new()?;
    let tmp_file = tmp_dir.path().join(format!("{}.tar.gz", tarball_tmp_name));

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

/// Takes care of the entire installation process (High Level Function)
/// PRECONDITION: CWD must be in the root directory of a kiln project
/// This *will* take care of chained dependncies
pub async fn resolve_adding_package(
    config: &mut config::Config,
    owner: &str,
    proj_name: &str,
    version: Option<&str>,
) -> Result<(), PkgError> {
    // TODO: Add a better error message by providing the link to see all the github repo's tags
    if let None = config.dependency {
        config.dependency = Some(vec![]);
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
            let kiln_dcf_deps = config.dependency.as_mut().unwrap();
            config::KilnIngot::add_dependency(kiln_dcf_deps, cfg);
            deps.extend(chain_deps);
        }
    }

    Ok(())
}

/// Takes care of the remote to global to local instalation process
/// This pseudo-recursive helper function to [fn resolve_adding_package]
async fn add_package(
    owner: String,
    proj_name: String,
    version: Option<String>,
) -> Result<(Vec<[String; 3]>, KilnIngot), PkgError> {
    // TODO: Add a better error message by providing the link to see all the github repo's tags
    let repo_name = format!("https://github.com/{}/{}", owner, proj_name);

    let tags = find_tags(&owner, &proj_name).await?;
    if tags.len() == 0 {
        return Err(PkgError::UsrErr(format!(
            "No versions available for {}",
            repo_name
        )));
    }

    let mut tag: &Tag = &tags[0];
    if let Some(v) = version {
        let mut assigned = false;
        for t in &tags {
            if t.name == v {
                tag = t;
                assigned = true;
            }
        }
        if !assigned {
            let msg = format!(
                "Version {} does not exist for https:://{}/{}",
                v, owner, proj_name
            );
            return Err(PkgError::UsrErr(msg));
        }
    } else {
        tag = tags.last().unwrap();
    }

    let pkg = KilnIngot::new(&owner, &proj_name, &tag.name);

    install_globally(&pkg, &tag).await?;

    let mut chain_dep_ids = vec![];

    if let Some(mut cfg) = pkg.get_kiln_cfg()? {
        if let None = cfg.dependency {
            cfg.dependency = Some(vec![]);
        }
        let chain_deps = cfg.dependency.as_ref().unwrap();
        for chain_dep in chain_deps {
            let (chain_owner, chain_repo) = parse_github_uri(&chain_dep.uri)?;

            chain_dep_ids.push([
                chain_owner.to_string(),
                chain_repo.to_string(),
                chain_dep.version.clone(),
            ]);
        }
    }

    Ok((chain_dep_ids, pkg))
}

/// Ensures that all the packages listed in the Kiln.toml config file are
/// all installed globally. Any that are listed but are not installed will be
/// returned
pub fn check_pkgs<'a>(config: &'a Config) -> Vec<[String; 3]> {
    let mut not_installed = vec![];
    let mut pkgs_visited: HashSet<String> = HashSet::new();

    if let Some(deps) = &config.dependency {
        for dep in deps {
            check_pkg_h(dep, &mut not_installed, &mut pkgs_visited);
        }
    }

    not_installed
}

fn check_pkg_h(
    dep: &KilnIngot,
    output: &mut Vec<[String; 3]>,
    pkgs_visited: &mut HashSet<String>,
) {
    if pkgs_visited.contains(dep.uri.as_str()) {
        return;
    }
    pkgs_visited.insert(dep.uri.clone());

    if !dep.get_global_path().exists() {
        let pkg = [
            dep.owner().to_string(),
            dep.repo_name().to_string(),
            dep.version.clone(),
        ];

        if !output.contains(&pkg) {
            output.push(pkg);
        }
        return;
    }

    if let Some(kiln_cfg) = dep.get_kiln_cfg().unwrap() {
        if let Some(chain_deps) = &kiln_cfg.dependency {
            for chain_dep in chain_deps {
                check_pkg_h(chain_dep, output, pkgs_visited);
            }
        }
    }
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
