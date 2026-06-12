# Backlog Grooming — 2026-06-12

Full-backlog grooming pass over all 31 open issues. Each issue was independently
assessed against the codebase (implementation status verified in-tree), then
synthesized into a prioritized backlog. Result: 19 issues closed, 1 consolidated
epic opened (#96), 13 issues remain — all carrying a P0–P3 label.

## Outcome summary

- **Before**: 31 open issues, 19 of them a supply-chain checklist cluster (#29–53), most labeled `needs-triage`.
- **After**: 13 open issues, every one prioritized. Supply-chain cluster reduced to 3 active threads (#38, #52, #96) plus parked items.

## Closed as completed (verified shipped)

| Issue | Evidence |
|---|---|
| #91 skill clobber guard | v0.52.0 (`0e45caa`): sha256 fingerprints, `.skill-lock.json`, `RefuseReason`, `update --force`, 7 tests |
| #90 MCP gateway epic | All 4 phases shipped v0.47.0–v0.50.0; sub-issues #86/#87/#88/#89 done |
| #87 gateway meta-tools | `mcp_catalog` / `mcp_list_tools` / `mcp_search_tools` / `mcp_call` in `meta/src/plugins/mcp/mcp_server.rs` |
| #82 config TUI | v0.36.0–v0.41.0; `tui_editor.rs` catalog-driven CRUD, 21+ tests |
| #41 cargo-audit CI | `security.yml`: push/PR/daily cron, fails on advisories, auto-issue notify |
| #42 cargo-deny CI | Blocking job in `security.yml`; deny.toml allowlists present |
| #50 cargo-geiger | `unsafe-code-check` job in `security.yml`, intentionally informational |
| #48 release checksums | `release-binaries.yml` uploads per-asset `.sha256` |
| #46 signed commits/tags | gpgsign enabled, recent tags signed; leftover fingerprint doc folded into #53 |
| #31 deny.toml hardening | ~90% shipped; only delta is `unknown-git` warn→deny one-liner |

## Closed as not planned (high upkeep, low payoff for a solo project)

| Issue | Rationale |
|---|---|
| #29 cargo-vet | Manual certification of ~211 transitive crates, re-certified every bump |
| #30 cargo-crev | Sparse community DB, ongoing manual source audits |
| #32 exact-version pins | Committed Cargo.lock already pins builds; `=x.y.z` pins are an app-crate anti-pattern |
| #33 cackle sandbox | Per-crate capability manifests re-reviewed every bump; Linux-only, brittle in CI |
| #39 reproducible builds | Bit-for-bit across 5-target matrix is high-maintenance; existing posture covers practical risk |
| #45 typosquat scan | 18 stable direct deps; deny.toml source lockdown already blocks the vector |
| #49 rustls over OpenSSL | Only OpenSSL is transitive via git2/libgit2, which must stay; no Rust TLS client exists to convert |

## Consolidated

- **#96 (new epic)** absorbs #40 (SLSA provenance) + #47 (cosign) + #48 (verify docs).
  Plan: `actions/attest-build-provenance` for SLSA-style provenance + sigstore signing,
  one documented verify recipe. Cosign and standalone slsa-github-generator superseded.
- **#46 leftover** (GPG fingerprint documentation) absorbed into #53 (SECURITY.md expansion).

## Prioritized backlog (13 open)

### P0 — now
- **#38 crates.io trusted publishing (OIDC)** — `release-plz.yml` still injects a static
  `CARGO_REGISTRY_TOKEN`; removing the last long-lived secret is a one-time config change.
- **#52 threat model document** — expanded into the triage anchor for the supply-chain
  cluster: trust anchors, attacker capabilities, mitigations in place, residual risks,
  and a prioritized table mapping each #29–53 issue to a threat.

### P1 — next
- **#84 `meta project check`** — reshaped from gitignore rename into a general
  workspace-hygiene drift check (.meta vs disk vs .gitignore), `--fix`, non-zero exit for CI.
- **#81 config cascade write/run phases** — read layer shipped v0.42.0 (`discover_chain_from`);
  remaining: write targeting, run-script cascade, child init.
- **#96 release-artifact verification epic** — see above.

### P2 — later
- **#53 SECURITY.md supply-chain expansion** (+ fingerprint doc from #46; fix email placeholder)
- **#80 env vars → config with precedence** — small now that #74 (ConfigSetting trait) landed
- **#76 configurable bundled skill install path** — `skill_root()` hardcodes
  `.claude/skills/meta-tool` while stolen skills honor `[skill] dest`; real inconsistency

### P3 — icebox
- **#83** skill audit patterns/dest roots — niche config knobs
- **#77** richer adapt templating — most placeholders redundant; only `{prompt_file}`/stdin marginal
- **#44** SBOMs — no downstream audience; Cargo.lock already pins the tree
- **#35** vendored deps — `--locked` + checksummed Cargo.lock already defeats the core threat
- **#51** periodic dep-reduction review — recurring rituals rot; only durable piece is a
  one-line cargo-machete CI step if ever wanted

## Strategic direction

The supply-chain cluster was a checklist dump, not a risk-driven plan. The security
pipeline is already mature (audit / deny / geiger / fuzz / signed commits /
source-locked deny.toml / `--locked` builds). Focus returns to the core mission —
multi-repo workspace management — via #84 and #81, with #38 and the threat model (#52)
as the only near-term security work.
