# Instalando duckduckgo-search-cli no Windows (v0.7.5+)

Este guia cobre os quatro pré-requisitos que `cargo install duckduckgo-search-cli` exige no **Windows MSVC nativo** desde a v0.7.3 (BoringSSL via `wreq 6.0.0-rc.29`):

1. **Assembler NASM** (GAP-WS-28, preflight adicionado na v0.7.4)
2. **CMake 3.20+** (GAP-WS-29, preflight adicionado na v0.7.5)
3. **Compilador + linker MSVC** (GAP-WS-30, preflight adicionado na v0.7.5)
4. **Interpretador Perl** (GAP-WS-31, preflight adicionado na v0.7.5)

`cargo install` sempre compila do código-fonte. O crates.io NÃO distribui **NENHUM** binário pré-compilado. Não existe `apt install duckduckgo-search-cli` para Windows. Todo usuário Windows precisa satisfazer esses quatro pré-requisitos.

> **TL;DR — caminho mais rápido:** execute `scripts/install-windows.ps1` a partir de um Developer PowerShell for VS 2022. Ele auto-instala o que pode ser auto-instalado (NASM, CMake, Perl) e imprime instruções acionáveis para o que não pode (MSVC).

---

## Pré-requisitos

- Windows 10 versão 1903 ou superior, ou Windows 11
- PowerShell 5.1+ (Windows PowerShell) ou PowerShell 7+ (recomendado)
- Shell de administrador (clique direito → "Executar como administrador") para instalações que tocam `Program Files`
- 5 GB de espaço em disco livre (Visual Studio Build Tools é grande; o resto é pequeno)

---

## Método A — Visual Studio Installer + ferramentas standalone (recomendado)

Melhor para usuários que já têm o Visual Studio Installer ou querem uma fonte oficial limpa para cada ferramenta.

### Passo 1 — Instalar Visual Studio Build Tools com os sub-componentes corretos

1. Baixe Visual Studio Build Tools 2022 de <https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022> (SKU Community ou Build Tools ambos funcionam).
2. Execute o instalador, clique em **Modify** em uma instalação existente (ou instale do zero).
3. Na aba **Workloads**, marque **Desktop development with C++**.
4. No painel direito **Installation details**, **expanda** esse workload e marque **C++ CMake tools for Windows**. **Este sub-componente vem desmarcado por padrão** — sem ele, o build do `cmake` falhará com `failed to execute command: program not found` minutos adentro.
5. Clique em **Modify** para instalar. Isso adiciona `cl.exe`, `link.exe`, `cmake.exe`, o Windows SDK e as bibliotecas padrão MSVC.

### Passo 2 — Abrir o shell correto

Após a instalação, as ferramentas MSVC (cl.exe, link.exe) NÃO estão no PATH global. Você DEVE executar seu build a partir de um destes shells:

- **Menu Iniciar** → **Developer Command Prompt for VS 2022** (baseado em cmd)
- **Menu Iniciar** → **Developer PowerShell for VS 2022** (baseado em PowerShell)
- Ou, a partir de um PowerShell regular, faça source do script de env:
  ```powershell
  & "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1"
  ```
  (Ajuste `Community` para `BuildTools` / `Professional` / `Enterprise` conforme sua SKU.)

O shell agora tem `PATH`, `INCLUDE` e `LIB` configurados para MSVC. Sem isso, `cargo build` falha com `no compiler found`.

### Passo 3 — Instalar NASM (GAP-WS-28)

No seu PowerShell elevado:
```powershell
winget install -e --id NASM.NASM --accept-source-agreements --accept-package-agreements
$env:Path += ";C:\Program Files\NASM"
nasm --version
```

### Passo 4 — Instalar Strawberry Perl (GAP-WS-31)

No seu PowerShell elevado:
```powershell
winget install -e --id StrawberryPerl.StrawberryPerl --accept-source-agreements --accept-package-agreements
perl --version
```

### Passo 5 — Verificar a toolchain

```powershell
.\scripts\check-windows-toolchain.ps1
```

Saída esperada (modo texto):
```
Tool        Found   Path                                                         Version             Status
----        -----   ----                                                         -------             ------
cargo       yes     C:\Users\voce\.cargo\bin\cargo.exe                            1.88.0              ok
rustc       yes     C:\Users\voce\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\rustc.exe  1.88.0  ok
cmake       yes     C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe  3.27.0  ok
nasm        yes     C:\Program Files\NASM\nasm.exe                               2.16.03             ok
cl.exe      yes     C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.40.33807\bin\Hostx64\x64\cl.exe  19.40  ok
link.exe    yes     C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.40.33807\bin\Hostx64\x64\link.exe  14.40  ok
perl        yes     C:\Strawberry\perl\bin\perl.exe                                5.38.0              ok

[OK] All 7 tools present. duckduckgo-search-cli v0.7.5 should build cleanly on this host.
```

### Passo 6 — Instalar duckduckgo-search-cli

```powershell
cargo install duckduckgo-search-cli --version 0.7.5 --force
duckduckgo-search-cli --version
```

Esperado: `duckduckgo-search-cli 0.7.5`

---

## Método B — Tudo standalone via winget (sem Visual Studio Installer)

Melhor para usuários que querem tudo em uma passagem automatizada e não se importam com múltiplas fontes winget.

