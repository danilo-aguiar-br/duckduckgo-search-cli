// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (gzip/deflate decode) offloaded via `run_cpu_bound`.
// Bottleneck: inflate CPU, not network. Tokio worker must not block.
// Parallelism: NOT rayon — one body per task; multi-query already uses
// JoinSet+Semaphore. Rayon would only fight the blocking pool.
// GAP-PAR-017/039: admit via process-wide `blocking_cpu_semaphore` through
// `concurrency::run_cpu_bound` (same gate as SERP extract / readability).
// Memory: hard caps on compressed input and decompressed output (gzip bomb).
// Latency: success-path logs at debug (release_max_level_info strips them in
// release); measure via decompress_bench, not wall-clock search RTT.
//! Transparent decompression of HTTP response bodies.
//!
//! The HTTP client enables the `gzip` and `deflate` features but does NOT
//! always decompress the body returned by `Response::text()` or `Response::bytes()`.
//! The DDG upstream always replies with `Content-Encoding: gzip` for HTML
//! responses, so the body reaching the caller is a stream of gzip-compressed
//! bytes (~9 KB instead of the ~14 KB plain text body). Downstream consumers
//! like `detect_interstitial_with_match` perform literal substring searches
//! (e.g. `body.contains("anomaly-modal")`) and fail on compressed bytes — the
//! root cause of GAP-AUD-003 being inoperant in production.
//!
//! This module streams response bodies with a hard byte cap (never unbounded
//! `read_to_end` / `bytes()` alone) and inspects `Content-Encoding` to
//! dispatch to the correct decoder. Safety caps:
//! - [`DECOMPRESSION_MAX_INPUT`] — compressed wire body (Content-Length + stream)
//! - [`DECOMPRESSION_MAX_OUTPUT`] — decompressed bytes (gzip bomb protection)

use std::io::Read;

use crate::error::CliError;

/// Maximum number of compressed wire bytes accepted before decode.
///
/// Protects against unbounded heap growth when the peer omits or lies about
/// `Content-Length`. Legitimate DuckDuckGo SERP HTML is typically well under
/// 1 MiB compressed; 16 MiB leaves headroom without permitting multi-GB buffers.
pub const DECOMPRESSION_MAX_INPUT: usize = 16 * 1024 * 1024;

/// Maximum number of bytes accepted after decompression.
///
/// Protects against gzip bombs: an attacker serves a small body that
/// decompresses to gigabytes. Set to 32 MiB which is well above the
/// largest legitimate HTML result page but bounded enough to abort a
/// bomb attempt before exhausting memory.
pub const DECOMPRESSION_MAX_OUTPUT: usize = 32 * 1024 * 1024;

/// Initial capacity for the decode buffer: `min(3 * compressed_len, cap)`.
///
/// Avoids `len * 3` overflow on huge inputs and never reserves more than
/// the decompression cap.
fn decode_buffer_capacity(compressed_len: usize) -> usize {
    compressed_len
        .saturating_mul(3)
        .min(DECOMPRESSION_MAX_OUTPUT)
}

/// Fallible pre-allocation for the decode buffer (untrusted wire input).
///
/// Caps already bound `cap`; `try_reserve` turns allocator failure into a
/// domain `Result` instead of aborting the process on OOM-capable platforms.
fn reserve_decode_buf(cap: usize) -> Result<Vec<u8>, CliError> {
    let mut out = Vec::new();
    out.try_reserve(cap).map_err(|err| {
        CliError::decompression_io(std::io::Error::other(format!(
            "failed to reserve {cap} bytes for decompression: {err}"
        )))
    })?;
    Ok(out)
}

/// Rejects a wire/decompressed size that exceeds `max`.
fn reject_if_too_large(actual: usize, max: usize) -> Result<(), CliError> {
    if actual > max {
        return Err(CliError::PayloadTooLarge { max, actual });
    }
    Ok(())
}

