# Threat Model

Status: living document (issue #52). Update on major architectural changes —
new distribution channels, new plugin execution paths, changes to the release
pipeline, or new trust anchors.

This document records what metarepo trusts, what attacker capabilities we
defend against, the mitigations already in place, and the residual risks we
have explicitly accepted. It is the shared context for all supply-chain
hardening decisions, so tradeoffs are not re-litigated per issue.

## Scope

Covered: the path from source code to a malicious binary or crate reaching a
user — dependencies, CI, the release pipeline, and the plugin/skill
installation surface that metarepo itself implements.

Out of scope:

- **User-directed installs.** `meta plugin add git+...`, `meta skill steal`,
  and `file:` plugins execute code the user explicitly chose to install from a
  source they named. We verify *integrity* (the bytes match what was declared)
  but not *trustworthiness* of the source — that judgment belongs to the user.
- **Local machine compromise.** An attacker with the maintainer's machine has
  the GPG key and git push access; no repo-side control survives that.
- **Vulnerabilities in metarepo's own logic** (path traversal, command
  injection). Those are handled by the security test suite and fuzz targets
  (`fuzz/fuzz_targets/command_injection.rs`, `path_traversal.rs`), not by this
  supply-chain model.

## Trust anchors

What we trust, and what its compromise would mean:

| Anchor | Trusted for | Compromise impact |
|---|---|---|
| Maintainer GPG key | Signing commits and release tags | Forged "authentic" history and releases |
| Maintainer GitHub account | Push to main, repo settings, secrets | Full pipeline control: malicious code, workflow edits, secret exfiltration |
| GitHub Actions runners | Building and publishing releases | Malicious artifacts signed-looking and checksummed by the same compromised run |
| crates.io registry + index | Serving the crates Cargo.lock pins | Malicious dependency code at build time (mitigated by lockfile checksums; registry-level compromise of *existing* versions would be caught by checksum mismatch) |
| RustSec advisory DB | Vulnerability intelligence | Silence: missing advisories, not malicious code |
| ~29 direct dependencies (211 packages total in Cargo.lock, including the 3 workspace crates) | Correct, non-malicious code | Arbitrary code in every build and in users' binaries |
| Pinned GitHub Actions (checkout, dtolnay/rust-toolchain, release-plz, cache) | CI steps with checkout and secret access | Build tampering, secret theft — pinning by full SHA means compromise requires a PR changing the pin, not a silent tag move |

## Attacker capabilities considered

1. **Publishes a malicious new version of an existing dependency.**
   Blocked at build time: Cargo.lock is committed and release builds use
   `--locked`, so new versions never enter a build without a reviewed lockfile
   diff. cargo-audit (daily cron + every push/PR) flags known-bad versions.

2. **Compromises an existing, already-pinned dependency version on the
   registry.** Cargo verifies the checksum recorded in Cargo.lock; swapped
   bytes for a pinned version fail to build.

