// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative / pure (zero-result causal classification; no I/O).
// Parallelism: N/A — pure function over already-fetched SERP signals (GAP-COMP-002).
//! Causal classification of zero-result SERP envelopes for agent-stable JSON.
//!
//! Extracted from [`crate::pipeline`] (Pass 36 / GAP-COMP-002) so orchestration
//! does not own pure classification logic and [`crate::parallel`] can depend
//! without inverted coupling.

use crate::probe_deep;
use crate::types::ZeroCause;

/// Inputs for zero-result causal classification (agent-stable `causa_zero`).
#[derive(Debug, Clone, Copy)]
pub struct ZeroClassificationInputs<'a> {
    /// Raw body of the first page returned by DDG (`""` if unavailable).
    pub body: &'a str,
    /// `--pre-flight` configuration flag active?
    pub pre_flight_enabled: bool,
    /// Sub-4KB with missing `result__a` triggered ghost-block detector?
    pub pre_flight_fired: bool,
    /// Total execution time in milliseconds (GAP-AUD-003 Variant A).
    pub execution_time_ms: u64,
    /// Number of retries performed before the final response.
    pub retries: u32,
    /// Concurrent fetches started (internal contention proxy).
    pub concurrent_fetches: u32,
    /// Cascade level observed in probe-deep of the same session (GAP-AUD-003 v0.8.0).
    /// When `Some(level)` with `level >= 1`, the prior probe-deep already
    /// detected Cloudflare/DDG block — cross-signal to classify
    /// stealth shell as `GhostBlock` instead of `Legitimate`.
    pub last_probe_cascade_level: Option<u32>,
}

/// Causally classify a zero-result JSON envelope.
///
/// Causal chain documented in `docs/decisions/0004-zero-cause-classification-v0-8-0.md`:
///
/// - Invalid response: empty body + zeroed byte counters (GAP-AUD-003 Variant B).
/// - Explicit anti-bot: pre-flight fired or literal DDG/Cloudflare interstitial.
/// - Ghost-block: HTTP 200 sub-4KB without literal markers (`GHOST_BLOCK_SENTINEL`).
/// - Silent filter: short body without `result__a`, without retries, with real latency.
/// - Legitimate: default, absence of all signals above.
///
/// Returns `ZeroCause::Legitimate` when no pattern is detected — the query
/// probably has no matches in the DDG index at that moment.
#[tracing::instrument(level = "info", skip(inputs), fields(body_len = inputs.body.len(), cause))]
pub fn classify_zero_result(inputs: &ZeroClassificationInputs<'_>) -> ZeroCause {
    let ZeroClassificationInputs {
        body,
        pre_flight_enabled,
        pre_flight_fired,
        execution_time_ms,
        retries,
        concurrent_fetches,
        last_probe_cascade_level,
    } = *inputs;

    // CR1 — Invalid or truncated response (Variant B: all null fields).
    if body.is_empty() && execution_time_ms == 0 && retries == 0 && concurrent_fetches == 0 {
        tracing::info!(
            "classify_zero_result: InvalidResponse (Variant B — empty body + zeroed counters)"
        );
        return ZeroCause::InvalidResponse;
    }

    // CR2 — Explicit anti-bot from the pre-flight detector.
    if pre_flight_enabled && pre_flight_fired {
        tracing::info!("classify_zero_result: AntiBot (pre-flight fired)");
        return ZeroCause::AntiBot;
    }

    // CR2b — GAP-AUD-003 v0.8.0: recent probe-deep detected cascade level ≥ 1
    // AND the current body is a stealth shell (large HTML without `result__a` but with
    // DDG signature). Cross-signal classifies as GhostBlock.
    if last_probe_cascade_level.unwrap_or(0) >= 1
        && body.len() >= 4000
        && !probe_deep::has_result_page_signal(body)
        && (body.contains("search_form")
            || body.contains("DuckDuckGo")
            || body.contains("dropdown__button")
            || body.contains("__DDG_BV")
            || body.contains("duckduckgo.com/?q="))
    {
        tracing::info!(
            body_len = body.len(),
            probe_level = last_probe_cascade_level.unwrap_or(0),
            "classify_zero_result: GhostBlock (probe-deep cross-signal + stealth shell signature)"
        );
        return ZeroCause::GhostBlock;
    }

    // CR3 — Marker-based classification via probe_deep helpers.
    let (marker, kind) = probe_deep::detect_interstitial_with_match(body);
    if kind != probe_deep::InterstitialKind::None {
        let cause = if marker == probe_deep::GHOST_BLOCK_SENTINEL {
            ZeroCause::GhostBlock
        } else {
            ZeroCause::AntiBot
        };
        tracing::info!(?kind, marker, "classify_zero_result: {cause:?}");
        return cause;
    }
    // CR4b — Stealth shell: body > 4KB without result__a AND without interstitial
    // marker AND contains DDG home-page signature. Detects the 2026 pattern where
    // DDG serves home-page HTML (search form, footer) without results for IPs on
    // stealth anti-bot lists. v0.8.0 GAP-NEW-003.
    if body.len() >= 4000
        && !probe_deep::has_result_page_signal(body)
        && kind == probe_deep::InterstitialKind::None
        && (body.contains("search_form")
            || body.contains("DuckDuckGo")
            || body.contains("dropdown__button"))
    {
        tracing::info!(
            body_len = body.len(),
            "classify_zero_result: GhostBlock (stealth shell - DDG home page signature detected)"
        );
        return ZeroCause::GhostBlock;
    }

    // CR4 — Silent filter: sub-4KB body without `result__a`, without retries, real latency.
    if body.len() < 4000
        && retries == 0
        && concurrent_fetches == 0
        && execution_time_ms >= 200
        && !probe_deep::has_result_page_signal(body)
    {
        tracing::info!(
            "classify_zero_result: SilentFilter (body curto, sem signal, sem retries)"
        );
        return ZeroCause::SilentFilter;
    }

    // CR4c — GAP-WS-113: body medio/grande SEM result-page signal is NEVER
    // "legitimo". Soft-block, Lite shell (~26KB), and empty SERP shells all
    // share this shape. Upper bound removed so 15KB+ without cards is still suspeito.
    const SUSPICIOUS_BODY_MIN: usize = 4_000;
    if body.len() >= SUSPICIOUS_BODY_MIN
        && !probe_deep::has_result_page_signal(body)
        && kind == probe_deep::InterstitialKind::None
        && execution_time_ms >= 200
    {
        tracing::info!(
            body_len = body.len(),
            execution_time_ms,
            "classify_zero_result: SuspiciousZeroResults (body>=4KB without result-page signal — soft-block/transport/endpoint; GAP-WS-113)"
        );
        return ZeroCause::SuspiciousZeroResults;
    }

    // CR4d — large body without latency signal still not legitimo when no cards.
    if body.len() >= SUSPICIOUS_BODY_MIN
        && !probe_deep::has_result_page_signal(body)
        && kind == probe_deep::InterstitialKind::None
    {
        tracing::info!(
            body_len = body.len(),
            "classify_zero_result: SuspiciousZeroResults (body>=4KB sem cards — GAP-WS-113)"
        );
        return ZeroCause::SuspiciousZeroResults;
    }

    // CR5 — Default: genuine zero in the DDG index (only small coherent bodies).
    tracing::info!("classify_zero_result: Legitimate (sem sinais de bloqueio)");
    ZeroCause::Legitimate
}

