#!/usr/bin/env bash
#
# Convert local directories into their own git repositories
#
# Turns a loose directory into a git repo: git init, a default .gitignore, an
# initial commit, optionally a GitHub remote (via gh), and registration as a
# project in the surrounding metarepo workspace (via meta project add).
#
# Usage:
#   Single directory:
#     .github/scripts/dir-to-repo.sh <dir>
#     .github/scripts/dir-to-repo.sh <dir> --remote --public
#
#   Batch (every loose subdir of a parent):
#     .github/scripts/dir-to-repo.sh --scan <parent>
#     .github/scripts/dir-to-repo.sh --all          # same as --scan .
#
#   JSON stdin:
#     echo '{"dir":"./foo","remote":true,"public":false,"register":true}' \
#       | .github/scripts/dir-to-repo.sh --json
#
#   Environment variables:
#     DIR_TO_REPO_DIR=./foo DIR_TO_REPO_REMOTE=true .github/scripts/dir-to-repo.sh
#
#   Options:
#     --scan <parent>          Batch-convert each loose subdir of <parent>
#     --all                    Shorthand for --scan .
#     --remote, --push         Create a GitHub repo (gh) and push (private)
#     --public                 Make the created GitHub repo public (implies --remote)
#     --no-register            Skip meta project add (pure git init)
#     --gitignore-template <n> Template name (default: default)
#     --json                   Read input from JSON stdin
#     --silent                 Suppress non-error output (useful for automation)
#     --help, -h               Show this help message

set -euo pipefail

SILENT=false
DIR=""
SCAN=""
REMOTE=false
PUBLIC=false
REGISTER=true
GITIGNORE_TEMPLATE="default"
JSON=false

# Show help
if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    head -n 34 "$0" | tail -n +3 | sed 's/^# //;s/^#//'
    exit 0
fi

# Function to output only if not silent
log() {
    if [[ "$SILENT" == "false" ]]; then
        echo "$@"
    fi
}

err() {
    echo "Error: $*" >&2
}

# Ensure git is available
if ! command -v git &> /dev/null; then
    err "git is not installed."
    exit 1
fi

# --- Argument parsing ---------------------------------------------------------
# First pass: detect --json so we can read stdin, and --silent.
ARGS=()
for arg in "$@"; do
    case "$arg" in
        --json) JSON=true ;;
        --silent) SILENT=true ;;
        *) ARGS+=("$arg") ;;
    esac
done

if [[ "$JSON" == "true" ]]; then
    if ! command -v jq &> /dev/null; then
        err "jq is required for JSON input mode."
        echo "Install it from: https://stedolan.github.io/jq/" >&2
        exit 1
    fi
    JSON_INPUT=$(cat)
    DIR=$(echo "$JSON_INPUT" | jq -r '.dir // empty')
    SCAN=$(echo "$JSON_INPUT" | jq -r '.scan // empty')
    [[ "$(echo "$JSON_INPUT" | jq -r '.remote // empty')" == "true" ]] && REMOTE=true
    [[ "$(echo "$JSON_INPUT" | jq -r '.public // empty')" == "true" ]] && { PUBLIC=true; REMOTE=true; }
    [[ "$(echo "$JSON_INPUT" | jq -r '.register // empty')" == "false" ]] && REGISTER=false
    tmpl=$(echo "$JSON_INPUT" | jq -r '.gitignore_template // empty')
    [[ -n "$tmpl" ]] && GITIGNORE_TEMPLATE="$tmpl"
    [[ -n "$SCAN" ]] && SCAN="${SCAN}"
elif [[ ${#ARGS[@]} -gt 0 ]]; then
    # Command-line flags + positional dir
    i=0
    while [[ $i -lt ${#ARGS[@]} ]]; do
        a="${ARGS[$i]}"
        case "$a" in
            --scan)
                i=$((i + 1))
                SCAN="${ARGS[$i]:-}"
                ;;
            --all) SCAN="." ;;
            --remote|--push) REMOTE=true ;;
            --public) PUBLIC=true; REMOTE=true ;;
            --no-register) REGISTER=false ;;
            --gitignore-template)
                i=$((i + 1))
                GITIGNORE_TEMPLATE="${ARGS[$i]:-default}"
                ;;
            -*)
                err "Unknown option: $a"
                exit 1
                ;;
            *)
                DIR="$a"
                ;;
        esac
        i=$((i + 1))
    done
