//! Walk a directory tree and list the meta modules it contains.

use anyhow::Result;
use colored::Colorize;
use metarepo_core::{MetaModuleManifest, MODULE_MANIFEST_FILENAMES};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// List every module manifest found under `path`.
pub fn run(path: &str) -> Result<()> {
    let root = Path::new(path);
    let manifests = find_modules(root);
    if manifests.is_empty() {
        println!("{}", "no modules found".dimmed());
        return Ok(());
    }
    println!(
        "{} in {}",
        format!("found {} module(s)", manifests.len()).bold(),
        root.display()
    );
    for m in manifests {
        match MetaModuleManifest::from_file_auto(&m) {
            Ok(manifest) => {
                println!(
                    "  {} {} v{} — {} plugin(s), {} skill(s)",
                    "•".cyan(),
                    manifest.module.name.bold(),
                    manifest.module.version,
                    manifest.module.plugins.len(),
                    manifest.module.skills.len(),
                );
                println!("    {}", m.display().to_string().dimmed());
            }
            Err(e) => eprintln!("  {} {}: {}", "!".red(), m.display(), e),
        }
    }
    Ok(())
}

/// Locate `meta.module.*` files beneath `root`, skipping noise directories.
pub fn find_modules(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !matches!(name.as_ref(), ".git" | "node_modules" | "target")
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| MODULE_MANIFEST_FILENAMES.contains(&n))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn finds_modules_and_skips_noise() {
        let tmp = tempdir().unwrap();
        let good = tmp.path().join("a");
        fs::create_dir_all(&good).unwrap();
        fs::write(
            good.join("meta.module.toml"),
            "[module]\nname = \"a\"\nversion = \"0.1.0\"\n[[module.skills]]\npath = \"s\"\n",
        )
        .unwrap();

        let noise = tmp.path().join("target/x");
        fs::create_dir_all(&noise).unwrap();
        fs::write(noise.join("meta.module.toml"), "ignored").unwrap();

        let found = find_modules(tmp.path());
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("a/meta.module.toml"));
    }
}
