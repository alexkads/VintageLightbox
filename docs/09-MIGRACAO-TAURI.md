# Migração da UI para Tauri — planejamento

**Escrito em**: 15 de agosto de 2026
**Estado**: ❌ **avaliado e descartado no mesmo dia.** A decisão foi **GPUI** —
veja [10-MIGRACAO-GPUI.md](10-MIGRACAO-GPUI.md)
**Base**: código em `dev` na data acima (com a árvore suja de 25 arquivos, veja §2.1)

> 🚫 **Este documento não é o plano.** Ele foi escrito antes de a motivação real ficar clara — "o
> egui fica feio e quebrado, e a IA não me ajuda rápido" — e continua valendo por um motivo só:
> **é o registro de por que Tauri foi recusado.** O Tauri resolvia o mesmo problema, mas cobrava uma
> camada de IPC entre o slider e o pixel (§3), gerenciamento de cor entregue ao webview, npm no
> build, e 15–21 semanas. O GPUI dá o mesmo ganho sem nada disso e preservando mais código.
>
> O que continua útil aqui: o inventário medido (§2), o mapa de riscos (§11) e a §15.1, que avaliou
> o GPUI e o reprovou **por um peso errado** — Windows em alpha, num projeto sem nenhum usuário.

> Este documento planeja **trocar a camada de apresentação** — hoje `crates/ui` em egui 0.31 sobre
> eframe/wgpu — por uma aplicação **Tauri 2** com frontend web. Ele **não** propõe reescrever o
> aplicativo: 14.961 das 19.357 linhas de UI são o que se reescreve; as outras quatro camadas
> (14.961 linhas de domínio, casos de uso, adapters e infraestrutura) atravessam intactas.
>
> 🚨 **Leia §1 antes de qualquer coisa.** Falta a resposta de uma pergunta — *por que Tauri?* — e ela
> muda a forma do plano, não só a prioridade. E leia §3: existe **uma** decisão técnica que carrega
> todas as outras, e ela se decide medindo, não discutindo.

---

## 0. Resumo em uma página

| Pergunta | Resposta curta |
|----------|----------------|
| O que muda? | Só a camada de apresentação. `crates/ui` (19.357 LOC) sai; entra `crates/desktop` (Tauri) + `frontend/` (TypeScript) |
| O que não muda? | `domain`, `use-cases`, `infrastructure` — **zero linhas**. `adapters` cresce e finalmente ganha testes |
| Quantos testes sobrevivem? | **332 de 478** sem tocar. Os 146 da UI (47 unit + 99 E2E `egui_kittest`) morrem com o crate |
| Qual é o risco concentrado? | O **develop view**: hoje o slider de exposição repinta via compute shader wgpu direto na textura do egui. Num webview, entra IPC no meio (§3) |
| Dá para fazer em pedaços? | **Não dentro de um processo.** É corte único por aplicativo. O que dá é manter o `ui` em egui compilando e rodando até o fim, como rollback (§13) |
| Quanto custa? | Ordem de grandeza: **14 a 21 semanas** de trabalho focado de uma pessoa (§12) — com todas as ressalvas que uma estimativa dessas merece |
| Qual é o gate? | Fase 1 é um **spike de 1 semana com número de saída** (§10). Se a latência do develop view não passar, o plano morre ali — e isso é sucesso, não fracasso |

---

## 1. 🚨 Por que Tauri — a pergunta que ainda não tem resposta escrita

Este plano foi escrito a pedido, sem a motivação declarada. Isso importa, porque **três motivações
plausíveis levam a três planos diferentes**:

| Se a motivação real for… | …o plano muda assim |
|---------------------------|---------------------|
| **A) Iterar UI mais rápido** (CSS/HTML/React em vez de layout imediato em Rust) | O plano abaixo, como está. O ganho aparece na fase 4 e o risco todo está na fase 5 |
| **B) Preparar cliente web/mobile depois** | A fase 3 (contrato) vira a fase mais importante e mais cara: os comandos precisam ser transporte-agnósticos, e `PreviewManager` precisa de um caminho remoto. O develop view local passa a ser caso particular, não o caso |
| **C) Aparência/densidade profissional** (o egui tem cara de egui) | Vale medir antes: boa parte do que incomoda em `import_view` já foi resolvida na segunda passada de 15/ago (STATUS §"Tela de importação reescrita") **dentro do egui**. Talvez o problema não seja o framework |

⚠️ **Se a resposta for (C) sozinha, a recomendação é não migrar.** 19 mil linhas de UI funcionando,
com 99 testes E2E de comportamento, são caras demais para trocar por estética que o design system
atual pode alcançar. Se for (A) ou (B) — ou (C) somada a uma delas — o plano se justifica e o gate
da fase 1 decide o resto.

**O que a migração dá, concretamente**, e que hoje não existe:

- **Layout declarativo com reflow** — o `dock_viewer.rs` (1.114 LOC) e o `import_view.rs` (2.107 LOC)
  são, em boa parte, aritmética de retângulo que CSS grid/flex faz sozinho.
