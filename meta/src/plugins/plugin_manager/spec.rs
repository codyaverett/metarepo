//! Parsing and formatting of plugin specs as stored in `.metarepo` under
//! `plugins.<name>`.
//!
//! Three source kinds are supported:
//! - `crates:<crate>` or `crates:<crate>@<version>` — install from crates.io.
//! - `file:<path>` — a local executable.
//! - `git+<url>` — clone and build from a git repository.
//!
//! For backwards compatibility a bare string (e.g. `"0.1.0"`, `"*"`,
//! `"^latest"`) is treated as a crates.io version for the conventional crate
//! name `metarepo-plugin-<plugin>`.

use anyhow::{bail, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginSpec {
    Crates {
        crate_name: String,
        version: Option<String>,
    },
    File {
        path: String,
    },
    Git {
        url: String,
    },
}

/// Conventional crates.io crate name for a plugin command name.
pub fn default_crate_name(plugin_name: &str) -> String {
    format!("metarepo-plugin-{plugin_name}")
}

fn is_unpinned(version: &str) -> bool {
    matches!(version, "" | "*" | "latest" | "^latest")
}

fn split_name_version(rest: &str) -> (String, Option<String>) {
    match rest.rsplit_once('@') {
        Some((name, version)) if !name.is_empty() => {
            let version = if is_unpinned(version) {
                None
            } else {
                Some(version.to_string())
            };
            (name.to_string(), version)
        }
        _ => (rest.to_string(), None),
    }
}

impl PluginSpec {
    /// Parse a stored spec string. `plugin_name` supplies the default crate
    /// name for the bare and prefix-less `crates:` forms.
    pub fn parse(plugin_name: &str, spec: &str) -> Result<Self> {
        let spec = spec.trim();

        if let Some(path) = spec.strip_prefix("file:") {
            if path.is_empty() {
                bail!("file: spec is missing a path");
            }
            return Ok(PluginSpec::File {
                path: path.to_string(),
            });
        }

        if let Some(url) = spec.strip_prefix("git+") {
            if url.is_empty() {
                bail!("git+ spec is missing a URL");
            }
            return Ok(PluginSpec::Git {
                url: url.to_string(),
            });
        }

        if let Some(rest) = spec.strip_prefix("crates:") {
            let (crate_name, version) = split_name_version(rest);
            let crate_name = if crate_name.is_empty() {
                default_crate_name(plugin_name)
            } else {
                crate_name
            };
            return Ok(PluginSpec::Crates {
                crate_name,
                version,
            });
        }

        // Bare back-compat form: a crates.io version for the default crate.
        let version = if is_unpinned(spec) {
            None
        } else {
            Some(spec.to_string())
        };
        Ok(PluginSpec::Crates {
            crate_name: default_crate_name(plugin_name),
            version,
        })
    }

    /// Build a spec from an explicit `--from` value plus an optional `--version`.
    /// When `from` is None the source defaults to crates.io for the
    /// conventional crate name.
    pub fn from_args(plugin_name: &str, from: Option<&str>, version: Option<&str>) -> Result<Self> {
        let mut spec = match from {
            Some(f) => PluginSpec::parse(plugin_name, f)?,
            None => PluginSpec::Crates {
                crate_name: default_crate_name(plugin_name),
                version: None,
            },
        };
        if let Some(v) = version {
            match &mut spec {
                PluginSpec::Crates { version, .. } => *version = Some(v.to_string()),
                _ => bail!("--version only applies to crates.io plugins"),
            }
        }
        Ok(spec)
    }

    /// Canonical string to persist in `.metarepo`.
    pub fn to_spec_string(&self) -> String {
        match self {
            PluginSpec::File { path } => format!("file:{path}"),
            PluginSpec::Git { url } => format!("git+{url}"),
            PluginSpec::Crates {
                crate_name,
                version,
            } => match version {
                Some(v) => format!("crates:{crate_name}@{v}"),
                None => format!("crates:{crate_name}"),
            },
        }
    }

    /// The version declared by the spec, if any (crates.io only).
    pub fn declared_version(&self) -> Option<&str> {
        match self {
            PluginSpec::Crates { version, .. } => version.as_deref(),
            _ => None,
        }
    }

    pub fn source_label(&self) -> &'static str {
        match self {
            PluginSpec::Crates { .. } => "crates.io",
            PluginSpec::File { .. } => "file",
            PluginSpec::Git { .. } => "git",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_file_spec() {
        let s = PluginSpec::parse("hello", "file:/tmp/bin").unwrap();
        assert_eq!(
            s,
            PluginSpec::File {
                path: "/tmp/bin".into()
            }
        );
        assert_eq!(s.to_spec_string(), "file:/tmp/bin");
    }

    #[test]
    fn parses_git_spec() {
        let s = PluginSpec::parse("hello", "git+https://example.com/p.git").unwrap();
        assert_eq!(
            s,
            PluginSpec::Git {
                url: "https://example.com/p.git".into()
            }
        );
        assert_eq!(s.to_spec_string(), "git+https://example.com/p.git");
    }

    #[test]
    fn parses_crates_with_and_without_version() {
        let pinned = PluginSpec::parse("hello", "crates:metarepo-plugin-hello@0.2.0").unwrap();
        assert_eq!(
            pinned,
            PluginSpec::Crates {
                crate_name: "metarepo-plugin-hello".into(),
                version: Some("0.2.0".into())
            }
        );
        assert_eq!(pinned.declared_version(), Some("0.2.0"));

        let unpinned = PluginSpec::parse("hello", "crates:metarepo-plugin-hello").unwrap();
        assert_eq!(unpinned.declared_version(), None);
    }

    #[test]
    fn bare_string_is_crates_version_for_default_crate() {
        let s = PluginSpec::parse("hello", "0.1.0").unwrap();
        assert_eq!(
            s,
            PluginSpec::Crates {
                crate_name: "metarepo-plugin-hello".into(),
                version: Some("0.1.0".into())
            }
        );
    }

    #[test]
    fn legacy_caret_latest_is_unpinned() {
        let s = PluginSpec::parse("hello", "^latest").unwrap();
        assert_eq!(s.declared_version(), None);
        assert_eq!(s.to_spec_string(), "crates:metarepo-plugin-hello");
    }

    #[test]
    fn from_args_defaults_to_crates() {
        let s = PluginSpec::from_args("hello", None, None).unwrap();
        assert_eq!(s.to_spec_string(), "crates:metarepo-plugin-hello");
    }

    #[test]
    fn from_args_applies_version_to_crates() {
        let s = PluginSpec::from_args("hello", Some("crates:metarepo-plugin-hello"), Some("1.0.0"))
            .unwrap();
        assert_eq!(s.declared_version(), Some("1.0.0"));
    }

    #[test]
    fn from_args_version_on_file_is_error() {
        let err = PluginSpec::from_args("hello", Some("file:/tmp/x"), Some("1.0.0")).unwrap_err();
        assert!(err.to_string().contains("--version"));
    }

    #[test]
    fn empty_file_spec_errors() {
        assert!(PluginSpec::parse("hello", "file:").is_err());
    }
}
