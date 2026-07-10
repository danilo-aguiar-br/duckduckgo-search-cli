# Installing duckduckgo-search-cli on Windows (v0.8.6+)

Since v0.8.6, `duckduckgo-search-cli` uses `reqwest` with `rustls-tls` instead of `wreq`/BoringSSL. This eliminates the need for NASM, CMake, Perl, and MSVC. The only prerequisite is Rust.


## Prerequisites

- Windows 10 version 1903 or newer, or Windows 11
- Rust toolchain installed via [rustup](https://rustup.rs/)


## Installation

```powershell
cargo install duckduckgo-search-cli
duckduckgo-search-cli --version
```

That is it. No special shell, no extra compilers, no assembler.


## Required: Chrome (production network transport, v0.9.4)

See [ADR-0016](decisions/0016-chrome-only-universal-v0-9-4.md) / **GAP-WS-113** for the Chrome-only production policy.

- Chrome/Chromium is **required for production** (feature `chrome` is default; GAP-WS-113). Search, news, `deep-research`, `--probe`, `--probe-deep`, `--pre-flight`, and `--fetch-content` all use chromiumoxide/CDP
- Without a usable Chrome (or with `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`) network ops **fail closed with exit 2**
- On Windows Chrome runs headless=new since v0.9.3 (Linux uses a private Xvfb display)
- Install Google Chrome from https://www.google.com/chrome/
- No `xvfb` needed on Windows
- Chrome is auto-detected in standard installation paths


## Historical: v0.7.3 to v0.8.5 (BoringSSL era)

Versions v0.7.3 through v0.8.5 depended on `wreq`/BoringSSL, which required four native build tools on Windows:

1. NASM assembler
2. CMake 3.20+
3. MSVC compiler + linker (Visual Studio Build Tools)
4. Strawberry Perl

If you are installing an older version (v0.7.3 to v0.8.5), you still need these tools. Refer to the [v0.8.5 version of this document](https://github.com/daniloaguiarbr/duckduckgo-search-cli/blob/v0.8.5/docs/INSTALL-WINDOWS.md) for the full step-by-step guide.

Since v0.8.6, none of these are required.


## Troubleshooting

### `cargo install` fails with network errors

Ensure your Rust toolchain is up to date: `rustup update stable`

### Want to install a specific version

```powershell
cargo install duckduckgo-search-cli --version 0.8.6 --force
```


## See also

- `docs/CROSS_PLATFORM.md` — overview of build prerequisites per platform
- `docs/decisions/0016-chrome-only-universal-v0-9-4.md` — Chrome-only production (GAP-WS-113)
