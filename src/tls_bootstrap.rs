// SPDX-License-Identifier: MIT OR Apache-2.0
//! Process-wide rustls [`CryptoProvider`] bootstrap (Pass 40 / ADR-0021).
//!
//! ## Policy
//!
//! - **Sole provider:** `aws-lc-rs` (performance + post-quantum readiness).
//! - **Install once** at the start of the binary `main`, **before** the Tokio
//!   multi-thread runtime (and before any residual `reqwest` TLS).
//! - **Libraries must not** call [`install_rustls_crypto_provider`] on behalf of
//!   external callers; only the consuming binary installs. Tests use
//!   [`ensure_for_tests`].
//!
//! Production SERP TLS is the Chrome subprocess (ADR-0016). This module only
//! covers residual Rust HTTP (`reqwest` + rustls).

use std::sync::Once;

use rustls::crypto::{aws_lc_rs, CryptoProvider};

use crate::error::CliError;

/// Install the process-default rustls crypto provider (`aws-lc-rs`).
///
/// Idempotent: if a default provider is already installed, returns `Ok(())`.
///
/// # Errors
///
/// Returns [`CliError::InvalidConfig`] when installation fails and no default
/// provider is present afterwards.
pub fn install_rustls_crypto_provider() -> Result<(), CliError> {
    if CryptoProvider::get_default().is_some() {
        return Ok(());
    }
    match aws_lc_rs::default_provider().install_default() {
        Ok(()) => Ok(()),
        Err(_returned_provider) => {
            // Another thread won the race, or a double-call in the same process.
            if CryptoProvider::get_default().is_some() {
                Ok(())
            } else {
                Err(CliError::InvalidConfig {
                    message: "failed to install rustls CryptoProvider (aws-lc-rs)".into(),
                })
            }
        }
    }
}

/// Ensure the crypto provider is installed once per test process.
///
/// Call from unit/integration tests that construct `reqwest::Client` under the
/// `rustls-tls-webpki-roots-no-provider` feature (no bundled provider).
///
/// # Panics
///
/// Panics if installing the process-wide rustls `CryptoProvider` fails (should
/// not happen with the aws-lc-rs feature set used by this crate).
pub fn ensure_for_tests() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if let Err(err) = install_rustls_crypto_provider() {
            panic!("tls_bootstrap::ensure_for_tests: {err}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_is_idempotent_and_sets_default() {
        install_rustls_crypto_provider().expect("first install");
        install_rustls_crypto_provider().expect("second install");
        assert!(
            CryptoProvider::get_default().is_some(),
            "default CryptoProvider must be installed"
        );
    }

    #[test]
    fn ensure_for_tests_is_idempotent() {
        ensure_for_tests();
        ensure_for_tests();
        assert!(CryptoProvider::get_default().is_some());
    }
}
