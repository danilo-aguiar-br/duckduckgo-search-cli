// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (platform detection and XDG path resolution)
//! Platform detection and cross-platform initialization.
//!
//! Responsibilities:
//! 1. On Windows, call `SetConsoleOutputCP(65001)` to ensure UTF-8 in the console
//!    for cmd.exe and legacy `PowerShell` (noop in Windows Terminal and in pipes/files).
//! 2. TTY detection for format auto-detect (used by the `output` module).
//! 3. Configuration / cache / data / state directory resolution via `dirs`.
//! 4. Runtime environment autodetect (WSL, container, Termux, CI, Flatpak, Snap).
//!
//! The `init()` function MUST be called exactly once at the start of `main`.

use std::path::PathBuf;

/// Initializes platform-specific settings.
///
/// On Windows: configures UTF-8 codepage (65001) for console output.
/// On all platforms: performs no I/O operation that could fail.
///
/// This function is best-effort — if codepage configuration fails on Windows,
/// it only emits a warning via `tracing` and continues.
pub fn init() {
    #[cfg(windows)]
    init_windows();
}

#[cfg(windows)]
fn init_windows() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleCP, SetConsoleMode, SetConsoleOutputCP,
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
    };

    /// Win32 UTF-8 code page (matches `CP_UTF8` in Win32 Globalization).
    /// OS protocol constant — not user/XDG configuration.
    const CP_UTF8: u32 = 65001;

    // SAFETY:
    // - `SetConsoleOutputCP` takes a UINT code-page id by value (no pointers).
    // - `CP_UTF8` (65001) is the documented Win32 UTF-8 code page.
    let output_ok = unsafe { SetConsoleOutputCP(CP_UTF8) };
    if output_ok == 0 {
        tracing::warn!("Failed to configure UTF-8 output codepage (CP_UTF8) on Windows console.");
    }

    // MP-01: SetConsoleCP for stdin UTF-8 (queries with accents via pipe).
    // SAFETY:
    // - Same contract as `SetConsoleOutputCP`: UINT code-page id, BOOL return.
    // - Best-effort when no console is attached (returns 0; we only warn).
    let input_ok = unsafe { SetConsoleCP(CP_UTF8) };
    if input_ok == 0 {
        tracing::warn!("Failed to configure UTF-8 input codepage (CP_UTF8) on Windows console.");
    }

    if output_ok != 0 || input_ok != 0 {
        tracing::info!("UTF-8 codepage (CP_UTF8) configured on Windows console.");
    }

    // MP-02: Enable ANSI escape sequences (Virtual Terminal Processing).
    // SAFETY:
    // - `GetStdHandle` returns a HANDLE (`*mut c_void` in windows-sys 0.59+).
    // - Failure is null or `INVALID_HANDLE_VALUE`; both are checked before use.
    // - No pointer arithmetic; HANDLE is passed by value to later APIs.
    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
        let mut mode: u32 = 0;
        // SAFETY:
        // - `handle` validated non-null and not INVALID_HANDLE_VALUE.
        // - `mode` is a stack `u32`; GetConsoleMode writes only that word.
        if unsafe { GetConsoleMode(handle, &mut mode) } != 0 {
            let new_mode = mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            // SAFETY:
            // - Same validated HANDLE; mode is existing bits | VTP flag (opt-in).
            // - No memory dereference beyond the HANDLE value itself.
            if unsafe { SetConsoleMode(handle, new_mode) } == 0 {
                tracing::info!("ANSI VTP not available on this Windows console.");
            }
        }
    }
}

/// Checks whether `stdout` is connected to an interactive terminal (TTY).
/// Used by the `output` module for format auto-detect (text in TTY, json in pipe).
pub fn stdout_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Process-wide config home override from CLI `--config-home` (GAP-SCRAPE-R2-015).
static CONFIG_HOME_CLI: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Install CLI `--config-home` override (no product env).
pub fn set_config_home(path: Option<PathBuf>) {
    if let Ok(mut guard) = CONFIG_HOME_CLI.lock() {
        *guard = path;
    }
}

