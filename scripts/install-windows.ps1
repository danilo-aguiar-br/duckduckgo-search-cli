#Requires -Version 5.1
<#
.SYNOPSIS
Installs duckduckgo-search-cli on Windows, handling the BoringSSL prerequisites.

.DESCRIPTION
Since v0.7.3 the CLI links BoringSSL (via wreq -> btls-sys), whose CMake build
requires on native Windows MSVC:
  - NASM assembler (closed GAP-WS-28 in v0.7.4)
  - CMake 3.20+ (closed GAP-WS-29 in v0.7.5)
  - MSVC C/C++ compiler (cl.exe) + linker (link.exe) (closed GAP-WS-30 in v0.7.5)
  - Perl interpreter (closed GAP-WS-31 in v0.7.5)
The C++ CMake tools for Windows is a sub-component of the C++ workload in the
Visual Studio Installer; it is NOT included by default and must be selected
manually. The MSVC toolchain is also NOT auto-added to PATH by the VS Installer;
users must run `Launch-VsDevShell.ps1` to set PATH, INCLUDE, and LIB.

This script detects each of the four tools, installs the auto-installable ones
(cmake, perl, nasm) via winget (choco fallback), prints actionable instructions
for MSVC (cannot auto-install), fixes the session PATH, and then runs
`cargo install`. The optional `--check-only` mode skips installation and
emits a tabular report, suitable for CI gates and human troubleshooting.

.EXAMPLE
.\scripts\install-windows.ps1
.\scripts\install-windows.ps1 --version 0.7.5 --force
.\scripts\install-windows.ps1 --check-only
Extra arguments are forwarded verbatim to `cargo install`.
#>
[CmdletBinding()]
param(
    [switch]$CheckOnly
)

$ErrorActionPreference = 'Stop'

function Find-Tool {
    param(
        [Parameter(Mandatory)] [string] $ExeName,
        [Parameter(Mandatory)] [string[]] $KnownDirs
    )
    $cmd = Get-Command $ExeName -ErrorAction SilentlyContinue
    if ($cmd) { return [pscustomobject]@{ Found = $true; Path = $cmd.Source; Source = 'PATH' } }
    foreach ($dir in $KnownDirs) {
        $candidate = Join-Path $dir $ExeName
        if (Test-Path $candidate) {
            Write-Host "$ExeName found at '$dir' but not in PATH - adding to this session's PATH."
            $env:Path = "$env:Path;$dir"
            return [pscustomobject]@{ Found = $true; Path = $candidate; Source = "KnownDir($dir)" }
        }
    }
    return [pscustomobject]@{ Found = $false; Path = $null; Source = $null }
}

function Install-Tool {
    param(
        [Parameter(Mandatory)] [string] $WingetId,
        [Parameter(Mandatory)] [string] $ChocoId,
        [Parameter(Mandatory)] [string] $DisplayName
    )
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        Write-Host "Installing $DisplayName via winget ($WingetId)..."
        winget install -e --id $WingetId --accept-source-agreements --accept-package-agreements
    } elseif (Get-Command choco -ErrorAction SilentlyContinue) {
        Write-Host "Installing $DisplayName via choco ($ChocoId)..."
        choco install $ChocoId -y
    } else {
        Write-Error "Neither winget nor choco is available. Install $DisplayName manually."
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo not found. Install Rust first: https://rustup.rs/"
}

# 1. NASM assembler (GAP-WS-28, v0.7.4+)
$nasm = Find-Tool -ExeName 'nasm.exe' -KnownDirs @(
    "$env:ProgramFiles\NASM",
    "${env:ProgramFiles(x86)}\NASM",
    "$env:LOCALAPPDATA\bin\NASM"
)
if (-not $nasm.Found) {
    if ($CheckOnly) {
        Write-Host "[MISSING] nasm.exe -- install via winget install -e --id NASM.NASM"
    } else {
        Install-Tool -WingetId 'NASM.NASM' -ChocoId 'nasm' -DisplayName 'NASM assembler'
        $nasm = Find-Tool -ExeName 'nasm.exe' -KnownDirs @(
            "$env:ProgramFiles\NASM",
            "${env:ProgramFiles(x86)}\NASM",
            "$env:LOCALAPPDATA\bin\NASM"
        )
        if (-not $nasm.Found) {
            Write-Error "NASM was installed but nasm.exe is still not reachable. Open a new terminal or add the NASM directory to PATH, then re-run this script."
        }
    }
} else {
    Write-Host "[OK] nasm.exe -> $($nasm.Path)"
}

