# Installing duckduckgo-search-cli on Windows (current: v1.0.6; TLS notes from v0.8.6+)

[Português (Brasil)](INSTALL-WINDOWS.pt-BR.md)

Since v0.8.6, `duckduckgo-search-cli` uses `reqwest` with `rustls-tls` instead of `wreq`/BoringSSL. This eliminates the need for NASM, CMake, Perl, and MSVC. The only prerequisite is Rust. Current release: v1.0.6.


## Prerequisites
- Windows 10 version 1903 or newer, or Windows 11
- Rust toolchain installed via [rustup](https://rustup.rs/)


## Registry state — read this before you install
- v1.0.6 is the first release with the cfg-stripping defect closed (GAP-REL-001)
- MEASURED 2026-08-21: crates.io serves `1.0.2` as `max_stable_version`, and `1.0.2` does NOT compile on macOS or Windows
- Until `1.0.6` is published, `cargo install ... --version 1.0.6` fails immediately with `could not find duckduckgo-search-cli with version 1.0.6`
- That fast, named failure is DELIBERATE: installing without a version pin silently resolves to the broken `1.0.2` and dies minutes later with a cryptic `E0432`
- A fast failure that names what is missing beats a slow one that does not
- Check the live registry yourself before reporting an install bug: `cargo run --bin verify_published --features release-gate`
- That gate exits 0 only when the registry actually serves the version this tree carries


## Installation

```powershell
cargo install duckduckgo-search-cli --locked --version 1.0.6
duckduckgo-search-cli --version
```

- ALWAYS pin the version, because a bare `cargo install duckduckgo-search-cli` can resolve to a build that does NOT compile on Windows
- Published `1.0.2` and `1.0.1` do NOT compile on Windows or macOS, failing with `E0432` unresolved imports
- You MUST install `1.0.6` or newer, which is the first published version with that defect class closed
- The safe invocation is `cargo install duckduckgo-search-cli --locked --version 1.0.6`

No special shell, no extra compilers, no assembler.


## Post-install verification
- The binary exposes exactly ten top-level commands: `buscar`, `init-config`, `completions`, `deep-research`, `commands`, `schema`, `doctor`, `locale`, `man`, `config`
- The `config` subcommand exposes exactly six operations: `path`, `list`, `get`, `set`, `unset`, `effective`
- The listing subcommand is `config list`, and `config list-keys` does NOT exist in this binary
- Run `doctor` first, because it is the only check that probes the Chrome that production actually needs

```powershell
duckduckgo-search-cli doctor
duckduckgo-search-cli commands
duckduckgo-search-cli schema
duckduckgo-search-cli locale
duckduckgo-search-cli man
duckduckgo-search-cli init-config
duckduckgo-search-cli config list
duckduckgo-search-cli config effective
```

- `doctor` reports Chrome detection and exits non-zero under `--strict` when Chrome is missing
- `commands` prints the machine-readable command inventory, so it proves the installed surface without guessing
- `schema` prints the JSON Schema catalog, and `--name` narrows it to a single schema
- `locale` prints the resolved UI locale, and `man` prints the roff man page built from the same clap tree as `--help`
- `init-config` writes the XDG config file, and `config path` tells you where it landed


## Required: Chrome (production network transport, v0.9.4+)

See [ADR-0016](decisions/0016-chrome-only-universal-v0-9-4.md) / GAP-WS-113 for the Chrome-only production policy.
See [ADR-0018](decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md) for v0.9.8 agent-ready defaults.

- Chrome/Chromium is REQUIRED for production (feature `chrome` is default; GAP-WS-113). Search, news, `deep-research`, `--probe`, `--probe-deep`, `--pre-flight`, and content fetch all use chromiumoxide/CDP
- Without a usable Chrome network ops fail closed with exit 2 (product env `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` is REMOVED / not read — Chrome required via feature `chrome`)
- On Windows Chrome runs headless=new since v0.9.3 (Linux uses a private Xvfb display)
- Since v0.9.6 the Chrome process tree is reaped on exit (one-shot ownership); production still needs Chrome installed for network ops (see [ADR-0017](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md))
- v1.0.0 disk one-shot (GAP-WS-TMP-PROFILE-ORPHAN-001 / [ADR-0020](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md)): Chrome profile prefix is `ddg-chrome-*` under the process temp dir; process tree AND profile dir are reaped on cooperative exit (`force_reap` + `ExitReapGuard`); residual after SIGKILL is cleaned on the next run via `sweep_orphan_profiles` of ONLY owned `ddg-chrome-*`. Hard policy: NEVER bulk-rm foreign `.tmp*` or `org.chromium.Chromium.*` (or other Chromium temp). Audit residual under `%TEMP%` / `$env:TEMP` by listing only directories named `ddg-chrome-*`. See [ADR-0017](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md) + [ADR-0020](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md)
- v1.0.1 / Pass 52: multi-query `--stream` / `-f ndjson` NDJSON; dual `config` API + `config effective`; BrokenPipe → exit 141 with SIG_IGN oneshot reap; wire PT serialize BC + EN deserialize aliases ([ADR-0023](decisions/0023-wire-pt-bc-english-deserialize-aliases.md)); product config is CLI+XDG only
- v1.0.2: English wire default ([ADR-0027](decisions/0027-wire-en-default-v1-0-2.md)) + `--wire-keys en|pt`; agent ops; budget dual/contention; Chrome always muted ([ADR-0026](decisions/0026-chrome-mute-audio-operational-standard-v1-0-2.md)); FETCH_CAP default 4; DEFAULT_PAGES=1
- v0.9.8: default `--vertical all` and content fetch ON (top web + news, cap 4 (v1.0.2 default)). Prefer longer timeouts (e.g. PowerShell `Start-Process` / external timeout 180s+) when accepting defaults; thin SERP path: `--vertical web --no-fetch-content` with ~60s
- Install Google Chrome from https://www.google.com/chrome/
- No `xvfb` needed on Windows
- Chrome is auto-detected in standard installation paths; override with CLI `--chrome-path` or XDG `config set chrome_path` (`CHROME_PATH` env is NOT read)


## Historical: v0.7.3 to v0.8.5 (BoringSSL era)

Versions v0.7.3 through v0.8.5 depended on `wreq`/BoringSSL, which required four native build tools on Windows:

- NASM assembler
- CMake 3.20+
- MSVC compiler + linker (Visual Studio Build Tools)
- Strawberry Perl

If you are installing an older version (v0.7.3 to v0.8.5), you still need these tools. Refer to the [v0.8.5 version of this document](https://github.com/danilo-aguiar-br/duckduckgo-search-cli/blob/v0.8.5/docs/INSTALL-WINDOWS.md) for the full step-by-step guide.

Since v0.8.6, none of these are required.


## Troubleshooting

### `cargo install` fails with network errors

Ensure your Rust toolchain is up to date: `rustup update stable`

### Want to install a specific version

```powershell
cargo install duckduckgo-search-cli --locked --version 1.0.6 --force
```


## See also
- `docs/CROSS_PLATFORM.md` — overview of build prerequisites per platform
- `docs/decisions/0016-chrome-only-universal-v0-9-4.md` — Chrome-only production (GAP-WS-113)
- `docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md` — process one-shot (ADR-0017 / GAP-WS-LIFECYCLE-001)
- `docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md` — disk one-shot + `ddg-chrome-*` (ADR-0020 / GAP-WS-TMP-PROFILE-ORPHAN-001)
- `docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md` — agent-ready defaults (v0.9.8)
- `docs/MIGRATION.md` — v0.9.7 → v0.9.8 breaking defaults; v0.9.9/v0.9.10 → v1.0.0 disk one-shot