- **Ferramentas de inspeção** — DevTools, profiler de layout, hot reload. Hoje, ajustar um espaçamento
  é recompilar.
- **Ecossistema de componentes** — virtualização de listas, docking, tabelas, gráficos, todos prontos.
- **Contrato explícito** — hoje a UI chama caso de uso direto e a camada `adapters` é decorativa
  (798 LOC, **0 testes**, `LibraryController` só tem `new`). Com Tauri, ela vira fronteira de
  processo e passa a ser testável de fora.

**O que a migração cobra**, e não é pouco:

- **146 testes de UI** viram zero, e o substituto no macOS é pior (§8).
- **Uma camada de IPC** entre o slider e o pixel, onde hoje não há nenhuma (§3).
- **WebView2 como dependência de runtime no Windows** — nova, e não existia com egui (§9).
- **Duas linguagens e dois ecossistemas de build** onde hoje há `cargo`.

---

## 2. O que se mexe, medido

### 2.1 ⚠️ Pré-condições — nada de Tauri começa antes disto

Três pendências do STATUS bloqueiam a migração, e todas por motivo prático:

1. **Commitar a árvore de trabalho.** Há 25 arquivos modificados e 5 não rastreados em `dev`,
   incluindo a reescrita inteira da tela de importação. Migrar por cima de trabalho não commitado
   torna impossível separar "quebrou na migração" de "já estava quebrado".
2. **Resolver as migrations 16–19** (STATUS §"Bloqueio que restou"). O app Tauri roda as mesmas
   migrations no start-up; o problema viaja junto, e no meio da migração ele vai parecer defeito novo.
3. **Catálogo redirecionável por variável de ambiente.** Hoje `AppPaths::catalog_root()` é caminho
   fixo em `~/Pictures/`. Sem redirecionar, **não existe teste E2E automatizável** do app novo — e a
   fase 8 depende disso. É trabalho pequeno em `crates/infrastructure/src/paths.rs` e vale por si só.

### 2.2 Inventário por camada

| Camada | LOC | Testes | Destino na migração |
|--------|----:|-------:|---------------------|
| `domain` | 5.736 | 202 | ✅ **intacta** |
| `use-cases` | 4.697 | 65 | ✅ **intacta** |
| `infrastructure` | 3.730 | 65 | ✅ **intacta** (SQLite, RAW/LibRaw, EXIF, cache L1/L2/L3, organizador) |
| `adapters` | 798 | **0** | 🔄 **cresce e vira a fronteira** — comandos Tauri, serialização, testes |
| `ui` (egui) | 19.357 | 146 | ❌ **substituída** — mas ~4.400 LOC de lógica migram para Rust, não para TS (§2.3) |

*(O total de testes declarado no README é 522; a soma por camada do STATUS dá 478. A divergência é do
próprio STATUS e não foi conferida aqui.)*

**Traduzindo**: 78% do código do produto não é tocado. A migração é grande porque a UI é grande, não
porque o sistema seja frágil — a Clean Architecture está cobrando o prêmio que ela vende.

### 2.3 Dentro do `crates/ui`: o que vira TypeScript e o que continua Rust

Nem tudo em `ui/` é interface. Separando pelo que o arquivo realmente faz:

| Arquivo | LOC | Vai para |
|---------|----:|----------|
| `gpu_processor.rs` + `shaders/image_adjustments.wgsl` | 578 + 456 | 🦀 **Rust** (ou WebGPU — é a decisão da §3) |
| `async_loader.rs` (`ProcessedCache`, prefetch de vizinhos) | 1.172 | 🦀 **Rust** — vira estado do app Tauri |
| `image_processing.rs` (crop/rotate em CPU) | 629 | 🦀 **Rust** |
| `monitors.rs` | 79 | 🦀 **Rust** — trocado pela API de monitores do Tauri |
| `state.rs` (`EditSnapshot`, undo/redo, filtros) | 1.355 | 🦀 **majoritariamente Rust** — veja §5 |
| `app.rs`, `views/`, `components/`, `panels/`, `docking/`, `design_system/`, `keyboard.rs` | ~14.961 | 🌐 **TypeScript** |

Ou seja: **~4.400 linhas de Rust se mudam de crate**, e ~15.000 linhas de UI imediata viram
componentes declarativos — que tipicamente encolhem, porque metade delas é layout manual.

---

## 3. 🚨 A decisão que carrega todas as outras: onde o pixel é renderizado

Hoje o caminho do slider de exposição até a tela é: `f32` → buffer uniforme wgpu → compute shader
(`@workgroup_size(16,16)`, escrita em `texture_storage_2d<rgba8unorm>`) → `ColorImage` → textura do
egui. Zero cópias entre processos, zero codificação de imagem.

Num webview, **o pixel precisa atravessar a fronteira**. Três caminhos, e eles não empatam:

### Caminho A — renderiza em Rust, entrega por protocolo customizado ✅ recomendado como base

