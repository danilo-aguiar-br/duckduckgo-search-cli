# ADR-0020 — Prefixo `ddg-chrome-` e one-shot de disco (v1.0.0)

## Contexto

- Residual de **GAP-WS-LIFECYCLE-001** / **ADR-0017**: processo Chromium/Xvfb era reaped, mas o **user-data-dir** usava `tempfile::tempdir()` com prefixo padrão **`.tmp`**, e `force_reap` / `reap_all_registered` **não** removiam o diretório.
- docs.rs `tempfile::TempDir` (Resource Leaking): se o processo sai sem destructor (`process::exit`, sinal sem unwind), o diretório **não** é apagado.
- Evidência host Fedora 2026-07-15: órfãos `/tmp/.tmp*` + stubs `org.chromium.Chromium.*`.
- `deep-research` criava `CancellationToken::new()` isolado — SIGTERM do `main` não cancelava o fan-out.
- GraphRAG: `rules-rust-cli-one-shot` (Tempfiles DEVEM ser removidos), `rules-rust-processos-externos`, família `rules-rust-shutdown-*`.
- Documentação consultada via MCP **docs-rs** (equivalente a context7 neste ambiente) e pesquisa via `duckduckgo-search-cli`.

## Decisão

1. Prefixo auditável **`ddg-chrome-`** (`USER_DATA_DIR_PREFIX`) via `tempfile::Builder` no launch; permissões `0o700` em Unix.
2. `force_reap` remove o `user_data_dir` (kill → settle → `remove_dir_all` + 1 retry).
3. `ExitReapGuard` no `main` + panic hook + reap síncrono em timeout global / fim de pipeline / deep-research.
4. `sweep_orphan_profiles()` na primeira instalação do panic hook — só `ddg-chrome-*` sem processo vivo (nunca `.tmp*` genérico).
5. Propagar o `CancellationToken` do `main` para deep-research + fence `tokio::time::timeout` com reap.
6. `Config::default().global_timeout_seconds` alinhado a `DEFAULT_GLOBAL_TIMEOUT` (180).
7. Sem telemetria remota; atomwrite de saída de usuário inalterado (`paths::atomic_write`).
8. **Não** usar `libc::atexit` (async-signal-safety); rede de segurança = RAII + reap explícito.

## Consequências

- One-shot honesto em **processo e disco** após exit cooperativo.
- Operadores filtram residual com `find … -name 'ddg-chrome-*'`.
- SIGKILL/OOM ainda podem deixar residual; a próxima invocação tenta limpar só o prefixo desta CLI.
- **Política dura (código):** (1) nunca auto-rm `.tmp*`; (2) nunca auto-rm `org.chromium.Chromium.*`; (3) residual SIGKILL/OOM → sweep na próxima run **somente** `ddg-chrome-*` via `ExitReapGuard::new` + panic-hook Once.
- Guards: `is_cli_owned_profile_name`, `is_forbidden_bulk_delete_name`, `remove_user_data_dir` recusa path estrangeiro.
- Versão de release: **1.0.0**.

## Alternativas consideradas

- `tempdir_in(XDG_RUNTIME_DIR)` — adiado (muda semântica de TMP); nota futura sem gap aberto.
- Apagar `/tmp/.tmp*` em massa — rejeitado (colide com outras apps Rust).
- `libc::atexit` — rejeitado; preferir `ExitReapGuard`.

## Relacionado

- `gaps.md` **GAP-WS-TMP-PROFILE-ORPHAN-001** (RESOLVIDO v1.0.0)
- ADR-0017 (one-shot processo)
- ADR-0019 (e2e v0.9.9)
- CHANGELOG `[1.0.0]`