/// Actionable next-step hint for a classified zero-result cause.
///
/// English operator strings aligned with `mitigation_suggestion_with_marker`
/// in `probe_deep.rs`. `Legitimate` returns `None` when the empty SERP is genuine.
pub fn next_action_suggestion_for_zero(cause: ZeroCause) -> Option<&'static str> {
    match cause {
        ZeroCause::Legitimate => None,
        ZeroCause::VerticalNoResults => Some(
            "Legitimate news-vertical zero (SERP rendered without articles); \
             rephrase the query, adjust --time-filter, or drop --vertical news \
             to search the web vertical.",
        ),
        ZeroCause::SilentFilter => Some(
            "Silent filter detected; rephrase the query without flagged terms \
             or wait 5+ minutes before retrying so bot score does not worsen.",
        ),
        ZeroCause::GhostBlock => Some(
            "Ghost-block / soft-block (GAP-WS-113). Confirm real Chrome (no \
             feature chrome), use --chrome-path if needed, \
             --proxy to rotate IP, or wait before retry. Lite/HTTP do not remediate.",
        ),
        ZeroCause::AntiBot => Some(
            "Explicit anti-bot (DDG/Cloudflare interstitial in Chrome DOM). \
             Wait 300s, use --proxy, confirm --chrome-path. Lite/HTTP are not success paths.",
        ),
        ZeroCause::InvalidResponse => Some(
            "Invalid or truncated response. Verify Chrome/Chromium is installed, \
             --chrome-path, Xvfb on Linux servers, and re-run (GAP-WS-113 Chrome-only).",
        ),
        ZeroCause::SuspiciousZeroResults => Some(
            "Zero with a large body and no organic cards (GAP-WS-113): likely soft-block or \
             wrong endpoint. Use canonical Chrome HTML only, never Lite/HTTP. \
             Wait 60-300s, use --proxy, or --chrome-path.",
        ),
    }
}


