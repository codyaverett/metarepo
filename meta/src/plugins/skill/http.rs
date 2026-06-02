//! Minimal HTTP GET via the system `curl`.
//!
//! The skills.sh registry is the only place this plugin fetches JSON over the
//! network, so rather than pull an async HTTP stack into the binary we shell out
//! to `curl`, which is ubiquitous on developer machines. Keep this surface tiny.

use anyhow::{anyhow, Context, Result};
use std::process::Command;

/// GET `url`, returning the response body. When `bearer` is set it is sent as an
/// `Authorization: Bearer` header. Non-2xx responses are turned into errors that
/// include the response body (curl `--fail-with-body`).
pub fn get(url: &str, bearer: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("curl");
    cmd.args([
        "-sS",
        "--fail-with-body",
        "-m",
        "30",
        "-H",
        "Accept: application/json",
    ]);
    if let Some(token) = bearer {
        cmd.arg("-H").arg(format!("Authorization: Bearer {token}"));
    }
    cmd.arg(url);

    let out = match cmd.output() {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(anyhow!(
                "curl is required to reach skills.sh but was not found on PATH"
            ));
        }
        Err(e) => return Err(e).context("running curl"),
    };

    if !out.status.success() {
        let body = String::from_utf8_lossy(&out.stdout);
        let errs = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!(
            "request to {} failed: {}{}",
            url,
            body.trim(),
            errs.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Percent-encode a string for use in a URL query value (RFC 3986 unreserved set
/// is passed through; everything else is `%XX`-escaped).
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn encodes_query_values() {
        assert_eq!(encode("react hooks"), "react%20hooks");
        assert_eq!(encode("a/b&c"), "a%2Fb%26c");
        assert_eq!(encode("plain-Text_1.0~"), "plain-Text_1.0~");
    }
}
