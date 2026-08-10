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
            "\n\nDica: a flag `--{flag}` existe, mas deve aparecer ANTES do \
             subcomando (ex.: `duckduckgo-search-cli --{flag}` ou \
             `duckduckgo-search-cli --{flag} doctor`). Algumas flags também \
             são aceitas no próprio subcomando (veja --help)."
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
        Message::DeepResearchBudgetUnderflow => {
            "Erro: --global-timeout {timeout}s está abaixo da estimativa com margem do \
deep-research {gated}s (bruto ~{estimate}s). Aumente o timeout, reduza a carga ou use \
--allow-under-budget."
        }
        Message::DeepResearchBudgetAllowOverride => {
            "Aviso: --global-timeout {timeout}s está abaixo da estimativa com margem \
{gated}s (bruto ~{estimate}s); continuando porque --allow-under-budget está ativo."
        }
        // Agent-native reduction refusals (v1.0.5). This is what a PERSON
        // reads on stderr; the contract text an agent reads on stdout is the
        // English rendering of the same template.
        Message::AgentOpsRowOpUnsupported => {
            "{op} não é suportado por `{surface}`: este envelope não tem array de \
             linhas. Rode `duckduckgo-search-cli commands` e leia `agent_ops` \
             para saber o que esta superfície aceita."
        }
        Message::AgentOpsEnvelopeNotObject => "`{surface}` não emitiu um objeto JSON",
        Message::AgentOpsRowsKeyMissing => {
            "`{surface}` declara suas linhas em `{rows}`, que está ausente do envelope \
             — isto é um defeito do produto, por favor reporte"
        }
        Message::AgentOpsFilterInvalid => {
            "--filter inválido `{expr}`: esperado `chave=valor`, `chave!=valor` ou \
             `chave~subcadeia`"
        }
        Message::AgentOpsSortDirectionInvalid => {
            "direção de --sort inválida `{direction}`: esperado `asc` ou `desc`"
        }
        Message::AgentOpsSortKeyMissing => "--sort inválido: chave ausente",
        Message::AgentOpsSortKeyUnknown => {
            "chave de --sort inválida `{key}`: nenhuma linha em `{rows}` a possui. \
             Disponíveis: {available}"
        }
        Message::AgentOpsFieldsEmpty => "--fields inválido: nenhum caminho informado",
        Message::AgentOpsFieldsPathUnknownTop => {
            "caminho de --fields inválido `{path}`: `{segment}` não é chave do nível \
             superior de `{surface}`. Disponíveis ali: {available}"
        }
        Message::AgentOpsFieldsPathUnknownNested => {
            "caminho de --fields inválido `{path}`: `{segment}` não é chave de \
             `{parent}`. Disponíveis ali: {available}"
        }
        Message::AgentOpsFieldsPathInvalid => {
            "caminho de --fields inválido `{path}` para `{surface}`"
        }
        Message::AgentOpsNothingToTruncate => {
            "--truncate-content não é suportado por `{surface}`: toda string \
             deste envelope é um identificador devolvido a um programa, então \
             encurtar uma produziria saída que parece válida e não é. Rode \
             `duckduckgo-search-cli commands` e leia `agent_ops` para saber o \
             que esta superfície aceita."
        }
        // Corpos de `CliError` (v1.0.5). Isto é o que uma PESSOA lê em stderr;
        // o texto de contrato que um agente lê em stdout continua sendo o
        // `Display` em inglês, imóvel.
        Message::ErrorRateLimited => "limitação de taxa detectada pelo DuckDuckGo",
        Message::ErrorBlocked => "bloqueio anti-bot detectado (anomalia de http 202)",
        Message::ErrorNoResults => "zero resultados em todas as consultas",
        Message::ErrorCancelled => "operação cancelada via sigint/sigterm",
        Message::ErrorBrokenPipe => "pipe fechado pelo consumidor (broken pipe)",
        Message::ErrorChromeDisabled => {
            "transporte Chrome indisponível (recompile com --features chrome)"
        }
        Message::ErrorPayloadTooLarge => {
            "payload excede {max} bytes (recebidos {actual})"
        }
        Message::ErrorUnsupportedEncoding => "content-encoding não suportado: {encoding}",
        Message::ErrorInvalidUtf8 => "o corpo da resposta não é utf-8 válido",
        Message::ErrorDecompressionIo => "erro de e/s na descompressão: {error}",
        Message::ErrorHttpClient => "erro do cliente http",
        // Rótulos de `CliError` (v1.0.5). A prosa do chamador vem logo depois e
        // NUNCA é traduzida — o produto não reescreve texto que não escreveu.
        Message::ErrorLabelHttp => "erro de http",
        Message::ErrorLabelProxy => "erro de proxy",
        Message::ErrorLabelNetwork => "erro de rede",
        Message::ErrorLabelPipelineInvariant => "violação de invariante do pipeline",
        Message::ErrorLabelChromeNotFound => "chrome não encontrado",
        Message::ErrorLabelChromeUnavailable => "chrome indisponível",
    }
}