O wgpu continua como está. O resultado é codificado (JPEG/WebP) e servido por um esquema próprio
registrado no Tauri (`vintage://preview/<id>?v=<hash-dos-ajustes>`), que o frontend consome como
`<img>` normal.

- ✅ **O `gpu_processor.rs` e o WGSL não mudam** — 1.034 linhas testadas ficam de pé.
- ✅ O protocolo customizado não passa por JSON nem base64; e para respostas de comando o Tauri 2
  aceita bytes crus (`tauri::ipc::Response`), sem inflar 33%.
- ✅ Cache de HTTP do webview funciona a favor: chave versionada, revalidação de graça.
- ❌ **Custo novo por quadro**: codificar + decodificar. É isto que o spike mede.
- **Mitigação**: durante o arraste do slider, renderizar reduzido (~1024px) e coalescer para ~30fps;
  ao soltar, render em resolução de tela. É o que o Lightroom faz, aliás.

### Caminho B — WebGPU no próprio webview, reaproveitando o WGSL

O RAW é decodificado uma vez em Rust, sobe como textura RGBA para o webview, e o **mesmo shader**
roda lá dentro. O arraste do slider passa a ser 60fps sem nenhum IPC.

- ✅ Latência volta a ser a de hoje — possivelmente melhor.
- ✅ O WGSL é padrão WebGPU; a portabilidade do shader é plausível quase verbatim.
- ⚠️ **Depende de WebGPU estar disponível no WKWebView (macOS) e no WebView2 (Windows)** — é
  exatamente o tipo de coisa que se confere rodando, não lendo release notes. **Item nº 1 do spike.**
- ❌ Duplica o pipeline: uma versão em Rust (para exportação, impressão e miniaturas, que não podem
  depender do webview) e uma no webview. Duas verdades para o mesmo shader é a receita conhecida de
  "a exportação não bate com o que eu vi na tela".
- **Se B for viável, ele não substitui A — ele acelera A.** O Rust continua sendo a verdade; o
  webview vira prévia rápida durante a interação, com reconciliação ao soltar.

### Caminho C — webview transparente sobre superfície nativa

A janela desenha o wgpu nativamente e o webview fica por cima, transparente, só com os controles.

- ✅ Latência nativa de verdade, sem cópia.
- ❌ Depende de detalhe de plataforma (NSView/Metal no macOS, HWND no Windows), fora do que o Tauri
  garante, e quebra a cada versão. Redimensionar, DPI e multi-monitor ficam por sua conta.
- ❌ Adeus DevTools no que interessa.
- **Recomendação**: só se A e B falharem o gate — e nesse caso a conclusão honesta provavelmente é
  "não migre", não "faça C".

### O que o spike tem de responder, com número

| Medida | Como | Barra sugerida |
|--------|------|----------------|
| Latência slider → pixel novo (arraste) | RAW de 24MP real, preview 2048px, cronometrado no frontend | **≤ 50 ms** para não parecer travado |
| Latência ao soltar (render cheio) | idem, resolução de tela | ≤ 250 ms |
| Grade de 2.000 miniaturas | catálogo sintético, rolagem contínua | 60fps com virtualização; primeira pintura ≤ 1 s |
| WebGPU disponível? | mesmo teste no WKWebView e no WebView2 | sim/não por plataforma |
| Janela secundária em fullscreen no 2º monitor | `WebviewWindowBuilder` + `available_monitors()` | abre no monitor certo, sem barra |

🚨 **Se a primeira linha falhar em A e B, o plano para na fase 1.** O develop view é o produto; um
slider com 200ms de atraso não é um Lightroom.

---

## 4. O contrato: `adapters` vira a fronteira

Hoje `crates/ui/src/main.rs` monta 6 repositórios, ~20 casos de uso e 6 controllers à mão e entrega
tudo para o `VintageLightboxApp`. Esse bloco de montagem (linhas 33–130 de `main.rs`) migra quase
inalterado para o `setup` do Tauri — é a parte mais barata da migração inteira.

O que muda de verdade é que **a chamada passa a atravessar processo**, e isso impõe regras:

### 4.1 Serialização — trabalho concreto e pequeno

Hoje só `PhotoViewModel` deriva `Serialize`. `ImportCandidateViewModel`, `DuplicateCheckViewModel` e
`ImportProgressViewModel` (`crates/adapters/src/view_models.rs:85-109`) são `Debug, Clone` apenas.
Todos os view models precisam de `Serialize`, e os de entrada de `Deserialize`.

### 4.2 Tipos no frontend, gerados — nunca escritos à mão

`tauri-specta` (ou `ts-rs`) gera o `.d.ts` a partir dos view models. **Tipo TypeScript escrito à mão
é dívida que só aparece em produção**: o Rust muda, o TS continua compilando, e o campo chega
`undefined`.

### 4.3 Comandos, por controller

