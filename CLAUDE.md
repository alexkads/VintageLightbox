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

> 🩸 **Contrato de sangue (dono, 2026-09-17; revisto em 2026-09-20) — duas peças, dois papéis:**
>
> | Peça | Papel |
> |---|---|
> | `frontend/.../dashboard/sessoes-fotograficas` (no e-commerce) | **Em produção** — os funcionários já estão usando, e é a referência de comportamento |
> | **`crates/ui-gpui`** | **A interface final e definitiva do desktop**, por ser mais performática |
>
> 🚨 **O `ui-gpui` não está mais em pausa**: **todo trabalho novo de fluxo é feito aqui**, no GPUI.
>
> 🔑 **A web é a referência, e não uma fonte de código.** Não há código a mover de lá para cá, há
> **comportamento a reproduzir**. Portar é reescrever com a regra entendida. O jeito de conferir é
> abrir a janela do app e a do site lado a lado (`crates/ui-gpui/rodar-local.sh` contra o `make up`
> do e-commerce), repetir o mesmo gesto e anotar onde diferem — cada diferença é uma linha da lista
> de trabalho. Quando os dois divergirem, **quem está certo é a web**: é a que tem gente dentro.
>
> Regra de negócio vai para `use-cases`, onde as duas telas a encontram.
> O objetivo acima, "em Rust com GPUI", continua sendo o destino — e agora é o único.

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

## 🚨 Contrato da foto (compartilhado com o site)

O que o app sobe e lê do pós-venda segue o contrato do site:
`../recordarfotos-e-commerce/docs/CONTRATO_DA_FOTO.md` — **bruto** (nunca muda), **parâmetros** (os
171 por nome, nenhum descartado), **versão revelada** (arquivo próprio) e **caches**. Desclassificar devolve
bruto + parâmetros ao SQLite antes de a nuvem apagar (C21), e cada versão dos parâmetros fica no
histórico, que anda com a foto (C23–C26). As divergências deste app estão lá como D7, D8, D14 e D15. Onde o código diverge do contrato, o código está errado.

## 🗣️ Revelar e editar — os nomes na tela

`../recordarfotos-e-commerce/docs/REVELAR_E_EDITAR.md` (dono, 28/set/2026): **revelar** é o Lightroom
(parâmetros sobre o bruto — a tela Revelação) e **editar** é o Photoshop (camadas — o editor, aberto
por "Editar Foto"). Nenhum texto da tela diz **"revelação"**, "edição" nunca quer dizer parâmetro, e
"o editor" nunca é a Revelação. Nomes internos do código podem ficar.

## Build and Test Commands

**Comece por `make`** — sem argumento ele lista tudo que segue, com uma linha cada:

```bash
make            # a lista dos alvos
make testar     # cargo test --workspace
make lint       # fmt + clippy -D warnings, como no CI
make rodar      # abre o app (sempre em release)
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

# 🔑 Contra a pilha local, pelo script do crate — ele aponta a API (8080) e
#    o site (8001) juntos e conferem se a API responde antes de compilar. As duas
#    variáveis nunca andam sozinhas: só a API em localhost abria a autorização em
#    produção, e o operador entrava na conta real achando que estava local
#    (17/set/2026). Antes: `make up` no recordarfotos-e-commerce.
crates/ui-gpui/rodar-local.sh     # o app, contra a pilha local

# O motor de revelação para o navegador (entrega ao recordarfotos-e-commerce)
scripts/construir-web.sh [caminho/do/frontend]
cargo clippy -p revelacao-web --target wasm32-unknown-unknown -- -D warnings

# A grade da biblioteca para o navegador — só o motor; a tela é React lá
scripts/construir-biblioteca.sh [caminho/do/frontend]
cargo clippy -p biblioteca-web --target wasm32-unknown-unknown -- -D warnings

# Check code (faster than build)
cargo check --workspace

# Format and lint
cargo fmt --all
cargo clippy --workspace
```