else
    # Environment-variable mode (falls back to interactive for the dir)
    DIR="${DIR_TO_REPO_DIR:-}"
    SCAN="${DIR_TO_REPO_SCAN:-}"
    [[ "${DIR_TO_REPO_REMOTE:-}" == "true" ]] && REMOTE=true
    [[ "${DIR_TO_REPO_PUBLIC:-}" == "true" ]] && { PUBLIC=true; REMOTE=true; }
    [[ "${DIR_TO_REPO_REGISTER:-}" == "false" ]] && REGISTER=false
    GITIGNORE_TEMPLATE="${DIR_TO_REPO_GITIGNORE_TEMPLATE:-default}"
    if [[ -z "$DIR" && -z "$SCAN" ]]; then
        read -r -p "Directory to convert (or leave blank to scan current dir): " DIR
        [[ -z "$DIR" ]] && SCAN="."
    fi
fi

if [[ -z "$DIR" && -z "$SCAN" ]]; then
    err "Provide a directory, or use --scan <parent> / --all."
    exit 1
fi

# meta binary: prefer META_BIN, then PATH, then local debug build.
META_BIN="${META_BIN:-}"
if [[ -z "$META_BIN" ]]; then
    if command -v meta &> /dev/null; then
        META_BIN="meta"
    elif [[ -x "target/debug/meta" ]]; then
        META_BIN="target/debug/meta"
    elif [[ -x "target/release/meta" ]]; then
        META_BIN="target/release/meta"
    fi
fi

# --- Helpers ------------------------------------------------------------------

# Find the metarepo workspace root (dir containing .meta or .metarepo) by walking
# up from $1. Echoes the absolute path, or empty if none found.
find_workspace_root() {
    local d
    d="$(cd "$1" 2>/dev/null && pwd -P)" || return 0
    while [[ -n "$d" && "$d" != "/" ]]; do
        for f in .meta .metarepo .metarepo.json .metarepo.yaml .metarepo.yml .metarepo.toml; do
            if [[ -e "$d/$f" ]]; then
                echo "$d"
                return 0
            fi
        done
        d="$(dirname "$d")"
    done
}

# Write the default .gitignore unless one already exists.
write_gitignore() {
    local dir="$1"
    if [[ -f "$dir/.gitignore" ]]; then
        return 0
    fi
    # MVP ships a single cross-language template. Language auto-detection is a
    # future enhancement keyed off --gitignore-template.
    case "$GITIGNORE_TEMPLATE" in
        default|*)
            cat > "$dir/.gitignore" <<'EOF'
# OS / editor
.DS_Store
.idea/
.vscode/
*.swp

# Logs / env
*.log
.env
.env.local

# Dependencies / build output
node_modules/
target/
dist/
build/
__pycache__/
*.py[cod]
EOF
            ;;
    esac
}

# Returns 0 if $1 (a directory) sits inside an existing git work tree whose root
# is a *parent* of $1 (i.e. it would be nested inside another repo).
is_nested_in_other_repo() {
    local dir="$1" top
    top="$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null || true)"
    if [[ -n "$top" ]]; then
        local abs
        abs="$(cd "$dir" && pwd -P)"
        if [[ "$top" != "$abs" ]]; then
            return 0
        fi
    fi
    return 1
}

CONVERTED=0
SKIPPED=0
FAILED=0