| Controller | Comandos (leitura) | Comandos (escrita) |
|------------|--------------------|--------------------|
| `Library` | `listar_fotos`, `filtrar`, `contar` | — (hoje só tem `new`; a UI fala direto com os casos de uso) |
| `Photo` | `obter_foto`, `metadados` | `avaliar`, `rotular_cor`, `marcar_flag`, `excluir` |
| `Import` | `listar_origens`, `escanear_origem`, `descrever_candidatos`, `conferir_duplicatas` | `importar_com_opcoes` |
| `Editor` | `obter_edicoes`, `histograma` | `salvar_edicoes` (⚠️ os 55 parâmetros posicionais — §4.5) |
| `Export` | `estimar_saida` | `exportar` |
| `Preset` | `listar_presets` | `salvar_preset`, `excluir_preset` |

### 4.4 Eventos — o que muda por fora da tela

O egui repinta 60× por segundo e lê o estado; um frontend web não. Tudo que muda sem clique precisa
de `emit` do Tauri para o frontend:

- progresso de importação (`ImportProgressViewModel` já existe como enum — é o evento pronto),
- miniatura ficou pronta (o `AsyncThumbnailLoader` hoje é `poll_results()`; vira `emit`),
- prévia processada do develop view,
- cartão de memória inserido/removido (`infrastructure/src/devices/`),
- sincronização da janela secundária com a seleção da principal.

Três regras, que valem aqui pelo mesmo motivo que valem em qualquer barramento:

- **Publicar depois de a operação dar certo**, nunca antes.
- **O evento avisa, não descreve** — chegou o evento, o frontend relê pelo comando. Evento perdido
  atrasa a tela; evento como verdade faz a tela mentir.
- **Nem todo evento merece interromper** — 2.000 eventos de "miniatura pronta" precisam ser
  agregados por janela de tempo, ou o React re-renderiza 2.000 vezes.

### 4.5 ⚠️ O que **não** atravessa o IPC

- **Bytes de imagem em JSON.** Miniatura e preview vão por protocolo customizado ou resposta crua.
- **Conexão de banco.** O frontend nunca vê SQLite. Nem por plugin, nem por "só para o filtro".
- **Caminho de arquivo vindo do frontend sem validação.** Hoje a UI é confiável porque é o mesmo
  processo. Um webview não é — o comando valida que o caminho está sob uma raiz permitida.
- **`SavePhotoEditsUseCase::execute` com 55 parâmetros posicionais.** Não dá para serializar isso com
  cara de gente, e o STATUS já registra que a assinatura quebrou a suíte em silêncio por um mês.
  **Trocar por um struct `PhotoEdits` é pré-requisito da fase 3** — a migração torna obrigatório o
  conserto que já era certo.

---

## 5. Onde mora o estado — a segunda decisão que não dá para adiar

`crates/ui/src/state.rs` tem 1.355 linhas e mistura três coisas: estado de sessão de edição
(`EditSnapshot` com ~50 campos + pilha de undo/redo), estado de navegação (seleção, filtros, ordem) e
estado puramente visual (painel aberto, zoom, rolagem).

**Recomendação: a verdade de edição e seleção fica em Rust**, num `AppState` do Tauri; o frontend
guarda só o visual.

Por quê, concretamente:

- **Undo/redo** precisa das mesmas ~50 campos que o pipeline de render usa. Duplicar em TS significa
  duas cópias divergindo a cada feature nova de edição — e o STATUS já registra que o crop ficou
  **fora** do `EditSnapshot` (lacuna nº 2) mesmo com tudo num processo só.
- **A janela secundária** (monitor do cliente) precisa refletir a seleção da principal. Com a verdade
  em Rust, é um `emit` para as duas janelas. Com a verdade em TS, é sincronizar dois webviews.
- **Auto-apply do crop na navegação** (commit `47ca89f`) é regra de negócio disfarçada de UI. Em TS,
  ela vira regra que os testes de domínio não alcançam.

⚠️ **O custo dessa escolha é um round-trip por interação de slider.** É o mesmo custo que a §3 já
está medindo — por isso as duas decisões se resolvem no mesmo spike, e não separadamente.

---

## 6. Stack do frontend — recomendação

| Peça | Escolha | Por quê |
|------|---------|---------|
| Framework | **React 19 + TypeScript + Vite** | É o que o autor já usa no e-commerce; Vite é o caminho padrão do Tauri |
| Estado do cliente | **Zustand** | Estado visual é pequeno (§5); Redux seria cerimônia |
| Dados do backend | **TanStack Query** | Cache, revalidação e invalidação por evento — casa com §4.4 |
| Docking | **Dockview** | Substitui `egui_dock`; salva/restaura layout em JSON |
| Grade e filmstrip | **`@tanstack/react-virtual`** | Sem virtualização, 2.000 miniaturas matam o DOM |
| Estilo | **CSS Modules + variáveis CSS** | Os 5 temas do `design_system/theme.rs` já são tokens; viram `:root[data-tema]`. Tailwind é opcional e não paga o custo aqui |
| Ícones | **Phosphor (React)** | Mesma família do `egui-phosphor` — os ícones não mudam |
| Gráficos | **Canvas próprio** para histograma e curva de tons | São 3 gráficos interativos e específicos; biblioteca genérica atrapalharia (o `egui_plot` já é usado assim) |

