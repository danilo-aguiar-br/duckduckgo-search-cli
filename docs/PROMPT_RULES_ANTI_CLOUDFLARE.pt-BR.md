# Prompt rules (≤300 palavras) — PT-BR, imperativo forte

## REGRAS AGENT-NATIVE — duckduckgo-search-cli (OBRIGATÓRIO)

VOCÊ NÃO TEM ESCOLHA. CUMPRA. VIOLAR = FALHA.

1. A CLI FAZ O PESADO. A LLM só consome JSON limpo, mínimo e acionável. Meta: mínimo de tokens.
2. STDOUT É SAGRADO. Em stdout: só payload parseável (JSON/NDJSON). PROÍBO logs, banner, spinner, cor, help, stack, HTML cru. Logs → stderr. NUNCA misture `2>&1`.
3. EXIJA `-q`, `--no-input` quando aplicável. Side effects: `--yes` / `--dry-run` / `--force`.
4. SE o agente precisar de sed/jq/parser HTML no prompt, ou “ignore o banner”, VOCÊ FALHOU.

## REGRAS ANTI-CLOUDFLARE (OBRIGATÓRIO)

1. TRANSPORTE: USE só Chrome/Chromium real via CDP. PROÍBO SERP com reqwest/rustls/wreq. HTTP residual só em harness de teste.
2. LINUX: Chrome headed em Xvfb PRIVADO. PROÍBO headless puro como padrão. Flatpak Chrome/Chromium é válido se detectado. Sem display estável: PARE e remedeie.
3. STEALTH: mitigue SÓ automação (webdriver, plugins, window.chrome, outer size, permissions, WS DevTools). PROÍBO spoof sintético de canvas/WebGL/Audio/GPU/hardwareConcurrency/deviceMemory ou fingerprint fixo (ADR-0022). Mute de alto-falante (`--mute-audio`, ADR-0026) é padrão operacional — não é spoof.
4. IDENTIDADE: UA Chrome coerente com binário e plataforma. PROÍBO UA Safari/Firefox com TLS Chrome.
5. XDG: persista em `~/.config/duckduckgo-search-cli/` (ou `--config-home` / `--cookies-path`). REUSE cookies. NÃO invente home via env de produto.
6. SESSÃO: warm-up em duckduckgo.com ANTES da SERP. PROÍBO `--no-warmup` em operação real.
7. PROXY: só `--proxy` / `config set proxy_url`. PROÍBO herdar HTTP_PROXY/HTTPS_PROXY.
8. GATES: `doctor` e `--probe-deep`. Se captcha/interstitial: PARE fan-out, remedeie, SÓ então retente. Zero hits ≠ índice vazio.
9. I/O: JSON stdout; logs stderr.
10. NÃO “melhore” stealth com fingerprint sintético. Host real + Chrome nativo + XDG + warm-up É a defesa. OBEDEÇA.
11. ÁUDIO: a CLI SEMPRE lança Chrome com `--mute-audio` + autoplay policy (ADR-0026). PROÍBO reintroduzir som de página no host. Mute ≠ spoof de AudioContext (ADR-0022).
