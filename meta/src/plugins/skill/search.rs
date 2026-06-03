//! Search the skills.sh registry for Claude Code skills.
//!
//! Uses the public, unauthenticated search endpoint. Installing a result is
//! handled by the `add` subcommand (see `registry.rs`).

use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use serde::Deserialize;

use super::http;

/// Default skills.sh search endpoint, used when `[skill] search-url` is unset.
pub const DEFAULT_SEARCH_URL: &str = "https://skills.sh/api/search";

/// A single search hit. `id` is the canonical `owner/repo/slug` install id.
#[derive(Debug, Deserialize)]
pub struct SkillHit {
    pub id: String,
    #[serde(rename = "skillId", default)]
    pub skill_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub installs: u64,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    skills: Vec<SkillHit>,
}

/// Query the registry at `base_url`, returning up to `limit` hits.
pub fn search(query: &str, limit: usize, base_url: &str) -> Result<Vec<SkillHit>> {
    let url = format!(
        "{base_url}?q={}&limit={}",
        http::encode(query),
        limit.clamp(1, 200)
    );
    let body = http::get(&url, None)?;
    parse(&body)
}

fn parse(body: &str) -> Result<Vec<SkillHit>> {
    let resp: SearchResponse =
        serde_json::from_str(body).context("parsing skills.sh search response")?;
    Ok(resp.skills)
}

/// `meta skill search <query>` — print matching skills from `base_url`.
pub fn run(query: &str, limit: usize, base_url: &str) -> Result<()> {
    if query.trim().chars().count() < 2 {
        return Err(anyhow!("search query must be at least 2 characters"));
    }
    let hits = search(query, limit, base_url)?;
    if hits.is_empty() {
        println!("  {} No skills found for {}", "·".dimmed(), query.cyan());
        return Ok(());
    }
    println!(
        "{} {} result(s) for {}",
        "Skills:".bold(),
        hits.len(),
        query.cyan()
    );
    for h in &hits {
        println!(
            "  {:>10}  {}",
            format_installs(h.installs).dimmed(),
            h.id.bold()
        );
        if !h.name.is_empty() && h.name != h.skill_id {
            println!("              {}", h.name.dimmed());
        }
    }
    println!("\n  Install with: {}", "meta skill add <id>".bright_cyan());
    Ok(())
}

/// Render an install count compactly (e.g. 443377 -> "443.4k").
fn format_installs(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_response() {
        let body = r#"{"query":"react","searchType":"fuzzy","skills":[
            {"id":"vercel-labs/agent-skills/vercel-react-best-practices","skillId":"vercel-react-best-practices","name":"vercel-react-best-practices","installs":443377,"source":"vercel-labs/agent-skills"}
        ],"count":1}"#;
        let hits = parse(body).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].id,
            "vercel-labs/agent-skills/vercel-react-best-practices"
        );
        assert_eq!(hits[0].installs, 443377);
        assert_eq!(hits[0].source, "vercel-labs/agent-skills");
    }

    #[test]
    fn empty_skills_is_ok() {
        let hits = parse(r#"{"skills":[]}"#).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn format_installs_scales() {
        assert_eq!(format_installs(42), "42");
        assert_eq!(format_installs(3_842), "3.8k");
        assert_eq!(format_installs(1_500_000), "1.5M");
    }
}
