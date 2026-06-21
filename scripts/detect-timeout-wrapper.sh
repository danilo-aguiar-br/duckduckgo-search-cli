#!/usr/bin/env bash
# Detects whether the `timeout` binary on PATH is GNU coreutils (recommended
# for use with duckduckgo-search-cli -vv -v flags) or the Rust `timeout-cli`
# crate wrapper (which re-parses subprocess args and breaks clap's
# ArgAction::Count). GAP-NEW-001 v0.8.0.
#
# Exit codes:
#   0 — GNU coreutils timeout detected (use /usr/bin/timeout safely)
#   1 — `timeout` not on PATH
#   2 — Non-GNU timeout detected (workaround: use /usr/bin/timeout)
#
# Usage: scripts/detect-timeout-wrapper.sh
set -euo pipefail

timeout_path="$(command -v timeout 2>/dev/null || true)"

if [[ -z "$timeout_path" ]]; then
    echo "ERROR: \`timeout\` not on PATH" >&2
    echo "       Install GNU coreutils (e.g. apt install coreutils) or set PATH" >&2
    exit 1
fi

# GNU coreutils timeout is dynamically linked against libc.
# Rust timeout-cli v0.1.0 is statically linked.
if file "$timeout_path" 2>/dev/null | rg -q 'statically linked'; then
    echo "WARN: Non-GNU timeout detected at $timeout_path"
    echo "      This is the Rust timeout-cli crate wrapper that re-parses subprocess args."
    echo "      Symptom: 'timeout 60 duckduckgo-search-cli -vv' returns"
    echo "      'the argument --verbose cannot be used multiple times' (exit 2)."
    echo "      Workaround: use /usr/bin/timeout GNU coreutils explicitly:"
    echo "        /usr/bin/timeout 60 duckduckgo-search-cli -vv -q -f json 'query'"
    echo ""
    echo "      Run scripts/detect-timeout-wrapper.sh after fixing to verify."
    exit 2
fi

# Verify the binary is actually GNU (heuristic: --help mentions coreutils)
if "$timeout_path" --help 2>&1 | rg -q 'coreutils'; then
    echo "OK: GNU coreutils timeout at $timeout_path"
    exit 0
fi

# Fallback: file type didn't match statically linked but neither was coreutils
echo "WARN: Unknown timeout variant at $timeout_path"
echo "      file output:"
file "$timeout_path" | sed 's/^/        /'
echo "      Workaround: use /usr/bin/timeout GNU explicitly to be safe."
exit 2
