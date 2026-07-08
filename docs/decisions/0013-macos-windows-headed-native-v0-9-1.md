# ADR-0013 — macOS/Windows Headed Nativo (v0.9.1)


## Contexto
- A CLI v0.9.0 era inutilizável no macOS, retornando exit 6 com causa_zero anti-bot
- ADR-0009 presumia Linux na cascata Xvfb, sem tratar macOS e Windows
- A cascata de launch() em src/browser.rs exigia display virtual Xvfb, que só existe no Linux
- Em macOS/Windows sem Xvfb, a CLI caía para headless, detectável pelo Cloudflare
- GAP-WS-107b: o filtro de UA em src/pipeline.rs deixava passar UA Chrome de plataforma errada
- Evidência: query `rust language` no macOS retornava exit 6, 0 resultados, stderr anti-bot


## Decisão
- macOS e Windows rodam Chrome HEADED no display nativo Quartz (macOS) e DWM (Windows)
- Xvfb permanece Linux-only como transporte privado de display virtual
- A janela Chrome é movida off-screen via `--window-position=-32000,-32000 --window-size=1920,1080`
- Essas flags já existiam em `flags_stealth` e são reutilizadas sem adicionar novas dependências
- UA Chrome é sempre coerente com o SO do host via `identity::ua_platform_matches_host()` (GAP-WS-107b)
- Quando o pool seleciona plataforma errada, o filtro força `chrome_only_ua_for_platform()`
- SUPERSEDED no macOS/Windows em v0.9.3 (ver ADR-0015) — headed nativo causava janela visível; revertido para headless=new


## Consequências
- A CLI é funcional em macOS e Windows; v0.9.3 reverteu macOS/Windows para headless=new (ADR-0015) para eliminar janela visível
- Pinagens cross-plataforma (ex.: `chrome-linux` em host macOS) são corrigidas para a plataforma do host
- O fingerprint TLS do Chrome bate com o UA apresentado, eliminando detecção anti-bot
- Linux mantém Xvfb privado sem mudança de comportamento
- `has_native_display()` e `spawn_virtual_display()` só atuam em Linux via cfg-gating


## Alternativas Consideradas
- `.hide()` do chromiumoxide — fallback se a janela visível se tornar problema
- `--headless=new` puro — detectável pelo Cloudflare, rejeitado
- XQuartz no macOS — dependência pesada e fora do escopo, rejeitado
- Mover Xvfb para cross-plataforma — complexidade desnecessária, rejeitado


## Relacionado
- ADR-0009 (headed-inside-Xvfb, v0.8.7) — anterior que presumia Linux
- ADR-0007 (Chrome primary transport) — base do transporte Chrome
- gaps.md (GAP-WS-107 e GAP-WS-107b)
- CHANGELOG.md [0.9.1]