## Sem CI: quem confere é a bateria local e o próprio balcão (desde 27/set/2026)

> *"Não precisa de .dmg e appveyor!"* — dono, 27/set/2026.

O AppVeyor rodava o instalador do balcão no Windows e no Linux a cada push, e saiu. Naquele dia ele
chegou por último em todas as perguntas: a fila tinha seis builds parados (um job por vez, cerca de
uma hora cada), enquanto o Mac e o Fedora do dono já tinham compilado e instalado a 0.1.25 pela
atualização do próprio app. No Windows ele só fazia `cargo check`, sem ligar o binário.

- **A rede é a bateria local antes de subir**: `make testar` e `make lint` no `dev`.
- 🎬 **E o ciclo de vida da sessão, contra a API de verdade** (dono, 03/out/2026: *"a garantia
  que tudo vai funcionar a cada versão lançada"*): `make e2e-ciclo` roda a janela do app, com a
  montagem do balcão (`crates/ui-gpui/src/montagem.rs`, a mesma do `main.rs`), contra o
  `servidor-do-ciclo` do e-commerce — Postgres e Redis descartáveis — e percorre criação da
  sessão, seleção com o cliente, revelação, venda no caixa, estorno, fechamento e pós-venda
  (`e2e::ciclo_de_vida`). O `make producao` do e-commerce o roda (`--origin-dev`) antes de subir
  o app ou o backend; vermelho, nada sobe. Precisa do Docker de pé.
- **Uma versão que não compila num balcão não quebra nada**: o instalador compila numa pasta à parte,
  confere o app novo (`--versao`) e só então troca, guardando o anterior. O custo é o balcão ficar
  uma versão atrás até a correção, que sai numa versão nova e maior.
- ⚠️ O que se perdeu: um erro que só aparece no Windows ou só no Linux (código com `cfg(windows)`,
  dependência nativa, linker) chega sem aviso prévio. Ao mexer nesse tipo de código, diga ao dono
  para conferir num balcão daquele sistema.

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

🔁 **`crates/ui-gpui` é a interface do desktop** (2026-09-20). O fluxo de
`/dashboard/sessoes-fotograficas` sempre vai precisar de validação: a mesma sessão levada pelo app e
pelo site tem de dar o mesmo resultado, e quando não dá, um deles tem defeito — e quem está certo é
o site. Regra de negócio vai para `use-cases`, onde os dois a encontram.

**GPUI 0.2 + gpui-component 0.5** — o egui saiu em 17/ago/2026, com a migração
concluída (`docs/historico/10-MIGRACAO-GPUI.md`). Quem procura o app antigo o encontra no
histórico do git; o que ele fazia está listado, comportamento a comportamento,
em `docs/historico/PARIDADE-UI.md`.

- As telas ficam em `crates/ui-gpui/src/{biblioteca,revelacao,importacao,impressao}/`
- As duas telas grandes vivem num **dock**: os painéis se arrastam e se
  redimensionam, e o arranjo é gravado ao lado do catálogo (`arranjo-*.json`)
- 🎨 **O visual sai do `crates/ui-gpui/template.toml`** (o código de um preset do
  ui.shadcn.com/create, ou eixo por eixo: estilo, cores, fonte, ícones, raio). Cores, medidas
  e cantos das telas derivam dele (`tema::medidas`, `tema::canto`); fonte e ícones que não são
  do sistema/lucide se baixam com `scripts/baixar-do-template.py`. Matrix e Cyberpunk são temas
  fixos, fora do template. Para experimentar: `VLB_TEMPLATE=lyra cargo run -p ui-gpui`.
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

⏱️ **Para achar onde o tempo vai no app aberto**, é o botão **Desempenho** do rodapé: quadros,
etapas de CPU e GPU (timestamp query), máquina e drivers, sessões no banco e comparação entre
sistemas. Como medir, o que cada número é e o que não dá para medir: [`docs/DESEMPENHO.md`](docs/DESEMPENHO.md).

## Como uma funcionalidade nova atravessa as camadas

1. Entidade / value object no `domain`, com teste
2. Use case em `use-cases`, dependendo só de traits do domain
3. Implementação em `infrastructure` (repositório, disco, GPU)
4. Controller em `adapters`, ligando use case e interface
5. **Montagem no `crates/ui-gpui/src/main.rs`** e tela em
   `crates/ui-gpui/src/{biblioteca,revelacao,importacao,impressao}/`.

🚨 **O passo 5 é o que mais some, e some em silêncio.** `ExportPhotoUseCase`, `ExportController` e
`ImageExporterImpl` existem, estão testados — e **nunca são construídos no `main.rs`**. O app não
exporta nada, e nada acusa isso: os testes das camadas de dentro passam todos. Camada pronta não é
funcionalidade entregue; a pergunta é sempre **"que clique chega até aqui?"**.

## A distribuição: projeto aberto, fora das lojas, e o app se atualiza compilando

O VintageLightbox é **software livre sob licença MIT** (7/set/2026) e **não passa por loja nenhuma**.
Desde 27/set/2026 **também não há pacote**: todo balcão — macOS, Windows e Linux — instala e se
atualiza **compilando o `main`** pelo `scripts/instalar-vintagelightbox-gpui.cmd` (dono: *"a
atualização somente por script de build tá sendo a melhor opção"*; *"não precisa de .dmg e
appveyor"*). Sem conta de desenvolvedor Apple, o `.dmg` saía sem notarização e o macOS barrava a
primeira abertura; compilado na própria máquina, ele abre direto. O que ficou em `empacotamento/`,
e por que não sai, está em [`empacotamento/README.md`](empacotamento/README.md) — **ler antes de
tocar em versão, `atualizacao/`, `chave-publica.txt` ou `enderecos-de-atualizacao.txt`.**

**Como funciona.** Ao abrir, o app lê o [`docs/novidades.json`](docs/novidades.json) do `main`
(`raw` do GitHub, depois o Pages). Se a versão de lá for maior que a `CARGO_PKG_VERSION`, aparece a
faixa com as novidades, e o "Atualizar" roda o instalador, que baixa o `main` e recompila
(`atualizacao/compilar.rs`). O binário novo só entra depois de provar que abre.

**Lançar** é um commit com **três arquivos** — `Cargo.toml` (`[workspace.package] version`),
`docs/novidades.json`, com o texto para o operador, e a mesma entrada (com `"data"`) no topo de
`docs/historico-de-novidades.json`, que o diálogo "Novidades da versão" usa para navegar pelas
versões anteriores — e o `make producao` do e-commerce levar o `dev` ao `main`. Os testes
`novidades::…` prendem os três. O roteiro é a skill
`lancar-o-app-desktop`, no repositório do e-commerce.

🚨 **Produção só sai do `main`, e só com o `main` dos três projetos em dia** (dono, 24/set/2026):
este, o e-commerce e a landing `fotoamodaantiga`. O trabalho fica no `dev`. Quem confere e quem leva
o `dev` ao `main` nos três é `../recordarfotos-e-commerce/scripts/mains.sh` (`make mains` lá). Nunca
`git push origin main` daqui à mão.

🚨 **O commit da versão é o último commit de código.** Quem compilou a 0.1.N não recompila outra
0.1.N: uma correção que entra depois não chega a ninguém. E **não há volta de versão**: um lançamento
ruim se corrige com outro, maior.

⚠️ **Os `.dmg` antigos ainda existem nos balcões que os instalaram.** Eles consultam o `latest.json`
(R2 `vintagelightbox`, depois o Pages), que ficou na 0.1.24; sem pacote novo, compilam na próxima
atualização. Por isso o bucket, o `docs/latest.json`, a `chave-publica.txt` e os endereços de
atualização **ficam**, e endereço de atualização **só se acrescenta**: o app já instalado só conhece
os endereços com que foi compilado.

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
