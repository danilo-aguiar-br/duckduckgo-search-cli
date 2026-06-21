# Installing duckduckgo-search-cli on Windows (v0.7.5+)

This guide covers the four prerequisites that `cargo install duckduckgo-search-cli` requires on **native Windows MSVC** since v0.7.3 (BoringSSL via `wreq 6.0.0-rc.29`):

1. **NASM assembler** (GAP-WS-28, preflight added in v0.7.4)
2. **CMake 3.20+** (GAP-WS-29, preflight added in v0.7.5)
3. **MSVC compiler + linker** (GAP-WS-30, preflight added in v0.7.5)
4. **Perl interpreter** (GAP-WS-31, preflight added in v0.7.5)

`cargo install` always compiles from source. crates.io ships **NO** pre-built binaries. There is no `apt install duckduckgo-search-cli` for Windows. Every Windows user must satisfy these four prerequisites.

> **TL;DR — fastest path:** run `scripts/install-windows.ps1` from a Developer PowerShell for VS 2022. It auto-installs what can be auto-installed (NASM, CMake, Perl) and prints actionable instructions for what cannot (MSVC).

---

## Prerequisites

- Windows 10 version 1903 or newer, or Windows 11
- PowerShell 5.1+ (Windows PowerShell) or PowerShell 7+ (recommended)
- Administrator shell (right-click → "Run as administrator") for installs that touch `Program Files`
- 5 GB of free disk space (Visual Studio Build Tools is large; the rest is small)

---

## Method A — Visual Studio Installer + standalone tools (recommended)

Best for users who already have Visual Studio Installer or want a clean, official source of each tool.

### Step 1 — Install Visual Studio Build Tools with the right sub-components

1. Download Visual Studio Build Tools 2022 from <https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022> (Community or Build Tools SKU both work).
2. Run the installer, click **Modify** on an existing install (or install fresh).
3. In the **Workloads** tab, check **Desktop development with C++**.
4. In the right pane **Installation details**, **expand** that workload and check **C++ CMake tools for Windows**. **This sub-component is deselected by default** — without it, the `cmake` build will fail with `failed to execute command: program not found` minutes in.
5. Click **Modify** to install. This adds `cl.exe`, `link.exe`, `cmake.exe`, the Windows SDK, and the MSVC standard libraries.

### Step 2 — Open the right shell

After install, the MSVC tools (cl.exe, link.exe) are NOT on the global PATH. You MUST run your build from one of these shells:

- **Start menu** → **Developer Command Prompt for VS 2022** (cmd-based)
- **Start menu** → **Developer PowerShell for VS 2022** (PowerShell-based)
- Or, from a regular PowerShell, source the env script:
  ```powershell
  & "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1"
  ```
  (Adjust `Community` to `BuildTools` / `Professional` / `Enterprise` based on your SKU.)

The shell now has `PATH`, `INCLUDE`, and `LIB` set for MSVC. Without this, `cargo build` fails with `no compiler found`.

### Step 3 — Install NASM (GAP-WS-28)

In your elevated PowerShell:
```powershell
winget install -e --id NASM.NASM --accept-source-agreements --accept-package-agreements
$env:Path += ";C:\Program Files\NASM"
nasm --version
```

### Step 4 — Install Strawberry Perl (GAP-WS-31)

In your elevated PowerShell:
```powershell
winget install -e --id StrawberryPerl.StrawberryPerl --accept-source-agreements --accept-package-agreements
perl --version
```

### Step 5 — Verify the toolchain

```powershell
.\scripts\check-windows-toolchain.ps1
```

Expected output (text mode):
```
Tool        Found   Path                                                         Version             Status
----        -----   ----                                                         -------             ------
cargo       yes     C:\Users\you\.cargo\bin\cargo.exe                             1.88.0              ok
rustc       yes     C:\Users\you\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\rustc.exe  1.88.0  ok
cmake       yes     C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe  3.27.0  ok
nasm        yes     C:\Program Files\NASM\nasm.exe                               2.16.03             ok
cl.exe      yes     C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.40.33807\bin\Hostx64\x64\cl.exe  19.40  ok
link.exe    yes     C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.40.33807\bin\Hostx64\x64\link.exe  14.40  ok
perl        yes     C:\Strawberry\perl\bin\perl.exe                                5.38.0              ok

[OK] All 7 tools present. duckduckgo-search-cli v0.7.5 should build cleanly on this host.
```

### Step 6 — Install duckduckgo-search-cli

```powershell
cargo install duckduckgo-search-cli --version 0.7.5 --force
duckduckgo-search-cli --version
```

Expected: `duckduckgo-search-cli 0.7.5`

---

## Method B — All standalone via winget (no Visual Studio Installer)

Best for users who want everything in one automated pass and don't mind multiple winget sources.

