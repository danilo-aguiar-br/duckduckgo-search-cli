# Inventário de gaps — duckduckgo-search-cli

## Metadados do inventário

| Campo | Valor |
|-------|--------|
| Projeto | `duckduckgo-search-cli` |
| Versão da CLI | **0.9.8** |
| Data | 2026-07-14 |
| Idioma | Português do Brasil com acentuação |
| Caminho canônico no git | `docs/gaps.md` |

---

## Histórico — gaps resolvidos

### GAP-WS-LIFECYCLE-001 — One-shot Chromium/Xvfb

| Campo | Valor |
|-------|--------|
| Status | **RESOLVIDO** em **v0.9.6** (patch Windows **v0.9.7**) |
| ADR | `docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md` |

### GAP-WS-113 — Chrome-only universal

| Campo | Valor |
|-------|--------|
| Status | **RESOLVIDO** em **v0.9.4** |
| ADR | `docs/decisions/0016-chrome-only-universal-v0-9-4.md` |

### GAP-WS-AGENT-READY-001 — CLI agent-ready completa

| Campo | Valor |
|-------|--------|
| Status | **RESOLVIDO** em **v0.9.8** |
| ADR | `docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md` |
| Severidade original | Alta |

#### Problema (histórico)

Chrome multi-canal incompleto (Flatpak shell rejeitado); dual web+news não era default da search; texto limpo opt-in e sem news; flags de transporte não globais; metadados `usou_chrome` mentirosos em news-only; fan-out e deep sem `chrome_path_resolvido`/`chrome_canal`.

#### Causa raiz

Contrato agent-first incompleto (C1–C15): detecção multi-canal, defaults assimétricos, fetch opt-in, ergonomia/metadados, inventário fora do git.

#### Correção v0.9.8 (L-01…L-08 + residuais R-01…R-12)

| ID | Tema | Status |
|----|------|--------|
| L-01 | Flatpak resolve export→ELF | **RESOLVIDO** |
| L-02 | Ordem multi-canal | **RESOLVIDO** |
| L-03 | Dual default `all` | **RESOLVIDO** |
| L-04 | News multi-seletor + metadata honesta | **RESOLVIDO** (incl. multi-query + deep) |
| L-05 | Texto limpo agent-default + news body | **RESOLVIDO** |
| L-06 | Flags transporte `global = true` | **RESOLVIDO** |
| L-07 | UA fan-out + one-shot | **RESOLVIDO** |
| L-08 | Docs/schemas/skill/ADR/CHANGELOG/inventário no git | **RESOLVIDO** |
| R-01 | path/canal no fan-out `parallel.rs` | **RESOLVIDO** |
| R-02 | path/canal/`usou_chrome` no deep-research | **RESOLVIDO** |
| R-03 | metadata chrome em envelopes de falha | **RESOLVIDO** |
| R-04 | clippy `manual_clamp` | **RESOLVIDO** |
| R-12 | `surface_invalid_messages` no launch | **RESOLVIDO** |

#### Mandatos

- Produção: **somente chromiumoxide/CDP** (sem telemetria remota).
- Cada invocação: **one-shot** (NASCE, EXECUTA, MORRE).
- Disco: **atomwrite** (`paths::atomic_write`).
- Metadados agent (`chrome_path_resolvido`, `chrome_canal`) **não** são telemetria.

#### Residuais legítimos (não gaps de produto abertos)

- Anti-bot de rede/IP pode zerar news em alguns hosts (degradação honesta se web>0).
- SIGKILL externo da CLI (limite do SO).
- Preferência crate readability externa e flag `--agent` única: **decisões fechadas no ADR-0018** (defaults já agent-ready; pipeline interno mantido).

#### Oportunidades fechadas em 0.9.8

1. Metadados chrome em search single, multi, falha e deep.
2. `surface_invalid_messages`.
3. Inventário versionado em `docs/gaps.md`.
4. Skill EN/PT com paths Fedora/Flatpak e defaults 0.9.8.
5. E2E opcional `DUCKDUCKGO_FLATPAK_E2E=1`.

---

## Política deste arquivo

- Status só muda para RESOLVIDO com versão de release e ADR.
- Português do Brasil com acentuação; sem afirmar correção inexistente.
- Sem telemetria no produto.
