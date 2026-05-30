//! Integrity checks for external plugins: version-requirement matching and
//! SHA-256 digests. See `docs/PLUGIN_INTEGRITY.md`.

use anyhow::{Context, Result};
use metarepo_core::PluginManifest;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

/// Whether a plugin's self-reported `reported` version satisfies the `declared`
/// version from its `.metarepo` spec.
///
/// `declared` is interpreted as a semver *requirement*: a bare `X.Y.Z` becomes
/// a caret requirement (`^X.Y.Z`), matching Cargo's default, while explicit
/// requirements (`=1.2.3`, `>=1.2, <2.0`) are honored as written. Returns
/// `false` if either side fails to parse — an unverifiable version is treated
/// as a mismatch rather than waved through.
pub fn version_satisfies(declared: &str, reported: &str) -> bool {
    let (Ok(req), Ok(version)) = (
        semver::VersionReq::parse(declared.trim()),
        semver::Version::parse(reported.trim()),
    ) else {
        return false;
    };
    req.matches(&version)
}

/// Compute the lowercase hex SHA-256 digest of a file's contents.
pub fn sha256_file(path: &Path) -> Result<String> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open {} for hashing", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .with_context(|| format!("Failed to read {} while hashing", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

/// The file whose bytes define a plugin's identity for checksum purposes.
///
/// For a `plugin.manifest.*` path this is the executable the manifest
/// references (so we detect tampering with the actual binary, not just the
/// manifest); for any other path it is the path itself. Both the installer and
/// the loader resolve through this so they hash the same bytes.
pub fn integrity_target(path: &Path) -> Result<PathBuf> {
    if PluginManifest::is_manifest_path(path) {
        let manifest = PluginManifest::from_file_auto(path)?;
        manifest.resolve_binary(path)
    } else {
        Ok(path.to_path_buf())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn caret_match_for_bare_version() {
        // Bare X.Y.Z behaves like ^X.Y.Z: same major, >= minor/patch.
        assert!(version_satisfies("1.2.3", "1.2.3"));
        assert!(version_satisfies("1.2.3", "1.4.0"));
        assert!(!version_satisfies("1.2.3", "2.0.0"));
        assert!(!version_satisfies("1.2.3", "1.2.2"));
    }

    #[test]
    fn exact_requirement_is_strict() {
        assert!(version_satisfies("=1.2.3", "1.2.3"));
        assert!(!version_satisfies("=1.2.3", "1.2.4"));
    }

    #[test]
    fn range_requirement() {
        assert!(version_satisfies(">=1.2, <2.0", "1.9.9"));
        assert!(!version_satisfies(">=1.2, <2.0", "2.0.0"));
    }

    #[test]
    fn unparseable_sides_are_mismatch() {
        assert!(!version_satisfies("not-a-version", "1.0.0"));
        assert!(!version_satisfies("1.0.0", "not-a-version"));
    }

    #[test]
    fn sha256_known_vector() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"abc").unwrap();
        let digest = sha256_file(f.path()).unwrap();
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_detects_change() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"original").unwrap();
        let before = sha256_file(f.path()).unwrap();
        std::fs::write(f.path(), b"tampered").unwrap();
        let after = sha256_file(f.path()).unwrap();
        assert_ne!(before, after);
    }
}