```powershell
# Open PowerShell as Administrator
winget install -e --id Microsoft.VisualStudio.2022.BuildTools --accept-source-agreements --accept-package-agreements
winget install -e --id NASM.NASM --accept-source-agreements --accept-package-agreements
winget install -e --id Kitware.CMake --accept-source-agreements --accept-package-agreements
winget install -e --id StrawberryPerl.StrawberryPerl --accept-source-agreements --accept-package-agreements

# After the VS Build Tools installer finishes, modify the install to add the
# C++ CMake tools for Windows sub-component:
#   winget cannot do this in one shot. Use:
#   "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vs_installer.exe" modify ^
#     --installPath "C:\Program Files\Microsoft Visual Studio\2022\BuildTools" ^
#     --add Microsoft.VisualStudio.Component.VC.CMake.Project --quiet --norestart

# Now run in a Developer PowerShell for VS 2022:
cargo install duckduckgo-search-cli --version 0.7.5 --force
```

---

## Method C — Chocolatey only (Windows + one package manager)

Best for users already on Chocolatey.

```powershell
# Open PowerShell as Administrator
choco install visualstudio2022buildtools -y
choco install nasm -y
choco install cmake -y --installargs 'ADD_CMAKE_TO_PATH=System'
choco install strawberryperl -y

# Then add the C++ CMake tools sub-component via vs_installer.exe (see Method B)
# Run in a Developer PowerShell for VS 2022:
cargo install duckduckgo-search-cli --version 0.7.5 --force
```

---

## Method D — Run the helper script (most automated)

`scripts/install-windows.ps1` checks and auto-installs what it can:

```powershell
# Open a Developer PowerShell for VS 2022 (for MSVC tools)
.\scripts\install-windows.ps1

# Or, with a specific version:
.\scripts\install-windows.ps1 --version 0.7.5 --force

# Or, dry-run for CI gates and human troubleshooting:
.\scripts\install-windows.ps1 --check-only
```

`--check-only` emits a tabular report of which tools are present and which are missing, suitable for CI gates and support tickets. Exit code 0 if all present, 1 if any missing.

The script does NOT auto-install MSVC (5+ GB download, requires admin, and is too invasive for a one-shot script). It prints the exact `Launch-VsDevShell.ps1` invocation to run after installing VS Build Tools.

---

## Method E — Standalone diagnostic (read-only, no installs)

For support tickets and CI gates:

```powershell
.\scripts\check-windows-toolchain.ps1          # human-readable
.\scripts\check-windows-toolchain.ps1 --json  # machine-readable
```

JSON output:
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

### Build aborts with `failed to execute command: program not found`

`cmake.exe` is not in PATH. Most common cause: the **C++ CMake tools for Windows** sub-component was not selected in the Visual Studio Installer. Fix: re-run the VS Installer → Modify → Workloads → Desktop development with C++ → expand → check **C++ CMake tools for Windows**.

### Build aborts with `No CMAKE_ASM_NASM_COMPILER could be found`

`nasm.exe` is not in PATH. Fix: `winget install -e --id NASM.NASM` then `$env:Path += ";C:\Program Files\NASM"`.

### Build aborts with `no compiler found` or `linker not found`

`cl.exe` and/or `link.exe` not in PATH. Fix: open a **Developer PowerShell for VS 2022** shell, or run `Launch-VsDevShell.ps1` (see Step 2 above).

### Build aborts with `perl not found`

`perl.exe` is not in PATH. Fix: `winget install -e --id StrawberryPerl.StrawberryPerl`.

### Build aborts 30+ minutes in with `C1083: Cannot open include file: 'stdio.h'`

`INCLUDE` and `LIB` env vars are not set. You are in a regular PowerShell instead of a Developer PowerShell. Switch shells (see Step 2 above).

### Want to skip the preflight

For custom toolchain setups, you can bypass individual preflight checks:

```powershell
$env:DDG_SKIP_NASM_CHECK=1
$env:DDG_SKIP_CMAKE_CHECK=1
$env:DDG_SKIP_MSVC_CHECK=1
$env:DDG_SKIP_PERL_CHECK=1
cargo install duckduckgo-search-cli --version 0.7.5 --force
```

This is useful when, e.g., you have cmake in a non-standard location and the known-dir scan didn't find it.

---

## Chrome Requirement (v0.8.0)
- Install Google Chrome from https://www.google.com/chrome/
- Chrome is used as the PRIMARY search transport since v0.8.0
- No `xvfb` needed on Windows (native display is used)
- Chrome is auto-detected in standard installation paths


## See also

- `scripts/install-windows.ps1` — auto-install helper (this guide's Method D)
- `scripts/check-windows-toolchain.ps1` — diagnostic tool (this guide's Method E)
- `build.rs` — the Rust preflight that aborts the build in seconds if a tool is missing
- `gaps.md` GAP-WS-28, GAP-WS-29, GAP-WS-30, GAP-WS-31 — full root-cause analysis
- `docs/CROSS_PLATFORM.md` — overview of build prerequisites per platform
