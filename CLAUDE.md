# CLAUDE.md

Guia de trabalho neste repositório.

## 🎯 Objetivo canônico — fonte única

> **Um editor de fotos completo e funcional, no formato do Lightroom, em Rust com GPUI.**
>
> **Completo**: importar, organizar, triar, revelar e **entregar arquivo**.
> **Funcional**: **todo controle que a tela oferece move a foto.** Um slider que existe e não faz
> nada é defeito, não pendência.

A integração com a API de pós-venda **existe desde 2/set/2026** (tecla `B` + botão "Pós-venda";
`docs/PARIDADE-LIGHTROOM.md`, seção "Pós-venda").

⚠️ **O princípio "o app tem de ser útil sozinho" caiu em 6/set/2026.** O dono pediu que o app peça a
conta do site para ser usado — *"eu preciso autenticar na web para conseguir usar o
VintageLightbox"*. A primeira versão da reversão guardou uma saída, o botão **"trabalhar offline"**,
e a saída **caiu no mesmo dia**, com o motivo que decide: *"o propósito dele é integração com o
pós-venda da RecordarFotos"*. O app abre pedindo a conta (`crates/ui-gpui/src/entrada.rs`), e não há
outro jeito de abrir.

🚨 **Trabalhar sem rede não foi descartado — foi adiado com forma própria: sincronização.** O app vai
guardar o que foi feito sem internet e conciliar quando ela voltar. Um botão que só desliga o site é
o contrário disso: ele deixa o operador triar 200 fotos e descobrir no balcão que nada subiu, e não
guarda nada para conciliar depois. **Enquanto a sincronização não existe, sem rede não se trabalha** —
e é preferível a um app que mente sobre o que salvou.

⚠️ **Este objetivo substituiu outro em 17/ago/2026**, e a diferença importa no dia a dia. O anterior
era migrar a interface de egui para GPUI **com paridade**; ele foi **alcançado** (`8c7df32`, o
`crates/ui` fora do workspace, zero pacotes `egui` no `Cargo.lock`). Mas as regras dele continuavam
valendo — em especial **"nenhuma feature nova"**, que é o motivo de o painel de Revelação ter 19
sliders que não fazem nada: o app antigo também não os aplicava, e o porte foi fiel ao defeito.

**Fidelidade ao app antigo deixou de ser virtude.** Detalhes em
[`docs/00-OBJETIVO.md`](docs/00-OBJETIVO.md).

### Teste de alinhamento

1. **Um fotógrafo faz isto no Lightroom?** Se não, pergunte antes.
2. **A tela promete e não entrega?** É **defeito**, e vem antes de funcionalidade nova.
3. **Dá para conferir sem abrir o app?** Se não dá para escrever um teste que falha hoje, o trabalho
   ainda não está entendido.
4. **É o mais barato que destrava mais coisa?**

**Comece sempre por [`docs/PARIDADE-LIGHTROOM.md`](docs/PARIDADE-LIGHTROOM.md)** — é a lista medida
do que funciona, do que promete e não faz, e do que não existe. É a fila de trabalho.

## Build and Test Commands

**Comece por `make`** — sem argumento ele lista tudo que segue, com uma linha cada:

```bash
make            # a lista dos alvos
make testar     # cargo test --workspace
make lint       # fmt + clippy -D warnings, como no CI
make rodar      # abre o app (sempre em release)
make mac        # .app + .dmg universal
make linux      # .deb + .AppImage, por Docker
make windows    # explica por que o Windows sai do .ps1, e nao daqui
make publicar   # sobe dist/ para o site
make faxina     # apaga o cache de debug do cargo
```

Os comandos crus, para quando for preciso desviar do atalho:

