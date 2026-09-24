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
make publicar   # publica dist/ no R2 e espelha no GitHub (o único caminho de lançar)
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

# Os instaladores — **cada máquina gera a sua**, e nada atravessa plataforma
make mac                 # .app + .dmg universal (só no macOS)
make linux               # .deb + .AppImage    (só num Linux)
.\scripts\empacotar.ps1  # .msi + .exe         (só num Windows 11)
make publicar            # junta dist/ das três máquinas e publica (sem GitHub Actions)
# 🔑 **Uma máquina por plataforma, e isso é decisão de desenho** (dono,
#    7/set/2026: *"quero deixar tudo nativo mesmo"*). O `empacotar.sh` gera **só
#    o sistema em que ele roda**; um alvo de outro sistema recusa de imediato.
#
# 🚫 **As duas alternativas foram construídas, funcionaram, e foram recusadas**
#    em 7/set/2026: o Windows por `cargo-xwin` (gerou um `.exe` de 34,9 MB) e o
#    Linux por contêiner Docker (gerou o `.deb`). O motivo é o mesmo nos dois, e
#    é o que decide: **o que sai de uma máquina que não é a de destino, ninguém
#    abre para conferir** — um contêiner não tem X11 nem GPU; um `.exe` cruzado
#    não roda no Mac. Não refazer: `empacotamento/README.md` tem o registro.

# Check code (faster than build)
cargo check --workspace

# Format and lint
cargo fmt --all
cargo clippy --workspace
```

## CI: o AppVeyor roda o instalador do balcão (desde 24/set/2026)

O GitHub Actions está travado por cobrança desde 17/set, e o `.github/workflows/` não roda. Quem
confere agora é o **AppVeyor** (<https://ci.appveyor.com/project/alexkads/vintagelightbox>),
configurado em [`appveyor.yml`](appveyor.yml). É gratuito para projeto público e roda um job por
vez.

- **O que ele roda é o instalador**, `scripts/instalar-vintagelightbox-gpui.cmd`. No Windows é o
  bloco PowerShell, extraído do checkout como o `.cmd` faz; no Linux é o `sh`, com a mesma linha
  do balcão. Ele não roda `cargo test`. A pergunta que ele responde é "o próximo balcão que repetir
  a instalação compila?".
- **Ele roda no `dev`, antes do `main`.** O balcão compila o `main`, e o `dev` só chega lá pelo
  `make mains` do e-commerce, que avisa se o AppVeyor do `dev` não estiver verde. É a última chance
  de pegar um `dev` que não compila antes de ele virar produção.
- **Ele compila o branch como está no GitHub**, porque o instalador baixa
  `archive/refs/heads/<branch>`. E o `Cargo.lock` não é versionado: uma build vermelha sem commit
  novo costuma ser dependência nova quebrada, a mesma que o balcão pegaria.
- **Quando falha no Linux**, o registro do instalador sobe como artefato da build. É o mesmo
  arquivo que o balcão mandaria.
- 🚨 **Nenhum segredo vai para lá.** Ele não assina nem publica. A chave minisign e o wrangler do
  R2 ficam na máquina de quem lança; um CI de terceiro com a chave privada poderia mandar
  "atualização" a todo balcão.

Status da última build:
`curl -s https://ci.appveyor.com/api/projects/alexkads/vintagelightbox | python3 -m json.tool | grep -m3 status`.

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
   `crates/ui-gpui/src/{biblioteca,revelacao,importacao,impressao}/`.

🚨 **O passo 5 é o que mais some, e some em silêncio.** `ExportPhotoUseCase`, `ExportController` e
`ImageExporterImpl` existem, estão testados — e **nunca são construídos no `main.rs`**. O app não
exporta nada, e nada acusa isso: os testes das camadas de dentro passam todos. Camada pronta não é
funcionalidade entregue; a pergunta é sempre **"que clique chega até aqui?"**.

## A distribuição: projeto aberto, fora das lojas, e o app se atualiza sozinho

