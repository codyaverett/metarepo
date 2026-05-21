#!/usr/bin/env bash
# Example metarepo manifest plugin. metarepo execs this with the resolved
# subcommand and parsed args as argv, plus METAREPO_* context env vars and
# METAREPO_ARG_<NAME> for each declared argument.
set -euo pipefail

subcommand="${1:-}"

case "$subcommand" in
  hello)
    name="${METAREPO_ARG_NAME:-world}"
    greeting="Hello, ${name}!"
    if [ "${METAREPO_ARG_LOUD:-}" = "1" ]; then
      greeting="$(printf '%s' "$greeting" | tr '[:lower:]' '[:upper:]')"
    fi
    echo "$greeting"
    echo "  (workspace root: ${METAREPO_ROOT:-none})"
    ;;
  *)
    echo "usage: meta greet hello <name> [--loud]" >&2
    exit 1
    ;;
esac
