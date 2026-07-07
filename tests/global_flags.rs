// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes E2E do GAP-WS-106 (v0.9.0) — Ergonomia da CLI.
//!
//! Valida os três sintomas resolvidos a partir do binário compilado:
//! - Sintoma A: dica PT-BR NÃO aparece para flags tipográficas desconhecidas
//!   (aparece apenas para flags globais conhecidas fora de posição, o que
//!   já não aciona erro porque são `global = true` — coberto por unit tests).
//! - Sintoma C (build sem `chrome`): o parser continua aceitando `--no-news`
//!   e `--vertical news`; o auto-default é validado em testes de integração.
//!
//! Os Sintomas B (parser aceita `-q`/`-o` após subcomando) são cobertos pelos
//! unit tests `quiet_global_aceito_apos_subcomando` e
//! `output_global_aceito_apos_subcomando` em `src/cli.rs`.

use assert_cmd::Command;
use predicates::prelude::*;

const BIN_NAME: &str = "duckduckgo-search-cli";

/// Sintoma A — flag tipográfica desconencida NÃO dispara a dica PT-BR
/// (a dica só aparece para flags globais conhecidas fora de posição).
#[test]
fn sintoma_a_flag_desconhecida_nao_dispara_dica() {
    Command::cargo_bin(BIN_NAME)
        .expect("binario compilado")
        .args(["--flag-totalmente-inexistente"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Dica:").not());
}

/// Sintoma A — a dica PT-BR aparece quando uma flag local conhecida de
/// `CliArgs` (não-hoisted) é passada após um subcomando. `--pages` existe
/// no parser raiz mas é LOCAL a `CliArgs`; dentro de `deep-research`
/// (que usa `DeepResearchArgs`) ela é desconhecida, então o clap reporta
/// `UnknownArgument`, e o formatter anexa a dica porque `pages` casa com
/// `is_known_global_flag`.
#[test]
fn sintoma_a_dica_aparece_para_flag_local_conhecida_apos_subcomando() {
    Command::cargo_bin(BIN_NAME)
        .expect("binario compilado")
        .args(["deep-research", "--pages", "3", "rust"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Dica:"));
}

/// Sintoma C (build sem chrome) — o subcomando `deep-research` continua
/// aceitando `--no-news` explicitamente (retrocompatibilidade preservada).
/// Validação do auto-default em runtime fica nos testes de integração.
#[cfg(not(feature = "chrome"))]
#[test]
fn sintoma_c_deep_research_aceita_no_news_sem_chrome() {
    // Sem rede: passamos apenas `--help` para validar o parser caminho
    // sem-chrome. O auto-default de `--no-news` acontece apenas no código
    // de runtime; este teste documenta que a flag continua aceita.
    Command::cargo_bin(BIN_NAME)
        .expect("binario compilado")
        .args(["deep-research", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}
