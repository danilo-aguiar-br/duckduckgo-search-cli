// SPDX-License-Identifier: MIT OR Apache-2.0
//! FACTORY seed constants for runtime resolution (CLI > XDG > FACTORY).
//!
//! Product knobs must not be hard-coded at call sites. Seed defaults live here
//! (or in `types/bounded` MAX_* / clap `default_value_t` via `cli` module) and
//! are overridden only by XDG `config set` or explicit CLI flags.
//!
//! # Precedence
//! 1. Explicit CLI flag (non-default clap value)
//! 2. XDG `~/.config/duckduckgo-search-cli/config.toml`
//! 3. OS locale (SERP lang/country only)
//! 4. FACTORY seed (this module + `cli` DEFAULT_* constants)

// Re-export clap/domain factory seeds so agents have one import path.
pub use crate::cli::{
    DEFAULT_FETCH_CONTENT_CAP, DEFAULT_GLOBAL_TIMEOUT, DEFAULT_PARALLELISM, DEFAULT_SERP_COUNTRY,
    DEFAULT_SERP_LANG,
};
pub use crate::error::{DEFAULT_CHROME_SESSION_RETRIES, MAX_CHROME_SESSION_RETRIES};
pub use crate::types::bounded::{MAX_GLOBAL_TIMEOUT_SECONDS, MAX_PARALLELISM};

/// Default effective `--num` when the flag is omitted (factory seed).
pub const FACTORY_DEFAULT_NUM_RESULTS: u32 = 15;

/// DDG SERP page size used for auto-pagination (`ceil(num/page_size)`).
pub const FACTORY_SERP_PAGE_SIZE: u32 = 10;

/// Max pages auto-pagination will raise to (aligned with `PageCount` bound).
pub const FACTORY_MAX_AUTO_PAGES: u32 = 5;