**Descartados de propósito**: Next.js (SSR não faz sentido em app local, e o roteador atrapalha),
qualquer biblioteca de componentes com opinião visual forte (MUI, Chakra) — o design system já existe
e é escuro, denso, de ferramenta profissional.

---

## 7. As sete coisas que o egui faz hoje e o webview não faz de graça

Cada uma é trabalho que **não aparece em nenhuma lista de features**, e é onde migrações estouram
prazo:

1. **Atalhos de teclado** — `keyboard.rs` tem 23 teclas mapeadas. No webview, ⌘A seleciona texto,
   ⌘+/− dá zoom no documento, F11 e ⌘W fazem coisa do navegador. Todos precisam de
   `preventDefault()` explícito, e alguns só se resolvem por menu nativo do Tauri.
2. **Docking** — `egui_dock` 0.16 com serde já persiste o layout. O Dockview persiste também, mas o
   formato é outro: **o layout salvo do usuário não migra**, e isso é aceitável desde que dito.
3. **Multi-monitor e fullscreen** — hoje é `ViewportBuilder` + `display-info`. No Tauri é
   `WebviewWindowBuilder` + `available_monitors()` + `set_fullscreen`. Provavelmente **melhor** que
   hoje (janela de verdade em vez de viewport do egui), mas é reescrita de `secondary_window.rs` e
   `filmstrip_secondary_windows.rs`.
4. **Seletor de arquivos nativo** — já é `rfd` (STATUS explica por que o modal forçou a troca).
   Vira `tauri-plugin-dialog`, que é `rfd` por baixo. **Custo quase zero** — e a armadilha do modal
   escurecendo o seletor deixa de existir, porque agora é janela do SO de qualquer jeito.
5. **Impressão** — `print_view.rs` (1.049 LOC) desenha o layout no egui. Duas saídas possíveis:
   `window.print()` com `@page` em CSS (barato, mas gerenciamento de cor e margem ficam a cargo do
   webview) ou **gerar PDF em Rust** e mandar para o sistema (mais trabalho, resultado previsível).
   Para produto que vende impressão a cliente, a segunda. **Decisão pendente (§16).**
6. **Zoom e pan da imagem** — o `image_viewer.rs` faz por mesh/UV com o crop aplicado. No DOM,
   `transform: scale()` + `will-change` chega perto, mas o crop com rotação e flip por UV não tem
   equivalente CSS direto — vai para canvas ou WebGL, ou o Rust entrega já cortado.
7. **Arrastar arquivo para dentro da janela** — o Tauri intercepta (`onDragDropEvent`), mas é preciso
   desligar o comportamento padrão do webview de *navegar* para o arquivo solto. Esquecer isso é o
   clássico "meu app virou visualizador de JPEG".

---

## 8. ⚠️ Testes — o custo escondido, e o maior deles

**146 testes morrem com o `crates/ui`**: 47 unitários e **99 E2E `egui_kittest`** em 18 arquivos,
com snapshots. Eles cobrem crop (4 arquivos), filmstrip (4), filtros, atalhos, seleção, print view,
settings, janela secundária, e a tela de importação recém-reescrita.

E o substituto é pior no ambiente do autor:

- 🚨 **`tauri-driver` não roda no macOS.** O WebDriver do Tauri cobre Linux (WebKitWebDriver) e
  Windows (Edge Driver); o WKWebView do macOS não expõe WebDriver. Como a máquina de desenvolvimento
  é um M2 Pro, **o E2E do app empacotado só roda no CI, em Windows/Linux**. (Conferir no spike — se
  mudou, muda para melhor.)
- ✅ O que roda no macOS é **Playwright ou Vitest contra o frontend servido pelo Vite, com os
  comandos Tauri mockados**. Cobre interação, layout e estado — não cobre a fronteira com o Rust.
- ✅ Em compensação, **os comandos passam a ser testáveis diretamente em Rust**, sem UI — coisa que
  hoje não existe, porque a camada `adapters` tem 0 testes.

**Regra que evita a perda silenciosa** (§13, regra 4): **antes de apagar qualquer arquivo de teste do
`ui`, extrair dele uma lista de comportamentos** para `docs/PARIDADE-UI.md`. Aqueles 99 testes são a
única especificação executável do comportamento atual da interface; jogá-los fora sem transcrever é
perder o que o produto sabe fazer.

---

## 9. Empacotamento, distribuição e CI

| Item | Hoje (egui) | Depois (Tauri) |
|------|-------------|----------------|
| Runtime no Windows | nenhum | ⚠️ **WebView2** — evergreen no Win11, bootstrapper no Win10. **Custo novo** |
| Runtime no macOS | nenhum | WKWebView, parte do sistema — sem custo |
| Tamanho do binário | grande (wgpu + LibRaw) | **igual**, porque o wgpu continua (§3). O ganho clássico do Tauri **não se aplica aqui** |
| Assinatura/notarização | já necessária para distribuir | igual — **não é custo novo** |
| Build no CI | `cargo build` | `cargo` + `npm` + `tauri build`; matriz de 3 SOs continua |
| Atualização automática | não existe | `tauri-plugin-updater` — ganho real, se importar |

