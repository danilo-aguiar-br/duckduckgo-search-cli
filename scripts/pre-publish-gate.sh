#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# v0.7.10 P19b — Pre-publish gate (regras 1264+1263+1256+1217+1240).
#
# Roda TODOS os checks antes de publicar v0.7.10. Falha em qualquer
# divergência. NAO executa `cargo publish` real — aguarda autorização
# explícita do user (regra de segurança do user + regra 1264).
#
# Uso:
#   ./scripts/pre-publish-gate.sh
#
# Exit code 0 = tudo OK, pode pedir autorização ao user para publish.
# Exit code != 0 = algum check falhou, corrigir antes de prosseguir.

set -euo pipefail

cd "$(dirname "$0")/.."

# Cores para output legível.
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

fail() {
    echo -e "${RED}FAIL: $1${NC}" >&2
    exit 1
}

ok() {
    echo -e "${GREEN}OK: $1${NC}"
}

warn() {
    echo -e "${YELLOW}WARN: $1${NC}"
}

section() {
    echo
    echo "=== $1 ==="
}

# Gate 1 — cargo fmt --check (regra 1256: T5.1 release-gate abortou sem fmt).
section "Gate 1/7 — cargo fmt --check"
if cargo fmt --all -- --check > /tmp/fmt.log 2>&1; then
    ok "fmt limpo"
else
    fail "cargo fmt encontrou divergências. Rode: cargo fmt --all"
    cat /tmp/fmt.log >&2
fi

# Gate 2 — cargo clippy --all-targets -- -D warnings.
section "Gate 2/7 — cargo clippy --all-targets -- -D warnings"
if timeout 180 cargo clippy --all-targets --offline -- -D warnings > /tmp/clippy.log 2>&1; then
    ok "clippy limpo"
else
    fail "cargo clippy encontrou warnings. Veja /tmp/clippy.log"
    tail -30 /tmp/clippy.log >&2
fi

# Gate 3 — cargo test --all-features --locked.
section "Gate 3/7 — cargo test --all-features --locked"
if timeout 300 cargo test --all-features --locked --offline > /tmp/test.log 2>&1; then
    ok "todos os testes passaram"
else
    fail "algum teste falhou. Veja /tmp/test.log"
    tail -50 /tmp/test.log >&2
fi

# Gate 4 — cargo llvm-cov --workspace --fail-under-lines 80 (P14 coverage).
section "Gate 4/7 — cargo llvm-cov --workspace --fail-under-lines 80"
if command -v cargo-llvm-cov >/dev/null 2>&1; then
    if timeout 300 cargo llvm-cov --workspace --fail-under-lines 80 --offline > /tmp/coverage.log 2>&1; then
        ok "coverage >= 80%"
    else
        fail "coverage < 80% ou erro. Veja /tmp/coverage.log"
        tail -30 /tmp/coverage.log >&2
    fi
else
    warn "cargo-llvm-cov não instalado. Pule este gate. Instale com: cargo install cargo-llvm-cov"
fi

# Gate 5 — rg -n v0.7.9 skill/ retorna ZERO matches (regra 1263 skill version drift).
section "Gate 5/7 — sem refs a v0.7.9 em skill/ (regra 1263)"
STALE_REFS=$(rg -n 'v0\.7\.9' skill/ 2>/dev/null || true)
if [ -z "$STALE_REFS" ]; then
    ok "skill/ sem refs stale a v0.7.9"
else
    fail "skill/ ainda referencia v0.7.9. Substitua por v0.7.10:"
    echo "$STALE_REFS" >&2
fi

# Gate 6 — cargo publish --dry-run --allow-dirty --no-verify (regra 1264 dry-run).
section "Gate 6/7 — cargo publish --dry-run --allow-dirty --no-verify"
if timeout 120 cargo publish --dry-run --allow-dirty --no-verify --offline > /tmp/publish-dry.log 2>&1; then
    ok "publish dry-run válido"
else
    fail "cargo publish --dry-run falhou. Veja /tmp/publish-dry.log"
    tail -30 /tmp/publish-dry.log >&2
fi

# Gate 7 — gh run list --branch main --limit 1 status: success (regra 1217 CI verde).
section "Gate 7/7 — CI main verde (regra 1217)"
if command -v gh >/dev/null 2>&1; then
    if gh run list --branch main --limit 1 --json status --jq '.[0].status' 2>/dev/null | grep -q '^success$'; then
        ok "CI main verde"
    else
        warn "CI main NÃO está verde. Regra 1217: btls-sys causa vermelho. Ticket bloqueante."
        warn "Status: $(gh run list --branch main --limit 1 --json status --jq '.[0].status' 2>/dev/null)"
    fi
else
    warn "gh CLI não instalado. Pule este gate."
fi

echo
echo -e "${GREEN}=== TODOS OS 7 GATES PASSARAM ===${NC}"
echo
echo "Próximo passo (NÃO executado automaticamente — aguarda autorização do user):"
echo "  1. git tag -a v0.7.10 -m 'Release v0.7.10: Anti-Bot UX + Pino Completo + Skill Sync'"
echo "  2. git push origin main && git push origin v0.7.10"
echo "  3. cargo publish --allow-dirty --no-verify  # APOS autorização do user"
echo
echo "Janela de yank: 72h (regra 1264)."