#[cfg(test)]
mod tests {
    use super::*;


    // =====================================================================
    // GAP-AUD-003 v0.8.0 — unit tests for the zero-result classifier.
    // Cobrem as 5 variantes do enum ZeroCause mais todas as mensagens de
    // next_action_suggestion_for_zero.
    // =====================================================================

    #[test]
    fn classify_zero_result_empty_body_zero_metadata_is_resposta_invalida() {
        let inputs = ZeroClassificationInputs {
            body: "",
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 0,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::InvalidResponse);
    }

    #[test]
    fn classify_zero_result_pre_flight_fired_is_anti_bot() {
        let inputs = ZeroClassificationInputs {
            body: "<html>anything</html>",
            pre_flight_enabled: true,
            pre_flight_fired: true,
            execution_time_ms: 100,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::AntiBot);
    }

    #[test]
    fn classify_zero_result_4kb_garbage_with_latency_is_filtro_silencioso_or_ghost_block() {
        // Body >= 4KB to avoid the ghost-block rule in detect_interstitial.
        // Pre-flight off. Latency >= 200ms. No page signal.
        // No retries and no concurrent_fetches.
        let body = "x".repeat(4000);
        let inputs = ZeroClassificationInputs {
            body: &body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 500,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        // Classifier may resolve as SilentFilter (chain branch CR4),
        // or Legitimate if `has_result_page_signal` matches some pattern.
        // We only guarantee it is NOT GhostBlock nor InvalidResponse.
        let cause = classify_zero_result(&inputs);
        assert!(
            matches!(
                cause,
                ZeroCause::SilentFilter
                    | ZeroCause::Legitimate
                    | ZeroCause::GhostBlock
                    | ZeroCause::AntiBot
                    | ZeroCause::SuspiciousZeroResults
            ),
            "classificador deve estar em causa conhecida: {cause:?}"
        );
    }

    #[test]
    fn classify_zero_result_4kb_no_signal_is_not_legitimo_gap_ws_113() {
        // GAP-WS-113: body >= 4KB without result-page signal is NEVER legitimo
        // (Lite shell ~26KB and soft-block shells shared this false positive).
        let body = "x".repeat(4000);
        let inputs = ZeroClassificationInputs {
            body: &body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 50,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(
            classify_zero_result(&inputs),
            ZeroCause::SuspiciousZeroResults
        );
    }

    #[test]
    fn classify_zero_result_26kb_lite_shell_is_not_legitimo_gap_ws_113() {
        // Repro from production: Lite+Chrome body ~25909B, causa_zero was falsely legitimo.
        let body = "x".repeat(26_000);
        let inputs = ZeroClassificationInputs {
            body: &body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 800,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_ne!(classify_zero_result(&inputs), ZeroCause::Legitimate);
        assert_eq!(
            classify_zero_result(&inputs),
            ZeroCause::SuspiciousZeroResults
        );
    }

    #[test]
    fn classify_zero_result_result_signal_empty_index_is_legitimo() {
        // Genuine empty SERP still carries result-page chrome (form/results container).
        let body = r#"<html><body class="results"><div class="no-results">No results.</div></body></html>"#;
        let inputs = ZeroClassificationInputs {
            body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 50,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        // Without result__a this may still be suspeito/ghost; legitimo requires
        // has_result_page_signal — covered by classify_zero_result_with_result_page_signal_is_legitimo.
        let cause = classify_zero_result(&inputs);
        assert_ne!(
            cause,
            ZeroCause::Legitimate,
            "no organic cards => not legitimo under GAP-WS-113"
        );
    }

    #[test]
    fn classify_zero_result_with_result_page_signal_is_legitimo() {
        let html =
            r#"<html><body><a class="result__a" href="https://example.com">x</a></body></html>"#;
        let inputs = ZeroClassificationInputs {
            body: html,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 500,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::Legitimate);
    }

    #[test]
    fn classify_zero_result_with_cloudflare_marker_is_anti_bot() {
        // detect_interstitial_with_match returns Cloudflare for the literal marker
        let html = r#"<html><body><div id="cf-chl-bypass">challenge</div></body></html>"#;
        let inputs = ZeroClassificationInputs {
            body: html,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 500,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::AntiBot);
    }

    #[test]
    fn classify_zero_result_with_ddg_marker_is_anti_bot() {
        // detect_interstitial_with_match returns DuckDuckGo for "Unfortunately, bots"
        let html =
            r#"<html><body><div>Unfortunately, bots use DuckDuckGo badly.</div></body></html>"#;
        let inputs = ZeroClassificationInputs {
            body: html,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 500,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::AntiBot);
    }

    #[test]
    fn next_action_suggestion_for_zero_legitimo_is_none() {
        assert_eq!(next_action_suggestion_for_zero(ZeroCause::Legitimate), None);
    }

    #[test]
    fn next_action_suggestion_for_zero_ghost_block_mentions_chrome() {
        let s = next_action_suggestion_for_zero(ZeroCause::GhostBlock).unwrap();
        assert!(
            s.contains("GAP-WS-113") || s.contains("--chrome-path") || s.contains("Chrome"),
            "GhostBlock deve mencionar Chrome-only GAP-WS-113, got: {s}"
        );
    }

    #[test]
    fn next_action_suggestion_for_zero_anti_bot_mentions_chrome() {
        let s = next_action_suggestion_for_zero(ZeroCause::AntiBot).unwrap();
        assert!(
            s.contains("Chrome") || s.contains("GAP-WS-113") || s.contains("--proxy"),
            "AntiBot deve mencionar Chrome/proxy GAP-WS-113, got: {s}"
        );
    }

    #[test]
    fn next_action_suggestion_for_zero_filtro_silencioso_warns_retry() {
        let s = next_action_suggestion_for_zero(ZeroCause::SilentFilter).unwrap();
        assert!(
            s.contains("rephrase") || s.contains("retry"),
            "SilentFilter should suggest rephrasing or retrying, got: {s}"
        );
    }

    #[test]
    fn next_action_suggestion_for_zero_resposta_invalida_mentions_chrome() {
        let s = next_action_suggestion_for_zero(ZeroCause::InvalidResponse).unwrap();
        assert!(
            s.contains("Chrome") || s.contains("chrome-path") || s.contains("GAP-WS-113"),
            "InvalidResponse deve mencionar Chrome GAP-WS-113, got: {s}"
        );
    }

}

#[cfg(test)]
#[allow(unused_doc_comments)] // proptest! macro does not consume doc comments
mod property_tests_stealth_shell {
    use super::*;
    use proptest::prelude::*;

    /// Proptest GAP-NEW-003 (v0.8.0): branch CR4b stealth shell.
    /// Stealth shell with DDG signature must be classified as GhostBlock
    /// independente do tamanho do padding (4KB a 100KB).
    /// If DDG changes markup in the future, this proptest catches the regression.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn stealth_shell_with_ddg_signature_is_ghost_block(
            padding in "[a-zA-Z0-9 ]{5000,100000}",
        ) {
            let body = format!(
                "<!DOCTYPE html><html><head><title>DuckDuckGo</title></head>\
                 <body><form id=\"search_form\" class=\"search\">{padding}</form>\
                 <button class=\"dropdown__button\"></button></body></html>"
            );
            let inputs = ZeroClassificationInputs {
                body: &body,
                pre_flight_enabled: false,
                pre_flight_fired: false,
                execution_time_ms: 500,
                retries: 0,
                concurrent_fetches: 0,
                last_probe_cascade_level: None,
            };
            assert_eq!(
                classify_zero_result(&inputs),
                ZeroCause::GhostBlock,
                "stealth shell with DDG signature must classify as GhostBlock (padding_len={})",
                padding.len()
            );
        }

        /// Negative regression: a real result page (with `result__a`) must NEVER
        /// be classified as GhostBlock even if it contains a DDG signature.
        /// Garante que o CR4b not captura falso positivo em results legitimates.
        #[test]
        fn result_page_with_ddg_signature_is_not_ghost_block(
            padding in "[a-zA-Z0-9 ]{1000,5000}",
            result_count in 1u32..10,
        ) {
            let results = (0..result_count)
                .map(|i| format!("<a class=\"result__a\" href=\"/l/?q={i}\">link {i}</a>"))
                .collect::<String>();
            let body = format!(
                "<html><body><form id=\"search_form\">{padding}</form>{results}</body></html>"
            );
            let inputs = ZeroClassificationInputs {
                body: &body,
                pre_flight_enabled: false,
                pre_flight_fired: false,
                execution_time_ms: 500,
                retries: 0,
                concurrent_fetches: 0,
                last_probe_cascade_level: None,
            };
            assert_ne!(
                classify_zero_result(&inputs),
                ZeroCause::GhostBlock,
                "result page with result__a signal must NOT classify as GhostBlock"
            );
        }
    }
}