O CI atual (`.github/workflows/ci.yml`, 3 SOs, fmt + clippy `-D warnings` + tarpaulin) **está
vermelho desde 27/dez/2025** e fica verde quando os consertos de 15/ago forem commitados. Ele precisa
estar verde **antes** da fase 2 — senão a migração herda um sinal quebrado e ninguém sabe de quem é a
culpa.

---

## 10. Fases

Cada fase tem **entregável observável** e **critério de saída**. Fase sem critério de saída é
desejo.

### Fase 0 — Pré-condições (≈ 1 semana) — *não é Tauri ainda*

Os três itens da §2.1: commitar a árvore, decidir as migrations 16–19, catálogo por variável de
ambiente. Mais: CI verde.

**Saída**: `cargo test --workspace` verde no CI nos 3 SOs, e `VINTAGE_CATALOG_DIR=/tmp/x cargo run -p ui`
sobe contra catálogo descartável.

### Fase 1 — 🚨 Spike de decisão (timebox **1 semana**, com gate)

Um app Tauri mínimo, descartável, fora do workspace de produção. Ele faz **quatro coisas** e mede as
cinco linhas da tabela da §3: abre um RAW real, mostra um slider de exposição, mostra uma grade de
2.000 miniaturas, e abre uma janela em fullscreen no segundo monitor.

**Saída**: `docs/historico/SPIKE-TAURI.md` com os números medidos e a escolha entre os caminhos A/B/C.

🚨 **Gate**: latência de arraste > 50 ms nos caminhos A **e** B → **o plano para aqui**. Uma semana
gasta é o preço de não gastar quatro meses.

### Fase 2 — Fundação (1–2 semanas)

`crates/desktop` com Tauri 2; o bloco de montagem do `main.rs` migrado para o `setup`; protocolo
customizado de miniaturas; frontend Vite/React com os 5 temas portados para variáveis CSS; **3
comandos de leitura**.

**Saída**: a janela nova lista as fotos do catálogo real, com miniaturas, e o `crates/ui` continua
subindo intacto.

### Fase 3 — Contrato (1 semana)

`Serialize` em todos os view models; `PhotoEdits` no lugar dos 55 parâmetros; geração de tipos TS; e
os **primeiros testes da camada `adapters`** — que hoje tem zero.

**Saída**: tipos TS gerados no build; cobertura de `adapters` saindo de 0.

### Fase 4 — Biblioteca e importação (3–4 semanas)

Grade virtualizada, filmstrip, filtros, avaliação/cor/flag, árvore de pastas. Depois a importação
inteira — o modal de 4 etapas assíncronas, que exercita o barramento de eventos de ponta a ponta.

**Saída**: importar um cartão de verdade pelo app novo, com paridade conferida contra a lista
extraída de `import_view_e2e_test.rs`.

### Fase 5 — 🚨 Develop view (4–6 semanas) — o risco concentrado

Pipeline de imagem conforme decidido na fase 1, histograma, curva de tons, HSL 8 canais, correção de
lente, ruído/nitidez, crop overlay, undo/redo, presets.

**Saída**: editar um RAW e exportar com **o mesmo resultado de pixel** do app egui. Comparação
automatizada de imagem, não olhômetro.

⚠️ Aqui aparecem as lacunas que o STATUS já registra (exportação ignora o crop; undo/redo ignora o
crop). **Elas se consertam nesta fase**, porque reproduzir defeito conhecido de propósito custa mais
do que arrumar — mas o conserto vai com teste que registra o que mudou.

### Fase 6 — Multi-monitor, docking, impressão, exportação, atalhos (3–4 semanas)

**Saída**: apresentação para cliente no segundo monitor, layout persistido, impressão conferida em
papel de verdade.

### Fase 7 — Empacotamento e desligamento (1–2 semanas)

Bundles assinados nos dois SOs, CI verde com os dois frontends, e **só então** `crates/ui` sai do
workspace — com o commit de remoção isolado, para o `git revert` ser um comando.

**Saída**: instalador funcionando em máquina limpa nos dois SOs.

---

## 11. Riscos, e o que dispara aborto

| Risco | Probabilidade | Impacto | Gatilho de aborto |
|-------|---------------|---------|-------------------|
| Latência do develop view | **média** | fatal | Fase 1, tabela §3 |
| WebGPU indisponível nos dois webviews | média | contorna com caminho A | — |
| E2E ausente no macOS trava o desenvolvimento | **alta** | alto | Se o ciclo de trabalho ficar insuportável na fase 4, reavaliar |
| Paridade se perde silenciosamente | **alta** se a §8 for ignorada | alto | — (é processo, não gate) |
| Cansaço de escopo no meio (fase 5) | média | fatal por abandono | Fase 5 passando de 8 semanas → parar e reavaliar com o `ui` ainda vivo |
| Gerenciamento de cor pior no webview | média | médio na impressão | Decisão §16.3 |

