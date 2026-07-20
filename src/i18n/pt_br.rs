// SPDX-License-Identifier: MIT OR Apache-2.0
//! Brazilian Portuguese UI strings — exhaustive match (no catch-all).

use super::message::Message;

/// Translates `msg` to Brazilian Portuguese. Must list every [`Message`] variant.
pub fn translate(msg: Message) -> &'static str {
    match msg {
        Message::ConfigurationErrorPrefix => "Erro de configuração",
        Message::ErrorPrefix => "Erro",
        Message::GlobalTimeoutExceeded => {
            "Erro: tempo limite global de {seconds}s excedido"
        }
        Message::DeepResearchTimeoutExceeded => {
            "Erro: tempo limite global de {seconds}s excedido (deep-research)"
        }
        Message::FlagMustPrecedeSubcommand => {
            "\n\nDica: a flag `-{flag}` existe, mas deve aparecer ANTES do \
             subcomando (ex.: `duckduckgo-search-cli -{flag} deep-research \"consulta\"`)."
        }
        Message::XvfbAutoInstallAttempt => {
            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Xvfb não encontrado — \
             tentando instalação automática via sudo sem senha..."
        }
        Message::XvfbInstalledOk => {
            "\x1b[32m[duckduckgo-search-cli]\x1b[0m Xvfb instalado com sucesso."
        }
        Message::XvfbAutoInstallFailed => {
            "\x1b[31m[duckduckgo-search-cli]\x1b[0m Instalação automática falhou \
             (sudo sem senha indisponível)."
        }
        Message::XvfbImmutableDistro => {
            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Distro imutável detectada ({distro}) — \
             instalação automática de Xvfb não é possível."
        }
        Message::XvfbUnknownDistro => {
            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Distro não reconhecida ({distro}) — \
             instalação automática de Xvfb indisponível."
        }
        Message::XvfbPackageManagerFailed => {
            "\x1b[31m[duckduckgo-search-cli]\x1b[0m Falha ao executar o gerenciador de pacotes: {error}"
        }
        Message::XvfbInstallManually => "\x1b[33m  Instale manualmente:\x1b[0m",
        Message::XvfbInstallManuallyFull => "\x1b[33m  Instale Xvfb manualmente:\x1b[0m\n",
        Message::XvfbUnavailableHeadlessFallback => {
            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Xvfb indisponível — \
             o Chrome rodará em headless (evasão anti-bot mais fraca)."
        }
        Message::CancelCooperativeStarted => {
            "duckduckgo-search-cli: {signal} — cancelamento cooperativo iniciado; \
             saída forçada {exit} em {grace}s (segundo sinal encerra imediatamente)"
        }
        Message::CancelSecondSignalForceExit => {
            "duckduckgo-search-cli: segundo sinal durante a graça — saída forçada imediata {exit} (one-shot)"
        }
        Message::CancelGraceExpiredForceExit => {
            "duckduckgo-search-cli: período de graça ({grace}s) expirou — saída forçada {exit} (one-shot)"
        }
        Message::DeepResearchZeroResultsRequire => {
            "deep-research produziu zero resultados para a consulta {query}; \
             --require-results ativo → saindo com código não zero"
        }
        Message::StdoutWriteFailed => "falha ao escrever em stdout: {error}",
        Message::DeepResearchSerializeFailed => {
            "Erro ao serializar saída do deep-research: {error}"
        }
        Message::DeepResearchFailed => "deep-research falhou: {error}",
        Message::CommandsTreeEmitFailed => "falha ao emitir árvore de comandos: {error}",
        Message::CommandsTreeSerializeFailed => {
            "falha ao serializar árvore de comandos: {error}"
        }
        Message::DoctorEmitFailed => "falha ao emitir relatório doctor: {error}",
        Message::DoctorSerializeFailed => "falha ao serializar relatório doctor: {error}",
        Message::SchemaInvalidJson => "schema embutido {id} não é JSON válido",
        Message::SchemaEmitFailed => "falha ao emitir schema {id}: {error}",
        Message::SchemaJsonEmitFailed => "falha ao emitir schema JSON: {error}",
        Message::SchemaSerializeFailed => "falha ao serializar schema JSON: {error}",
        Message::LocaleEmitFailed => "falha ao emitir relatório de locale: {error}",
        Message::LocaleSerializeFailed => "falha ao serializar relatório de locale: {error}",
        Message::SynthesisRecentNewsHeading => "### Notícias recentes\n\n",
        Message::SynthesisRecentNewsLabel => "Notícias recentes:\n\n",
        Message::NoResultsPlaceholder => "\n(sem resultados)\n",
        Message::MarkdownResultsHeading => "# Resultados: {query}\n\n",
        Message::MarkdownMetaLine => {
            "**Motor:** {engine} | **Endpoint:** {endpoint} | **Total:** {total}\n\n"
        }
    }
}