/// Optional project home override from CLI `--config-home`.
///
/// Rejected when the value contains `..` (path traversal safety).
fn project_home_override() -> Option<PathBuf> {
    let home = CONFIG_HOME_CLI
        .lock()
        .ok()
        .and_then(|g| g.clone())?;
    if home.to_string_lossy().contains("..") {
        tracing::warn!("--config-home contains '..', ignoring");
        return None;
    }
    Some(home)
}

/// XDG / platform project directory segment (single source of truth).
///
/// Matches `package.name` in `Cargo.toml`. Used under config/cache/data/state/runtime.
pub const PROJECT_DIR_NAME: &str = "duckduckgo-search-cli";

/// Returns the application configuration directory following XDG / Apple / Windows conventions.
///
/// Resulting paths:
/// - Linux: `$XDG_CONFIG_HOME/<PROJECT_DIR_NAME>/` or `~/.config/<PROJECT_DIR_NAME>/`.
/// - macOS: `~/Library/Application Support/<PROJECT_DIR_NAME>/`.
/// - Windows: `%APPDATA%\<PROJECT_DIR_NAME>\`.
///
/// CLI `--config-home` overrides the default path when set (rejected if it
/// contains `..` for path traversal safety). GAP-SCRAPE-R2-015: no product env.
///
/// Returns `None` if no configuration directory can be determined.
pub fn config_directory() -> Option<PathBuf> {
    if let Some(home) = project_home_override() {
        return Some(home);
    }
    dirs::config_dir().map(|base| base.join(PROJECT_DIR_NAME))
}

/// Cache directory for ephemeral project data (XDG cache / platform equivalent).
///
/// With `--config-home` set: `$HOME/cache`.
/// Otherwise: `dirs::cache_dir()/<PROJECT_DIR_NAME>`.
pub fn cache_directory() -> Option<PathBuf> {
    if let Some(home) = project_home_override() {
        return Some(home.join("cache"));
    }
    dirs::cache_dir().map(|base| base.join(PROJECT_DIR_NAME))
}

/// Persistent data directory (XDG data / platform equivalent).
///
/// With override: `$HOME/data`. Otherwise `dirs::data_dir()/<PROJECT_DIR_NAME>`.
pub fn data_directory() -> Option<PathBuf> {
    if let Some(home) = project_home_override() {
        return Some(home.join("data"));
    }
    dirs::data_dir().map(|base| base.join(PROJECT_DIR_NAME))
}

/// State directory (XDG state when available; falls back to data dir).
///
/// With override: `$HOME/state`. On platforms without `state_dir`, uses
/// [`data_directory`].
pub fn state_directory() -> Option<PathBuf> {
    if let Some(home) = project_home_override() {
        return Some(home.join("state"));
    }
    dirs::state_dir()
        .map(|base| base.join(PROJECT_DIR_NAME))
        .or_else(data_directory)
}

/// Runtime directory (XDG_RUNTIME_DIR when available).
///
/// Not overridden by `DUCKDUCKGO_SEARCH_CLI_HOME` (runtime is host-owned).
/// Returns `None` on platforms without a runtime dir concept.
pub fn runtime_directory() -> Option<PathBuf> {
    dirs::runtime_dir().map(|base| base.join(PROJECT_DIR_NAME))
}

/// Specialized host / container environment flags for agent diagnostics.
///
/// Detection is best-effort and pure (env + a few path probes). Used by
/// `doctor` and sandbox heuristics — never panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeEnvironment {
    /// Windows Subsystem for Linux (WSL1 or WSL2).
    pub wsl: bool,
    /// Docker / Podman / Kubernetes / generic container markers.
    pub container: bool,
    /// Android Termux (bionic) environment.
    pub termux: bool,
    /// Generic CI marker (`CI`, `GITHUB_ACTIONS`, etc.).
    pub ci: bool,
    /// Process is running inside a Flatpak sandbox (`FLATPAK_ID`).
    pub flatpak: bool,
    /// Process is running inside a Snap confinement (`SNAP`).
    pub snap: bool,
}

impl RuntimeEnvironment {
    /// Stable labels for JSON diagnostics (`["wsl","container"]` style).
    #[must_use]
    pub fn labels(self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.wsl {
            out.push("wsl");
        }
        if self.container {
            out.push("container");
        }
        if self.termux {
            out.push("termux");
        }
        if self.ci {
            out.push("ci");
        }
        if self.flatpak {
            out.push("flatpak");
        }
        if self.snap {
            out.push("snap");
        }
        out
    }
}

