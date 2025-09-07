use anyhow::Result;
use clap::{ArgMatches, Command};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::HashMap;

pub use crate::iterator::ProjectIterator;
pub use crate::plugin::LoopPlugin;

mod iterator;
mod plugin;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaConfig {
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub projects: HashMap<String, String>,
    #[serde(default)]
    pub plugins: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: PathBuf,
    pub repo_url: String,
    pub exists: bool,
}

impl ProjectInfo {
    pub fn new(name: String, path: PathBuf, repo_url: String) -> Self {
        let exists = path.exists();
        Self {
            name,
            path,
            repo_url,
            exists,
        }
    }
    
    pub fn is_git_repo(&self) -> bool {
        if !self.exists {
            return false;
        }
        self.path.join(".git").exists()
    }
}