# Convert a single directory. Returns non-zero on hard failure.
convert_dir() {
    local dir="$1"

    if [[ ! -d "$dir" ]]; then
        err "'$dir' is not a directory."
        FAILED=$((FAILED + 1))
        return 1
    fi

    local abs
    abs="$(cd "$dir" && pwd -P)"
    local name
    name="$(basename "$abs")"

    # Already a git repo at this exact path? Idempotent skip.
    if [[ -e "$abs/.git" ]] && git -C "$abs" rev-parse --git-dir &>/dev/null; then
        local top
        top="$(git -C "$abs" rev-parse --show-toplevel 2>/dev/null || true)"
        if [[ "$top" == "$abs" ]]; then
            log "↪ Skipping '$dir' (already a git repository)"
            SKIPPED=$((SKIPPED + 1))
            return 0
        fi
    fi

    # Nested inside another repo's work tree? Refuse.
    if is_nested_in_other_repo "$abs"; then
        err "'$dir' is nested inside an existing git repository; refusing to convert."
        FAILED=$((FAILED + 1))
        return 1
    fi

    log "🌱 Converting '$dir' → git repository"

    git -C "$abs" init -b main >/dev/null 2>&1 || git -C "$abs" init >/dev/null

    write_gitignore "$abs"

    git -C "$abs" add -A
    # Disable GPG/SSH commit signing for this throwaway initial commit so the
    # script never blocks on a signing passphrase/key prompt.
    if ! git -C "$abs" -c commit.gpgsign=false commit -m "chore: initialize repository" >/dev/null 2>&1; then
        git -C "$abs" -c commit.gpgsign=false commit --allow-empty -m "chore: initialize repository" >/dev/null 2>&1 || true
    fi
    log "   ✅ git initialized + initial commit"

    # Optional remote via gh.
    local remote_url=""
    if [[ "$REMOTE" == "true" ]]; then
        if ! command -v gh &> /dev/null; then
            err "gh (GitHub CLI) not installed; skipping remote for '$dir'. Local repo left intact."
        elif ! gh auth status &>/dev/null; then
            err "gh is not authenticated (run 'gh auth login'); skipping remote for '$dir'. Local repo left intact."
        else
            local visibility="--private"
            [[ "$PUBLIC" == "true" ]] && visibility="--public"
            remote_url=$( (cd "$abs" && gh repo create "$name" --source . --push "$visibility") 2>&1 \
                | tee /dev/stderr | grep -o 'https://[^ ]*' || true)
            if [[ -n "$remote_url" ]]; then
                log "   ✅ remote created + pushed: $remote_url"
            else
                err "Failed to create remote for '$dir'. Local repo left intact."
            fi
        fi
    fi

    # Optional registration as a metarepo workspace project.
    local registered="no"
    if [[ "$REGISTER" == "true" ]]; then
        local root
        root="$(find_workspace_root "$abs")"
        if [[ -z "$root" ]]; then
            log "   ⚠ no metarepo workspace found; skipping registration"
        elif [[ -z "$META_BIN" ]]; then
            log "   ⚠ meta binary not found (set META_BIN or build it); skipping registration"
        elif [[ "$abs" != "$root/"* ]]; then
            log "   ⚠ '$dir' is outside the workspace root ($root); skipping registration"
        else
            local relname="${abs#"$root"/}"
            if (cd "$root" && "$META_BIN" project add "$relname" --init-git) >/dev/null 2>&1; then
                registered="yes"
                log "   ✅ registered as workspace project '$relname'"
            else
                log "   ⚠ registration skipped (already registered, or no .meta at workspace root)"
            fi
        fi
    fi

    CONVERTED=$((CONVERTED + 1))
    # Machine-readable summary line (always printed, even in --silent).
    echo "converted: $abs (remote=${remote_url:-none}, registered=$registered)"
    return 0
}

# --- Drive single or batch ----------------------------------------------------

if [[ -n "$SCAN" ]]; then
    if [[ ! -d "$SCAN" ]]; then
        err "--scan target '$SCAN' is not a directory."
        exit 1
    fi
    log "🔎 Scanning '$SCAN' for loose directories..."
    shopt -s nullglob
    for child in "$SCAN"/*/; do
        child="${child%/}"
        base="$(basename "$child")"
        [[ "$base" == ".git" ]] && continue
        convert_dir "$child" || true
    done
    shopt -u nullglob
else
    convert_dir "$DIR" || true
fi

log ""
log "Done. Converted: $CONVERTED, Skipped: $SKIPPED, Failed: $FAILED"

if [[ "$CONVERTED" -eq 0 && "$FAILED" -gt 0 ]]; then
    exit 1
fi
exit 0