/// Streams a response body with a hard wire-byte cap (rules-rust rede I/O).
///
/// Never calls unbounded `bytes()` alone: reads via `chunk()` and aborts as
/// soon as the accumulated size exceeds `max_bytes`. Also rejects when
/// `Content-Length` (if present) advertises a body larger than `max_bytes`.
///
/// # Errors
///
/// - [`CliError::PayloadTooLarge`] when the peer exceeds `max_bytes`
/// - [`CliError::HttpClient`] on transport failure mid-stream
pub async fn read_body_capped(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, CliError> {
    // Early reject when Content-Length advertises an oversize wire body.
    if let Some(cl) = response.headers().get(reqwest::header::CONTENT_LENGTH) {
        if let Ok(size_str) = cl.to_str() {
            if let Ok(size) = size_str.parse::<u64>() {
                if size > max_bytes as u64 {
                    return Err(CliError::PayloadTooLarge {
                        max: max_bytes,
                        actual: usize::try_from(size).unwrap_or(usize::MAX),
                    });
                }
            }
        }
    }

    // Pre-size conservatively from Content-Length when present and honest.
    let mut acc = Vec::new();
    if let Some(cl) = response.headers().get(reqwest::header::CONTENT_LENGTH) {
        if let Ok(size_str) = cl.to_str() {
            if let Ok(size) = size_str.parse::<usize>() {
                let reserve = size.min(max_bytes);
                acc.try_reserve(reserve).map_err(|err| {
                    CliError::decompression_io(std::io::Error::other(format!(
                        "failed to reserve {reserve} bytes for response body: {err}"
                    )))
                })?;
            }
        }
    }

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(CliError::http_client)?
    {
        let next = acc.len().saturating_add(chunk.len());
        if next > max_bytes {
            return Err(CliError::PayloadTooLarge {
                max: max_bytes,
                actual: next,
            });
        }
        acc.try_reserve(chunk.len()).map_err(|err| {
            CliError::decompression_io(std::io::Error::other(format!(
                "failed to grow response body buffer by {}: {err}",
                chunk.len()
            )))
        })?;
        acc.extend_from_slice(&chunk);
    }
    Ok(acc)
}

/// Reads the response body and returns the decoded UTF-8 string.
///
/// Inspects the `Content-Encoding` header and dispatches:
/// - `identity` (or absent) — passes bytes through unchanged.
/// - `gzip` — decodes via [`flate2::read::MultiGzDecoder`] (handles
///   concatenated gzip streams transparently).
/// - `deflate` — decodes via [`flate2::read::ZlibDecoder`].
/// - `br` — returns [`CliError::UnsupportedEncoding`] (brotli removed in
///   v0.8.6 with the wreq-to-reqwest migration; `DuckDuckGo` never serves brotli for HTML endpoints).
/// - Anything else — [`CliError::UnsupportedEncoding`].
///
/// Returns [`CliError::PayloadTooLarge`] if the compressed body exceeds
/// [`DECOMPRESSION_MAX_INPUT`] (via stream cap) or if decompression exceeds
/// [`DECOMPRESSION_MAX_OUTPUT`].
/// Returns [`CliError::InvalidUtf8`] if the decoded bytes are not valid UTF-8.
///
/// # Errors
///
/// - [`CliError::HttpClient`] if reading the response body fails at the
///   transport layer (DNS, TLS, connection reset).
/// - [`CliError::UnsupportedEncoding`] if the `Content-Encoding` header
///   carries an encoding this module does not handle (e.g. `zstd`).
/// - [`CliError::PayloadTooLarge`] if compressed or decompressed size exceeds
///   the configured safety caps.
/// - [`CliError::DecompressionIo`] if the decoder returns an I/O error
///   (corrupt stream, truncated payload).
/// - [`CliError::InvalidUtf8`] if the decoded bytes are not valid UTF-8.
pub async fn response_body_string(response: reqwest::Response) -> Result<String, CliError> {
    let encoding = response
        .headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("identity")
        .to_ascii_lowercase();

    let bytes = read_body_capped(response, DECOMPRESSION_MAX_INPUT).await?;

    // GAP-PAR-039: flate2 is CPU-bound — admit via central `run_cpu_bound`
    // (spawn_blocking + blocking_cpu_semaphore + JoinError is_panic/cancel).
    let decoded = crate::concurrency::run_cpu_bound({
        let encoding = encoding.clone();
        move || decode_bytes(&bytes, &encoding)
    })
    .await??;
    String::from_utf8(decoded).map_err(CliError::from)
}

/// Decompresses raw bytes using the given `Content-Encoding` value.
///
/// Public so call sites that already hold a `Vec<u8>` (e.g.
/// [`crate::content`]) can reuse the same decoder dispatch without
/// re-reading the response. `encoding` is expected to already be
/// lowercased by the caller; if not, this function lowercases it.
///
/// # Errors
///
/// - [`CliError::UnsupportedEncoding`] for encodings this module does
///   not handle.
/// - [`CliError::PayloadTooLarge`] if compressed input exceeds
///   [`DECOMPRESSION_MAX_INPUT`] or decompressed output exceeds
///   [`DECOMPRESSION_MAX_OUTPUT`].
/// - [`CliError::DecompressionIo`] for corrupt or truncated streams.
pub fn decode_bytes(bytes: &[u8], encoding: &str) -> Result<Vec<u8>, CliError> {
    reject_if_too_large(bytes.len(), DECOMPRESSION_MAX_INPUT)?;

    let encoding = encoding.to_ascii_lowercase();
    let decoded = match encoding.as_str() {
        "identity" | "" => {
            // Fallible reserve before copy — same budget as compressed input.
            let mut owned = reserve_decode_buf(bytes.len())?;
            owned.extend_from_slice(bytes);
            owned
        }
        "gzip" => {
            let out = reserve_decode_buf(decode_buffer_capacity(bytes.len()))?;
            decode_with_cap(bytes, out, |slice, mut out| {
                flate2::read::MultiGzDecoder::new(slice)
                    .take(u64::try_from(DECOMPRESSION_MAX_OUTPUT).unwrap_or(u64::MAX) + 1)
                    .read_to_end(&mut out)?;
                Ok(out)
            })?
        }
        "deflate" => {
            let out = reserve_decode_buf(decode_buffer_capacity(bytes.len()))?;
            decode_with_cap(bytes, out, |slice, mut out| {
                flate2::read::ZlibDecoder::new(slice)
                    .take(u64::try_from(DECOMPRESSION_MAX_OUTPUT).unwrap_or(u64::MAX) + 1)
                    .read_to_end(&mut out)?;
                Ok(out)
            })?
        }
        "br" => {
            return Err(CliError::UnsupportedEncoding(
                "br (brotli removed in v0.8.6)".to_string(),
            ))
        }
        other => return Err(CliError::UnsupportedEncoding(other.to_string())),
    };

    let bytes_in = bytes.len();
    let bytes_out = decoded.len();
    // Hot path: every successful body hits this once — debug only to avoid
    // info-level format/subscriber cost under RUST_LOG=info (latency rules).
    tracing::debug!(
        encoding = %encoding,
        bytes_in,
        bytes_out,
        "decompressed response body"
    );

    Ok(decoded)
}

/// Helper that runs a decoder closure and enforces the [`DECOMPRESSION_MAX_OUTPUT`] cap.
///
/// The caller pre-allocates `out` via [`reserve_decode_buf`] (fallible). The
/// closure MUST honor the `.take(cap + 1)` semantic so we can detect when the
/// stream exceeds the cap and abort cleanly. If the returned `Vec` is exactly
/// `cap + 1` long, the cap was hit and we return [`CliError::PayloadTooLarge`]
/// with the actual size reported as `cap + 1` (a lower bound on the true size).
fn decode_with_cap<F>(bytes: &[u8], out: Vec<u8>, decoder: F) -> Result<Vec<u8>, CliError>
where
    F: FnOnce(&[u8], Vec<u8>) -> std::io::Result<Vec<u8>>,
{
    let out = decoder(bytes, out).map_err(CliError::decompression_io)?;
    reject_if_too_large(out.len(), DECOMPRESSION_MAX_OUTPUT)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_constants_are_documented_sizes() {
        assert_eq!(DECOMPRESSION_MAX_INPUT, 16 * 1024 * 1024);
        assert_eq!(DECOMPRESSION_MAX_OUTPUT, 32 * 1024 * 1024);
        assert!(DECOMPRESSION_MAX_INPUT <= DECOMPRESSION_MAX_OUTPUT);
    }

    #[tokio::test]
    async fn read_body_capped_rejects_stream_over_limit() {
        // Stream a real body larger than the cap — aborts mid-read without
        // buffering the full payload (hyper rejects lying Content-Length).
        let server = wiremock::MockServer::start().await;
        let big = vec![b'x'; 2048];
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes(big))
            .mount(&server)
            .await;
        crate::tls_bootstrap::ensure_for_tests();
        let client = reqwest::Client::new();
        let response = client
            .get(server.uri())
            .send()
            .await
            .expect("send");
        let err = read_body_capped(response, 1024)
            .await
            .expect_err("must reject body over cap");
        match err {
            CliError::PayloadTooLarge { max, actual } => {
                assert_eq!(max, 1024);
                assert!(actual > 1024);
            }
            other => panic!("expected PayloadTooLarge, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_body_capped_streams_small_body() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("hello-capped"))
            .mount(&server)
            .await;
        crate::tls_bootstrap::ensure_for_tests();
        let client = reqwest::Client::new();
        let response = client
            .get(server.uri())
            .send()
            .await
            .expect("send");
        let body = read_body_capped(response, 64 * 1024)
            .await
            .expect("small body");
        assert_eq!(body, b"hello-capped");
    }

    #[test]
    fn decode_buffer_capacity_never_exceeds_output_cap() {
        assert_eq!(decode_buffer_capacity(0), 0);
        assert_eq!(decode_buffer_capacity(100), 300);
        assert_eq!(
            decode_buffer_capacity(DECOMPRESSION_MAX_OUTPUT),
            DECOMPRESSION_MAX_OUTPUT
        );
        // Saturating mul must not panic or overflow past the output cap.
        assert_eq!(
            decode_buffer_capacity(usize::MAX / 2),
            DECOMPRESSION_MAX_OUTPUT
        );
    }

    #[test]
    fn reserve_decode_buf_succeeds_for_capped_sizes() {
        let buf = reserve_decode_buf(1024).expect("small reserve");
        assert!(buf.capacity() >= 1024);
        let buf = reserve_decode_buf(0).expect("zero reserve");
        assert_eq!(buf.capacity(), 0);
    }

    #[test]
    fn identity_decode_preserves_bytes_via_try_reserve() {
        let input = b"hello-identity";
        let out = decode_bytes(input, "identity").expect("identity");
        assert_eq!(out, input);
    }

    #[test]
    fn decode_bytes_rejects_oversize_compressed_input() {
        let huge = vec![0u8; DECOMPRESSION_MAX_INPUT + 1];
        match decode_bytes(&huge, "identity") {
            Err(CliError::PayloadTooLarge { max, actual }) => {
                assert_eq!(max, DECOMPRESSION_MAX_INPUT);
                assert_eq!(actual, huge.len());
            }
            other => panic!("expected PayloadTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn decoder_helper_rejects_oversize_payload() {
        // Build a payload that the decoder closure reports as 2x the cap.
        let oversize = vec![0u8; DECOMPRESSION_MAX_OUTPUT + 1024];
        let result = decode_with_cap(&[], Vec::new(), |_, _| Ok(oversize.clone()));
        match result {
            Err(CliError::PayloadTooLarge { max, actual }) => {
                assert_eq!(max, DECOMPRESSION_MAX_OUTPUT);
                assert_eq!(actual, oversize.len());
            }
            other => panic!("expected PayloadTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn decoder_helper_accepts_undersize_payload() {
        let small = vec![0u8; 1024];
        let result = decode_with_cap(&[], Vec::new(), |_, _| Ok(small.clone()));
        assert_eq!(result.unwrap().len(), 1024);
    }
}
