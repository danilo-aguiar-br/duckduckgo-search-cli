#Requires -Version 5.1
<#
.SYNOPSIS
Standalone diagnostic for the Windows toolchain required by duckduckgo-search-cli v0.7.5+.

.DESCRIPTION
Validates the four prerequisites for building BoringSSL on native Windows MSVC:
  - cargo (Rust toolchain — checked but not auto-installed)
  - rustc (Rust compiler — checked but not auto-installed)
  - cmake (GAP-WS-29, fixed in v0.7.5)
  - nasm (GAP-WS-28, fixed in v0.7.4)
  - cl.exe (GAP-WS-30, MSVC compiler — checked but not auto-installed)
  - link.exe (GAP-WS-30, MSVC linker — checked but not auto-installed)
  - perl (GAP-WS-31, fixed in v0.7.5)

Unlike `scripts/install-windows.ps1`, this script NEVER installs anything.
Its sole purpose is to produce a report that support staff and CI can consume.
Output is text by default, JSON via `--json` for pipelines.

.EXAMPLE
.\scripts\check-windows-toolchain.ps1
.\scripts\check-windows-toolchain.ps1 --json
#>
[CmdletBinding()]
param(
    [switch]$Json
)

$ErrorActionPreference = 'Stop'

$tools = @(
    @{ Name = 'cargo';     Args = @('cargo',     '--version');     Pattern = '^cargo\s+(\d+\.\d+\.\d+)' },
    @{ Name = 'rustc';     Args = @('rustc',     '--version');     Pattern = '^rustc\s+(\d+\.\d+\.\d+)' },
    @{ Name = 'cmake';     Args = @('cmake',     '--version');     Pattern = '^cmake\s+version\s+(\d+\.\d+\.\d+)' },
    @{ Name = 'nasm';      Args = @('nasm',      '-v');            Pattern = '^NASM\s+version\s+(\d+\.\d+)' },
    @{ Name = 'cl.exe';    Args = @('cl.exe',    '/?') | Out-Null;  Pattern = $null },
    @{ Name = 'link.exe';  Args = @('link.exe',  '/?') | Out-Null;  Pattern = $null },
    @{ Name = 'perl';      Args = @('perl',      '--version');     Pattern = 'v(\d+\.\d+\.\d+)' }
)

$results = @()
foreach ($tool in $tools) {
    $entry = [ordered]@{
        name    = $tool.Name
        found   = $false
        path    = $null
        version = $null
        status  = 'missing'
    }
    $cmd = Get-Command $tool.Name -ErrorAction SilentlyContinue
    if ($cmd) {
        $entry.found = $true
        $entry.path = $cmd.Source
        try {
            $versionOutput = & $tool.Name ($tool.Args | Where-Object { $_ -ne $null }) 2>&1 | Select-Object -First 1
            if ($tool.Pattern -and $versionOutput -match $tool.Pattern) {
                $entry.version = $matches[1]
                $entry.status = 'ok'
            } else {
                $entry.status = 'wrong_version'
                $entry.version = ($versionOutput -split "`n")[0]
            }
        } catch {
            $entry.status = 'error'
        }
    }
    $results += [pscustomobject]$entry
}

$allOk = ($results | Where-Object { $_.status -ne 'ok' }).Count -eq 0

if ($Json) {
    $payload = [pscustomobject]@{
        all_ok    = $allOk
        tools     = $results
        timestamp = (Get-Date).ToUniversalTime().ToString('o')
    }
    $payload | ConvertTo-Json -Depth 5
    if (-not $allOk) { exit 1 }
    exit 0
}

# Text table
Write-Host ''
Write-Host ('Tool'.PadRight(12) + ('Found'.PadRight(8) + ('Path'.PadRight(60) + ('Version'.PadRight(20) + 'Status'))))
Write-Host ('----'.PadRight(12) + ('-----'.PadRight(8) + ('----'.PadRight(60) + ('-------'.PadRight(20) + '------'))))
foreach ($r in $results) {
    $pathShort = if ($r.path) { if ($r.path.Length -gt 58) { '...' + $r.path.Substring($r.path.Length - 55) } else { $r.path } } else { '-' }
    $verShort = if ($r.version) { if ($r.version.Length -gt 18) { $r.version.Substring(0, 18) } else { $r.version } } else { '-' }
    $foundStr = if ($r.found) { 'yes' } else { 'NO' }
    Write-Host ($r.name.PadRight(12) + $foundStr.PadRight(8) + $pathShort.PadRight(60) + $verShort.PadRight(20) + $r.status)
}

Write-Host ''
if ($allOk) {
    Write-Host '[OK] All 7 tools present. duckduckgo-search-cli v0.7.5 should build cleanly on this host.'
    exit 0
} else {
    Write-Host '[FAIL] One or more tools missing. Run scripts/install-windows.ps1 to auto-install what can be installed (cmake, nasm, perl). MSVC cannot be auto-installed — use the Visual Studio Installer.'
    exit 1
}