3. **Typosquats a dependency name.** deny.toml locks sources to the crates.io
   index only (`unknown-registry = "deny"`); the direct dependency set is
   small and stable, and any new name arrives via a reviewable manifest diff.
   Dedicated scanning was evaluated and rejected as low-value (#45).

4. **Moves a tag on a GitHub Action we use.** All workflow actions are pinned
   to full commit SHAs with a version comment; a moved tag has no effect.

5. **Steals the crates.io publish token.** Today this is the live worst gap:
   `release-plz.yml` injects a static `CARGO_REGISTRY_TOKEN` from repo
   secrets. A leak (or any code that can read repo secrets) publishes
   malicious crate versions under our name. Fix in flight: crates.io trusted
   publishing via GitHub OIDC (#38, P0) removes the long-lived token.

6. **Steals or abuses the `RELEASE_TOKEN` PAT.** Used by release-plz for
   checkout and release PRs with `contents: write`. Workflows default to
   `contents: read` and escalate per job, limiting what other jobs expose,
   but the PAT itself remains a long-lived credential (accepted for now;
   revisit when fine-grained tokens fit the release-plz flow).

7. **Tampers with release artifacts after build.** `release-binaries.yml`
   builds 5 targets with `--locked` and uploads a per-asset `.sha256`.
   Limitation: the checksums live in the same GitHub release as the binaries,
   so an attacker who can replace one can replace both. Independent trust
   path (provenance attestation, verify docs) is epic #96 (P1).

8. **Forges commits or tags as the maintainer.** Commit signing and tag
   signing are enabled; signed tags anchor releases to the GPG key rather
   than to GitHub account state alone.

9. **Swaps an installed plugin binary on the user's disk.** metarepo's own
   integrity layer (docs/PLUGIN_INTEGRITY.md): version enforcement on every
   load (semver check against the `.metarepo` pin, hard failure on mismatch)
   plus opt-in checksum verification of exact bytes. Skill installs are
   fingerprinted (sha256 in `.skill-lock.json`) so updates refuse to clobber
   modified files (v0.52.0).

10. **Introduces memory-unsafety through dependencies.** cargo-geiger runs in
    CI (informational — transitive unsafe from git2/tokio is expected and
    unavoidable). Workspace-side unsafe is limited to three blocks in
    `meta-core/src/tui/app.rs`; clippy security lints (`mem_forget`,
    `unused_io_amount`, etc.) are denied in CI.

## Mitigations in place

- **cargo-audit**: push, PR, and daily 2 AM UTC cron; fails on any advisory
  not in the explicit `.cargo/audit.toml` ignore list.
- **cargo-deny** (blocking): license allowlist, `wildcards = "deny"`,
  `yanked = "deny"`, registry source lockdown to crates.io,
  `unknown-git = "warn"`.
- **Committed Cargo.lock + `--locked` release builds**: no unreviewed
  dependency drift can reach a release.
- **SHA-pinned actions** across all workflows, with least-privilege
  `permissions:` blocks (default `contents: read`, per-job escalation).
- **Signed commits and tags** (gpgsign enabled).
- **Per-asset SHA-256 checksums** on release binaries.
- **Plugin version enforcement + opt-in checksum integrity**; skill install
  fingerprinting with clobber refusal.
- **Security clippy lints denied in CI**; cargo-geiger unsafe reporting.
- **Fuzz targets** for command injection and path traversal.

## Accepted residual risks

| Risk | Why accepted |
|---|---|
| Static `CARGO_REGISTRY_TOKEN` until #38 lands | Fix is queued as P0; one-time config change |
| `RELEASE_TOKEN` is a long-lived PAT | Needed by release-plz for cross-workflow pushes; scoped permissions limit blast radius |
| Checksums share a trust channel with binaries until #96 | Provenance attestation epic is P1 |
| 4 ignored RUSTSEC advisories (paste, lru, bytes, git2) | All transitive, no patched upstream release yet; listed explicitly in audit.toml and deny.toml, revisited on dependency bumps |
| `unknown-git = "warn"` not `"deny"` in deny.toml | `allow-git = []` and no git dependencies exist today; flip to deny is a known one-liner (noted in #31 closure) |
| CI security tools installed unpinned (`cargo install cargo-audit` etc.) | Tools run in read-only jobs without secrets; a malicious tool could lie about findings but not touch releases |
| No manual dependency audits (cargo-vet #29, cargo-crev #30, cackle #33) | Certification of ~200 transitive crates re-done every bump is unsustainable for a solo project |
| No exact `=x.y.z` version pins (#32) | Committed Cargo.lock already pins builds; manifest pins are an app-crate anti-pattern |
| No reproducible builds (#39) | Bit-for-bit reproducibility across a 5-target matrix is high-maintenance; `--locked` + checksums cover the practical risk |
| No SBOM publication (#44) | No downstream audience today; Cargo.lock already records the full tree. Icebox |
| No vendored dependencies (#35) | `--locked` + checksummed Cargo.lock defeats the registry-swap threat vendoring would address. Icebox |
| OpenSSL (vendored, via git2) instead of rustls (#49) | Only TLS consumer is libgit2, which has no rustls path; vendoring pins the exact OpenSSL source |

## Issue-to-threat map (#29-53 supply-chain cluster)

| Issue | Threat addressed | Status |
|---|---|---|
| #29 cargo-vet | Malicious dependency code (capability 1) | Closed not planned — unsustainable upkeep |
| #30 cargo-crev | Malicious dependency code (1) | Closed not planned — sparse community DB |
| #31 deny.toml hardening | Unvetted sources (3) | Closed shipped (~90%); unknown-git flip remains a known one-liner |
| #32 exact version pins | Dependency drift (1) | Closed not planned — Cargo.lock suffices |
| #33 cackle sandbox | Capability abuse by deps (1) | Closed not planned — brittle, Linux-only |
| #35 vendored deps | Registry compromise (2) | Open P3 icebox |
| #38 trusted publishing OIDC | Publish-token theft (5) | Open P0 — next up |
| #39 reproducible builds | Build tampering (7) | Closed not planned — high maintenance |
| #41 cargo-audit CI | Known-vulnerable deps (1) | Shipped (security.yml) |
| #42 cargo-deny CI | Sources, licenses, yanked crates (1, 3) | Shipped (security.yml) |
| #44 SBOM publication | Downstream auditability (7) | Open P3 icebox |
| #45 typosquat scanning | Typosquatting (3) | Closed not planned — deny.toml covers the vector |
| #46 signed commits/tags | Identity forgery (8) | Shipped; fingerprint doc folded into #53 |
| #48 release checksums | Artifact tampering (7) | Shipped (release-binaries.yml) |
| #49 rustls migration | OpenSSL exposure (10) | Closed not planned — no rustls path for libgit2 |
| #50 cargo-geiger CI | Unsafe-code creep (10) | Shipped, intentionally informational |
| #51 periodic dep reduction | Attack-surface growth (1) | Open P3 icebox |
| #52 threat model | Shared context for all of the above | This document |
| #53 SECURITY.md expansion | Reporting and disclosure gaps | Open P2 |
| #96 provenance attestation epic | Same-channel checksum weakness (7) | Open P1 |

## Review triggers

Re-read and update this document when any of the following changes:

- A new distribution channel is added (homebrew tap, package managers, container images).
- The release pipeline changes (replacing release-plz, new publish targets).
- Plugins or skills gain a new execution or installation path.
- A new long-lived credential is added to repo secrets.
- #38 or #96 lands (move the corresponding rows out of residual risks).
