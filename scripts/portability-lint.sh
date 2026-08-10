#!/usr/bin/env bash
# Portability lint — blocks the E0432 class that shipped broken in v1.0.2.
#
# Rust strips `cfg`-disabled items BEFORE name resolution. An ungated `use` of
# a platform-gated item therefore fails to compile on every *other* platform,
# even when every call site is correctly gated. That is precisely how
# `src/browser/session/mod.rs` broke macOS and Windows in v1.0.2:
#
#     use super::xvfb::{detect_linux_distro, ...};   // no cfg here
#     #[cfg(target_os = "linux")]
#     pub(crate) fn detect_linux_distro() -> String  // only exists on Linux
#
# This is a CHEAP pre-check meant to fail in milliseconds, not a substitute for
# a real cross-target build. The authoritative gate is `cargo check-windows`
# (see .cargo/config.toml), which runs rustc against a non-Linux target.
#
# Coverage and limits:
#   - Detects: ungated `use` of an item whose every declaration is behind a
#     platform cfg with no complementary `not(...)` declaration.
#   - Does NOT detect: type errors inside gated blocks, unused bindings that
#     only appear off-platform, or macro-generated items. Use the real
#     cross-target build for those.
#
# Usage:
#   scripts/portability-lint.sh [SRC_DIR]     # SRC_DIR defaults to ./src
#
# Exit codes:
#   0 — no ungated import of a platform-only item
#   1 — at least one violation found (details on stdout)
#   2 — usage error or missing dependency

set -uo pipefail

SRC_DIR="${1:-src}"

if ! command -v rg >/dev/null 2>&1; then
    echo "portability-lint: ripgrep (rg) is required" >&2
    exit 2
fi
if [ ! -d "$SRC_DIR" ]; then
    echo "portability-lint: '$SRC_DIR' is not a directory" >&2
    exit 2
fi

# Platform cfg predicates that make an item conditional on the target OS.
PLATFORM_CFG='target_os\s*=|target_family\s*=|^\s*unix\s*$|^\s*windows\s*$'

ITEM_KW='fn|struct|enum|union|trait|type|const|static|mod'

# ---------------------------------------------------------------------------
# Pass 1 — collect every declaration that sits directly under a platform cfg.
# Emits "cfg<TAB>symbol" lines.
# ---------------------------------------------------------------------------
gated_decls="$(
    rg -U --no-filename --no-line-number -o \
       "#\\[cfg\\(([^\\n]*)\\)\\]\\n(?:\\s*#\\[[^\\n]*\\]\\n|\\s*//[^\\n]*\\n)*\\s*(?:pub(?:\\([^)]*\\))?\\s+)?(?:unsafe\\s+)?(?:async\\s+)?(?:${ITEM_KW})\\s+([A-Za-z_][A-Za-z0-9_]*)" \
       --replace '$1'$'\t''$2' \
       -g '*.rs' "$SRC_DIR" 2>/dev/null \
    | rg "$PLATFORM_CFG" \
    | sort -u
)"

if [ -z "$gated_decls" ]; then
    echo "portability-lint: no platform-gated declarations found under '$SRC_DIR'"
    exit 0
fi

# ---------------------------------------------------------------------------
# Pass 2 — a symbol is "platform-only" when it has at least one gated
# declaration, no declaration under a complementary not(...) cfg, and no
# ungated declaration anywhere in the tree.
# ---------------------------------------------------------------------------
platform_only=""
while IFS= read -r sym; do
    [ -n "$sym" ] || continue

    # A complementary `not(...)` declaration keeps the symbol resolvable.
    if printf '%s\n' "$gated_decls" | rg -q "^[^\t]*\bnot\s*\(.*\t${sym}$"; then
        continue
    fi

    gated_count="$(printf '%s\n' "$gated_decls" | rg -c "\t${sym}$" || true)"
    [ -n "$gated_count" ] || gated_count=0

    total_count="$(
        rg --no-filename --no-line-number -o \
           "\b(?:${ITEM_KW})\s+${sym}\b" -g '*.rs' "$SRC_DIR" 2>/dev/null \
        | rg -c '' || true
    )"
    [ -n "$total_count" ] || total_count=0

    # Every declaration is gated -> the symbol vanishes off-platform.
    if [ "$total_count" -le "$gated_count" ]; then
        platform_only="${platform_only}${sym}"$'\n'
    fi
