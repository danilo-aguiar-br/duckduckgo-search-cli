# Prompt rules (≤400 palavras) — PT-BR, imperativo forte


## REGRAS AGENT-NATIVE — duckduckgo-search-cli (OBRIGATÓRIO)
- VOCÊ NÃO TEM ESCOLHA
- CUMPRA
- VIOLAR É FALHA
- A CLI FAZ O PESADO
- A LLM só consome JSON limpo, mínimo e acionável
- Meta: mínimo de tokens
- STDOUT É SAGRADO
- Em stdout: só payload parseável, JSON ou NDJSON
- PROÍBO log, banner, spinner, cor, help, stack e HTML cru em stdout
- Logs vão para stderr
- NUNCA misture `2>&1`
- EXIJA `-q` e `--no-input` quando aplicável
- A raiz do binário NÃO tem `--yes`, e esse token vira QUERY POSICIONAL ou sai com exit 2
- As flags de efeito colateral são `--dry-run` e `--force`, e elas existem SOMENTE em `init-config`
- SE o agente precisar de sed, jq ou parser HTML no prompt, VOCÊ FALHOU
- SE o prompt precisar dizer "ignore o banner", VOCÊ FALHOU


## REGRAS ANTI-CLOUDFLARE (OBRIGATÓRIO)
- TRANSPORTE: USE só Chrome/Chromium real via CDP
- PROÍBO SERP com reqwest, rustls ou wreq
- HTTP residual só em harness de teste
- LINUX: Chrome headed em Xvfb PRIVADO
- PROÍBO headless puro como padrão
- Flatpak Chrome/Chromium é válido se detectado
- Sem display estável: PARE e remedeie
- STEALTH: mitigue SÓ automação, como webdriver, plugins, window.chrome, outer size, permissions e WS DevTools
- PROÍBO spoof sintético de canvas, WebGL, Audio, GPU, hardwareConcurrency, deviceMemory ou fingerprint fixo (ADR-0022)
- Mute de alto-falante (`--mute-audio`, ADR-0026) é padrão operacional, e NÃO é spoof
- IDENTIDADE: UA Chrome coerente com binário e plataforma
- PROÍBO UA Safari ou Firefox com TLS Chrome
- XDG: persista em `~/.config/duckduckgo-search-cli/`, ou use `--config-home` / `--cookies-path`
- REUSE cookies
- NÃO invente home via env de produto
- SESSÃO: warm-up em duckduckgo.com ANTES da SERP
- PROÍBO `--no-warmup` em operação real
- PROXY: só `--proxy` ou `config set proxy_url`
- PROÍBO herdar HTTP_PROXY e HTTPS_PROXY
- GATES: `doctor` e `--probe-deep`
- Com captcha ou interstitial: PARE o fan-out, remedeie, e SÓ então retente
- Zero hits NÃO é índice vazio
- I/O: JSON no stdout e logs no stderr
- NÃO "melhore" stealth com fingerprint sintético
- Host real, Chrome nativo, XDG e warm-up SÃO a defesa
- OBEDEÇA
- ÁUDIO: a CLI SEMPRE lança Chrome com `--mute-audio` e autoplay policy (ADR-0026)
- PROÍBO reintroduzir som de página no host
- Mute NÃO é spoof de AudioContext (ADR-0022)