```bash
# Build the entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p domain
cargo test -p use-cases
cargo test -p infrastructure

# Run a single test by name
cargo test test_name --workspace

# Run the application
# 🚨 **Sempre com `--release`.** Em `debug` uma miniatura custa 38,45 ms contra
#    0,67 ms — 57× (medido em 6/set/2026, `medir-miniaturas`, 125 fotos). Um
#    quadro de 60fps tem 16,7 ms: em `debug` uma miniatura sozinha estoura dois
#    quadros, e a Revelação, que varre a imagem inteira três vezes por resultado
#    da GPU, paga isso a cada milímetro de slider. Já foi confundido com "o
#    framework é lento" duas vezes (docs/STATUS.md).
cargo run --release -p ui-gpui

# O motor de revelação para o navegador (entrega ao recordarfotos-e-commerce)
scripts/construir-web.sh [caminho/do/frontend]
cargo clippy -p revelacao-web --target wasm32-unknown-unknown -- -D warnings

# A grade da biblioteca para o navegador — só o motor; a tela é React lá
scripts/construir-biblioteca.sh [caminho/do/frontend]
cargo clippy -p biblioteca-web --target wasm32-unknown-unknown -- -D warnings

# Os instaladores, e a publicação — fora das lojas
./scripts/empacotar.sh mac-universal --publicar   # gera e sobe para o site
./scripts/empacotar.sh linux                      # .deb + .AppImage, por Docker
./scripts/publicar.py --seco                      # o que seria publicado
# 🔑 **São dois scripts, um por plataforma, e isso é decisão de desenho**
#    (dono, 7/set/2026): `empacotar.sh` faz macOS e Linux; `empacotar.ps1` faz
#    Windows, numa máquina Windows de verdade. Cada alvo é gerado onde pode ser
#    gerado **e conferido**.
#
# 🚫 Cross-compilação para Windows **funciona e foi recusada**. O `cargo-xwin`
#    gerou um `.exe` de 34,9 MB em 7/set/2026, ao custo de bifurcar o `gpui` e o
#    `rsraw-sys` e de entregar um binário que ninguém aqui abre para conferir.
#    Decisão do dono: tudo nativo. Não refazer — `empacotamento/README.md`.

# Check code (faster than build)
cargo check --workspace

# Format and lint
cargo fmt --all
cargo clippy --workspace
```

## Architecture

VintageLightbox follows **Clean Architecture** with 4 layers as separate crates:

```
┌─────────────────────────────────────────────────────┐
│  ui-gpui (GPUI)                                     │
│  - main.rs: entrada, tema, teclas, janela           │
│  - app.rs: a raiz — qual tela está no ar            │
│  - biblioteca/ revelacao/ importacao/ impressao/    │
│  - cliente.rs: a segunda tela; configuracoes.rs     │
├─────────────────────────────────────────────────────┤
│  adapters                                           │
│  - controllers/: ImportController, EditorController │
│  - presenters.rs, view_models.rs                    │
├─────────────────────────────────────────────────────┤
│  use-cases                                          │
│  - Business logic orchestration                     │
│  - ImportPhotoUseCase, SavePhotoEditsUseCase, etc.  │
├─────────────────────────────────────────────────────┤
│  infrastructure                                     │
│  - database/: SQLite repositories (sqlx)            │
│  - exif_reader.rs, thumbnail_generator.rs           │
│  - raw_processing.rs, image_exporter.rs             │
│  - gpu_adjustments.rs / transformacao.rs: re-export │
│    do revelacao-core + leitura da entidade          │
│  - migrations/: SQL migration files                 │
├─────────────────────────────────────────────────────┤
│  revelacao-core (sem domain, sem janela, sem banco) │
│  - ajustes.rs: os 46, por nome e por posição        │
│  - shaders/: corpo.wgsl + 2 entradas (compute/frag) │
│  - motor.rs, transformacao.rs (Corte), jpeg.rs      │
│  → revelacao-web: o mesmo, em wasm, para o site     │
├─────────────────────────────────────────────────────┤
│  biblioteca-core (ZERO dependências)                │
│  - grade.rs, selecao.rs, acervo.rs, miniaturas.rs   │
│  - a mesma conta para o site e para ui-gpui         │
│  → biblioteca-web: só a grade, em wasm, para o site │
│    (a tela é React lá; egui entra só como pintor)   │
├─────────────────────────────────────────────────────┤
│  domain (innermost - no external dependencies)      │
│  - entities/: Photo, Collection                     │
│  - value_objects/: PhotoId, Rating, ColorLabel      │
│  - repositories.rs: Trait definitions               │
│  - errors.rs: DomainError, DomainResult             │
└─────────────────────────────────────────────────────┘
```

