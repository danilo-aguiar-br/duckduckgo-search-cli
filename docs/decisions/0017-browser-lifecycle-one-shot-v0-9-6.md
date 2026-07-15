# ADR-0017 — One-shot lifecycle Chromium/Xvfb (v0.9.6)


## Contexto
- Auditoria 2026-07-12 (Fedora 44, ~62 GiB RAM, uptime multi-dia) encontrou centenas de Chromium de automação, dezenas de Xvfb e milhares de `/tmp/.tmp*` residuais
- Causa raiz: a CLI não era one-shot de verdade no eixo de processos externos — `kill_on_drop` do chromiumoxide só mata o Child raiz; Xvfb e netos sobreviviam
- Rules GraphRAG: `rules-rust-cli-one-shot`, `rules-rust-processos-externos`, graceful shutdown, atomwrite
- chromiumoxide 0.9.1 não expõe `pre_exec` no spawn do Chrome


## Decisão
- Introduzir `src/process_lifecycle.rs` com kill de process group, árvore e marker por `user-data-dir` único
- Xvfb sobe com `setpgid(0,0)` + `PR_SET_PDEATHSIG(SIGKILL)` (Linux) e `XvfbGuard` RAII
- `ChromeBrowser::shutdown` com deadline cooperativo + force reap; `Drop` sempre força reap se necessário
- `content_fetch` finaliza com `shutdown` assíncrono após drenar tasks
- SIGTERM cancela o `CancellationToken` (além de SIGINT)
- Escrita atômica em output, config e cookie jar
- Sem telemetria remota


## Consequências
- Cada invocação deve deixar zero Chromium/Xvfb/perfil desta sessão
- Sessões longas de agentes deixam de acumular enxame de memória
- SIGKILL externo da CLI permanece limite do SO (PDEATHSIG cobre Xvfb; marker não limpa órfãos de runs mortos por SIGKILL sem group kill)
- Versão de release: **0.9.6** (não 0.9.3 — já alocada ao ADR-0015)


## Alternativas Consideradas
- Spawn próprio do Chrome + `Browser::connect` com `pre_exec` no browser — rejeitado nesta versão (refactor grande); marker+tree cobre
- Flag `--single-process` do Chromium — rejeitado (quebra stealth e estabilidade)
- Limpeza manual do host do usuário — fora do contrato da CLI


## Relacionado
- `gaps.md` GAP-WS-LIFECYCLE-001
- ADR-0009 (HeadedXvfb)
- GAP-WS-089 (stale lock Xvfb) — complementar no kill intencional
- CHANGELOG `[0.9.6]`
- Residual de perfil/prefixo em disco: **ADR-0020** / GAP-WS-TMP-PROFILE-ORPHAN-001 (v1.0.0)