**O padrão de falha mais provável não é técnico**: é chegar na fase 5, cansar, e ficar com dois
frontends pela metade. Por isso o `crates/ui` fica vivo e funcionando até a fase 7 — abandonar a
migração no meio precisa custar zero.

---

## 12. Esforço — ordem de grandeza, com as ressalvas

| Fase | Semanas |
|------|--------:|
| 0 — pré-condições | 1 |
| 1 — spike | 1 |
| 2 — fundação | 1–2 |
| 3 — contrato | 1 |
| 4 — biblioteca + importação | 3–4 |
| 5 — develop view | 4–6 |
| 6 — multi-monitor, docking, print | 3–4 |
| 7 — empacotamento | 1–2 |
| **Total** | **15–21 semanas** |

⚠️ **O que torna esse número errado**: ele supõe uma pessoa em dedicação e nenhuma feature nova no
meio. A fase 5 é a mais provável de dobrar. E a estimativa **não** inclui reescrever os 99 testes E2E
— inclui só extrair a lista de paridade deles.

---

## 13. Regras invioláveis durante a migração

1. ❌ **Nenhuma feature nova enquanto a migração acontece.** A conferência é **paridade**; feature
   nova torna impossível saber se a diferença é bug ou escopo.
2. ✅ **O `crates/ui` compila e roda até a fase 7.** É o rollback, e é a referência de paridade.
3. ❌ **Nunca copiar componente egui para TS traduzindo linha a linha.** Layout imediato traduzido
   para DOM vira o pior dos dois mundos. Portar é reescrever com a regra entendida.
4. 🚨 **Nenhum teste do `ui` é apagado antes de virar linha em `docs/PARIDADE-UI.md`** (§8).
5. ❌ **O frontend nunca fala com SQLite, com o sistema de arquivos ou com LibRaw.** Só comandos.
6. ✅ **Um incremento por commit**, com mensagem contando o que foi **encontrado**, não só o que foi
   feito — como no resto do projeto.
7. ✅ **Toda migration nova continua sendo assunto do `infrastructure`**, e o problema das
   16–19 (§2.1) não viaja para dentro da migração.

---

## 14. O que este plano **não** propõe

- Reescrever domínio, casos de uso ou infraestrutura. **Zero linhas.**
- Trocar SQLite, LibRaw, wgpu ou o modelo de cache L1/L2/L3.
- Redesenhar a interface. Paridade primeiro; redesenho é decisão separada, **depois**.
- Levar o app para a web ou para mobile. A §4 deixa a porta aberta se a motivação (B) for a real, mas
  **não é objetivo deste plano**.
- Resolver as lacunas do STATUS que não estão no caminho — coleções na UI, testes de `adapters` além
  do que a fase 3 exige.

---

## 15. Alternativas consideradas

