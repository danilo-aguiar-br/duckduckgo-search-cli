# ADR-0014 — Eliminar Vazamento de Automação do Chrome (v0.9.2)


## Contexto
- v0.9.1 não resolveu o bloqueio anti-bot persistente na auditoria rules-rust
- O usuário validou empiricamente que o IP NÃO está bloqueado
- Um browser real acessa duckduckgo.com normalmente na mesma máquina
- O bloqueio era específico do Chrome automatizado da CLI
- O banner "gerenciado por testes automatizados" aparecia na barra do Chrome
- O chromiumoxide 0.9.1 injeta `--enable-automation` em DEFAULT_ARGS (config.rs:481)
- A flag `--user-agent=` do pool não atualiza Client Hints (userAgentData.brands, sec-ch-ua)
- WebRTC vazava o IP real fora do proxy via ICE candidate gathering
- QUIC mantinha UDP ativo, escapando do controle de proxy TCP


## Decisão
- GAP-WS-108: `.disable_default_args()` no launch() remove `enable-automation` do chromiumoxide
- 23 defaults seguros do chromiumoxide são re-adicionados via `CHROMIUMOXIDE_SAFE_DEFAULTS`
- GAP-WS-109: versão do UA é alinhada ao Chrome real instalado via `detect_chrome_major_version()`
- `Emulation.setUserAgentOverride` aplica `UserAgentMetadata` coerente (brands, platform, mobile)
- GAP-WS-110: `--force-webrtc-ip-handling-policy=disable_non_proxied_udp` em flags_stealth
- `--disable-webrtc-hw-decoding` complementa o bloqueio de leak WebRTC
- GAP-WS-111: `--disable-quic` força HTTP/2 sobre TCP, alinhando-se ao proxy


## Consequências
- O banner "gerenciado por testes automatizados" é removido
- UA, Client Hints e fingerprint TLS ficam coerentes com a versão real do Chrome
- WebRTC não vaza o IP real do host
- Tráfego segue TCP/HTTP/2 consistente com o proxy configurado
- A superfície de detecção anti-bot é reduzida em quatro vetores independentes
- Linux mantém Xvfb privado; macOS/Windows mantinham headed nativo em v0.9.2 (revertido para headless=new em v0.9.3 — ver ADR-0015)


## Alternativas Consideradas
- Parar de fazer override do UA — rejeitado: perde o design do pool de 12 identidades
- Não usar `disable_default_args` — rejeitado: `enable-automation` persiste no banner
- Override só de `navigator.userAgent` — rejeitado: userAgentData.brands fica divergente
- Desabilitar WebRTC via CDP `WebRTC.disable` — rejeitado: flag de launch é mais estável


## Relacionado
- ADR-0009 (headed-inside-Xvfb, v0.8.7)
- ADR-0013 (headed nativo macOS/Windows, v0.9.1)
- gaps.md (GAP-WS-108, GAP-WS-109, GAP-WS-110, GAP-WS-111)
- CHANGELOG.md [0.9.2]
- Regras Rust para Chromiumoxide (auditoria do usuário)
