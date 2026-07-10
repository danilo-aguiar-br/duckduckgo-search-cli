// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes de integração do deep-research com a vertical news (GAP-WS-105 v0.8.9).
//!
//! Cobrem o comportamento observável de ponta a ponta SEM rede real:
//! - binário `deep-research --no-news` contra `wiremock` (Chrome desabilitado
//!   via `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`): exit 0 e envelope com os campos
//!   news aditivos sempre presentes (`noticias: []`, `quantidade_noticias: 0`,
//!   `metadados.total_noticias_unicas: 0`) e `sub_queries` SEM campos news
//! - contrato aditivo: envelope v0.8.8 (sem campos news) continua
//!   desserializável em [`DeepResearchOutput`] com defaults
//! - síntese dual com `--no-news`: `sintese` presente nos 3 formatos
//! - F2b: multi-query + `--vertical all` passa pelo `build_config` e só
//!   falha no guard de ambiente do Chrome (exit 2)
//!
//! A validação FATAL do deep-research sem Chrome e sem `--no-news` (exit 2
//! citando `--no-news`) é coberta por
//! `integration_news_vertical::binario_deep_research_news_default_sem_chrome_exit_2`
//! — não duplicada aqui.
//!
//! Todos os env vars são passados por `Command::env` (escopo do subprocesso),
//! então NÃO há mutação de ambiente do processo de teste e nenhum lock é
//! necessário. Os testes com `MockServer` usam runtime multi-thread porque o
//! subprocesso bloqueia o worker enquanto o mock precisa responder.

use duckduckgo_search_cli::deep_research::DeepResearchOutput;
use std::process::{Command, Output, Stdio};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_duckduckgo-search-cli")
}

/// SERP HTML com 3 resultados orgânicos (mesma forma do helper canônico de
/// `integration_wiremock.rs`). O padding supera o limiar de detecção de
/// bloqueio silencioso (5 000 bytes).
fn html_com_3_resultados() -> String {
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/um">Resultado Um</a>
        <a class="result__snippet">Descrição do primeiro resultado.</a>
        <span class="result__url">exemplo.com/um</span>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/dois">Resultado Dois</a>
        <a class="result__snippet">Descrição do segundo resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/tres">Resultado Três</a>
        <a class="result__snippet">Descrição do terceiro resultado.</a>
      </div>
    </div>
    </body></html>"#
    )
}

/// Sobe um `MockServer` servindo a SERP com resultados em qualquer GET `/`.
async fn mock_serp_com_resultados() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_com_3_resultados())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&server)
        .await;
    server
}

/// Executa o binário `deep-research` contra o mock, com Chrome desabilitado
/// e URLs base redirecionadas para `base`. Bloqueante — chamar via
/// `spawn_blocking` dentro de runtime async.
fn run_deep_research_bin(base: String, extra_args: Vec<String>) -> Output {
    let mut cmd = Command::new(bin_path());
    cmd.arg("deep-research");
    cmd.args(&extra_args);
    cmd.env("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1")
        .env("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", &base)
        .env("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", &base)
        .stdin(Stdio::null());
    cmd.output().expect("binário deve executar")
}

// ---------------------------------------------------------------------------
// 1. Envelope aditivo do deep-research com --no-news (binário + wiremock)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn binario_no_news_emite_envelope_com_campos_news_aditivos() {
    let server = mock_serp_com_resultados().await;
    let base = format!("{}/", server.uri());

    let output = tokio::task::spawn_blocking(move || {
        run_deep_research_bin(
            base,
            vec![
                "rust async runtime".to_string(),
                "--no-news".to_string(),
                "--max-sub-queries".to_string(),
                "2".to_string(),
            ],
        )
    })
    .await
    .expect("join do subprocesso");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "--no-news com resultados web deve sair 0; stderr: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout deve ser JSON válido");

    // Discriminador e vertical web populada.
    assert_eq!(json["tipo"], "deep_research");
    let resultados = json["resultados"]
        .as_array()
        .expect("resultados deve ser array");
    assert!(
        !resultados.is_empty(),
        "fan-out contra o mock deve agregar resultados web"
    );

    // Contrato aditivo GAP-WS-105: campos news SEMPRE presentes com os
    // tipos corretos, mesmo com --no-news.
    let noticias = json["noticias"]
        .as_array()
        .expect("noticias deve ser array mesmo com --no-news");
    assert!(noticias.is_empty(), "--no-news implica noticias vazio");
    assert_eq!(
        json["quantidade_noticias"].as_u64(),
        Some(0),
        "quantidade_noticias deve ser número 0 com --no-news"
    );
    assert_eq!(
        json["metadados"]["total_noticias_unicas"].as_u64(),
        Some(0),
        "metadados.total_noticias_unicas deve ser número 0 com --no-news"
    );

    // Com --no-news as sub_queries NÃO carregam campos news (omitidos).
    let sub_queries = json["metadados"]["sub_queries"]
        .as_array()
        .expect("sub_queries deve ser array");
    assert!(!sub_queries.is_empty(), "deve haver sub-queries no fan-out");
    for sq in sub_queries {
        let obj = sq.as_object().expect("sub_query deve ser objeto");
        assert!(
            !obj.contains_key("quantidade_noticias"),
            "--no-news deve omitir quantidade_noticias na sub_query: {sq}"
        );
        assert!(
            !obj.contains_key("news_indisponivel"),
            "--no-news deve omitir news_indisponivel na sub_query: {sq}"
        );
    }

    // Round-trip no tipo público: o envelope emitido pelo binário é
    // desserializável em DeepResearchOutput.
    let parsed: DeepResearchOutput =
        serde_json::from_str(stdout.trim()).expect("round-trip DeepResearchOutput");
    assert!(parsed.news.is_empty());
    assert_eq!(parsed.news_count, 0);
    assert_eq!(parsed.metadata.unique_news_count, 0);
}