O VintageLightbox é **software livre sob licença MIT** (7/set/2026) e **não passa por loja nenhuma**.
A partir da primeira instalação ele se atualiza sozinho. O caminho inteiro e as regras estão em
[`empacotamento/README.md`](empacotamento/README.md); **ler a seção "O que não pode acontecer com
quem já tem o app" antes de tocar em versão, empacotamento, `atualizacao/`, `lancar-local.sh`,
`montar-manifesto.py` ou `enderecos-de-atualizacao.txt`.**

**Como funciona.** Ao abrir, o app lê `latest.json` nos endereços de
[`empacotamento/enderecos-de-atualizacao.txt`](empacotamento/enderecos-de-atualizacao.txt), em
ordem. Primeiro o bucket R2 `vintagelightbox`
(`https://pub-f97274c3a83f47ff903a64fc1578efb0.r2.dev`); se ele falhar, o GitHub Pages. Se a versão
do manifesto for maior que a `CARGO_PKG_VERSION`, aparece a faixa "Versão X disponível" com
**Atualizar**. O pacote é baixado, a assinatura minisign é conferida e só então ele é instalado.

**Lançar é `make publicar`** (`scripts/lancar-local.sh`), sempre. `make lancar` depende do GitHub
Actions, travado por cobrança desde 17/set/2026: empurra a tag e nada compila. O `publicar`:

- confere o R2 pelo `wrangler` logado;
- monta o manifesto a partir do `dist/` inteiro, com as três máquinas juntadas nele;
- **recusa o que prejudicaria quem já tem o app**: versão menor que a do ar, plataforma que some,
  plataforma que estreia sem decisão do dono e versões divergentes entre `Cargo.toml` e
  `packager.toml`;
- envia ao R2 com o `latest.json` por último;
- espelha no GitHub (Pages + Releases). Se o GitHub recusar, é só aviso.

O roteiro passo a passo é a skill `lancar-o-app-desktop`, no repositório do e-commerce.

🚨 **Produção só sai do `main`, e só com o `main` dos três projetos em dia** (dono, 24/set/2026):
este, o e-commerce e a landing `fotoamodaantiga`. O trabalho fica no `dev`. O balcão é produção:
o instalador compila o `main`, e o `make publicar` recusa se o checkout não for o `main` ou se algum
`main` estiver atrás do `dev`. Quem confere e quem leva o `dev` ao `main` nos três é
`../recordarfotos-e-commerce/scripts/mains.sh` (`make mains` lá). Nunca `git push origin main`
daqui à mão.

🚨 **Commit em `dev` não chega ao balcão empacotado.** Só uma versão nova publicada chega. E **não
há volta de versão**: o updater só instala versão maior, então um lançamento ruim se corrige com
outro, maior.

🚨 **A maior parte dos balcões instalou compilando** (`instalar-vintagelightbox-gpui.cmd`, que
compila o `main`), e não pelo pacote. Esses se atualizam repetindo o instalador. Em 24/set/2026 o
manifesto só tem macOS; publicar Windows ou Linux pela primeira vez faz o updater desses balcões
instalar o pacote por cima ou ao lado da instalação compilada. Está no README, em "Plataforma
nova", e o script recusa sem `--estrear-plataforma`.

🔑 **O que substitui a loja é a assinatura minisign.** Cada pacote é assinado por
`scripts/empacotar.sh`. A chave **pública** é compilada dentro do app
(`atualizacao::porta::CHAVE_PUBLICA`); a **privada** mora em `~/.vintagelightbox/atualizacao.key`,
fora do repositório.

🚨 **Perder a chave privada quebra a atualização de todo app já instalado.** Os que estão na rua só
aceitam pacote assinado por ela, e isso não se conserta pelo software. Trocar `chave-publica.txt` por
uma que não corresponda transforma toda atualização em "assinatura inválida", em silêncio.

⚠️ **Endereço de atualização só se acrescenta.** O app já instalado só conhece os endereços com que
foi compilado. Trocar o bucket, o r2.dev ou pôr um domínio próprio é pôr o novo **ao lado**, e tirar
o velho só quando todo balcão tiver passado por uma versão que conhece o novo. O GitHub Pages está
preso na lista por teste pelo mesmo motivo: o app 0.1.9 e anteriores só conhecem ele.

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
