#!/bin/bash

# Test script for dir-to-repo.sh — converting loose directories into git repos.
#
# Exercises: single local-only conversion, idempotent re-run, nested rejection,
# batch scan, workspace registration, and JSON mode. The GitHub-remote path is
# gated on `gh auth status` and skipped when gh is unavailable.

set -u

SCRIPT="/Users/caavere/Projects/metarepo/.github/scripts/dir-to-repo.sh"
META_BIN="/Users/caavere/Projects/metarepo/target/debug/meta"
export META_BIN
TESTROOT="/tmp/test-dir-to-repo"

PASS=0
FAIL=0
ok()   { echo "✓ $1"; PASS=$((PASS + 1)); }
bad()  { echo "✗ $1"; FAIL=$((FAIL + 1)); }

echo "=== Testing dir-to-repo.sh ==="
echo

if [[ ! -x "$META_BIN" ]]; then
    echo "Building meta binary first (cargo build)..."
    (cd /Users/caavere/Projects/metarepo && cargo build --bin meta) || {
        echo "Build failed; registration tests will be skipped."
    }
fi

rm -rf "$TESTROOT"
mkdir -p "$TESTROOT"

# --- Test 1: single, local-only --------------------------------------------
echo "1. Single directory, local-only (--no-register)..."
mkdir -p "$TESTROOT/proj"
echo "hi" > "$TESTROOT/proj/file.txt"
"$SCRIPT" "$TESTROOT/proj" --no-register --silent >/dev/null
if [[ -d "$TESTROOT/proj/.git" ]]; then ok "git repo created"; else bad "no .git created"; fi
if [[ -f "$TESTROOT/proj/.gitignore" ]]; then ok ".gitignore written"; else bad ".gitignore missing"; fi
commits=$(git -C "$TESTROOT/proj" rev-list --count HEAD 2>/dev/null || echo 0)
if [[ "$commits" == "1" ]]; then ok "exactly one commit"; else bad "expected 1 commit, got $commits"; fi
echo

# --- Test 2: idempotent re-run ---------------------------------------------
echo "2. Idempotent re-run..."
out=$("$SCRIPT" "$TESTROOT/proj" --no-register 2>&1 || true)
if echo "$out" | grep -qi "already a git repository"; then ok "re-run skipped"; else bad "re-run not skipped"; fi
commits=$(git -C "$TESTROOT/proj" rev-list --count HEAD 2>/dev/null || echo 0)
if [[ "$commits" == "1" ]]; then ok "no extra commit"; else bad "commit count changed to $commits"; fi
echo

# --- Test 3: nested rejection ----------------------------------------------
echo "3. Nested-inside-repo rejection..."
mkdir -p "$TESTROOT/proj/sub"
out=$("$SCRIPT" "$TESTROOT/proj/sub" --no-register 2>&1 || true)
if echo "$out" | grep -qi "nested inside"; then ok "nested dir refused"; else bad "nested dir not refused"; fi
echo

# --- Test 4: batch scan -----------------------------------------------------
echo "4. Batch scan (--scan)..."
mkdir -p "$TESTROOT/batch/a" "$TESTROOT/batch/b" "$TESTROOT/batch/c"
echo a > "$TESTROOT/batch/a/x"; echo b > "$TESTROOT/batch/b/x"
git -C "$TESTROOT/batch/c" init -q   # pre-existing repo
"$SCRIPT" --scan "$TESTROOT/batch" --no-register --silent >/dev/null
if [[ -d "$TESTROOT/batch/a/.git" && -d "$TESTROOT/batch/b/.git" ]]; then ok "a and b converted"; else bad "batch conversion failed"; fi
echo

# --- Test 5: workspace registration ----------------------------------------
# NOTE: `meta project add` currently only reads a `.meta` config file (it does
# not honor `.metarepo`, which `meta init` now writes by default). Registration
# is therefore validated against a `.meta` workspace here. See the tracked issue
# for the underlying `.metarepo` limitation.
echo "5. Workspace registration (.meta workspace)..."
if [[ -x "$META_BIN" ]]; then
    ws="$TESTROOT/ws"
    mkdir -p "$ws"
    printf '{\n  "projects": {}\n}\n' > "$ws/.meta"
    mkdir -p "$ws/loose"
    echo hi > "$ws/loose/file.txt"
    "$SCRIPT" "$ws/loose" >/dev/null 2>&1 || true
    if grep -q "loose" "$ws/.meta" 2>/dev/null; then ok "registered in .meta workspace config"; else bad "not found in $ws/.meta"; fi
else
    echo "  (skipped: meta binary not available)"
fi
echo

# --- Test 6: JSON mode ------------------------------------------------------
echo "6. JSON stdin mode..."
out=$(echo "{\"dir\":\"$TESTROOT/jsonproj\",\"register\":false}" | bash -c "mkdir -p $TESTROOT/jsonproj; cat | $SCRIPT --json --silent")
if echo "$out" | grep -q "^converted:"; then ok "JSON mode emitted summary"; else bad "JSON mode no summary: $out"; fi
echo

# --- Test 7: remote (gated) -------------------------------------------------
echo "7. GitHub remote (gated on gh auth)..."
if command -v gh &>/dev/null && gh auth status &>/dev/null; then
    echo "  gh authenticated — remote test could run here (skipped to avoid creating real repos)."
else
    echo "  (skipped: gh not authenticated)"
fi
echo

echo "=== Test Complete ==="
echo "Passed: $PASS, Failed: $FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