# 2. CMake (GAP-WS-29, v0.7.5+)
$cmake = Find-Tool -ExeName 'cmake.exe' -KnownDirs @(
    "$env:ProgramFiles\CMake\bin",
    "${env:ProgramFiles(x86)}\CMake\bin",
    "$env:ProgramFiles\Microsoft Visual Studio\2022\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin",
    "$env:ProgramFiles\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin"
)
if (-not $cmake.Found) {
    if ($CheckOnly) {
        Write-Host "[MISSING] cmake.exe -- fix via Visual Studio Installer (Modify -> Workloads -> Desktop development with C++ -> Installation details -> check C++ CMake tools for Windows) OR winget install -e --id Kitware.CMake"
    } else {
        Install-Tool -WingetId 'Kitware.CMake' -ChocoId 'cmake' -DisplayName 'CMake'
        $cmake = Find-Tool -ExeName 'cmake.exe' -KnownDirs @(
            "$env:ProgramFiles\CMake\bin",
            "${env:ProgramFiles(x86)}\CMake\bin"
        )
        if (-not $cmake.Found) {
            Write-Error "CMake was installed but cmake.exe is still not reachable. Open a new terminal or add the CMake directory to PATH, then re-run this script."
        }
    }
} else {
    Write-Host "[OK] cmake.exe -> $($cmake.Path)"
}

# 3. MSVC compiler + linker (GAP-WS-30, v0.7.5+) -- cannot auto-install
$cl = Find-Tool -ExeName 'cl.exe' -KnownDirs @(
    "$env:ProgramFiles\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\*\bin\Hostx64\x64",
    "$env:ProgramFiles\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\*\bin\Hostx64\x64",
    "$env:ProgramFiles\Microsoft Visual Studio\2022\Professional\VC\Tools\MSVC\*\bin\Hostx64\x64",
    "$env:ProgramFiles\Microsoft Visual Studio\2022\Enterprise\VC\Tools\MSVC\*\bin\Hostx64\x64"
)
$link = Find-Tool -ExeName 'link.exe' -KnownDirs @(
    "$env:ProgramFiles\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\*\bin\Hostx64\x64",
    "$env:ProgramFiles\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\*\bin\Hostx64\x64"
)
if (-not $cl.Found -or -not $link.Found) {
    $missing = @()
    if (-not $cl.Found) { $missing += 'cl.exe' }
    if (-not $link.Found) { $missing += 'link.exe' }
    if ($CheckOnly) {
        Write-Host "[MISSING] $($missing -join ', ') -- install Visual Studio Build Tools 2019+ with the C++ workload, then run Launch-VsDevShell.ps1 in this shell before cargo install"
    } else {
        Write-Host "MSVC toolchain incomplete: $($missing -join ', ')"
        Write-Host "Auto-install of MSVC is NOT performed (too intrusive)."
        Write-Host "Fix:"
        Write-Host "  1. Install Visual Studio Build Tools 2019+ with the C++ workload:"
        Write-Host "     https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022"
        Write-Host "  2. Open a Developer Command Prompt for VS 2022, or run in this shell:"
        Write-Host "     & '$env:ProgramFiles\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1'"
        Write-Host "  3. Re-run this script."
        Write-Error "MSVC toolchain incomplete: $($missing -join ', ')"
    }
} else {
    Write-Host "[OK] cl.exe -> $($cl.Path)"
    Write-Host "[OK] link.exe -> $($link.Path)"
}

# 4. Perl interpreter (GAP-WS-31, v0.7.5+)
$perl = Find-Tool -ExeName 'perl.exe' -KnownDirs @(
    "$env:ProgramFiles\Strawberry\perl\bin",
    "$env:ProgramFiles\Strawberry64\perl\bin",
    "$env:ProgramFiles\Perl64\bin",
    "$env:ProgramFiles\Perl\bin"
)
if (-not $perl.Found) {
    if ($CheckOnly) {
        Write-Host "[MISSING] perl.exe -- install via winget install -e --id StrawberryPerl.StrawberryPerl"
    } else {
        Install-Tool -WingetId 'StrawberryPerl.StrawberryPerl' -ChocoId 'strawberryperl' -DisplayName 'Strawberry Perl'
        $perl = Find-Tool -ExeName 'perl.exe' -KnownDirs @(
            "$env:ProgramFiles\Strawberry\perl\bin",
            "$env:ProgramFiles\Perl64\bin"
        )
        if (-not $perl.Found) {
            Write-Error "Perl was installed but perl.exe is still not reachable. Open a new terminal or add the Perl directory to PATH, then re-run this script."
        }
    }
} else {
    Write-Host "[OK] perl.exe -> $($perl.Path)"
}

if ($CheckOnly) {
    Write-Host "`n--check-only: all preflight checks complete. Skipping cargo install."
    Write-Host "Exit code 0 if all tools are present; 1 if any are missing."
    if (-not $nasm.Found -or -not $cmake.Found -or -not $cl.Found -or -not $link.Found -or -not $perl.Found) {
        exit 1
    }
    exit 0
}

Write-Host "`nAll prerequisites OK. Installing duckduckgo-search-cli via cargo (this compiles BoringSSL and takes several minutes)..."
cargo install duckduckgo-search-cli --locked @args
Write-Host "Done. Verify with: duckduckgo-search-cli --version"