⚠️ **`biblioteca-web` é motor, não tela** — como o `revelacao-web`. Em 5/set/2026 ele desenhou a
galeria inteira do site em egui, a pedido do dono, e no mesmo dia o dono reverteu ("deveria usar as
tecnologias de revelacao-web"). O registro, com a lista do que não refazer, está em
`docs/BIBLIOTECA_NO_NAVEGADOR.md` do `recordarfotos-e-commerce`.

**Dependency Rule**: Inner layers never depend on outer layers. The `domain` crate has no dependencies on other crates; `use-cases` depends only on `domain`; etc.

## Key Patterns

- **Repository Pattern**: Traits defined in `domain/src/repositories.rs`, implemented in `infrastructure/src/database/`
- **Async-First**: All repository methods use `async_trait` and `async fn`
- **TDD**: Tests use `mockall` for mocking repositories, `proptest` for property-based testing
- **Dependency Injection**: Use cases receive repository implementations via constructor

## Database

- SQLite with `sqlx` (runtime-tokio-native-tls)
- Migrations in `crates/infrastructure/migrations/`
- Uses `sqlx::migrate!` macro for embedded migrations
- Local database file: `vintage_lightbox.db`

## UI Framework

**GPUI 0.2 + gpui-component 0.5** — o egui saiu em 17/ago/2026, com a migração
concluída (`docs/historico/10-MIGRACAO-GPUI.md`). Quem procura o app antigo o encontra no
histórico do git; o que ele fazia está listado, comportamento a comportamento,
em `docs/historico/PARIDADE-UI.md`.

- As telas ficam em `crates/ui-gpui/src/{biblioteca,revelacao,importacao,impressao}/`
- As duas telas grandes vivem num **dock**: os painéis se arrastam e se
  redimensionam, e o arranjo é gravado ao lado do catálogo (`arranjo-*.json`)
- Imagem: `DynamicImage → RgbaImage → Frame → RenderImage` (`imagem.rs`)
  ⚠️ **em BGRA** — o GPUI espera essa ordem e o crate `image` produz RGBA
- Seletor de arquivos nativo via `rfd`; o motor de revelação é wgpu próprio,
  numa thread de fundo (`revelacao/processador.rs`)
- Teclas: as da raiz em `app.rs`, e sempre com contexto — ligação sem `!Input`
  come a letra de quem está digitando na busca

⚠️ **Toda medida de desempenho é em `--release`**: em `debug` uma miniatura
custa 56× mais, e a fase 1 quase condenou o framework por medir no perfil
errado. As réguas estão em `cargo run --release -p ui-gpui --bin medir-miniaturas`
e `--bin medir-abertura`.

## Como uma funcionalidade nova atravessa as camadas

1. Entidade / value object no `domain`, com teste
2. Use case em `use-cases`, dependendo só de traits do domain
3. Implementação em `infrastructure` (repositório, disco, GPU)
4. Controller em `adapters`, ligando use case e interface
5. **Montagem no `crates/ui-gpui/src/main.rs`** e tela em
   `crates/ui-gpui/src/{biblioteca,revelacao,importacao,impressao}/`

🚨 **O passo 5 é o que mais some, e some em silêncio.** `ExportPhotoUseCase`, `ExportController` e
`ImageExporterImpl` existem, estão testados — e **nunca são construídos no `main.rs`**. O app não
exporta nada, e nada acusa isso: os testes das camadas de dentro passam todos. Camada pronta não é
funcionalidade entregue; a pergunta é sempre **"que clique chega até aqui?"**.

## A distribuição: fora das lojas, e o app se atualiza sozinho

O app **não passa pela App Store nem pela Microsoft Store** (decisão do dono, 7/set/2026). Ele é
gerado aqui, publicado em `recordarfotos.com.br/vintageLightbox` e, a partir da primeira instalação,
se atualiza sozinho. O caminho inteiro está em [`empacotamento/README.md`](empacotamento/README.md).

🔑 **O que substitui a loja é a assinatura minisign.** Cada pacote é assinado por
`scripts/empacotar.sh`; a chave **pública** é compilada dentro do app
(`atualizacao::porta::CHAVE_PUBLICA`), a **privada** mora em `~/.vintagelightbox/atualizacao.key`,
fora do repositório. O app baixa, confere e só então instala.

🚨 **Perder a chave privada quebra a atualização de todo app já instalado** — os que estão na rua só
aceitam pacote assinado por ela, e isso não se conserta pelo software. Trocar `chave-publica.txt` por
uma que não corresponda transforma toda atualização em "assinatura inválida", em silêncio.

⚠️ **Publicar sem subir a versão do `[workspace.package]` não faz nada**: o app compara a própria
`CARGO_PKG_VERSION` com a do manifesto. E `packager.toml` carrega a mesma versão — o script aborta se
as duas divergirem.

## Portas para o mundo assíncrono — o padrão da casa

O GPUI **não roda futuros do tokio**, e os controllers são `async`. Toda ponte entre a tela e o banco
é uma `trait` de porta, com o `Handle` do tokio capturado no `main` (antes de `Application::run`
tomar a thread — um `tokio::spawn` de dentro do GPUI entra em pânico com *there is no reactor
running*).

Há quatro para copiar: `Gravador` e `GuardaDePresets` (revelação), `Marcador` (triagem), `Acervo`
(releitura da Biblioteca), e as quatro da importação (`Explorador`, `Importador`, `SeletorDePasta`,
`GeradorDeMiniaturas`).

Duas regras que já custaram caro:

- **A porta nunca devolve `Result` para a tela.** Avisar é acessório, e um `?` no meio faria a falha
  do acessório derrubar o principal.
- **Cada porta tem uma versão de mentira** (`mod mentira`, sob `#[cfg(test)]`), e é ela que permite
  ao teste afirmar **o que foi gravado** e **quando** — sem banco, sem disco, sem GPU.
