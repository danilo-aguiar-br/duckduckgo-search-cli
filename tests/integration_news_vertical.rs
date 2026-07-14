// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes de integração da vertical de notícias (GAP-WS-104 v0.8.9).
//!
//! Cobrem o comportamento observável de ponta a ponta sem rede:
//! - extração das 3 fixtures da SERP news (`tests/fixtures/ddg_news_serp*.html`)
//! - contrato JSON do envelope (`noticias[]`, `quantidade_noticias`,
//!   `vertical_usada`) e compatibilidade byte-idêntica do modo `web`
//! - construção da URL da vertical (`ia=news&iar=news`) com override por env
//! - guardas de configuração exercitadas via binário real (exit 2)
//! - round-trip serde do `ZeroCause::VerticalSemResultados` em kebab-case
//!
//! A pertinência do `VerticalSemResultados` à lista de zeros LEGÍTIMOS
//! (exit 5, nunca 6) é coberta pelo teste unitário
//! `lib::tests::vertical_sem_resultados_is_legitimo_zero` — a função
//! `zero_cause_is_non_legitimo` é privada por design.

use duckduckgo_search_cli::extraction::extract_news_results_with_cfg;
use duckduckgo_search_cli::search::build_news_search_url;
use duckduckgo_search_cli::types::{
    NewsResult, SafeSearch, SearchMetadata, SearchOutput, SelectorConfig, ZeroCause,
};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// `build_news_search_url` lê `DUCKDUCKGO_SEARCH_CLI_BASE_URL_SERP` via
/// `serp_base_url()`; `std::env::set_var` não é thread-safe, então TODOS os
/// testes que constroem URLs serializam o acesso ao ambiente por este lock.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn load_fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path.push(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("falha ao ler fixture {}: {e}", path.display()))
}

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_duckduckgo-search-cli")
}

fn metadata_stub() -> SearchMetadata {
    SearchMetadata {
        execution_time_ms: 0,
        selectors_hash: "abc123".to_string(),
        retries: 0,
        retries_configured: None,
        used_fallback_endpoint: false,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: false,
        chrome_attempted: false,
        user_agent: "Mozilla/5.0".to_string(),
        used_proxy: false,
        identity_used: None,
        cascade_level: None,
        pre_flight_fired: false,
        zero_cause: None,
        sugestao_proxima_acao: None,
        bytes_raw: None,
        bytes_decompressed: None,
        cascade_level_observed: None,
        result_count_compat: None,
        endpoint_used_compat: None,
        vertical_used: None,
        chrome_path_resolved: None,
        chrome_channel: None,
    }
}

fn output_stub() -> SearchOutput {
    SearchOutput {
        query: "noticias brasil".to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp: "2026-07-06T00:00:00Z".to_string(),
        region: "br-pt".to_string(),
        result_count: 0,
        results: vec![],
        pages_fetched: 1,
        error: None,
        message: None,
        metadata: metadata_stub(),
        news: None,
        news_count: None,
    }
}

// ---------------------------------------------------------------------------
// 1. Extração das fixtures
// ---------------------------------------------------------------------------

#[test]
fn fixture_estrategia_a_extrai_externos_unicos_com_todos_os_campos() {
    let cfg = SelectorConfig::default();
    let html = load_fixture("ddg_news_serp.html");
    let results = extract_news_results_with_cfg(&html, &cfg);

    // A fixture tem 6 <article>: 4 externos únicos + 1 armadilha interna
    // duckduckgo.com (filtrada) + 1 URL duplicada (deduplicada).
    assert_eq!(
        results.len(),
        4,
        "esperados 4 resultados externos únicos, obtidos {}",
        results.len()
    );
    assert!(
        results.iter().all(|r| !r.url.contains("duckduckgo.com")),
        "a armadilha interna duckduckgo.com deve ser descartada"
    );

    // Campos completos do primeiro card (data relativa PT).
    let first = &results[0];
    assert_eq!(first.position, 1);
    assert_eq!(
        first.title,
        "Governo anuncia novo pacote de investimentos em infraestrutura"
    );
    assert_eq!(first.url, "https://exemplo-veiculo-1.com/artigo-1");
    assert_eq!(first.source.as_deref(), Some("G1"));
    assert_eq!(first.relative_date.as_deref(), Some("há 2 horas"));
    let thumb = first.thumbnail.as_deref().expect("thumbnail presente");
    assert!(
        thumb.starts_with("https://"),
        "thumbnail protocol-relative deve ser resolvida para https, obtido {thumb:?}"
    );

    // Data relativa EN no segundo card.
    assert_eq!(results[1].source.as_deref(), Some("Reuters"));
    assert_eq!(results[1].relative_date.as_deref(), Some("3 hours ago"));

    // Posições densas 1-indexed após filtro e dedupe.
    for (i, r) in results.iter().enumerate() {
        assert_eq!(
            r.position,
            u32::try_from(i + 1).expect("posição cabe em u32")
        );
        assert!(
            !r.title.is_empty(),
            "título vazio na posição {}",
            r.position
        );
        assert!(
            r.url.starts_with("https://") || r.url.starts_with("http://"),
            "URL não absoluta na posição {}: {}",
            r.position,
            r.url
        );
    }
}

