# Integrations

`duckduckgo-search-cli` integrates with 16+ AI agents and automation platforms
via its stable JSON contract, deterministic exit codes, and zero-dependency
binary install. This file is a pointer to the full integration catalog.

## Full Catalog

See [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) for the complete
integration guide, including:

- 16 supported AI agents (Claude, GPT, Gemini, Cursor, OpenCode, etc.)
- Flag aliases introduced in each version
- Summary table consolidating all integrations
- Per-platform installation recipes
- Exit code semantics for agent decision-making
- Per-integration snippets with `timeout`, `jaq`, and `PIPESTATUS`

## Quick Reference

```bash
# Canonical invocation
timeout 60 duckduckgo-search-cli -q -f json --num 15 "query"

# Exit codes
0  success         → parse .resultados
1  runtime error   → read stderr; retry once with -v
2  config error    → re-run init-config --force
3  anti-bot block  → back off 300+ s; switch --endpoint lite
4  global timeout  → raise --global-timeout; reduce --parallel
5  zero results    → refine query or try different --lang

# Current version: v0.7.5
```

## v0.7.3 Highlights for Integrations
## v0.7.5 Highlights for Integrations

- **GAP-WS-29 fixed (CRITICAL, build experience, Windows)** — `cargo install` on native Windows MSVC without the **C++ CMake tools for Windows** sub-component of the Visual Studio Installer previously failed minutes into the BoringSSL build with the cryptic `program not found / is 'cmake' not installed?`. The `build.rs` preflight now detects this and aborts in SECONDS with the exact fix (`winget install -e --id Kitware.Cmake` OR Visual Studio Installer → Modify → Workloads → Desktop development with C++ → expand → check C++ CMake tools for Windows). New escape hatch: `DDG_SKIP_CMAKE_CHECK=1`.
- **GAP-WS-30 fixed (CRITICAL, build experience, Windows)** — BoringSSL CMake uses the Visual Studio 17 2022 generator which requires `cl.exe` (compiler) and `link.exe` (linker). The `build.rs` preflight now detects both and aborts with the fix (open a Developer PowerShell for VS 2022, or run `Launch-VsDevShell.ps1`). MSVC is NOT auto-installed (5+ GB download, too intrusive). New escape hatch: `DDG_SKIP_MSVC_CHECK=1`.
- **GAP-WS-31 fixed (CRITICAL, build experience, Windows)** — BoringSSL perlasm generator emits crypto assembly in NASM format and requires `perl.exe`. The `build.rs` preflight now detects perl and reports the fix (`winget install -e --id StrawberryPerl.StrawberryPerl`). New escape hatch: `DDG_SKIP_PERL_CHECK=1`.
- **GAP-WS-32/35/36 fixed (MEDIUM, documentation)** — All remaining claims that "pre-built binaries from `cargo install` are unaffected" (or its PT/EN variants) are now qualified across `skill/duckduckgo-search-cli-en/SKILL.md`, `skill/duckduckgo-search-cli-pt/SKILL.md`, `llms-full.txt`, `docs/CROSS_PLATFORM.md`, `README.md`, and `README.pt-BR.md`. **`crates.io` NEVER distributes binaries**; `cargo install` always compiles from source. Users on Windows must satisfy the four BoringSSL build prerequisites (NASM, CMake, MSVC, Perl) themselves before `cargo install` can succeed.
- **`build.rs` preflight coverage expanded** — v0.7.4 only checked for NASM. v0.7.5 checks for all four BoringSSL build prerequisites (nasm, cmake, cl.exe, link.exe, perl) and supports four independent `DDG_SKIP_*_CHECK=1` escape hatches.
- **New `scripts/check-windows-toolchain.ps1`** — standalone diagnostic (no installs) that checks all 7 tools (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) and emits text or JSON output. Exit code 0 if all present, 1 otherwise. Useful for support tickets and CI gates.
- **New `docs/INSTALL-WINDOWS.md` (EN) + `docs/INSTALL-WINDOWS.pt-BR.md` (PT)** — step-by-step guide covering 5 installation methods (VS Installer + standalone; all-winget standalone; Chocolatey; helper script; standalone diagnostic). Includes troubleshooting for each of the 4 GAPs and the `DDG_SKIP_*_CHECK` escape hatches.
- **CI Windows jobs updated** — `.github/workflows/ci.yml` and `.github/workflows/release.yml` now verify CMake, install Perl, and verify MSVC Build Tools (in addition to the existing NASM step) in every Windows job. This eliminates the implicit dependency on the `windows-2022` image's pre-installed tooling.
- **Zero breaking changes to JSON output schema**. All v0.7.4 fields remain present. All v0.7.3 fields remain present.


- **GAP-WS-27 fixed (CRITICAL)**: The macOS CAPTCHA interstitial that returned HTTP 200 with `quantidade_resultados: 0` while Windows returned full results is closed. TLS stack changed from `rustls` to BoringSSL via `wreq 6.0.0-rc.29`. `cargo install` always compiles from source — crates.io does not distribute pre-built binaries for any platform. The build toolchain change is the trade-off for the BoringSSL TLS fix (GAP-WS-27 closed). Source builds on Linux require `cmake`, `perl`, `pkg-config`, and `libclang-dev`; source builds on Windows require NASM, CMake, MSVC, and Perl (see `gaps.md` GAP-WS-28/29/30/31 and `docs/INSTALL-WINDOWS.md`).
- **`session` feature (cookie persistence + warm-up)**:
  - New flags: `--no-warmup`, `--no-cookie-persistence`, `--cookies-path <PATH>`.
  - Cookie jar persisted to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) with Unix permissions `0o600`.
  - Warm-up adds one `GET https://duckduckgo.com/` before the first real query to populate session cookies.
- **`probe-deep` feature (CAPTCHA interstitial detection)**:
  - New flags: `--probe-deep` (run a real search query and classify the body as `ok` or `captcha`), `--allow-lite-fallback` (opt-in to automatic `html → lite` fallback when CAPTCHA detected).
  - New JSON report fields on the probe response: `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status`, `latency_ms`.
- **Zero breaking changes to JSON output schema**. All v0.7.2 fields remain present.

## v0.7.0 Highlights for Integrations

- **New subcommand `deep-research`**: agents that need multi-hop answers can
  drop in `duckduckgo-search-cli deep-research "question" --synthesize`
  and get a Markdown report back, with no extra orchestration. Inherits
  every global flag (`-q -f json`, `--num`, `--parallel`, `--proxy`,
  `--fetch-content`) plus deep-research-specific knobs
  (`--max-sub-queries`, `--sub-queries-file`, `--aggregate`,
  `--budget-tokens`, `--synth-format`).
- **Backward-compatible**: zero changes to `buscar`, `init-config`,
  default-config JSON schema, or any exit code. Existing pipelines keep
  working unchanged.

## v0.6.5 Highlights for Integrations

- **MP-26 FIX**: Windows build now compiles. Use `cargo install duckduckgo-search-cli`
  on any platform without manual patches.
- **CI-01 FIX**: CI matrix now green on all 3 SOs (Linux/macOS/Windows).
  Agents running on Windows runners can rely on the binary.
- **WS-12 Circuit breaker**: `--fetch-content --parallel` no longer cascades
  failures across hosts — one slow domain won't block the rest of the crawl.
- **WS-25 ProgressBar**: `indicatif` output to stderr auto-hides in pipes,
  so JSON pipelines on stdout stay clean.

See `CHANGELOG.md` for the complete v0.6.5 changelog and migration notes
from earlier versions.
