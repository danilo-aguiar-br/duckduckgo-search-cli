# ADR-0015 — macOS/Windows Headless New (v0.9.3)


## Contexto
- ADR-0013 introduziu headed nativo Quartz/DWM no macOS/Windows em v0.9.1
- Compositores Quartz (macOS) e DWM (Windows) clampam `--window-position` aos bounds da tela
- A janela Chrome movida off-screen aparecia VISÍVEL ao usuário a cada busca
- A flag `--window-position=-32000,-32000` em `flags_stealth` é ignorada por esses compositores
- O fluxo de trabalho do usuário era interrompido por janelas piscando na tela


## Decisão
- `decide_head_mode()` retorna `ChromeHeadMode::Headless` no branch `has_native_display` para macOS e Windows
- `Headless` ativa `builder.new_headless_mode()` (Chrome `--headless=new` moderno)
- headless=new NÃO abre janela GUI por definição
- headless=new é viável no DDG porque as fixes v0.9.2 eliminaram vazamentos detectáveis
- Linux MANTÉM `HeadedXvfb` (Xvfb privado) sem mudança
- `DUCKDUCKGO_CHROME_VISIBLE=1` permanece como escape hatch forçando `HeadedNative`


## Consequências
- A CLI é silenciosa no macOS e Windows (sem janela visível)
- Detecção automática de SO via `cfg!(target_os = ...)` sem intervenção do usuário
- Linux permanece inalterado com Xvfb privado
- Modo de operação distinto por plataforma conforme decisão do usuário
- Mantém evasão anti-bot validada empiricamente (4/4 queries exit 0 no DDG)


## Alternativas Consideradas
- `.hide()` do chromiumoxide — rejeitado: implementação instável cross-plataforma
- XQuartz no macOS — rejeitado: dependência pesada e fora do escopo
- Manter headed nativo — rejeitado: janela visível prejudica o fluxo do usuário


## Relacionado
- ADR-0009 (headed-inside-Xvfb, v0.8.7) — parcialmente supersedido no macOS/Windows
- ADR-0013 (headed nativo macOS/Windows, v0.9.1) — supersedido no macOS/Windows
- ADR-0014 (vazamento de automação, v0.9.2) — habilitou headless=new viável
- gaps.md (GAP-WS-112)
- CHANGELOG.md [0.9.3]