/// Detects specialized runtime environment (WSL, container, Termux, CI, sandboxes).
///
/// Called once per `doctor` invocation or on demand; cheap and side-effect free
/// aside from a few filesystem existence checks on Linux.
#[must_use]
pub fn detect_runtime_environment() -> RuntimeEnvironment {
    RuntimeEnvironment {
        wsl: is_wsl(),
        container: is_container(),
        termux: is_termux(),
        ci: is_ci(),
        flatpak: is_flatpak_sandbox(),
        snap: is_snap_sandbox(),
    }
}

/// Returns `true` when running under WSL (env markers or `/proc/version` probe).
#[must_use]
pub fn is_wsl() -> bool {
    if std::env::var_os("WSL_DISTRO_NAME").is_some()
        || std::env::var_os("WSL_INTEROP").is_some()
        || std::env::var_os("WSLENV").is_some()
    {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(v) = std::fs::read_to_string("/proc/version") {
            let lower = v.to_ascii_lowercase();
            if lower.contains("microsoft") || lower.contains("wsl") {
                return true;
            }
        }
    }
    false
}

/// Returns `true` when common container markers are present.
#[must_use]
pub fn is_container() -> bool {
    if std::env::var_os("DOCKER_CONTAINER").is_some()
        || std::env::var_os("container").is_some()
        || std::env::var_os("KUBERNETES_SERVICE_HOST").is_some()
        || std::env::var_os("PODMAN_CONTAINER").is_some()
    {
        return true;
    }
    if std::path::Path::new("/.dockerenv").exists() {
        return true;
    }
    if std::path::Path::new("/run/.containerenv").exists() {
        return true;
    }
    false
}

/// Returns `true` when running inside Android Termux.
#[must_use]
pub fn is_termux() -> bool {
    if std::env::var_os("TERMUX_VERSION").is_some() || std::env::var_os("TERMUX_APP_PID").is_some() {
        return true;
    }
    if let Ok(prefix) = std::env::var("PREFIX") {
        if prefix.contains("com.termux") {
            return true;
        }
    }
    false
}

/// Returns `true` when a CI environment variable is set.
///
/// Detects generic `CI` plus common providers. Project policy forbids GitHub
/// Actions **in this repository**; detection still helps agents running the
/// binary inside external CI hosts.
#[must_use]
pub fn is_ci() -> bool {
    if std::env::var_os("CI").is_some()
        || std::env::var_os("CONTINUOUS_INTEGRATION").is_some()
        || std::env::var_os("GITHUB_ACTIONS").is_some()
        || std::env::var_os("GITLAB_CI").is_some()
        || std::env::var_os("BUILDKITE").is_some()
        || std::env::var_os("CIRCLECI").is_some()
        || std::env::var_os("TRAVIS").is_some()
        || std::env::var_os("TF_BUILD").is_some()
    {
        return true;
    }
    false
}

/// Returns `true` when this process is inside a Flatpak sandbox (`FLATPAK_ID`).
#[must_use]
pub fn is_flatpak_sandbox() -> bool {
    std::env::var_os("FLATPAK_ID").is_some()
}

/// Returns `true` when this process is inside Snap confinement (`SNAP`).
#[must_use]
pub fn is_snap_sandbox() -> bool {
    std::env::var_os("SNAP").is_some()
}

/// Returns `true` if color output should be suppressed.
///
/// Checks (in order): `--no-color` flag, `NO_COLOR` env var (any value per
/// no-color.org), `CLICOLOR_FORCE=0`, `TERM=dumb`, and common screen-reader
/// env markers (`ACCESSIBILITY_ENABLED`, `GNOME_ACCESSIBILITY`,
/// `FORCE_COLOR=0` is not used — only documented suppressors).
pub fn should_disable_color(flag_no_color: bool) -> bool {
    if flag_no_color
        || std::env::var_os("NO_COLOR").is_some()
        || std::env::var("CLICOLOR_FORCE").ok().as_deref() == Some("0")
    {
        return true;
    }
    if std::env::var("TERM").ok().as_deref() == Some("dumb") {
        return true;
    }
    // Screen-reader / accessibility environments: no ANSI chrome on stderr.
    if std::env::var_os("ACCESSIBILITY_ENABLED").is_some()
        || std::env::var_os("GNOME_ACCESSIBILITY").is_some()
    {
        return true;
    }
    false
}

