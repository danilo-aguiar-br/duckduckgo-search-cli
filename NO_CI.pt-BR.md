# SEM CI / SEM GitHub Actions

Leia em [English](NO_CI.md).

Este repositório **proíbe** integração contínua no GitHub Actions,
Dependabot, CI de pre-commit e qualquer pipeline remoto de publicação.

## Proibido

- `.github/workflows/**` (ci, release, audit, docs, etc.)
- Configs de Dependabot / Renovate que abrem PRs automáticos para CI
- `scripts/pre-publish-gate.sh` ou qualquer gate que chame `gh run list`
- Hooks de pre-commit que exijam um runner remoto
- Badges que aleguem status verde de CI

## Gates locais obrigatórios (mantenedores)

Rode antes de cada tag e antes de `cargo publish`:

```bash
./scripts/portability-lint.sh   # seconds; fails fast on ungated `use` of gated item
cargo check-all
cargo check-nohttp              # host, `chrome` profile only — catches items orphaned off-harness
cargo lint-nohttp
cargo check-windows             # rustc against a NON-Linux target (GNU ABI)
cargo check-windows-msvc        # the ABI Windows users actually install
./scripts/check-macos.sh        # rustc against aarch64-apple-darwin
./scripts/check-macos.sh x86_64-apple-darwin   # Intel Mac half of the universal binary
cargo lint
cargo lint-windows
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo docs
RUSTDOCFLAGS="-D warnings" cargo docs-nohttp   # idem, no conjunto de features PADRÃO
cargo test-all          # or at least: cargo test --lib --all-features --locked
cargo deny check        # when deny.toml is present
cargo publish --dry-run --locked
```

Os aliases vivem em [`.cargo/config.toml`](.cargo/config.toml).

### Por que um gate não-Linux é obrigatório

Proibir CI remoto **não** elimina a necessidade de verificar outras plataformas —
apenas move essa verificação para o host do mantenedor.

A v1.0.2 foi publicada no crates.io sem conseguir compilar em macOS **nem** em
Windows. Três defeitos independentes sobreviveram a todos os gates acima, porque
nenhum deles passava `--target`:

- `src/browser/session/mod.rs` importava itens `#[cfg(target_os = "linux")]`
  através de um `use` não gated (E0432). O stripping de `cfg` roda *antes* da
  resolução de nomes, então call sites gated não salvam um import não gated.
- `src/browser/detect.rs` casava `std::env::var_os` com `if let Ok(..)` em vez de
  `if let Some(..)` (E0308) — código exclusivo de Windows que nunca havia sido
  compilado.
- Dois bindings ficaram sem uso fora do Linux, o que `-D warnings` rejeita.

`cargo check` não linka, então `cargo check-windows` precisa apenas do std do alvo.
Pré-requisitos, uma vez por host — **sem root, sem pacote de sistema**:

```bash
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-pc-windows-msvc
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin
```

### v1.0.4: os dois alvos que estavam documentados mas sem gate

`docs/CROSS_PLATFORM` lista quatro alvos suportados. Dois deles —
`x86_64-pc-windows-msvc` e `x86_64-apple-darwin` — não tinham gate algum: o gate
de Windows mirava o ABI GNU, e o script de macOS assumia Apple Silicon por padrão
e nunca era invocado para Intel. Ambos agora rodam, e ambos custam apenas um
`rustup target add`, porque `cargo check` não linka.

O que esses gates continuam **não** provando permanece igual e vale repetir: eles
provam que o código **compila** para aqueles alvos. Nenhum binário jamais foi
**executado** em um host macOS ou Windows. O comportamento em runtime lá segue
não verificado.

### v1.0.3: sem toolchain C (ADR-0029)

Até a v1.0.3 esta seção também mandava rodar `sudo dnf install mingw64-gcc`, porque
`aws-lc-sys` (C, alcançado por `rustls` → `reqwest`) exigia um `cc` cross. Esse
requisito **acabou**: `reqwest` e `rustls` agora são opcionais e só são ativados por
`http-test-harness`, então o build padrão `chrome` não carrega nenhum C.