#[test]
fn fixture_ofuscada_cai_para_estrategia_b_e_extrai_titulo_e_url() {
    let cfg = SelectorConfig::default();
    let html = load_fixture("ddg_news_serp_ofuscada.html");
    let results = extract_news_results_with_cfg(&html, &cfg);

    // Sem <article>/<h3> e com classes 100% ofuscadas, somente a
    // Estratégia B (agnóstica de classe) recupera os cards.
    assert!(
        !results.is_empty(),
        "Estratégia B deve extrair da fixture ofuscada"
    );
    assert_eq!(results.len(), 3);
    assert_eq!(
        results[0].title,
        "Prefeitura confirma cronograma de obras no centro da cidade"
    );
    assert_eq!(results[0].url, "https://exemplo-veiculo-5.com/nota-5");
    for (i, r) in results.iter().enumerate() {
        assert_eq!(
            r.position,
            u32::try_from(i + 1).expect("posição cabe em u32")
        );
        assert!(!r.title.is_empty());
        assert!(!r.url.is_empty());
    }
}

#[test]
fn fixture_vazia_retorna_vec_vazio() {
    let cfg = SelectorConfig::default();
    let html = load_fixture("ddg_news_serp_vazia.html");
    let results = extract_news_results_with_cfg(&html, &cfg);
    assert!(
        results.is_empty(),
        "container presente sem articles deve produzir vec vazio"
    );
}

// ---------------------------------------------------------------------------
// 2. Contrato JSON do envelope
// ---------------------------------------------------------------------------

#[test]
fn envelope_news_serializa_renames_pt_br() {
    let mut output = output_stub();
    output.news = Some(vec![NewsResult {
        position: 1,
        title: "Manchete".to_string(),
        url: "https://veiculo.com/artigo".to_string(),
        source: Some("G1".to_string()),
        relative_date: Some("há 2 horas".to_string()),
        thumbnail: Some("https://img.example/t.jpg".to_string()),
        content: None,
        content_size: None,
        content_extraction_method: None,
    }]);
    output.news_count = Some(1);
    output.metadata.vertical_used = Some("news".to_string());

    let json = serde_json::to_string(&output).expect("serialização deve funcionar");

    // Campos novos com renames PT-BR.
    assert!(json.contains("\"noticias\""));
    assert!(json.contains("\"quantidade_noticias\":1"));
    assert!(json.contains("\"vertical_usada\":\"news\""));
    assert!(json.contains("\"posicao\":1"));
    assert!(json.contains("\"titulo\":\"Manchete\""));
    assert!(json.contains("\"url\":\"https://veiculo.com/artigo\""));
    assert!(json.contains("\"fonte\":\"G1\""));
    assert!(json.contains("\"data_relativa\":\"há 2 horas\""));
    assert!(json.contains("\"thumbnail\":\"https://img.example/t.jpg\""));

    // Nomes Rust em inglês NÃO devem vazar para o JSON.
    assert!(!json.contains("\"news\":"));
    assert!(!json.contains("\"news_count\""));
    assert!(!json.contains("\"vertical_used\""));
    assert!(!json.contains("\"relative_date\""));
    assert!(!json.contains("\"source\""));
}

#[test]
fn envelope_news_omite_campos_opcionais_ausentes() {
    let mut output = output_stub();
    output.news = Some(vec![NewsResult {
        position: 1,
        title: "Manchete".to_string(),
        url: "https://veiculo.com/artigo".to_string(),
        source: None,
        relative_date: None,
        thumbnail: None,
        content: None,
        content_size: None,
        content_extraction_method: None,
    }]);
    output.news_count = Some(1);

    let json = serde_json::to_string(&output).expect("serialização deve funcionar");
    assert!(!json.contains("\"fonte\""));
    assert!(!json.contains("\"data_relativa\""));
    assert!(!json.contains("\"thumbnail\""));
}

#[test]
fn envelope_modo_web_permanece_byte_compativel_com_v088() {
    let output = output_stub();
    let json = serde_json::to_string(&output).expect("serialização deve funcionar");

    // Modo web default: NENHUM campo novo pode aparecer (contrato
    // byte-idêntico à v0.8.8 para consumidores jaq existentes).
    assert!(!json.contains("\"noticias\":"));
    assert!(!json.contains("\"quantidade_noticias\":"));
    assert!(!json.contains("\"vertical_usada\":"));

    // Round-trip: o envelope antigo continua desserializável.
    let parsed: SearchOutput = serde_json::from_str(&json).expect("round-trip");
    assert!(parsed.news.is_none());
    assert!(parsed.news_count.is_none());
    assert!(parsed.metadata.vertical_used.is_none());
}