done < <(printf '%s\n' "$gated_decls" | choose -f '\t' 1 | sort -u)

platform_only="$(printf '%s' "$platform_only" | rg -v '^$' || true)"

if [ -z "$platform_only" ]; then
    echo "portability-lint: no platform-only symbols to guard"
    exit 0
fi

# ---------------------------------------------------------------------------
# Pass 3 — flag `use` statements that import a platform-only symbol without
# being gated themselves.
# ---------------------------------------------------------------------------
violations=0

# Modules whose own `mod` declaration is platform-gated: everything inside them
# inherits that gate, so an ungated `use` in such a file is correct Rust.
gated_mods="$(
    rg -U --no-filename --no-line-number -o \
       "#\\[cfg\\(([^\\n]*)\\)\\]\\n(?:\\s*#\\[[^\\n]*\\]\\n|\\s*//[^\\n]*\\n)*\\s*(?:pub(?:\\([^)]*\\))?\\s+)?mod\\s+([A-Za-z_][A-Za-z0-9_]*)\\s*;" \
       --replace '$1'$'\t''$2' \
       -g '*.rs' "$SRC_DIR" 2>/dev/null \
    | rg "$PLATFORM_CFG" \
    | choose -f '\t' 1 | sort -u
)"

# True when the file is the body of a platform-gated module.
file_is_gated_module() {
    local path="$1" base modname
    base="${path##*/}"
    if [ "$base" = "mod.rs" ]; then
        modname="${path%/*}"
        modname="${modname##*/}"
    else
        modname="${base%.rs}"
    fi
    [ -n "$gated_mods" ] || return 1
    printf '%s\n' "$gated_mods" | rg -q "^${modname}$"
}

while IFS= read -r file; do
    [ -n "$file" ] || continue
    if file_is_gated_module "$file"; then
        continue
    fi

    prev_is_cfg=0
    in_use=0
    use_start=0
    use_buf=""
    use_gated=0
    lineno=0

    while IFS= read -r line || [ -n "$line" ]; do
        lineno=$((lineno + 1))

        if [ "$in_use" -eq 0 ]; then
            case "$line" in
                *'use '*)
                    # Skip `use` that appears inside a comment or a string.
                    case "${line#"${line%%[![:space:]]*}"}" in
                        'use '*|'pub use '*|'pub(crate) use '*)
                            in_use=1
                            use_start=$lineno
                            use_buf="$line"
                            use_gated=$prev_is_cfg
                            ;;
                        *) ;;
                    esac
                    ;;
                *) ;;
            esac
        else
            use_buf="${use_buf} ${line}"
        fi

        if [ "$in_use" -eq 1 ]; then
            case "$use_buf" in
                *';'*)
                    if [ "$use_gated" -eq 0 ]; then
                        while IFS= read -r sym; do
                            [ -n "$sym" ] || continue
                            if printf '%s' "$use_buf" | rg -q "\b${sym}\b"; then
                                echo "VIOLATION ${file}:${use_start} — ungated \`use\` imports platform-only item \`${sym}\`"
                                echo "    gate this \`use\` with the same #[cfg(...)] as the declaration, or give the item a complementary not(...) stub"
                                violations=$((violations + 1))
                            fi
                        done < <(printf '%s\n' "$platform_only")
                    fi
                    in_use=0
                    use_buf=""
                    ;;
                *) ;;
            esac
        fi

        # Track whether the *next* line is preceded by a cfg attribute.
        case "${line#"${line%%[![:space:]]*}"}" in
            '#[cfg('*) prev_is_cfg=1 ;;
            ''|'//'*|'#['*) ;;                 # blank / comment / other attr: keep state
            *) prev_is_cfg=0 ;;
        esac
    done < "$file"
done < <(rg --files -g '*.rs' "$SRC_DIR" 2>/dev/null)

if [ "$violations" -gt 0 ]; then
    echo ""
    echo "portability-lint: ${violations} violation(s) — this breaks the build on other platforms"
    exit 1
fi

echo "portability-lint: OK ($(printf '%s\n' "$platform_only" | rg -c '' || echo 0) platform-only symbols guarded)"
exit 0