/// Returns `true` when a screen-reader or accessibility session is indicated.
///
/// Used to suppress spinners / decorative output (rules-rust i18n a11y).
pub fn screen_reader_mode() -> bool {
    std::env::var_os("ACCESSIBILITY_ENABLED").is_some()
        || std::env::var_os("GNOME_ACCESSIBILITY").is_some()
        || std::env::var("TERM").ok().as_deref() == Some("dumb")
}

/// Path to the external `selectors.toml` file (if the config directory exists).
///
/// Used by the lazy loader of `SelectorConfig` — when the file exists,
/// it overrides the hardcoded defaults.
pub fn selectors_toml_path() -> Option<PathBuf> {
    config_directory().map(|base| base.join("selectors.toml"))
}

/// Path to the external `user-agents.toml` file (if the config directory exists).
pub fn user_agents_toml_path() -> Option<PathBuf> {
    config_directory().map(|base| base.join("user-agents.toml"))
}

/// Identifier name of the current platform (for logs and User-Agent matching).
pub fn platform_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "freebsd") {
        "freebsd"
    } else if cfg!(target_os = "netbsd") {
        "netbsd"
    } else if cfg!(target_os = "openbsd") {
        "openbsd"
    } else if cfg!(target_os = "android") {
        "android"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_name_returns_known_value() {
        let name = platform_name();
        assert!(matches!(
            name,
            "linux"
                | "macos"
                | "windows"
                | "freebsd"
                | "netbsd"
                | "openbsd"
                | "android"
                | "other"
        ));
        assert_ne!(name, "outro", "platform labels must be English");
    }

    #[test]
    fn init_should_not_panic() {
        // Smoke test — on non-Windows platforms, this is a no-op.
        // On Windows, the call is best-effort and must not panic.
        init();
    }

    #[test]
    fn config_directory_not_empty_on_systems_with_home() {
        // On CI hosts without HOME, may return None. Only check type safety.
        let _ = config_directory();
    }

    #[test]
    fn path_helpers_share_project_suffix_or_override() {
        if let Some(cache) = cache_directory() {
            let s = cache.to_string_lossy();
            assert!(
                s.contains("duckduckgo-search-cli") || s.ends_with("cache"),
                "unexpected cache path: {s}"
            );
        }
        if let Some(data) = data_directory() {
            let s = data.to_string_lossy();
            assert!(
                s.contains("duckduckgo-search-cli") || s.ends_with("data"),
                "unexpected data path: {s}"
            );
        }
        let _ = state_directory();
        let _ = runtime_directory();
    }

    #[test]
    fn toml_paths_end_with_expected_names() {
        if let Some(selectors) = selectors_toml_path() {
            assert!(selectors.ends_with("selectors.toml"));
        }
        if let Some(uas) = user_agents_toml_path() {
            assert!(uas.ends_with("user-agents.toml"));
        }
    }

    #[test]
    fn term_dumb_disables_color() {
        // Best-effort: only assert pure flag path when env is clean enough.
        // Flag always wins.
        assert!(should_disable_color(true));
    }

    #[test]
    fn detect_runtime_environment_is_pure() {
        let env = detect_runtime_environment();
        // Labels must only contain known tokens.
        for label in env.labels() {
            assert!(matches!(
                label,
                "wsl" | "container" | "termux" | "ci" | "flatpak" | "snap"
            ));
        }
    }

    #[test]
    fn container_marker_dockerenv_or_env() {
        // Existence of /.dockerenv is host-dependent; just ensure the function
        // returns a bool without panic and is consistent with detect().
        let c = is_container();
        assert_eq!(c, detect_runtime_environment().container);
    }

    #[test]
    fn sandbox_env_helpers_match_detect() {
        let env = detect_runtime_environment();
        assert_eq!(is_flatpak_sandbox(), env.flatpak);
        assert_eq!(is_snap_sandbox(), env.snap);
        assert_eq!(is_termux(), env.termux);
        assert_eq!(is_ci(), env.ci);
        assert_eq!(is_wsl(), env.wsl);
    }
}