| Alternativa | Por que não |
|-------------|-------------|
| **Ficar no egui** e investir no design system | ✅ **É a alternativa séria**, e é a resposta certa se a motivação for só (C) da §1. Custo zero, risco zero, e a segunda passada da tela de importação (15/ago) é prova de que dá para chegar longe |
| **GPUI** (o framework de UI do Zed) | 🔍 **A alternativa mais séria depois de "ficar no egui"** — e reprovada por uma coisa só. Veja §15.1 |
| **Blade** (a biblioteca gráfica do kvark) | ❌ **Categoria errada, e obsoleta para este fim.** Blade não é framework de UI — é abstração de GPU, o lugar do `wgpu`. E o Zed **removeu** o Blade em favor do `wgpu` em 13/fev/2026 ([PR #46758](https://github.com/zed-industries/zed/pull/46758)), citando travamentos em NVIDIA/Wayland. O projeto já está em `wgpu 23`, que é exatamente onde o Zed foi parar |
| **Slint** | É o que a documentação antiga (`02`, `05`, `06`) ainda descreve e que já foi abandonado uma vez na prática. Repetir a troca sem motivo novo é trocar de problema |
| **Dioxus desktop** | Mesmo webview, ecossistema muito menor, e o Rust no frontend não resolve o problema de IPC da §3 — só o disfarça |
| **Electron** | Descartado: perderia o binário único e toda a integração nativa em Rust |
| **Migrar só uma tela para Tauri**, mantendo o resto em egui | ❌ Não existe: são dois processos, dois catálogos abertos no mesmo SQLite, dois caches. Corte é único por aplicativo |

### 15.1 GPUI, em detalhe — por que quase, e por que não

Avaliado em 15/ago/2026, a pedido, antes de decidir por Tauri.

**O que o GPUI resolveria, e não é pouco:**

- 🎯 **Mata a §3 inteira.** Renderização segue no processo, na GPU, sem IPC entre o slider e o pixel.
  O maior risco do plano Tauri — e o gate da fase 1 — simplesmente deixa de existir.
- ✅ **Os ~4.400 LOC de Rust da §2.3 ficam onde estão** (`gpu_processor`, `async_loader`,
  `image_processing`, `state`). No Tauri eles mudam de crate; aqui, nem isso.
- ✅ **Atende a queixa concreta da §1(A)**: layout flexbox e API de estilo no espírito do Tailwind, em
  vez da aritmética de retângulo que enche `dock_viewer.rs` e `import_view.rs`.
- ✅ **`gpui-component`** (longbridge) traz 60+ componentes, incluindo **docking serializável**,
  tabelas virtualizadas e gráficos — cobrindo §7.2 e boa parte da §6.

**O que reprova, e basta uma linha:**

- 🚨 **Windows.** O suporte do próprio Zed a Windows ainda é **alpha**, com relatos em 2026 de
  *DirectX device removal* e flicker ([issue #36798](https://github.com/zed-industries/zed/issues/36798)).
  Este produto declara **Windows 10+ como plataforma de primeira classe** (README e matriz do CI).
  Apostar a camada de apresentação inteira num renderizador em alpha na metade do público é a única
  linha que decide sozinha.
- ⚠️ **Pre-1.0** — `gpui 0.2.0` no crates.io desde out/2025, com quebras assumidas entre versões. Não
  é pior que o egui (que já obrigou 0.28 → 0.31 aqui), mas também **não é melhor**: troca-se de churn,
  não se sai dela.
- ⚠️ **Sem DevTools e sem hot reload.** Metade da motivação (A) — ajustar espaçamento sem recompilar —
  continua sem resposta. É o ponto em que o Tauri ganha limpo.
- ⚠️ **Os 146 testes de UI morrem do mesmo jeito** (§8). O GPUI não devolve nada equivalente aos 99
  `egui_kittest` com snapshot.
- ❌ **Não serve à motivação (B)** da §1. Se o objetivo for cliente web ou mobile depois, GPUI é
  desktop nativo e ponto final.

**Veredito**: GPUI só ganha do Tauri se a motivação for (A)+(C) **e** o Windows deixar de ser
primeira classe. Enquanto o Windows estiver no README e no CI, é não.

⚠️ **E o incômodo que o GPUI expõe**: o ganho principal dele sobre o egui — GPU no processo, Rust
puro, sem IPC — **o egui já dá hoje**. A troca seria reescrever ~15.000 LOC por ergonomia de layout.
Isso reforça, não enfraquece, a conclusão da §1: se a motivação não for (A) ou (B), a resposta certa
é ficar onde está.

---

## 16. Decisões pendentes

| # | Decisão | Quem decide | Quando |
|---|---------|-------------|--------|
| 1 | 🚨 **Qual é a motivação real** (§1: A, B ou C) | dono | **antes da fase 1** |
| 2 | Caminho de renderização A / B / C (§3) | medição | fase 1 |
| 3 | Impressão: `window.print()` ou PDF gerado em Rust (§7.5) | dono | fase 6 |
| 4 | Destino das migrations 16–19 (herdado do STATUS) | dono | fase 0 |
| 5 | Layout de docking do usuário: migrar ou recomeçar (§7.2) | dono | fase 6 |
| 6 | Se a fase 1 reprovar: ficar no egui ou tentar o caminho C | dono | fase 1 |

---

## Referências no código

| Assunto | Onde |
|---------|------|
| Montagem de dependências a migrar para o `setup` do Tauri | [crates/ui/src/main.rs:33-130](../crates/ui/src/main.rs#L33-L130) |
| Estado de edição e undo/redo (§5) | [crates/ui/src/state.rs:20-78](../crates/ui/src/state.rs#L20-L78) |
| Pipeline GPU e parâmetros (§3) | [crates/ui/src/gpu_processor.rs](../crates/ui/src/gpu_processor.rs) |
| Shader — 456 linhas, compute (§3) | [crates/ui/src/shaders/image_adjustments.wgsl](../crates/ui/src/shaders/image_adjustments.wgsl) |
| Cache de miniaturas/previews que alimenta o protocolo (§3) | [crates/infrastructure/src/cache/preview_manager.rs](../crates/infrastructure/src/cache/preview_manager.rs) |
| Carregamento assíncrono que vira eventos (§4.4) | [crates/ui/src/async_loader.rs](../crates/ui/src/async_loader.rs) |
| View models sem `Serialize` (§4.1) | [crates/adapters/src/view_models.rs:85-109](../crates/adapters/src/view_models.rs#L85-L109) |
| Janela secundária a reescrever (§7.3) | [crates/ui/src/components/secondary_window.rs](../crates/ui/src/components/secondary_window.rs) |
| Os 99 E2E que viram lista de paridade (§8) | [crates/ui/tests/](../crates/ui/tests/) |
| Caminho fixo do catálogo (§2.1) | [crates/infrastructure/src/paths.rs](../crates/infrastructure/src/paths.rs) |

---

**Status deste documento**: proposta. Nenhuma linha de código foi escrita, nenhuma dependência foi
adicionada, e a fase 0 não começou.