Verifique em vez de confiar:

```bash
cargo tree -e all -i aws-lc-sys --no-default-features --features chrome
# expected: "package ID specification `aws-lc-sys` did not match any packages"
```

A auditoria da v1.0.3 é a razão de isso importar. Os dois gates cross-platform foram
encontrados **inexecutáveis** no host do mantenedor — `cargo check-windows` morria no
build script de `aws-lc-sys`, e `scripts/check-macos.sh` saía com 2 por ausência de
`zig`. Um gate que não consegue executar é indistinguível de um que nunca rodou, então
a garantia do ADR-0028 havia caducado em silêncio. Remover a dependência de C recoloca
o gate sobre um terreno que este repositório controla.

### Limite conhecido dos gates cross-platform

Os gates cross rodam como `--no-default-features --features chrome` e portanto
**não passam `--all-targets`**. Os testes de integração usam `wiremock` e `reqwest`,
que só existem sob `http-test-harness`; ligar essa feature traria o toolchain C de
volta e derrotaria o propósito.

Consequência, declarada para que ninguém presuma cobertura maior do que existe: os
gates cross cobrem a **biblioteca e o binário** para Windows e macOS. Testes, benches
e examples são cobertos apenas no Linux, por `cargo check-all` e `cargo test-all`. Um
defeito que viva exclusivamente num bloco `#[cfg(test)]` em alvo não-Linux não seria
pego.

Windows satisfaz tanto `not(target_os = "linux")` quanto `not(unix)`, então aquele gate
sozinho já cobre toda a classe de regressão de `cfg` que quebrou o macOS. Ele não cobre
defeitos exclusivos de branches `cfg(target_os = "macos")`, e é por isso que
`scripts/check-macos.sh` também existe.

macOS ainda precisa do script próprio, mas **não** pelo motivo antigo. Até a v1.0.3,
`cargo check --target aarch64-apple-darwin` falhava de saída: `aws-lc-sys` (C, trazido
pelo rustls) acionava o `cc` do *host* com `-arch arm64`, o que um compilador Linux não
honra (upstream aws/aws-lc-rs#1023), e `cargo-zigbuild` era o contorno. O ADR-0029
removeu a dependência de C, então `cargo check` puro agora funciona e **nem `zig` nem
`cargo-zigbuild` são necessários**.

O que o script ainda faz e um alias do cargo não faria:

- verifica se o std do alvo está instalado e sai com **2** trazendo a linha exata de
  `rustup target add`, em vez de morrer dentro de um build script;
- re-executa a sonda da árvore de `aws-lc-sys` após um check bem-sucedido, para que no
  dia em que uma dependência recolocar C no caminho padrão este gate falhe alto em vez
  de voltar silenciosamente a precisar de um cross compiler.

A justificativa vive em
[`docs/decisions/0029-optional-http-stack-no-c-toolchain-v1-0-3.md`](docs/decisions/0029-optional-http-stack-no-c-toolchain-v1-0-3.md).
O [`0028`](docs/decisions/0028-local-cross-platform-gate-v1-0-3.md) registra por que o
gate existe; seus pré-requisitos de zig foram superados pelo 0029.

## Release (somente manual)

1. Suba `version` em `Cargo.toml` / `Cargo.lock` e atualize o `CHANGELOG.md`.
2. Passe pelos gates locais acima.
3. Faça commit na `main` (ou faça merge de uma branch de release na `main`).
4. Tag anotada: `git tag -a vX.Y.Z -m "Release vX.Y.Z: …"`.
5. Push: `git push origin main && git push origin vX.Y.Z`.
6. Notas de GitHub Release opcionais via `gh release create` (sem Actions).
7. Publique: `cargo publish --locked`.

**Não** existe upload automático para o crates.io no push de tag.

## Ferramentas locais opcionais

Ajustes específicos do host (mold/lld, sccache, `target-cpu=native`) pertencem ao
`~/.cargo/config.toml` do **usuário**, nunca à config publicada deste repositório.
Não embuta features de CPU do host em artefatos do crates.io.