```powershell
# Abrir PowerShell como Administrador
winget install -e --id Microsoft.VisualStudio.2022.BuildTools --accept-source-agreements --accept-package-agreements
winget install -e --id NASM.NASM --accept-source-agreements --accept-package-agreements
winget install -e --id Kitware.CMake --accept-source-agreements --accept-package-agreements
winget install -e --id StrawberryPerl.StrawberryPerl --accept-source-agreements --accept-package-agreements

# Após o instalador do VS Build Tools terminar, modificar a instalação para
# adicionar o sub-componente C++ CMake tools for Windows:
#   winget não consegue fazer isso em um único passo. Use:
#   "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vs_installer.exe" modify `
#     --installPath "C:\Program Files\Microsoft Visual Studio\2022\BuildTools" `
#     --add Microsoft.VisualStudio.Component.VC.CMake.Project --quiet --norestart

# Agora execute em um Developer PowerShell for VS 2022:
cargo install duckduckgo-search-cli --version 0.7.5 --force
```

---

## Método C — Apenas Chocolatey (Windows + um gerenciador de pacotes)

Melhor para usuários já em Chocolatey.

```powershell
# Abrir PowerShell como Administrador
choco install visualstudio2022buildtools -y
choco install nasm -y
choco install cmake -y --installargs 'ADD_CMAKE_TO_PATH=System'
choco install strawberryperl -y

# Depois adicione o sub-componente C++ CMake tools via vs_installer.exe (veja Método B)
# Execute em um Developer PowerShell for VS 2022:
cargo install duckduckgo-search-cli --version 0.7.5 --force
```

---

## Método D — Executar o script helper (mais automatizado)

`scripts/install-windows.ps1` verifica e auto-instala o que pode:

```powershell
# Abrir um Developer PowerShell for VS 2022 (para ferramentas MSVC)
.\scripts\install-windows.ps1

# Ou, com uma versão específica:
.\scripts\install-windows.ps1 --version 0.7.5 --force

# Ou, dry-run para portões de CI e troubleshooting humano:
.\scripts\install-windows.ps1 --check-only
```

`--check-only` emite um relatório tabular de quais ferramentas estão presentes e quais estão faltando, adequado para portões de CI e tickets de suporte. Exit code 0 se todas presentes, 1 se alguma faltando.

O script NÃO auto-instala MSVC (download de 5+ GB, requer admin, e é invasivo demais para um script one-shot). Ele imprime a invocação exata de `Launch-VsDevShell.ps1` para executar após instalar o VS Build Tools.

---

## Método E — Diagnóstico standalone (read-only, sem instalações)

Para tickets de suporte e portões de CI:

```powershell
.\scripts\check-windows-toolchain.ps1          # legível por humanos
.\scripts\check-windows-toolchain.ps1 --json  # legível por máquina
```

Saída JSON:
```json
{
  "all_ok": true,
  "tools": [
    { "name": "cargo",    "found": true,  "path": "...", "version": "1.88.0", "status": "ok" },
    { "name": "cmake",    "found": true,  "path": "...", "version": "3.27.0", "status": "ok" },
    ...
  ],
  "timestamp": "2026-06-14T00:00:00Z"
}
```

---

## Troubleshooting

### Build aborta com `failed to execute command: program not found`

`cmake.exe` não está no PATH. Causa mais comum: o sub-componente **C++ CMake tools for Windows** não foi selecionado no Visual Studio Installer. Correção: re-execute o VS Installer → Modify → Workloads → Desktop development with C++ → expanda → marque **C++ CMake tools for Windows**.

### Build aborta com `No CMAKE_ASM_NASM_COMPILER could be found`

`nasm.exe` não está no PATH. Correção: `winget install -e --id NASM.NASM` e depois `$env:Path += ";C:\Program Files\NASM"`.

### Build aborta com `no compiler found` ou `linker not found`

`cl.exe` e/ou `link.exe` não estão no PATH. Correção: abra um shell **Developer PowerShell for VS 2022**, ou execute `Launch-VsDevShell.ps1` (veja Passo 2 acima).

### Build aborta com `perl not found`

`perl.exe` não está no PATH. Correção: `winget install -e --id StrawberryPerl.StrawberryPerl`.

### Build aborta 30+ minutos adentro com `C1083: Cannot open include file: 'stdio.h'`

`INCLUDE` e `LIB` env vars não estão configuradas. Você está em um PowerShell regular ao invés de um Developer PowerShell. Mude de shell (veja Passo 2 acima).

### Quer pular o preflight

Para setups customizados de toolchain, você pode bypassar checagens individuais de preflight:

```powershell
$env:DDG_SKIP_NASM_CHECK=1
$env:DDG_SKIP_CMAKE_CHECK=1
$env:DDG_SKIP_MSVC_CHECK=1
$env:DDG_SKIP_PERL_CHECK=1
cargo install duckduckgo-search-cli --version 0.7.5 --force
```

Isso é útil quando, por exemplo, você tem cmake em uma localização não-padrão e o scan de known-dir não encontrou.

---

## Requisito do Chrome (v0.8.0)
- Instale o Google Chrome em https://www.google.com/chrome/
- Chrome é usado como transporte de busca PRIMÁRIO desde a v0.8.0
- Sem necessidade de `xvfb` no Windows (display nativo é usado)
- Chrome é auto-detectado nos caminhos de instalação padrão


## Veja também

- `scripts/install-windows.ps1` — helper de auto-instalação (Método D deste guia)
- `scripts/check-windows-toolchain.ps1` — ferramenta de diagnóstico (Método E deste guia)
- `build.rs` — preflight Rust que aborta o build em segundos se uma ferramenta estiver faltando
- `gaps.md` GAP-WS-28, GAP-WS-29, GAP-WS-30, GAP-WS-31 — análise completa de causa-raiz
- `docs/CROSS_PLATFORM.md` — overview de pré-requisitos de build por plataforma
