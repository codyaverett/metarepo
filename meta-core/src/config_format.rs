//! Configuration file format detection and serialization dispatch.
//!
//! Metarepo supports three on-disk formats for the workspace config: JSON,
//! YAML, and TOML. The format is detected from the file's name (or extension)
//! so callers never need to track it explicitly.

use anyhow::{anyhow, Result};
use std::path::Path;

/// The canonical (extensionless) filename for new workspaces. Existing
/// `.meta` files continue to work indefinitely.
pub const CANONICAL_FILENAME: &str = ".metarepo";

/// Legacy filename — predates the multi-format support. Always treated as JSON.
pub const LEGACY_FILENAME: &str = ".meta";

/// Filenames probed in each ancestor directory during discovery. Ordering is
/// informational only — when multiple matches exist in the same directory we
/// always error rather than picking one.
pub const KNOWN_FILENAMES: &[&str] = &[
    CANONICAL_FILENAME,
    LEGACY_FILENAME,
    ".metarepo.json",
    ".metarepo.yaml",
    ".metarepo.yml",
    ".metarepo.toml",
];

/// On-disk serialization format for a metarepo config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Json,
    Yaml,
    Toml,
}

impl ConfigFormat {
    /// Detect the format from a config file path.
    ///
    /// Returns `Some(format)` when the path's filename or extension matches
    /// one of the recognized patterns. Returns `None` for paths we don't know
    /// how to handle — callers can treat that as an error or fall back to JSON.
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;

        // Filenames without an extension that we still recognize.
        if name == CANONICAL_FILENAME || name == LEGACY_FILENAME {
            return Some(ConfigFormat::Json);
        }

        // Match by extension. Use rsplit_once so multi-dot names like
        // `.metarepo.yaml` work — the extension is whatever follows the last
        // dot.
        let (stem, ext) = name.rsplit_once('.')?;
        // Guard: the stem should still look like one of our config names.
        if stem != ".metarepo" && stem != "metarepo" && stem != ".meta" && stem != "meta" {
            return None;
        }
        match ext.to_ascii_lowercase().as_str() {
            "json" => Some(ConfigFormat::Json),
            "yaml" | "yml" => Some(ConfigFormat::Yaml),
            "toml" => Some(ConfigFormat::Toml),
            _ => None,
        }
    }

    /// Map a user-supplied format name (case-insensitive) to the enum.
    pub fn parse(name: &str) -> Result<Self> {
        match name.to_ascii_lowercase().as_str() {
            "json" => Ok(ConfigFormat::Json),
            "yaml" | "yml" => Ok(ConfigFormat::Yaml),
            "toml" => Ok(ConfigFormat::Toml),
            other => Err(anyhow!(
                "Unknown config format '{}'. Expected json, yaml, or toml.",
                other
            )),
        }
    }

    /// Canonical filename for a fresh init in this format. Always rooted at
    /// `.metarepo` (the JSON form is extensionless to match the legacy `.meta`
    /// look-and-feel).
    pub fn canonical_filename(self) -> &'static str {
        match self {
            ConfigFormat::Json => CANONICAL_FILENAME,
            ConfigFormat::Yaml => ".metarepo.yaml",
            ConfigFormat::Toml => ".metarepo.toml",
        }
    }

    /// Pretty display name for error messages.
    pub fn label(self) -> &'static str {
        match self {
            ConfigFormat::Json => "json",
            ConfigFormat::Yaml => "yaml",
            ConfigFormat::Toml => "toml",
        }
    }
}

/// Serialize a value to bytes in the requested format.
pub fn serialize_to_string<T: serde::Serialize>(value: &T, format: ConfigFormat) -> Result<String> {
    match format {
        ConfigFormat::Json => Ok(serde_json::to_string_pretty(value)?),
        ConfigFormat::Yaml => Ok(serde_yaml::to_string(value)?),
        ConfigFormat::Toml => Ok(toml::to_string_pretty(value)?),
    }
}

/// Deserialize from a string in the requested format.
pub fn deserialize_from_str<T: serde::de::DeserializeOwned>(
    content: &str,
    format: ConfigFormat,
) -> Result<T> {
    match format {
        ConfigFormat::Json => Ok(serde_json::from_str(content)?),
        ConfigFormat::Yaml => Ok(serde_yaml::from_str(content)?),
        ConfigFormat::Toml => Ok(toml::from_str(content)?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_canonical_and_legacy_as_json() {
        assert_eq!(
            ConfigFormat::from_path(&PathBuf::from(".metarepo")),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigFormat::from_path(&PathBuf::from(".meta")),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigFormat::from_path(&PathBuf::from("/x/y/.metarepo")),
            Some(ConfigFormat::Json)
        );
    }

    #[test]
    fn detects_extension_variants() {
        assert_eq!(
            ConfigFormat::from_path(&PathBuf::from(".metarepo.json")),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigFormat::from_path(&PathBuf::from(".metarepo.yaml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_path(&PathBuf::from(".metarepo.yml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_path(&PathBuf::from(".metarepo.toml")),
            Some(ConfigFormat::Toml)
        );
    }

    #[test]
    fn rejects_unrelated_paths() {
        assert_eq!(
            ConfigFormat::from_path(&PathBuf::from("package.json")),
            None
        );
        assert_eq!(
            ConfigFormat::from_path(&PathBuf::from(".meta.yaml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(ConfigFormat::from_path(&PathBuf::from(".rando")), None);
    }

    #[test]
    fn parses_format_names() {
        assert_eq!(ConfigFormat::parse("json").unwrap(), ConfigFormat::Json);
        assert_eq!(ConfigFormat::parse("YAML").unwrap(), ConfigFormat::Yaml);
        assert_eq!(ConfigFormat::parse("yml").unwrap(), ConfigFormat::Yaml);
        assert_eq!(ConfigFormat::parse("toml").unwrap(), ConfigFormat::Toml);
        assert!(ConfigFormat::parse("xml").is_err());
    }

    #[test]
    fn canonical_filenames_are_what_we_advertise() {
        assert_eq!(ConfigFormat::Json.canonical_filename(), ".metarepo");
        assert_eq!(ConfigFormat::Yaml.canonical_filename(), ".metarepo.yaml");
        assert_eq!(ConfigFormat::Toml.canonical_filename(), ".metarepo.toml");
    }
}