// ---------------------------------------------------------------------------
// 2. Contrato aditivo: envelope v0.8.8 (sem campos news) segue compatível
// ---------------------------------------------------------------------------

#[test]
fn envelope_v088_sem_campos_news_desserializa_com_defaults() {
    // Envelope como emitido pela v0.8.8 — SEM noticias, quantidade_noticias
    // e total_noticias_unicas. Os `#[serde(default)]` garantem o contrato
    // aditivo (consumidores antigos e payloads antigos seguem válidos).
    let antigo = r#"{
        "tipo": "deep_research",
        "query": "rust",
        "metadados": {
            "query_original": "rust",
            "sub_queries": [
                {
                    "texto": "rust overview",
                    "estrategia": "heuristic",
                    "status": "ok",
                    "tempo_ms": 10
                }
            ],
            "estrategia_agregacao": "rrf",
            "total_resultados_unicos": 0,
            "tempo_total_ms": 12,
            "nivel_cascata": null
        },
        "resultados": []
    }"#;

    let parsed: DeepResearchOutput =
        serde_json::from_str(antigo).expect("envelope v0.8.8 deve desserializar");
    assert!(parsed.news.is_empty(), "noticias ausente vira vec vazio");
    assert_eq!(parsed.news_count, 0, "quantidade_noticias ausente vira 0");
    assert_eq!(
        parsed.metadata.unique_news_count, 0,
        "total_noticias_unicas ausente vira 0"
    );
    let sq = &parsed.metadata.sub_queries[0];
    assert!(sq.news_count.is_none());
    assert!(sq.news_unavailable.is_none());
}

// ---------------------------------------------------------------------------
// 3. Síntese dual com --no-news nos 3 formatos
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn binario_no_news_synthesize_emite_sintese_nos_tres_formatos() {
    let server = mock_serp_com_resultados().await;

    // (valor da flag --synth-format, valor serializado em sintese.formato)
    for (flag, formato_esperado) in [
        ("markdown", "markdown"),
        ("plain-text", "plain_text"),
        ("json", "json"),
    ] {
        let base = format!("{}/", server.uri());
        let output = tokio::task::spawn_blocking(move || {
            run_deep_research_bin(
                base,
                vec![
                    "rust async runtime".to_string(),
                    "--no-news".to_string(),
                    "--max-sub-queries".to_string(),
                    "2".to_string(),
                    "--synthesize".to_string(),
                    "--synth-format".to_string(),
                    flag.to_string(),
                ],
            )
        })
        .await
        .expect("join do subprocesso");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(0),
            "--synthesize --synth-format {flag} deve sair 0; stderr: {stderr}"
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(stdout.trim()).expect("stdout deve ser JSON válido");

        let sintese = json
            .get("sintese")
            .unwrap_or_else(|| panic!("sintese deve estar presente no formato {flag}"));
        assert_eq!(
            sintese["formato"].as_str(),
            Some(formato_esperado),
            "sintese.formato incorreto para --synth-format {flag}"
        );
        let corpo = sintese["corpo"]
            .as_str()
            .expect("sintese.corpo deve ser string");
        assert!(
            !corpo.is_empty(),
            "sintese.corpo não pode ser vazio no formato {flag}"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. F2b: multi-query + --vertical all passa pelo build_config
// ---------------------------------------------------------------------------

// GAP-WS-113: multi-query + --vertical all + NO_CHROME => exit 2 fail-closed.
#[test]
fn binario_multi_query_vertical_all_no_chrome_fail_closed() {
    let output = Command::new(bin_path())
        .args(["--vertical", "all", "-q", "-f", "json", "rust", "tokio"])
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
        "GAP-WS-113: NO_CHROME deve falhar exit 2; stdout={stdout} stderr={stderr}"
    );
}