// ---------------------------------------------------------------------------
// 3. URL builder da vertical news
// ---------------------------------------------------------------------------

#[test]
fn build_news_search_url_inclui_ia_e_iar_news() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    std::env::remove_var("DUCKDUCKGO_SEARCH_CLI_BASE_URL_SERP");

    let url = build_news_search_url("rust programming", "pt", "br", None, SafeSearch::Moderate);
    assert!(
        url.starts_with("https://duckduckgo.com/"),
        "news deve usar a SERP principal, obtido {url}"
    );
    assert!(url.contains("ia=news&iar=news"), "faltou ia/iar em {url}");
    assert!(url.contains("kl=br-pt"), "faltou kl em {url}");
}

#[test]
fn build_news_search_url_codifica_a_query() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    std::env::remove_var("DUCKDUCKGO_SEARCH_CLI_BASE_URL_SERP");

    let url = build_news_search_url(
        "eleições 2026 & economia",
        "pt",
        "br",
        None,
        SafeSearch::Moderate,
    );
    assert!(
        url.contains("q=elei%C3%A7%C3%B5es%202026%20%26%20economia"),
        "query deve ser URL-encoded, obtido {url}"
    );
    assert!(
        !url.contains("eleições"),
        "query crua vazou para a URL: {url}"
    );
}

#[test]
fn build_news_search_url_respeita_env_de_override() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    std::env::set_var(
        "DUCKDUCKGO_SEARCH_CLI_BASE_URL_SERP",
        "http://127.0.0.1:9/serp",
    );
    let url = build_news_search_url("rust", "pt", "br", None, SafeSearch::Moderate);
    std::env::remove_var("DUCKDUCKGO_SEARCH_CLI_BASE_URL_SERP");

    assert!(
        url.starts_with("http://127.0.0.1:9/serp?q=rust"),
        "env DUCKDUCKGO_SEARCH_CLI_BASE_URL_SERP deve ser respeitada, obtido {url}"
    );
    assert!(url.contains("ia=news&iar=news"));
}

// ---------------------------------------------------------------------------
// 4. Guardas de configuração (via binário real — comportamento observável)
// ---------------------------------------------------------------------------

// GAP-WS-113: NO_CHROME=1 fail-closed (exit 2). Multi-query still allowed.
#[test]
fn binario_news_multi_query_no_chrome_fail_closed() {
    let output = Command::new(bin_path())
        .args(["--vertical", "news", "-q", "-f", "json", "rust", "tokio"])
        .env("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1")
        .stdin(Stdio::null())
        .output()
        .expect("binário deve executar");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stderr.contains("aceita apenas UMA query"),
        "o guard de multi-query deve permanecer removido (GAP-WS-105); stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "GAP-WS-113: NO_CHROME deve falhar com exit 2; stdout={stdout} stderr={stderr}"
    );
}

// GAP-WS-113: deep-research without Chrome fails closed (no auto --no-news).
#[test]
fn binario_deep_research_no_chrome_fail_closed() {
    let output = Command::new(bin_path())
        .args(["deep-research", "rust async"])
        .env("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1")
        .stdin(Stdio::null())
        .output()
        .expect("binário deve executar");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(2),
        "GAP-WS-113: deep-research com NO_CHROME deve exit 2; stdout={stdout}"
    );
}

// GAP-WS-113: --vertical news + NO_CHROME=1 => exit 2 (no silent web downgrade).
#[test]
fn binario_vertical_news_no_chrome_fail_closed() {
    let output = Command::new(bin_path())
        .args(["--vertical", "news", "-q", "-f", "json", "rust"])
        .env("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1")
        .stdin(Stdio::null())
        .output()
        .expect("binário deve executar");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(2),
        "GAP-WS-113: --vertical news com NO_CHROME deve exit 2; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// 5. ZeroCause::VerticalSemResultados
// ---------------------------------------------------------------------------

#[test]
fn zero_cause_vertical_sem_resultados_round_trip_kebab_case() {
    let json = serde_json::to_string(&ZeroCause::VerticalSemResultados)
        .expect("serialização deve funcionar");
    assert_eq!(json, "\"vertical-sem-resultados\"");
    let parsed: ZeroCause = serde_json::from_str(&json).expect("round-trip");
    assert_eq!(parsed, ZeroCause::VerticalSemResultados);
}

#[test]
fn zero_cause_vertical_sem_resultados_serializa_no_envelope() {
    let mut output = output_stub();
    output.metadata.zero_cause = Some(ZeroCause::VerticalSemResultados);
    let json = serde_json::to_string(&output).expect("serialização deve funcionar");
    assert!(json.contains("\"causa_zero\":\"vertical-sem-resultados\""));
}
