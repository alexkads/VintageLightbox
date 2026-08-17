# Migração da UI para GPUI — história

**Escrito em**: 15 de agosto de 2026
**Estado**: ✅ **concluída em 17/ago/2026** — as cinco fases fecharam, e o `crates/ui` saiu do
workspace (fase 5). O plano de Tauri ([09](09-MIGRACAO-TAURI.md)) foi avaliado e descartado.
**Base**: código em `dev` na data acima, mais um **spike compilado e rodando** (§1)

> 🚨 **Este documento deixou de ser plano e virou história em 17/ago/2026.** O objetivo do projeto
> mudou no mesmo dia: não é mais "trocar de framework com paridade", é
> [**um clone funcional do Lightroom**](00-OBJETIVO.md). As **regras da §7 estão revogadas** — em
> especial *"nenhuma feature nova"*, que existia para separar defeito de porte de escopo divergente e
> agora só impede o app de ficar pronto.
>
> ⚠️ **Mas ele não ficou obsoleto**: é o melhor registro que existe de **por que o código é como é**.
> O BGRA das texturas, o `uniform` que casa por posição, o `track_focus` que rastreia e não concede,
> o `SliderState::set_value` que não emite `Change`, a fluidez que só se mede em `--release` — tudo
> isso continua valendo, e cada linha custou pelo menos um commit para ser aprendida.
>
> A fila de trabalho de agora está em [`PARIDADE-LIGHTROOM.md`](PARIDADE-LIGHTROOM.md).

> Trocar `crates/ui` — hoje egui 0.31 sobre eframe/wgpu — por **GPUI**, o framework do Zed, com
> `gpui-component` por cima. Continua Rust puro, no mesmo processo, na GPU.
>
> **Por que não Tauri**: os dois resolviam o mesmo problema, mas o Tauri cobrava uma camada de IPC
> entre o slider e o pixel, gerenciamento de cor entregue ao webview, e npm no build. O GPUI dá o
> mesmo ganho de aparência e de velocidade de iteração sem nada disso — e **preserva mais código**.

---

## 0. A decisão, e o primeiro passo

| | |
|---|---|
| **O quê** | `gpui 0.2.2` + `gpui-component 0.5.1`, ambos do crates.io |
| **Por quê** | Layout flexbox de verdade e API de estilo no formato do Tailwind, em vez de aritmética de retângulo |
| **O que sobrevive** | As 4 camadas internas (14.961 LOC) **e** ~4.300 LOC de Rust dentro de `ui/` — acopladas ao egui em apenas **17 pontos** (§2) |
| **O que custa** | Reescrever ~15.000 LOC de interface e perder 146 testes de UI |
| **Prazo** | 11 a 15 semanas (§5) — menos que as 15–21 do Tauri, porque sobra mais código |
| **Primeiro passo** | Fase 1: a **Biblioteca**, num crate novo ao lado do atual. Duas semanas, e os dois apps rodando |

---

## 1. ✅ O que já foi provado — não é estimativa

Um spike foi construído em 15/ago/2026 antes de escrever este plano: a tela de **Revelação**
inteira, com os campos reais do `EditSnapshot`.

| Medida | Resultado |
|--------|-----------|
| Resolução de dependências | `gpui 0.2.2` + `gpui-component 0.5.1`, **direto do crates.io** — sem pin de git |
| Árvore | 756 pacotes |
| Compilação | ✅ **de primeira**, zero erro, em ~600 linhas de tela densa |
| Tempo de build da tela (deps quentes) | 27 s |
| Consultas à documentação | 2 (gradiente e métodos de estilo) |
| App | ✅ abre e responde a clique |

**A API é esta** — e é o ponto inteiro da migração:

```rust
div().flex().flex_col().gap(px(4.))
    .bg(rgb(0x1b1b1b)).rounded(px(3.))
    .border_b_1().border_color(rgb(0x303030))
```

Compare com o que existe hoje: [`advanced_slider.rs`](../crates/ui/src/components/advanced_slider.rs)
são **290 linhas** para *um* slider — `RAIL_HEIGHT`, `KNOB_RADIUS`, `pos2`, `Rect`, e
`ui.memory(|mem| mem.data.get_temp::<bool>(...))` para lembrar se o campo está em edição. O
`gpui-component` tem [`slider.rs`](https://github.com/longbridge/gpui-component) pronto.

### 🚨 O achado que não estava previsto: Metal Toolchain

**GPUI compila os shaders Metal em tempo de build.** O build falha com:

```
error: gpui@0.2.2: metal shader compilation failed:
error: cannot execute tool 'metal' due to missing Metal Toolchain
```

Conserto: `xcodebuild -downloadComponent MetalToolchain` (já feito nesta máquina em 15/ago).

⚠️ **Isto é dependência nova de build, e vale para o CI também.** O wgpu do egui compila shader em
*runtime* e nunca exigiu nada disso. O runner macOS do GitHub Actions precisa desse passo a mais.

### ⚠️ E o Linux do gpui 0.2.2 ainda é Blade

O lock resolve **`blade-graphics 0.7.1`**, e nenhum `wgpu`. Ou seja: a remoção do Blade em favor do
wgpu ([PR #46758](https://github.com/zed-industries/zed/pull/46758), merge em 13/fev/2026)
**ainda não chegou ao crate publicado**. No macOS isso não importa — lá o renderizador é Metal
direto. Importa no **runner Ubuntu do CI**, que passa a compilar o caminho que o próprio Zed chamou
de bagunça.

**Decisão**: tirar o Ubuntu da matriz do CI. O produto declara macOS e Windows
([README](../README.md)); o Linux estava lá por inércia e passa a custar caro.

---

## 2. O que sobrevive — medido, e é mais do que o Tauri preservaria

### 2.1 As quatro camadas internas: intactas

| Camada | LOC | Testes | Destino |
|--------|----:|-------:|---------|
| `domain` | 5.736 | 202 | ✅ zero linhas alteradas |
| `use-cases` | 4.697 | 65 | ✅ zero linhas alteradas |
| `adapters` | 798 | 0 | ✅ zero linhas alteradas |
| `infrastructure` | 3.730 | 65 | ✅ a única mudança já foi feita: `image` 0.24 → 0.25 (§3.2) |

Diferente do Tauri, **nada precisa virar comando serializável, nada precisa de `Serialize`, e o
`SavePhotoEditsUseCase::execute` com 55 parâmetros continua feio sem bloquear nada.** (Continua
valendo consertar — mas agora é dívida, não pré-requisito.)

### 2.2 Dentro de `ui/`: ~4.300 linhas de Rust ficam, com 17 pontos de solda

Esta é a medida que decidiu entre GPUI e Tauri:

| Arquivo | LOC | Referências a egui | O que fazer |
|---------|----:|-------------------:|-------------|
| `gpu_processor.rs` | 578 | **1** | Trocar o tipo de saída. O pipeline wgpu inteiro fica |
| `shaders/image_adjustments.wgsl` | 456 | 0 | ✅ intocado |
| `async_loader.rs` | 1.172 | **1** | Trocar o tipo de saída |
| `image_processing.rs` | 629 | 5 | Trocar `ColorImage`/`TextureHandle` |
| `state.rs` | 1.355 | 10 | `EditSnapshot` fica; a parte de textura sai |
| `monitors.rs` | 79 | 0 | ✅ intocado (`display-info` continua servindo) |

`ColorImage` aparece em **13 lugares**, `TextureHandle` em **8 arquivos**. É toda a fronteira.

⚠️ **O resto — `app.rs`, `views/`, `components/`, `panels/`, `docking/`, `design_system/`,
`keyboard.rs`, ~15.000 LOC — é reescrita.** Não existe versão barata disso em nenhum framework.

---

## 3. As duas fronteiras técnicas reais

### 3.1 `ColorImage` → `RenderImage`

O GPUI exibe imagem por `RenderImage`, que carrega `SmallVec<[Frame; 1]>` — `Frame` **do crate
`image`**, que a `infrastructure` já usa. O caminho fica mais curto que hoje:

```
hoje:  DynamicImage → ColorImage → egui::TextureHandle
depois: DynamicImage → RgbaImage → Frame → RenderImage
```

Some a conversão para `ColorImage`, que hoje é cópia pura de bytes.

### 3.2 ✅ `image` 0.24 → 0.25 — feito em 15/ago/2026

O `gpui` resolve **`image 0.25.10`**; `infrastructure` e `ui` estavam em **0.24**. Duas versões
convivem na árvore, mas `DynamicImage` de 0.24 **é um tipo diferente** de `DynamicImage` de 0.25 —
não há conversão implícita, e passar um para o outro não compila.

Era pré-requisito da fase 1, e **saiu barato**: quatro pontos de código, todos na
`infrastructure`. A API que mudou é uma só — `encoder.encode(...)` passou a exigir
`ExtendedColorType` no lugar de `ColorType`.

**Placar depois do bump: 522 passando, 0 falhando, 3 ignorados** (`cargo test --workspace`).

#### 🚨 O que o bump revelou: o cache de preview gravava JPEG com alfa

`save_thumbnail` e `save_preview` passavam `image.color()` direto ao encoder JPEG. Até o 0.24 isso
era **aceito sem reclamação** — bytes de 4 canais entregues a um formato que só tem 3. O 0.25
recusa:

```text
The encoder or decoder for Jpeg does not support the color type `Rgba8`
```

🔑 **O caminho certo já existia no mesmo crate**: `image_exporter.rs` sempre fez `to_rgb8()` antes
de encodar. Eram duas rotas para a mesma decisão e só uma estava certa — agora é uma função só
(`encode_jpeg`), com a razão escrita ao lado dela.

⚠️ **Não é uma quebra que o bump causou; é uma que ele expôs.** O teste
`test_cache_stats_and_clearing` já existia e passava — porque a versão antiga da biblioteca não
reclamava. Vale para o resto da migração: erro novo depois de subir dependência merece a pergunta
"isto passou a estar errado, ou passou a ser detectado?".

---

## 4. Mapa: o que era egui vira o quê

### Tem equivalente pronto no `gpui-component`

| Hoje | Depois | Onde usa |
|------|--------|----------|
| `egui_dock` 0.16 | `dock/` | `docking/dock_viewer.rs` (1.114 LOC) |
| `AdvancedSlider` (290 LOC) | `slider.rs` | os ~50 ajustes de revelação |
| `egui_plot` | `plot/`, `chart/` | `histogram_plot.rs`, `tone_curve.rs`, `metadata_charts.rs` |
| `egui-notify` | `notification.rs` | avisos |
| `egui-phosphor` | `icon.rs` | ícones |
| `folder_tree.rs` (317 LOC) | `tree.rs` | árvore de pastas |
| painéis sanfonados do develop | `accordion.rs`, `collapsible.rs` | `develop_view.rs` |
| `photo_grid` / `filmstrip` | `list/` (virtualizada) | grade e filmstrip |
| tabela de metadados | `table/` | painel de informações |
| undo/redo à mão em `state.rs` | `history.rs` | histórico de edição |
| `settings_dialog`, `import_dialogs` | `dialog.rs`, `sheet.rs`, `form/` | modais |
| 5 temas em `design_system/theme.rs` | `theme/` | design system |
| `egui_file` / `rfd` | **`rfd` continua** | seletor nativo — sem mudança |

### Não tem equivalente — é código nosso, dos dois lados

- **`crop_overlay.rs`** (432 LOC) e a renderização por mesh/UV — desenho customizado.
- **Janela secundária / multi-monitor** — GPUI abre múltiplas janelas; `monitors.rs` continua servindo.
- **`print_view.rs`** (1.049 LOC) — o layout de impressão é desenho nosso em qualquer framework.
- **A exibição da imagem editada** — §3.1.

---

## 5. Fases

### Fase 0 — Pré-condições ✅ **concluída em 15/ago/2026**

1. ✅ **Árvore commitada** — sobraram só os documentos desta decisão, que entraram junto.
2. ✅ **Migrations 16–19 resolvidas** — o catálogo antigo virou `.bak` e o app criou um limpo com
   as 15 do repositório ([STATUS](STATUS.md)). Conferido: o banco novo registra `15 migrations,
   última v15`.
3. ✅ **Catálogo por variável de ambiente** — `VLB_CATALOG` (`f906451`). 🚨 E não era hipotético:
   `develop_view_controls_e2e_test.rs` chamava `PreviewManager::new()`, então **rodar `cargo test`
   escrevia no cache real do fotógrafo**. O comentário ali dizia "no longer needs temp directory" e
   o `use tempfile::tempdir` seguia importado sem uso — o import era o rastro de que aquilo já
   tinha sido certo.
4. ✅ **`image` 0.24 → 0.25** (§3.2, `3d5ba78`) — quatro pontos de código, e um defeito de alfa no
   cache de preview que só apareceu porque a versão nova reclama.
5. ✅ **CI verde** (`0d86077`, `55a6904`, `fb95adc`), sem o Ubuntu (§1), com a Metal Toolchain.
   🚨 **O gatilho apontava para um branch `develop` que nunca existiu** — o trabalho é em `dev`.
   Enquanto isso durou, `fmt` e `clippy` acumularam reprovação sem ninguém ver: 146 arquivos fora
   de formatação e 125 avisos nas camadas internas. "CI verde" era uma afirmação que não tinha como
   ser conferida.

**Placar ao fim da fase 0**: 526 testes passando, 0 falhando, 3 ignorados; `fmt` limpo; `clippy`
com `-D warnings` limpo nas quatro camadas que sobrevivem à migração.

🔑 **O que os quatro achados têm em comum**: nenhum deles falhava. O JPEG com alfa era aceito, o
teste escrevia no catálogo real sem erro, o `to_string` sombreado dava o mesmo texto, e o CI não
reclamava porque não rodava. **Pré-condição de migração é onde o silêncio custa mais caro** — o
que não avisa agora vira "o GPUI quebrou isso" daqui a três meses.

### Fase 1 — Biblioteca, num crate ao lado ✅ **critério de saída atingido em 15/ago/2026**

`crates/ui-gpui` entra no workspace **sem tirar `crates/ui`**. Os dois compilam, os dois rodam:
`cargo run -p ui` e `cargo run -p ui-gpui`.

Entrega: grade virtualizada com miniaturas de verdade, filmstrip, árvore de pastas, filtros,
nota/cor/sinalizador. Inclui a ponte de imagem da §3.1 — sem ela não há miniatura.

**Critério de saída**: abrir o catálogo real e navegar 2.000 fotos a 60fps. ✅ **Atingido** — ver
abaixo. **É aqui que você decide se gosta de morar nisso**, e a decisão foi ficar.

#### O que já está de pé

| | |
|---|---|
| ✅ Crate no workspace, compilando ao lado do `ui` | `6ba696b` |
| ✅ Ponte de imagem `DynamicImage → RenderImage` (§3.1) | `6ba696b` |
| ✅ Grade virtualizada (`uniform_list`), com a lógica de linhas testada | `c42f504` |
| ✅ Miniaturas do cache, sob demanda | `e355676` |
| ✅ Cache com descarte (LRU), capacidade tirada da janela | `ff3ed2a` |
| ✅ Filtros de nota e sinalizador | `68dfc9d` |
| ✅ Árvore de pastas e filtro por cor | `c7ea9f9` |
| ✅ Seleção e filmstrip | `7bb02c7` |
| ✅ Busca por texto na barra | `cae826a` (já na fase 2 — dependia da base) |
| ✅ Carregamento assíncrono das fotos — **medido, e não era dívida** (abaixo) | `medir-abertura` |

### ✅ Critério de saída atingido — 15/ago/2026

**2.000 fotos, navegando fluido.** Conferido pelo dono, rolando a grade: *"agora ficou muito bom"*.
É a resposta que a fase 1 existia para dar, e ela veio antes de qualquer reescrita da Revelação.

#### 🚨 A armadilha que quase deu a resposta errada: medir fluidez em `debug`

A primeira conferência foi feita com `target/debug`, e o veredito foi *"não está fluido"*. Estava
certo — e o culpado não era o GPUI, nem a grade, nem o cache.

| | debug | release |
|---|------:|--------:|
| por miniatura (SQLite + decode JPEG + BGRA) | **47 ms** | **0,84 ms** |
| uma linha de 6 colunas | 280 ms — **17 quadros perdidos** | 5 ms |
| já em cache | 0,0004 ms | ~0 ms |

`[profile.dev] opt-level = 0` no workspace: decodificar JPEG e trocar canais sem otimização custa
**56× mais**. Um framework inteiro quase foi julgado pelo perfil de compilação.

🔑 **A regra que fica: fluidez, e qualquer número de desempenho, só se mede em `--release`.**
Entregar `debug` para alguém julgar é entregar a pergunta errada. Vale para o resto da migração,
e vale em dobro na fase 2, onde o slider tem de responder ao arrasto.

#### `medir-miniaturas` — a régua, para não depender de impressão

```bash
VLB_CATALOG=/tmp/catalogo-de-medicao cargo run --release -p ui-gpui --bin medir-miniaturas
```

Mede o caminho inteiro de uma miniatura e responde em quantas cabem nos **16,7 ms** de um quadro a
60fps. Foi ele que separou "o GPUI é lento" de "o binário estava sem otimização" — em segundos, e
sem opinião no meio.

⚠️ **E o orçamento não é folgado nem em release**: cabem ~20 miniaturas por quadro. Uma linha de 6
passa com sobra, mas janela larga (8–10 colunas) numa rolagem rápida, revelando várias linhas por
quadro, chega perto do limite. É o que o carregamento assíncrono resolve, e ele continua pendente.

**Estado medido**: 102 MB residentes e 0,3% de CPU parado (debug); 229 MB e ~13% logo após abrir
em release, com árvore, filtros, grade e filmstrip sobre 2.000 fotos.

✅ **A busca dependia de uma decisão, e ela foi tomada** — o campo de texto exigia adotar o `input`
do `gpui-component`, o que traz junto o tema e o estado global dele (`gpui_component::init`). Como
era adoção de base de UI, e não um campo solto, virou o primeiro passo da fase 2 (`98f8822`); o campo
veio logo atrás (`cae826a`).

#### 🚨 A última pendência da fase 1 não existia — `medir-abertura`, 17/ago/2026

*"Carregamento assíncrono das fotos (hoje bloqueia a abertura)"* atravessou as fases 2, 3 e 4 como
item aberto, e **ninguém tinha medido quanto ele bloqueia**. Dívida sem número não dá para
priorizar: 30 ms e 3 s pedem decisões opostas e se parecem na descrição.

```bash
VLB_CATALOG=/tmp/catalogo-2000 cargo run --release -p ui-gpui --bin medir-abertura
```

Com **2.000 fotos**, em release, quatro execuções seguidas:

| Etapa | Tempo |
|---|---:|
| abrir o banco | 1,5 ms |
| migrations | 1,4 ms |
| **ler todas as fotos** | 22–40 ms |
| ler os presets | 0,2 ms |
| **antes da janela aparecer** | **23–43 ms** |

✅ **Não é dívida.** A janela abre em menos de 50 ms com o acervo de referência inteiro — bem abaixo
dos ~100 ms em que a espera passa a ser notada. Tornar isso assíncrono acrescentaria estado ("as
fotos ainda não chegaram") a todas as telas para economizar 40 ms.

⚠️ **E dá para saber onde ele voltaria a ser dívida**: são **0,020 ms por foto**, então o limite dos
100 ms cai perto de **5.000 fotos** — aritmética sobre a medida, não outra medida. Um acervo
profissional passa disso. Quando passar, o binário responde de novo, com o número do dia.

🔑 **A régua é a mesma que a fase 1 usou para separar "o GPUI é lento" de "o binário estava sem
otimização"** — e serviu de novo, agora para separar "a abertura bloqueia" de "a abertura custa 40 ms".

#### `semear-catalogo` — como medir sem depender do acervo de ninguém

O catálogo real desta máquina está **vazio** desde que foi recriado limpo (migrations 16–19), e
esperar 2.000 fotos importadas para só então descobrir que a grade engasga é a ordem errada.

```bash
VLB_CATALOG=/tmp/catalogo-de-medicao cargo run -p ui-gpui --bin semear-catalogo -- 2000
VLB_CATALOG=/tmp/catalogo-de-medicao cargo run -p ui-gpui
```

As fotos sintéticas têm uma faixa listrada no topo de propósito: numa grade de cores chapadas não
dá para ver se a virtualização troca miniaturas de lugar durante a rolagem, que é o defeito que ela
introduz. As notas vão de 0 a 5 em partes iguais — com 2.000, filtrar por ★★★★★ tem de mostrar
**333**, e por ★★★ tem de mostrar **999**.

⚠️ Ele **recusa** escrever em catálogo que já tem fotos: `VLB_CATALOG` é texto livre e o padrão dele
é a biblioteca real.

#### As armadilhas desta fase, todas silenciosas

1. 🚨 **O GPUI quer BGRA; o crate `image` produz RGBA.** `RenderImage` é documentado como "in BGRA
   format" e o Metal cria as texturas com `BGRA8Unorm`, mas `image::Frame` carrega um `RgbaImage` —
   o tipo não diz qual ordem está lá. Entregar um pelo outro não falha: troca vermelho por azul em
   **toda** foto, e quem olha conclui que o motor de cor está errado.
2. 🚨 **A grade indexava o acervo, e não a lista filtrada.** Com filtro ativo, cada célula mostraria
   a foto errada — e continuaria bonita. Só apareceu porque o compilador acusou a variável não usada.
3. 🚨 **`Flag::as_code` dizia `2` para rejeitada e sempre gravou `-1`.** Um filtro escrito a partir
   da documentação devolveria lista vazia, parecendo "não há nenhuma".
4. ⚠️ **`Path::parent()` não serve para caminho vindo do banco.** No macOS ele não reconhece `\`, e
   `C:\Fotos\2024\a.nef` viraria uma pasta só. O corte é por texto, aceitando os dois separadores.
5. ⚠️ **Cada closure é um tipo concreto**, então `impl IntoElement` não unifica dois botões com
   ações diferentes num `Vec` — a fronteira que monta lista devolve `AnyElement`.

⚠️ **E um falso alarme que quase virou achado**: `ps -o pcpu` mostrou 100% com o app parado — ele
reporta a **média desde o início do processo**. O instantâneo (`top -l 3`) é 0,0%. Número de CPU só
vale instantâneo.

### Fase 2 — Revelação (3–4 semanas)

Sliders, histograma, curva de tons, HSL nos 8 canais, detalhe, lente, crop overlay, undo/redo,
presets. O `gpu_processor.rs` e o WGSL **não mudam** — só o último passo, que hoje devolve
`ColorImage`.

**Critério de saída**: ~~editar um RAW e exportar com **o mesmo resultado de pixel** do app egui~~ —
🚨 **o critério estava medindo a coisa errada. Ver abaixo.**

⚠️ Aqui se consertam as duas lacunas que o STATUS registra: a exportação ignora o crop, e o
undo/redo ignora o crop. Reproduzir defeito conhecido de propósito custa mais do que arrumar.

#### O motor e os primeiros 11 ajustes ✅

`4bd8e7d` (o motor), `e62a041` (os sliders). O caminho de um arrasto está de pé:

```text
slider → SliderEvent::Change → Ajustes → Pedido → (thread wgpu) → RenderImage
```

O plano acertou o custo do motor: `gpu_processor.rs` cria a **própria** `wgpu::Instance` numa thread
de fundo e nunca soube que existia eframe. A única referência a egui nas 578 linhas era o tipo de
saída.

🚨 **`SliderState::set_value` não emite `Change`** — só o caminho do ponteiro publica o evento. É o
**oposto** do `InputState::set_value` da busca, que emite. Mesmo nome, dois comportamentos, na mesma
biblioteca; custou dois testes que passavam por engano. E a assimetria virou carga estrutural: o
reset ao neutro na abertura da foto usa `set_value` **porque** ele não emite, senão abrir qualquer
foto viraria 11 pedidos à GPU. Tem teste prendendo isso.

🔑 **Os controles são uma tabela** ([`controles.rs`](../crates/ui-gpui/src/revelacao/controles.rs)),
não 11 blocos de interface iguais — o painel de HSL sozinho tem 24. E o neutro de cada um vem de
`Ajustes::default`, não de um número escrito ao lado: dois lugares dizendo qual é o neutro é ter um
deles errado mais cedo ou mais tarde.

⚠️ **Nem todo neutro é zero.** `contrast` é 1.0 e `sharpen_radius` é 1.0. Um `#[derive(Default)]`
daria zero nos dois e **toda** foto abriria alterada — sem erro, e parecendo decisão de cor de quem
escreveu o shader. É o que `o_neutro_devolve_o_pixel_intacto` cobra.

🚨 **Este parágrafo dizia "`lens_vignette_midpoint` é 50.0", e o 50 estava errado** (corrigido em
16/ago). Ele veio de `GpuEditParams::default` do `crates/ui` — uma `impl` que o app de lá **nunca
chama**: o único chamador em todo o repositório é um teste que confere só os 11 campos do Básico. O
caminho vivo é `AppState::new` e `reset_edits`, e nos dois o meio da vinheta é **`0.0`**. O sintoma
era o slider "Meio da vinheta" abrir em 50 aqui e em 0 lá, na mesma foto — e a fase 5 acusaria isso
como divergência de porte.

🔑 **A pergunta que separa os dois** não é "qual é o padrão declarado", é **"qual valor a foto recebe
quando ninguém mexeu em nada"**. `impl Default` responde a primeira, e ela pode ser código morto —
foi o segundo achado seguido em que o legado tem duas versões da mesma decisão e só uma está viva (o
outro foi o JPEG com alfa da fase 0).

⚠️ **E a resposta certa, para foto de verdade, estava num quarto lugar: o schema.** Meia hora depois,
o semeador acusou: `014_add_hsl_lens_fields.sql` cria a coluna com `DEFAULT 50.0`, então **toda** foto
importada volta do banco com o meio da vinheta preenchido — e é esse 50 que os dois apps mostram. O
neutro do `Ajustes` decide só o resto (estado antes de abrir foto, o futuro "redefinir", campo
`NULL`), e ali o legado diz 0.0. Quatro lugares dizendo qual é o neutro do mesmo controle:
`AppState::new`, `reset_edits`, `GpuEditParams::default` (morto) e a coluna. **Três concordam; o
morto era o que estava copiado.**

#### Os 42 controles ✅ — e a curva de tons, que não tem nenhum

`2f67abb`. Detalhe (4), HSL/cor (8), HSL/luminância (8), HSL/matiz (8) e Lente (3) entraram como
**dados na tabela**, não como código de interface: o HSL inteiro são 24 linhas. Seis seções
sanfonadas, fechadas menos o Básico — como no legado (`default_open(true)` só nele).

⚠️ **E 18 deles não movem a foto — no legado também não.** Descoberto no dia seguinte: o `uniform`
do shader tem 28 campos para os 46 que a CPU manda. Detalhe e Lente inteiros não chegam, HSL/matiz
chega deslocado. O porte está certo; o alvo é que não é o que o nome dizia. A tabela posição a
posição está mais abaixo, em "a **tela** também não aplica 46".

🚨 **A curva de tons ficou de fora, e é decisão de paridade.** `Ajustes` tem os quatro `tone_curve_*`
e o shader os aplica, mas **no legado nenhum controle os escreve**:

```bash
grep -rn "active_tone_curve" crates/ui/src/   # reset, undo/redo, carga do banco, preset. Nenhum slider.
```

A seção "Tone Curve" de lá desenha um gráfico calculado a partir de exposição, contraste, altas luzes,
sombras, brancos e pretos — ela **não toca** nos quatro parâmetros que levam o nome dela. Dar slider a
eles seria feature nova (§7.1), e o preço não é estético: com feature nova, qualquer diferença entre
os dois apps deixa de ser conferível, porque não dá para saber se é defeito de porte ou escopo que só
um lado tem. Há teste prendendo o número — **42 controles para 46 ajustes** — para o impulso de
"completar a tabela" falhar em vez de passar.

⚠️ Duas faixas que não davam para adivinhar, e por isso foram lidas uma a uma de `dock_viewer.rs`:
**matiz vai de -180 a 180**, o dobro das outras duas famílias de HSL, porque matiz é um círculo; e o
**raio da nitidez começa em 0,5**, porque raio zero não tem pixel de vizinhança.

Falta da fase: **a exibição da foto cortada e girada** — o overlay de corte já está de pé (abaixo). A
persistência dos ajustes, que o plano não listava e sem a qual nada disso se guarda, entrou logo
depois; o undo/redo e os presets vieram na sequência.

#### A foto abre com a revelação que ela já tinha ✅

Até aqui a Revelação abria **toda** foto no neutro, inclusive as já trabalhadas. Não é "faltou uma
tela": é o trabalho do fotógrafo sumindo da vista, com o arquivo cru na frente dele e os 42 sliders
parados no meio dizendo que está tudo zerado.

[`persistencia.rs`](../crates/ui-gpui/src/revelacao/persistencia.rs) lê os 46 `edit_*` do
`PhotoViewModel` — a mesma leitura que o legado faz ao selecionar ("Load saved edits FIRST",
`app.rs`). 🔑 **O padrão de campo ausente vem de `Ajustes::default`, campo a campo, e não de 46
números digitados**: é a mesma regra do `Definicao::neutro` dos sliders, e é o que evita repetir o
erro que o próprio legado cometeu com o meio da vinheta.

⚠️ **`unwrap_or` não é `unwrap_or_default`.** Contraste ausente virando `0.0` achataria a foto
inteira em cinza — e a suspeita cairia no motor de cor, não na leitura do banco. Tem teste, e o
gêmeo dele também: `Some(0.0)` no contraste **é** o fotógrafo tendo arrastado até o fim, e não pode
ser confundido com ausência.

#### 🔑 `semear-catalogo` virou a régua da cadeia inteira — e achou o quarto neutro

Nenhum teste unitário alcança a cadeia que importa aqui: são quatro etapas entre a coluna e o
slider (`row_to_photo` → entidade `Photo` → `PhotoViewModel` → `da_foto`), e **todas engolem campo
desconhecido em silêncio** — o repositório lê cada um com `.unwrap_or(None)`. Um campo que se perca
no meio não dá erro: dá "esta foto nunca foi revelada".

Então o semeador passou a gravar revelação em uma foto a cada cinco e, no fim, **reler pelo caminho
do app** e comparar com o que gravou:

```
✅ 400 delas voltam com a revelação que foi gravada, lida pelo caminho do app.
```

Conferido quebrando de propósito (a lição do `544a0cb`): comentar uma linha da macro faz o semeador
imprimir `gravadas: [-1.5, -1.0]` / `lidas: [0.0, 0.0]` e sair com erro.

🚨 **E foi ele que achou o quarto lugar onde mora o neutro.** A primeira versão contava as fotos com
`da_foto(foto) != Ajustes::default()` e encontrou **todas** — não uma em cada cinco. O motivo está no
schema: `edit_lens_vignette_midpoint` é criada com `DEFAULT 50.0`, então toda foto importada volta do
banco com esse campo preenchido. Não é revelação, é o padrão da coluna. Os dois apps leem o mesmo 50,
então não há divergência — o que fica é que **"difere do neutro" não significa "foi revelada" neste
banco**, e a otimização de não pedir revelação ao abrir quase nunca dispara com foto de verdade (o
legado pede sempre, então o pior caso é o comportamento dele).

#### E a volta: o que o slider move é gravado ✅

Os mesmos **500 ms** de espera do legado (`AUTO_SAVE_DEBOUNCE_MS`): um arrasto emite dezenas de
`Change` por segundo, e cada gravação é um `UPDATE` de 54 colunas. A espera é o que transforma o
arrasto inteiro em uma gravação só — e no GPUI ela é uma `Task` guardada, porque **descartar uma
`Task` a cancela**: cada movimento novo substitui a anterior e adia, em vez de enfileirar.

A espera é também uma janela de perda, então há **três portas**, e cada uma tem teste que falha
quando ela é fechada:

| Porta | O que se perderia sem ela |
|---|---|
| o fim da espera | nada — é o caminho normal |
| **trocar de foto** | a gravação atrasada sairia com os ajustes já substituídos: a revelação de uma foto gravada na outra |
| **sair da Revelação** (`Esc`) | arrastar e voltar em menos de meio segundo, e o ajuste some sem aviso |

🔑 **O gravador é uma porta (`trait Gravador`), e não o `EditorController` direto** — por duas
razões que se somam: o controller é `async` do tokio e o GPUI não roda futuros dele, e os testes de
tela precisam afirmar **o que foi gravado**. Com o gravador de mentira, "quatro movimentos viraram
uma gravação, com o valor onde o dedo parou" é uma linha.

⚠️ **O `Handle` do tokio é capturado no `main`, antes de `Application::run` tomar a thread.** Um
`tokio::spawn` de dentro do GPUI entraria em pânico com *there is no reactor running* — no meio de um
arrasto, sem relação visível com o que o dedo estava fazendo.

#### 🚨 Gravar um ajuste apagava o corte — e a Revelação nova nem sabe cortar

`SavePhotoEditsUseCase` recebe os oito campos de corte como `Option` e a entidade faz
`self.edit_crop_x = crop_x`: **atribuição direta, sem mesclar**. Gravar uma exposição passando `None`
neles apaga o enquadramento.

O crop overlay é o que falta da fase 2, e a ausência dele **piora** o risco em vez de diminuir: sem
tela de corte, a Revelação nova não teria motivo nenhum para mandar corte — e mexer num slider
apagaria, calado, o enquadramento feito no app de egui, sem erro, sem aviso e sem desfazer. Por isso
o corte é lido da foto ao abrir e **devolvido igual** em toda gravação.

Isso não dava para conferir com gravador de mentira: o defeito mora do controller para baixo. Três
testes com **banco de verdade** ([`tests/gravacao_no_banco.rs`](../crates/ui-gpui/tests/gravacao_no_banco.rs)):

1. os 46 campos sobrevivem à ida e volta inteira — `Ajustes` → controller → use case → entidade →
   SQLite → `row_to_photo` → `PhotoViewModel` → `da_foto`, sete etapas com nomes parecidos demais;
2. gravar ajuste não apaga o corte;
3. 🔑 **a contraprova**: sem reenviar o corte, ele **é** apagado. Sem ela, o teste 2 poderia estar
   passando porque o use case mescla — e a precaução seria adorno em vez de a única coisa que separa
   o enquadramento de sumir.

#### Desfazer e refazer ✅ — `Cmd+Z`, com duas diferenças assumidas

[`historico.rs`](../crates/ui-gpui/src/revelacao/historico.rs): uma pilha de `Ajustes` inteiros (46
`f32`, 184 bytes por passo, 3,6 KB cheia), teto de 20 como no legado, e o futuro morrendo quando se
edita depois de desfazer. As teclas são as de lá: `Cmd+Z` e `Cmd+Shift+Z`.

Duas coisas **não** são iguais ao legado, e as duas são decisão:

**1. Um passo por gesto, e não por quadro.** O legado empurra um snapshot a cada quadro do `update`
em que algum valor difere do anterior: um arrasto de meio segundo vira ~30 passos, e com o teto de 20
o `Cmd+Z` de lá desfaz um milímetro por vez com o resto do histórico já descartado. 🔑 **E o número
de passos depende da taxa de quadros** — a 120fps ele grava o dobro. Comportamento que muda com o
monitor não é paridade conferível. Aqui o passo é registrado no fim do gesto, o mesmo instante da
gravação.

**2. A primeira edição é desfazível.** Lá, `push_edit_snapshot` só roda quando algo mudou, então o
primeiro snapshot já é o estado **depois** da mudança — e `undo` faz `if index > 0`. A primeira coisa
que se faz numa foto no app de egui **não tem volta**. Aqui o estado da abertura é o passo zero.

⚠️ **O corte ainda não entra no histórico**, porque a Revelação nova não sabe cortar. Quando souber,
entra junto — e vale saber o que se encontra do outro lado: o `EditSnapshot` do legado **tem** o campo
`crop_settings` e o preenche, mas nem `undo` nem `redo` o leem de volta. É pior do que não guardar,
porque quem lê o struct conclui que funciona. (O STATUS dizia que o campo não existia; existe, e é
ignorado.)

#### 🚨 E o `Esc` da Revelação nunca funcionou

Descoberto ao escrever o primeiro teste que aperta uma tecla de verdade
(`VisualTestContext::simulate_keystrokes`) em vez de chamar o método. **`Cmd+Z` não chegava — e o
`Esc` também não**, desde o commit que o trouxe (`91a80dc`, dado por pronto com um teste que chamava
`voltar_para_biblioteca` direto).

A causa é de uma linha: **`track_focus` rastreia o foco, não o concede.** Sem ninguém focar a raiz, o
caminho de foco fica vazio e nenhuma ação de teclado dela é alcançada. Tecla que não casa não falha —
ela não faz nada, e a suspeita cai na funcionalidade, não na ligação.

E há uma segunda porta pelo mesmo buraco, que virou teste próprio: **quem digita na busca deixa o
foco no campo**, e o campo para de ser renderizado ao entrar na Revelação. O caminho de foco passa a
apontar um elemento fora da tela, e as teclas somem — "buscar uma foto antes de revelar desliga o
`Cmd+Z`" é uma relação que ninguém adivinharia. Por isso o foco volta para a raiz **a cada troca de
tela**, e não só na abertura.

🔑 A lição é a mesma da fase 1 (`544a0cb`): **teste que não exercita o caminho de verdade passa com o
código quebrado.** Aqui foram dois commits inteiros afirmando um atalho que nunca respondeu.

#### Presets ✅ — e o "B&W" do legado não deixa a foto em preto e branco

A entidade, os cinco de sistema e a gravação já existiam nas camadas internas e ficaram intactas;
[`presets.rs`](../crates/ui-gpui/src/revelacao/presets.rs) é só a ponte entre `PresetAdjustments` e
`Ajustes`. Lista com os de sistema e os do usuário, clique aplica, e "+ Salvar como preset" abre um
diálogo com o nome — como no legado, e sem o apagar, que **lá também não existe** (o menu de contexto
dele só fecha o menu).

⚠️ **Um preset move 15 dos 46 ajustes** — os 11 do Básico e os 4 da curva de tons. Campo `None` não é
tocado, então aplicar "Warm" sobre uma foto com HSL trabalhado **não apaga o HSL**; é o comportamento
do legado e tem teste. 🔑 É a mesma contagem de 15 do exportador, e não é coincidência: as duas listas
foram escritas quando o app tinha só esses ajustes, e nenhuma cresceu junto com o shader.

🚨 **E os presets de sistema estão numa escala que não é a do shader.** "B&W" pede
`saturation: -100.0`. Mas a saturação do shader é um fator — `factor = 1.0 + saturation` —, então
cinza é **-1.0**, que é por que o slider vai de -1 a 1. Com -100, o fator é -99: cada canal é jogado
99 vezes para o lado **oposto** do cinza. Não é ausência de cor, é cor invertida e estourada. Medido
na GPU, com os dois valores lado a lado, em `o_preset_bw_do_legado_nao_da_preto_e_branco`. "High
Contrast" (`contrast: 50.0` numa faixa de 0 a 2) e "Warm"/"Cool" (`±15.0` numa faixa de ±10) têm o
mesmo problema; "Auto" é um `exposure: 0.0` marcado como *Placeholder* e não faz nada.

**Não consertado**, pela regra de sempre: os presets vêm do `ListPresetsUseCase`, os dois apps leem os
mesmos valores, e mexer neles durante o porte misturaria "portei errado" com "estava errado". Fica o
teste, que **falha no dia em que alguém arrumar** — e aí os dois lados mudam juntos.

⚠️ **A dívida deste commit**: o preset recém-salvo aparece na lista com **id local**. O
`SavePresetUseCase` cria o `Preset` lá dentro, com id próprio, e a porta é `fire-and-forget` como a de
gravação — então até fechar o app o que se vê é um gêmeo com outro id. Nada depende do id hoje
(apagar preset não existe em nenhum dos dois), mas é a primeira coisa a consertar quando depender.

#### 🚨 O critério de saída não media o que a fase 2 constrói

Descoberto em 15/ago, ao portar o motor. **Exportar não passa pelo shader.** São dois caminhos
diferentes, com implementações diferentes da mesma matemática:

| | quem aplica | quantos ajustes |
|---|---|---|
| **A tela** (Revelação) | `image_adjustments.wgsl`, na GPU | **46** — ⚠️ **não**: são 23, ver logo abaixo |
| **O arquivo** (exportação) | `ImageExporterImpl::process_image`, na CPU | **15** |

E os dois vivem em lugares diferentes: o shader é o que a fase 2 está portando; o exportador está na
`infrastructure`, que o §2.1 declara **intocada**. Ou seja: os dois apps chamam o mesmo exportador, a
igualdade de pixel na exportação é grátis, e conferir por ali **não toca no motor que acabou de ser
portado**. Um critério que passa antes de o trabalho começar não é critério.

Conferível a qualquer momento:

```bash
python3 - <<'PY'
import re, pathlib
gpu = pathlib.Path("crates/ui/src/gpu_processor.rs").read_text()
campos = re.findall(r"pub (\w+): f32", gpu.split("pub struct GpuEditParams {")[1].split("}")[0])
exp = pathlib.Path("crates/infrastructure/src/image_exporter.rs").read_text()
usa = re.findall(r"(\w+): f32", exp.split("fn process_image(")[1].split(") -> ")[0])
print(len(campos), len(usa), [c for c in campos if c not in usa])
PY
```

**Critério de saída novo**: a mesma imagem, com os mesmos 46 ajustes, atravessando o motor dos dois
apps, tem de sair **byte a byte igual**. O que garante isso hoje são três testes em
[`processador.rs`](../crates/ui-gpui/src/revelacao/processador.rs): o WGSL é o mesmo arquivo
(conferido byte a byte), o neutro devolve o pixel intacto, e a exposição atravessa com o valor certo
no primeiro e no último pixel.

#### 🚨 E, de quebra, um defeito do produto que ninguém tinha medido

A exportação descarta **31 dos 46 ajustes**, em silêncio:

- a **curva de tons** inteira (4 zonas);
- o **HSL inteiro** — saturação, matiz e luminância nos 8 canais (24 parâmetros);
- a **lente** — distorção e as duas da vinheta (3).

O STATUS §"Lacunas" registrava só *"a exportação ignora o crop"* e dizia que ela "aplica os ajustes
tonais". Aplica **15 deles**. Quem revela uma foto mexendo em HSL vê o resultado na tela, exporta e
recebe outra imagem — sem erro, sem aviso.

⚠️ **Isto não é da migração consertar**: é defeito do produto, mora na `infrastructure`, e mexer nele
durante a fase 2 misturaria "portei errado" com "estava errado". Fica registrado aqui e no STATUS
para virar trabalho próprio depois do cutover. O que a migração **não** pode fazer é continuar
medindo a si mesma por ele.

#### 🚨 E o pior: a **tela** também não aplica 46. Aplica 23, e cinco no lugar errado

Descoberto em 16/ago, ao começar a persistência dos ajustes. A frase acima — *"a tela aplica 46"* —
**estava errada**, e a de cima dela, no `Ajustes`, dizia que "o `uniform` do outro lado declara os
mesmos 46 na mesma ordem". Basta abrir o arquivo:

```bash
python3 - <<'PY'
import re, pathlib
bloco = pathlib.Path("crates/ui/src/shaders/image_adjustments.wgsl").read_text() \
    .split("struct Params {")[1].split("}")[0]
campos = re.findall(r"^\s*(\w+)\s*:\s*f32", bloco, re.M)
print(len(campos), "campos no WGSL")     # 28
PY
```

**28 campos no shader; 46 saem da CPU.** O `uniform` viaja como bytes crus e casa por **posição**,
então a partir da posição 23 o shader lê o campo do vizinho:

| posição | o Rust manda | o shader lê como | o que o usuário vê |
|--------:|--------------|------------------|--------------------|
| 0–22 | Básico (11), curva de tons (4), HSL/cor (8) | os mesmos | ✅ certo |
| 23 | `hsl_red_hue` | `nr_luminance` | **borra a foto** |
| 24 | `hsl_orange_hue` | `nr_luminance` **outra vez** — declarado duas vezes | nada |
| 25 | `hsl_yellow_hue` | `nr_color` | tira ruído de cor |
| 26 | `hsl_green_hue` | `sharpen_amount` | afia |
| 27 | `hsl_aqua_hue` | `sharpen_radius` | nada sozinho |
| 28–45 | matiz (3), HSL/luminância (8), lente (3), ruído (2), nitidez (2) | **nada** | nada |

Ou seja: **18 dos 42 sliders do painel não movem um pixel**, e **5 movem outra coisa**. Os 4
controles de Detalhe — os únicos que o painel oferece para ruído e nitidez — estão entre os que não
chegam, enquanto o ruído e a nitidez são aplicados por três sliders de matiz.

🔑 **Nada disso falha em lugar nenhum.** O buffer é maior que o mínimo que o binding exige, então o
wgpu aceita e ignora a sobra; a duplicata de `nr_luminance` no WGSL o naga também aceita. Não há
erro, log, nem tela quebrada — há um controle que responde e uma foto que muda pelo motivo errado.

🚨 **E o defeito é do `crates/ui`**, não do porte: é o mesmo shader (arquivo igual byte a byte, com
teste prendendo) recebendo a mesma struct. Os dois apps erram igual — que é justamente por que o
critério de saída da fase 2, igualdade de pixel entre os motores, **passaria com isto no lugar**.
Um critério que compara dois lados só pega o que os distingue.

⚠️ **Ficou preso em teste, e não consertado** — mesma razão do exportador: conserto durante o porte
mistura "portei errado" com "estava errado", e aqui havia um agravante. As fotos já reveladas têm
`hsl_*_hue` gravado no banco; arrumar o alinhamento muda **retroativamente** a aparência delas —
o que era borrão vira giro de matiz.

✅ **Consertado em 17/ago/2026, depois do desligamento** — as duas razões do adiamento caíram juntas.
Ver §8.

🔑 **Por que ninguém viu antes**: o teste ao lado se chamava `o_layout_tem_46_campos_de_quatro_bytes`
e o comentário dele dizia "os 46 campos **que o WGSL declara**". Ele mede `size_of::<Ajustes>()` —
não sabe que existe shader. Era uma afirmação sobre o outro lado escrita ao lado de um teste que
nunca a conferiria, e ela se lia como conferida. É a mesma família dos quatro achados da fase 0: o
que não avisa agora vira "o GPUI quebrou isso" depois.

#### A base do `gpui-component` ✅ — 15/ago/2026

O primeiro passo não era um slider: era `gpui_component::init`, sem o qual **nenhum** componente da
biblioteca funciona — slider, campo de texto, diálogo e menu leem estado global que só ele cria. Três
coisas entraram juntas porque uma sozinha não roda (`98f8822`):

| | |
|---|---|
| `gpui_component::init(cx)` antes de qualquer janela | |
| A primeira camada da janela virou `Root` | hospeda diálogo, gaveta e aviso; o crate o procura com um `expect` |
| O tema **Vintage Dark**, em `tema.rs` | a paleta do `design_system/theme.rs`, em hexadecimal |

🚨 **O tema não é enfeite: o `init` troca o tema calado.** Ele instala o do shadcn (`#0a0a0a`, cantos
de 6px) e **sincroniza claro/escuro com o sistema** — adotar a biblioteca sem mais nada faria o app
abrir **branco** numa máquina em modo claro. Num programa de revelação o entorno é parte da medição
de cor.

🚨 **E as duas formas de errar um tema aqui são silenciosas**: cor ilegível não falha, *some* (o
`apply_config` cai no padrão dele sem dizer nada); chave errada não falha, é *ignorada* (o
`ThemeConfig` não recusa campo desconhecido). Nenhuma das duas dá erro, log ou tela quebrada — dão
*uma cor diferente*. Os três testes de `tema.rs` existem só para isso, e o de ida e volta —
serializar o que foi lido e cobrar cada chave escrita — é o único jeito de pegar a segunda sem
depender da lista de ~90 nomes do esquema.

🔑 **O que a adoção já devolveu no primeiro arquivo**: a armadilha 5 da fase 1 (cada closure é um tipo
concreto, então `Vec` de botões com ações diferentes não compila) **deixou de existir** — o `Button`
guarda o handler num `Rc<dyn Fn>` e todos voltam a ser o mesmo tipo. O `botao()` da Biblioteca não
precisa mais apagar tipo.

⚠️ **Achado de caminho**: `cargo run -p ui-gpui`, o comando que este documento manda rodar e que o
`semear-catalogo` imprime no fim, **não escolhia nada** — três binários no crate, e o cargo desiste
com mais de um. `default-run` no `Cargo.toml` fez a instrução escrita passar a ser verdade.

⚠️ **A dívida que fica**: o `selected` do botão secundário é `#3a3a3a` sobre `#2d2d2d`, dois cinzas a
5% de distância. Numa barra de 15 botões isso é o mesmo que não marcar nenhum, então o filtro aceso
virou `primary` enquanto não houver decisão melhor.

#### O palco ✅ — a Revelação existe, e a navegação também

`91a80dc`. Nenhum dos ~50 ajustes pode ser conferido sem a foto embaixo, então a Revelação começa
pelo palco: a imagem grande, e o caminho de ida e volta a partir da Biblioteca.

Com duas telas, alguém precisa saber qual está no ar — e esse alguém não pode ser nenhuma das duas.
Nasce o [`app.rs`](../crates/ui-gpui/src/app.rs), com o `Aplicativo` como raiz da janela.

🔑 **A selecão é copiada, não compartilhada.** As duas telas escolhem foto por razões diferentes: na
Biblioteca escolher é *comparar*, na Revelação é *editar*. Um estado só faria voltar à grade e clicar
noutra miniatura trocar, calado, a foto que está sendo editada — e o próximo ajuste cairia na foto
errada. Tem teste, e o teste descreve o defeito que impede.

⚠️ **`Esc` não pode ser global.** O campo de busca usa `Esc` para se limpar, e uma ligação sem
contexto roubaria a tecla dele. Com `key_context("Aplicativo")` o `Esc` só chega à raiz quando nenhum
campo de texto tem o foco — que é exatamente quando "voltar" é o que se quer. É a primeira vez que
duas peças disputam uma tecla neste app, e não será a última: a Revelação já tem `\` (antes/depois) e
`R` (crop) esperando, do lado do egui.

⚠️ **`abrir` lê e decodifica na thread da interface.** Preview "Large" é um JPEG de poucos
milissegundos, mas quando a Revelação carregar o RAW em resolução plena, é este o ponto que vira
assíncrono — a mesma pendência que a abertura da Biblioteca ainda tem.

#### ✅ `TestAppContext` está de pé — e a decisão foi tomada antes dos sliders

O substituto que o §6 prevê para os 146 testes de UI **existe e roda** (`544a0cb`). Veio agora, e não
depois da Revelação, porque são ~50 controles com a mesma forma de solda pela frente: componente
emite evento, tela assina, estado muda. O molde está em
[`tela.rs`](../crates/ui-gpui/src/biblioteca/tela.rs), no `mod testes`.

```bash
cargo test -p ui-gpui --lib tela::testes
```

⚠️ **`test-support` entrou em `[dev-dependencies]`, e não em `[dependencies]`**, mesmo custando uma
segunda compilação do gpui (~2,6 GB de `target`): a feature liga junto o `leak-detection`, que grava
backtrace a cada handle de entidade. Num projeto onde a fase 1 quase condenou o framework por medir
fluidez no perfil errado, carregar detector de vazamento no binário do produto é repetir o mesmo erro
de outro jeito.

🚨 **O achado: um dos dois testes passava com o código quebrado.** Depois de escrevê-los, quebrei a
solda de propósito (`drop` na `Subscription`) para conferir se eles falhavam:

| Teste | Com a solda quebrada |
|---|---|
| `digitar_na_busca_filtra_a_grade` | ❌ FAILED — como tinha de ser |
| `limpar_a_busca_devolve_o_acervo` | ✅ **ok** |

O segundo cobrava só o fim: se nada nunca filtra, a grade tem as três fotos no final, que era
exatamente a asserção. Faltava a do meio. 🔑 **Teste de volta precisa provar que houve ida** — e o
único jeito de descobrir isso é quebrar o código de propósito e olhar qual teste não reclama. Vale
para os ~50 sliders: cada um vai ter um "arrasta e volta ao padrão".

#### O corte ✅ — a geometria primeiro, a tela depois

O único item da fase 2 sem equivalente pronto no `gpui-component`, e o mais caro: 498 LOC de desenho
e interação no legado. Entrou em duas metades, e a divisão foi de propósito — a primeira dá para
conferir sem olhar, a segunda não.

**A geometria** ([`corte.rs`](../crates/ui-gpui/src/revelacao/corte.rs)): onde ficam as oito alças, o
que cada arrasto faz, o que os limites da foto fazem com o resultado. 16 testes, e o
`CropSettings` do `domain` intacto como tipo de ida e volta. Três armadilhas ficaram presas, e as
três foram conferidas quebrando o código de propósito:

| A armadilha | O sintoma na tela |
|---|---|
| a alça esquerda move `x` **e** encolhe a largura | só mover `x` faz "o corte andar sozinho quando eu tento apertá-lo" |
| o lado mínimo (1%) encosta na borda **parada** | do outro jeito, o retângulo salta a largura inteira no último milímetro |
| reconstruir `CropSettings` preserva os outros 4 campos | esquecê-los devolve a foto à orientação original no meio de um arrasto |

E duas decisões que no legado estão espalhadas: **arrastar o retângulo empurra** no limite, **puxar
uma alça encolhe** — gestos diferentes, respostas diferentes; e **girar 90° não gira o retângulo
junto**, porque ele mora no espaço da imagem original e é o `to_visual_space` (no `domain`) que
traduz para a tela. Girar aqui aplicaria a rotação duas vezes.

**A tela**: escurecimento em quatro faixas, retângulo, grade de terços e as oito alças — tudo `div`
absoluto. O GPUI não tem pincel e não precisa: retângulo é `div` com fundo, e o layout faz a conta.

🚨 **O arrasto usa `window.on_mouse_event`, e não `div().on_mouse_move`**, porque o ouvinte de um
`div` só recebe evento **dentro** dele. Arrastar uma alça para fora da foto — que é o gesto normal
para encolher até a borda — sairia do elemento e o arrasto morreria no meio, deixando o retângulo
preso a meio caminho. `on_mouse_event` exige a fase de pintura, e é por isso que existe um `canvas`
ali: ele mede o palco no prepaint e liga os ouvintes no paint.

🚨 **A caixa `relative` é a de dentro do respiro, e não a moldura.** A moldura tem 24px de padding; um
absoluto ancorado nela se mede pela caixa **com** o respiro, enquanto a foto ocupa a de dentro. Os
dois sistemas ficariam deslocados de 24px — pouco, e o suficiente para parecer erro de mira de quem
está clicando.

✅ **Girar, espelhar, endireitar e as proporções entraram junto com a exibição transformada** (logo
abaixo) — antes disso seriam botões mexendo num número invisível.

⚠️ **E o corte não entra no histórico.** `Cmd+Z` desfaz ajuste, não enquadramento. É o mesmo que o
legado entrega — lá o `EditSnapshot` guarda `crop_settings` e nem `undo` nem `redo` o leem —, mas aqui
é por ausência, e não por engano.

#### A foto exibida ✅ — e o legado tem duas ordens diferentes para a mesma coisa

A última divergência visível: até aqui a Revelação nova mostrava a foto inteira mesmo com corte
gravado. Quem enquadrou no app de egui via o corte sumir ao abrir no novo.

🔑 **A ordem sai do `image_viewer.rs`, e não de uma escolha nossa.** O legado desenha o resultado como
uma malha cujas UVs vão do quadro para a textura — descentraliza, corrige o aspecto, gira por
`-ângulo`, desfaz o giro de 90°, desfaz os espelhos. Lendo ao contrário, é o caminho de ida:

```text
original → espelhos → giro de 90° → endireitamento → recorte
```

⚠️ **E `ImageProcessor::apply_crop`, do mesmo legado, faz outra coisa**: recorta primeiro, espelha
depois, gira por último — e **ignora o ângulo**. É a função que gera miniatura, e é por isso que uma
foto endireitada aparece torta na grade e direita no viewer. Aqui vale a do viewer: é a Revelação que
esta tela porta.

O endireitamento pergunta "de onde vem este pixel" em vez de girar a imagem inteira e recortar depois:
sem imagem intermediária, e o que cai fora gruda na borda (o `clamp` da malha de lá) em vez de virar
buraco transparente. **Ângulo zero não passa pela reamostragem** — bilinear com deslocamento inteiro
ainda mistura vizinho, e toda foto sairia um fio menos nítida sem ninguém ter pedido.

#### Histograma ✅ e curva de tons ✅ — os dois desenhados com `paint_quad`

O histograma mede **a foto que está na tela** (o legado calcula depois do `process_image`), com a
altura normalizada pelo maior dos três canais — normalizar cada um pelo próprio máximo faria uma foto
azul-escura desenhar o vermelho tão alto quanto o azul, e o instrumento passaria a mentir sobre a
única coisa que ele existe para mostrar.

🔑 **256 colunas × 3 canais não podem ser 768 `div`s**: cada um é um nó de layout recalculado a cada
quadro, num painel que hoje tem menos de cem. `canvas` + `paint_quad` é o análogo do `painter` do egui,
e a curva de tons usa o mesmo caminho (101 pontos, a diagonal pontilhada e a curva por cima).

⚠️ **A curva de tons não é a dos `tone_curve_*`.** Ela lê exposição, contraste, altas luzes, sombras,
brancos e pretos e desenha o efeito combinado — é a mesma aproximação do `tone_curve.rs` de lá,
constante por constante, e **não** é o que o shader faz. Copiar a aproximação é o que mantém os dois
apps mostrando o mesmo desenho.

#### Antes/depois ✅ e redefinir ✅

`\` troca a **fonte** da imagem e mantém o enquadramento e os sliders: comparar cor com a foto pulando
de tamanho não compara nada, e um histograma que pulasse junto tiraria a régua da comparação.
"Redefinir ajustes" zera os 46 e **não** toca no corte — no legado o `reset_edits` também não toca, e
misturar os dois faria um botão de cor apagar trabalho de composição.

---

### ✅ Fase 2 concluída — 16/ago/2026

| Item que a fase listava | Onde ficou |
|---|---|
| Sliders (Básico, Detalhe, HSL ×3, Lente) | `controles.rs` — 42 numa tabela, mais o de endireitar |
| Histograma | `histograma.rs` + `paint_quad` |
| Curva de tons | `curva.rs` — o gráfico, que é o que o legado tem |
| Crop overlay | `corte.rs` (geometria) + o overlay e a barra em `tela.rs` |
| Undo/redo | `historico.rs` — um passo por gesto |
| Presets | `presets.rs` — listar, aplicar, salvar |
| **Persistência** (não estava na lista) | `persistencia.rs` — sem ela nada disso se guarda |

**Critério de saída** (igualdade de pixel entre os motores): garantido por três testes em
`processador.rs` — o WGSL é o mesmo arquivo byte a byte, o neutro devolve o pixel intacto, e a
exposição atravessa com o valor certo.

⚠️ **O que a fase 2 encontrou e não consertou** — quatro defeitos do legado, todos presos em teste e
registrados no STATUS: o `uniform` de 28 campos para 46 (18 sliders sem efeito, 5 aplicando outra
coisa); a exportação que descarta 31 ajustes; os cinco presets de sistema numa escala que não é a do
shader; e o `undo` que guarda o corte e não o restaura.

⚠️ **O que fica de dívida própria**: o preset recém-salvo com id local e o corte fora do histórico. (A
terceira que estava aqui — o carregamento síncrono na abertura — **foi medida em 17/ago e não era
dívida**: 23–43 ms com 2.000 fotos. Ver a fase 1.)

### Fase 3 — Importação 🔄 **em andamento**

O modal de 4 etapas assíncronas (`import_view.rs`, 2.101 LOC), reescrito em 15/ago e ainda fresco.
A ordem das leituras — escanear, descrever, miniaturar o visível, conferir duplicatas — é regra
conquistada e se preserva.

#### O estado ✅ — três corridas que ele precisa recusar

[`importacao/estado.rs`](../crates/ui-gpui/src/importacao/estado.rs): sem tela e sem disco, 17
testes. O desenho é o do legado e é bom — toda descoberta chega como `Recado` e sai como mudança de
estado mais, às vezes, um `Seguimento` ("agora vá ler isto"). Quem dispara trabalho é a tela, que tem
o controller em mãos; é o que permite testar a máquina inteira sem runtime, banco ou cartão plugado.

| A corrida | O que acontece sem a defesa |
|---|---|
| a varredura da origem **antiga** chega depois da troca | a grade mostra os arquivos da pasta anterior, sem erro e sem pista |
| as descrições voltam **fora de ordem** (são lidas em paralelo) | casar por índice dá a câmera de uma foto para outra — invisível num cartão só |
| ordenar por captura **antes** das horas chegarem | ordena por string vazia, e a grade se reembaralha sozinha quando elas chegam |

E o desempate da ordenação é sempre o nome do arquivo: sem ele, uma rajada troca de lugar a cada
reordenação.

⚠️ **Duas decisões são do usuário, e não da tela**: duplicata só desmarca com "pular duplicatas"
ligado (quem desligou quer reimportar), e o intervalo do Shift trabalha sobre o que está **visível** —
contar sobre o acervo marcaria arquivos que a pessoa não está vendo.

#### O modal ✅ — duas portas, e o silêncio que não é resposta

`Explorador` (varrer, detalhar) e `Importador` (importar) são traits **separadas**: explorar é grátis
e reversível, importar copia ou **move** arquivo. Uma só faria o explorador de mentira dos testes
precisar saber importar. O seletor de pasta é uma terceira — abrir diálogo é interação com o sistema,
e nenhum teste pode fazer aparecer janela na máquina de quem roda a suíte.

🔑 **O seletor responde sempre**, inclusive "desisti". Sem esse recado, a tela esperaria para sempre
uma pasta que nunca vem — e o laço de colheita acordaria a cada 100ms pelo resto da sessão. Silêncio
não é resposta.

Há teste cobrando a **ordem dos pedidos**, e não só o resultado: `["varrer:/cartao", "detalhar:2"]`.
E duas guardas: o botão de importar desliga enquanto o lote corre (dois cliques copiariam tudo de
novo com "pular duplicatas" desligado), e fechar o modal **não** joga a listagem fora — quem fecha
por engano depois de marcar 300 fotos não pode perder a marcação.

🔑 **`Aplicativo::novo` chegou a dez argumentos e virou `Portas`**: cinco `Arc<dyn …>` posicionais do
mesmo naipe, cuja ordem não é óbvia para ninguém — trocar dois de lugar compila e falha no primeiro
clique.

#### O lado "PARA" ✅ — e a prévia, que é a peça que vale portar

Modo (Add/Copy/Move), destino, organização, renomeação e "não importar duplicadas". Mas o que decide
é a **prévia**: mostrar para onde a primeira foto vai transforma quatro escolhas abstratas numa
decisão conferível antes de o botão ser apertado.

🚨 **O teste da prévia encontrou um defeito no `Candidato`.** `Path::file_name` no macOS não reconhece
`\`, e um cartão formatado no Windows chega com caminhos assim: o "nome do arquivo" virava o caminho
**inteiro**, e a célula da grade mostraria `C:\Fotos\2024\DSC_1.NEF`. É a mesma armadilha que a
árvore de pastas da fase 1 encontrou, do outro lado do app — e o corte agora é por texto, com os dois
separadores, nos dois lugares.

Duas regras da prévia com teste: sem EXIF vem o **placeholder** (`AAAA/MM/DD/`) e não uma data
inventada — uma pasta `1970/01/01` pareceria informação verdadeira; e `Add` **não prevê destino**,
porque ele cataloga onde está. Pelo mesmo motivo, destino, organização e renomeação só aparecem
quando o modo copia.

⚠️ **Ligar "não importar duplicadas" desmarca na hora.** Se a importação vai pular, a marcação tem de
dizer isso antes do clique — senão o rodapé promete 40 fotos e entram 32. Desligar de volta **não**
remarca: quem desmarcou à mão não pode ter a escolha desfeita por uma caixa de opção.

#### Teclas ✅ e miniaturas sob demanda ✅

`Enter`, `espaço`, `⌘A`, `↑↓` e Shift+clique, num contexto de teclado próprio — sem ele, as teclas
mais disputadas que existem roubariam a busca da Biblioteca desenhada atrás. O teste aperta as teclas
**de verdade**, pelo motivo de sempre: ligação que não casa não falha, ela não faz nada.

Três regras que só apareceriam usando: as setas andam sobre o que está **visível** (um passo numa
duplicata escondida pareceria seta que não funciona); `Enter` durante a importação **não** começa a
segunda cópia do mesmo lote (tecla não passa por botão desligado); e o clique fica na **linha**, não
na caixinha — é o `ClickEvent` da linha que traz os modificadores, sem os quais não há Shift+clique.

A grade virou `uniform_list`, e é o que transforma "gerar 2.000 miniaturas" em "gerar as 20 que se
está olhando": só as linhas visíveis são renderizadas, e é ali que a miniatura é pedida. As fotos
ainda não estão no catálogo — não há preview gravado, e cada uma custa abrir o arquivo no cartão.

🚨 **Cada arquivo é pedido uma vez só.** O `uniform_list` chama a renderização a cada quadro: sem a
lembrança do que já foi pedido, seriam 60 pedidos por segundo por célula visível. É o tipo de laço
que só aparece quando o cartão fica lento — e aí parece problema do cartão.

A chave no cache é `import::<caminho>` (a mesma decisão do legado): a tabela guarda as duas coisas, e
um caminho de cartão sem prefixo poderia colidir com o id de uma foto.

#### Cartões, lupa e rolagem ✅ — a fase 3 fecha

**Cartões e origens recentes** são pedidos a cada abertura do modal, e não uma vez na construção: um
cartão plugado depois de o app subir não apareceria numa lista buscada uma vez só, e o fotógrafo não
tem por que saber que a lista é velha. Eles chegam **sem derrubar** o que está listado — a resposta
pode voltar com uma varredura já em curso.

⚠️ **A `Origem` daqui não é a `ImportSource` do `domain`**: só nome e caminho. O tipo e o id de lá são
detalhes de quem detecta dispositivo, e carregá-los até a tela amarraria o modal ao repositório de
dispositivos.

🚨 **A rolagem acompanha as setas, e a posição é a da grade filtrada.** Sem ela, a seta parece não
funcionar: o foco anda, a linha destacada sai da área visível, e quem aperta ↓ dez vezes vê
exatamente nada acontecer. E rolar pelo índice do **acervo** pararia numa linha diferente da
destacada quando "só novos" estivesse ligado — a rolagem e o destaque discordariam, e só em algumas
listagens.

**A lupa** abre no duplo clique e fecha no seguinte, sem tocar na marcação. 🔑 No legado o duplo
clique **alternava a marcação**, contradizendo o clique simples — foi um dos consertos da reescrita de
15/ago, e o que se porta é o conserto, não o defeito.

⚠️ **Ela mostra a miniatura de 128px ampliada, e não a foto.** Ler o RAW inteiro para dar uma olhada
custaria segundos por foto num cartão, e a lupa da importação existe para responder "é esta mesmo?",
não para julgar foco. Quando precisar responder mais, é um preview maior que se pede — e não uma
escala diferente da mesma imagem.

---

### ✅ Fase 3 concluída — 16/ago/2026

| Item | Onde ficou |
|---|---|
| Estado e regras da grade | `importacao/estado.rs` — 24 testes, sem tela e sem disco |
| Prévia do destino | `importacao/destino.rs` |
| Portas para o disco | `importacao/explorador.rs` — explorar, importar, escolher pasta, gerar miniatura |
| O modal | `importacao/tela.rs` |

**A ordem das leituras**, que era a regra conquistada a preservar: varrer → descrever → duplicatas,
com as miniaturas geradas só para o que está na tela. Há teste cobrando a sequência dos **pedidos**, e
não só o resultado.

⚠️ **O que fica de fora, e é decisão**: pausar e cancelar a importação existem no controller e **não
têm botão** — nascem desligados, o que é melhor do que um botão que a tela não sabe desfazer. E a
ordenação da grade não tem a coluna "tipo de mídia" separada em RAW+JPEG, porque o legado também não
tem.

### Fase 4 — Impressão, multi-monitor, docking, atalhos 🔄 **em andamento**

#### 🚨 Antes de portar: o módulo de impressão do legado não imprime

`print_view.rs` (1.132 LOC) é uma **prévia de papel** — modelo, tamanho, margens, e onde as fotos
caem. O botão "Print" mostra `"Print feature coming soon!"` num aviso, e o "Export PDF" mostra
`"PDF export coming soon!"`. Portar é portar a prévia; não há caminho de impressão para preservar.

🔑 **E há dois módulos de impressão lá, um deles morto.** `print_dialog.rs` (288 LOC) tem o próprio
`PrintLayoutOption`, o próprio `PaperSizeOption` e a própria `OrientationOption` — mas
`state.show_print_dialog` **nunca é escrito como `true`** e `state.print_dialog_state` nunca recebe
`Some(...)`: o diálogo não tem como abrir. É o terceiro achado da mesma família (o `GpuEditParams::
default` da fase 2, o JPEG com alfa da fase 0): **duas versões da mesma decisão, e só uma viva.**
O que se porta é a viva.

#### A geometria da página ✅ — e três contas que o legado não faz

[`impressao/pagina.rs`](../crates/ui-gpui/src/impressao/pagina.rs): papel, orientação, margem,
espaçamento e grade, tudo **em milímetro**, sem tela. 16 testes.

🔑 **Milímetro, e não fração do papel** — que era a alternativa óbvia, e traz junto uma armadilha:
fração não é isotrópica. Numa A4 retrato, `0,5` na horizontal são 105 mm e na vertical são 148,5 mm;
encaixar foto sem esticar, que é conta de proporção, sairia deformado em silêncio. A tela pede uma
escala (`escala_para`) e multiplica — é a única ponte entre o módulo e o pixel.

🚨 **A prévia do legado desenha o papel com a proporção da janela.** A folha é
`available.x * 0.8 × available.y * 0.9`, e `paper_size`/`orientation` **não entram na conta em lugar
nenhum**: A4 e Tabloide desenham o mesmo retângulo, girar para paisagem não muda nada, e maximizar a
janela muda o formato do papel. Os dois controles existem, são gravados no estado e nunca chegam a um
pixel.

**Aqui a proporção é a do papel, e a divergência é decisão** — a mesma do passo de histórico por
gesto na fase 2: *prévia que muda de forma junto com a janela não é paridade conferível*, e uma
prévia de impressão que não mostra o papel não responde a única pergunta que ela existe para
responder.

⚠️ **A margem do legado não é a que o campo diz.** Ela é `margem_mm / 297.0` — a altura da A4 —
aplicada como fração **nos dois eixos, para qualquer papel**. Pedir 10 mm numa A4 dá 7,1 mm nas
laterais; num Tabloide dá 9,4 mm nas laterais e 14,5 mm em cima. O campo se chama "Margins (mm)" e
não descreve nenhuma das duas distâncias. Aqui a margem é a mesma distância nos quatro lados, e há
teste medindo os quatro.

🚨 **E a grade personalizada conta errado de um jeito que se contradiz na mesma tela.**
`PrintTemplate::Custom.photos_per_page()` responde **4** para qualquer combinação, porque
`grid_dimensions` devolve o `(2, 2)` que é só o valor inicial dos campos. A prévia desenha
`custom_cols × custom_rows` células: numa grade 6 × 8 ela mostra 48 fotos numa folha enquanto o
rodapé promete 12 páginas para as mesmas 48. Nenhum dos dois números avisa que discorda do outro.

⚠️ **Duas guardas que o legado não tem, e cujo sintoma é o mesmo: a folha aparece vazia.** Margem
acima de metade do papel (o campo vai até 50 mm, mas nada impede o resto da conta) e espaçamento
maior que a área útil produzem célula de largura **negativa** — retângulo invertido não falha, ele
some, e o desenho fica idêntico ao de "nenhuma foto escolhida". Aqui o piso é zero, com teste nos
dois casos.

⚠️ **O legado desenha a página 1 e só ela.** As células são preenchidas a partir do índice 0 da
lista e o rodapé diz "Page 1 of 7", sem caminho para as outras seis. `fotos_da_pagina` existe porque
a folha é a unidade da impressão, mas **botão de página não entra**: seria feature nova (§7.1), e é
o tipo de coisa que se acrescenta em uma linha no dia em que o dono pedir.

#### A folha na tela ✅ — a terceira tela do app

[`impressao/tela.rs`](../crates/ui-gpui/src/impressao/tela.rs), ligada à raiz como `Tela::Impressao`.
Os três painéis do legado: modelos e coleção à esquerda, papel/orientação/medidas à direita, a folha
no meio. O botão da barra só liga com seleção, como o de lá — e como o da Revelação.

A folha é um `div` de tamanho conhecido com uma célula absoluta para cada foto; a única conta de
pixel do arquivo é `mm × escala`. 🚨 **E a caixa `relative` é a folha sem padding nenhum** — as
margens já vêm da geometria, em milímetro, e um respiro no contêiner somaria a elas e faria o número
do campo deixar de descrever a distância na tela. É a mesma armadilha dos 24 px que o overlay de
corte encontrou na fase 2.

🚨 **`Invert` do legado embaralha a coleção.** Lá a inversão é `HashSet::difference`, e a ordem de
saída é a do hash. Como é a **posição na lista** que decide em qual célula cada foto cai, inverter e
desinverter devolve a mesma coleção com as fotos trocadas de lugar na folha — sem ninguém ter tocado
no leiaute, e com o desenho novo parecendo tão certo quanto o anterior. Aqui a inversão preserva a
ordem do acervo.

⚠️ **E o teste disso quase nasceu inútil**: escrito com o acervo de 5 fotos dos outros testes, ele
**passa** com a versão do legado no lugar — a ordem de um `HashSet` pequeno de inteiros sai crescente
com frequência alta demais. Com 40 fotos falha nas três execuções seguidas. É a lição do `544a0cb`
outra vez: só quebrando o código de propósito se descobre qual teste não reclama.

⚠️ **Duas coisas não foram portadas, e as duas são decisão:**

| O que ficou de fora | Por quê |
|---|---|
| A seção "Photo Info" (4 caixas) e o campo "Copies" | Escritos no estado e **lidos por ninguém** — a prévia do legado nunca desenha texto debaixo da foto. Portá-los seria portar a promessa |
| Os botões "Print" e "Export PDF" | O comportamento inteiro dos dois é um aviso de *"coming soon"* |

É a mesma decisão que a fase 3 tomou com pausar e cancelar a importação: melhor nascer sem o botão do
que com um que não faz o que diz. As duas linhas voltam no dia em que houver impressão de verdade —
e aí elas terão o que fazer.

⚠️ **O `Esc` sai da Impressão, e no legado não sai** (lá a condição é
`current_view == CurrentView::Develop`, e do módulo de impressão só se sai clicando em "Library"). A
alternativa é uma tecla que responde numa tela e emudece na outra — mais cara de aprender do que
qualquer uma das duas regras inteiras.

#### A faixa de escolher ✅ — e ela não pode ser uma fila horizontal

O filmstrip do rodapé é onde a coleção se ajusta **uma foto por vez**; os quatro botões da coluna da
esquerda a montam em bloco.

🚨 **Aqui ela é uma grade que rola na vertical, e no legado é uma fila horizontal.** O GPUI
virtualiza lista **vertical** (`uniform_list`) e só ela: uma fila horizontal com o acervo inteiro
seriam 2.000 `div`s e 2.000 consultas ao cache **por quadro** — exatamente o que a fase 1 mediu
engasgando. O egui não tem esse problema porque desenha em modo imediato e recorta o que sai da área.
Duas linhas visíveis é o que cabe sem tirar espaço da folha.

🔑 **Quem entra na coleção entra no fim** — é o `push` do legado. A ordem da coleção é a ordem das
células, então clicar em três fotos monta a folha na ordem dos cliques; inserir na ordem do acervo
tiraria de quem escolhe a única forma que existe hoje de dizer o que vai onde.

⚠️ **A marcação é lida da coleção inteira, e não das fotos da folha.** Lida da folha, tudo que passa
da primeira página pareceria não escolhido — e o clique seguinte tiraria da coleção o que quem
clicou queria acrescentar.

#### O empurrão dentro da célula ✅ — e o `on_mouse_event` de novo

O legado deixa arrastar a foto dentro da célula (`cell_offsets`), e o botão "Reset Photo Positions"
aparece só quando há o que redefinir. Os dois foram portados.

🚨 **`window.on_mouse_event`, e não `div().on_mouse_move`** — a mesma lição do overlay de corte: o
ouvinte de um `div` só recebe evento **dentro** dele, e empurrar a foto até encostar na borda da
célula é justamente o gesto que sai dela. O arrasto morreria no meio, com a foto parada a meio
caminho. E daí também o `canvas`: registrar ouvinte de mouse exige a fase de pintura.

⚠️ **O deslocamento é guardado em milímetro; no legado é em pixel de tela.** Em pixel, redimensionar
a janela mudaria o quanto a foto está deslocada **no papel** — o mesmo defeito do papel com a forma
da janela, outra vez, num campo diferente.

⚠️ **E ele para em 20% da célula**, com recorte na célula (`overflow_hidden`). Sem limite a foto sai
inteira por baixo do recorte: a célula fica idêntica a uma vazia, e quem arrastou não tem como saber
para que lado ela foi. (No legado o limite é `render_width * 0.2` mais metade do que sobra, medido em
pixel; aqui é um quinto da célula, em milímetro.)

⬜ **O que fica de fora desta tela**, e é decisão: navegação entre folhas (feature nova, §7.1), e a
`Origem`/filtro do filmstrip do legado — a faixa mostra o que a Biblioteca estava mostrando, que é o
mesmo recorte que os botões de coleção usam.

#### 🚨 E o atalho de recorte estava comendo a letra `r` da busca

Descoberto ao começar os atalhos da Biblioteca, antes de ligar a primeira tecla nova. **Digitar
"retrato" no campo de busca escrevia `etato`.**

O GPUI procura ligação em **todos os prefixos** do caminho de foco
(`KeyBindingContextPredicate::depth_of`, `keymap.rs`), então uma ligação declarada no contexto da
raiz continua casando enquanto se digita num campo de texto lá dentro — e **tecla que vira ação não
vira letra**. O `r` do recorte (`91a80dc`) estava ligado a `Some("Aplicativo")` desde que nasceu.

🔑 **Nada falhava.** A ação nem chegava a fazer efeito — `ao_alternar_corte` só age dentro da
Revelação —, então o único sintoma era a letra sumir. E a suspeita cai no campo de busca, que estava
certo o tempo todo.

**Conserto**: `"Aplicativo && !Input"` no `r` e no `\`. O `Not` do predicado varre a pilha inteira, e
`Input` é o contexto do campo do `gpui-component`. `Cmd+Z` não precisa (modificador não produz
letra), e `Esc` também não: o campo tem ligação **própria** para ele, e o GPUI prefere a mais
profunda.

⚠️ **É pré-requisito dos atalhos que vêm agora**, e não um conserto de caminho: as notas do legado
são `0`–`5`, as cores `6`–`9` e os sinalizadores `P`/`X`/`U` — treze teclas soltas. Sem esta regra,
buscar `DSC_0512` na Biblioteca daria nota 5, depois 1, depois 2, em fotos diferentes, enquanto o
número não aparecia no campo.

⚠️ **E o teste precisou da janela montada com o `Root`**, como a do `main.rs`: o campo procura o
`Root` com um `expect` ao inserir texto, e sem ele o teste morre antes de responder. É a primeira vez
que um teste daqui digita de verdade em vez de chamar `set_value` — que é justamente por que o
defeito sobreviveu a `digitar_na_busca_filtra_a_grade`.

#### As quinze teclas de triagem ✅ — nota, cor, sinalizador e as setas

`0`–`5` dão nota, `6`–`9` dão cor, `P`/`X`/`U` sinalizam e as setas andam pela grade. São os atalhos
mais usados de um programa de seleção: quem tria 800 fotos de um casamento passa por eles algumas
centenas de vezes.

| Peça | Onde |
|---|---|
| As três regras de alternância, sem tela | `biblioteca/marcacao.rs` |
| A porta que grava (`trait Marcador`) e o `MarcadorDoBanco` | idem |
| Os métodos da grade (`andar`, `dar_nota`, `dar_cor`, `sinalizar`) | `biblioteca/tela.rs` |
| As quinze ações e a tabela de ligações | `app.rs` |

🔑 **A nota é absoluta; a cor e o sinalizador alternam.** Teclar `7` numa foto já amarela **tira** o
amarelo, e `P` numa foto já escolhida a desmarca — é o que faz a mesma tecla ser "marcar" e
"desmarcar" sem uma segunda. `U` desmarca sempre. Errar isso não falha: só deixa de desmarcar.

🔑 **As ações moram na raiz, e não na Biblioteca** — mesma razão do `Cmd+Z` da Revelação: ação só é
alcançada em quem está no **caminho do foco**, que vai da raiz até o nó focado. Uma ligação declarada
no contexto da Biblioteca nasceria morta, e o sintoma seria "a tecla não faz nada".

⚠️ **A tela muda antes do banco responder** (o *optimistic update* do legado). Numa triagem se aperta
tecla mais rápido do que um `UPDATE` volta, e esperar faria a nota aparecer depois de a foto seguinte
já estar selecionada — o número certo na foto errada, do ponto de vista de quem olha. E marcar
**refiltra**: com "★★★ ou mais" ligado, baixar uma foto para 1 tira ela da grade na hora.

⚠️ **Na Revelação as teclas de triagem não fazem nada, e é decisão.** O legado tria de lá também
(`get_target_photos` aceita `Develop`). ✅ **A Revelação ganhou filmstrip depois disto** (abaixo), então
o impedimento de então — não haver em qual foto marcar — caiu: dar nota ali passou a ser uma linha, e
o que falta é a decisão de dono sobre marcar em lote a partir de uma tela que mostra uma foto.

⚠️ **Roxo não tem tecla, e no legado também não**: o menu de lá oferece cinco cores e o `keyboard.rs`
liga quatro. E `Delete`/`Backspace` (apagar foto) ficam de fora — apagar leva confirmação e remoção
de arquivo, que é trabalho próprio e não um atalho a mais.

#### Seleção múltipla ✅ — e os dois defeitos que ela expôs no legado

`Cmd+clique` alterna uma, `Shift+clique` estende o intervalo, `Cmd+A` seleciona a grade e `Cmd+D`
limpa. As treze teclas de triagem passam a valer para a seleção inteira — que é o que a triagem em
lote existe para fazer.

🚨 **`Shift+clique` do legado conta o intervalo no acervo, e a grade enumera o filtrado.** O índice
que a grade passa vem de `filtered_photos`; o `select_range` indexa `state.photos`. Com qualquer
filtro ligado, `Shift+clique` seleciona **outras fotos** — as que ocupam aquelas posições no acervo,
algumas nem visíveis. Nada falha: a grade marca células que ninguém apontou. É a mesma armadilha que
a grade da fase 1 encontrou.

🚨 **`Cmd+A` do legado seleciona o acervo inteiro**, ignorando o filtro (`select_all` percorre
`state.photos`) — e a tecla de nota seguinte cai em todas elas, inclusive nas que não estão na tela.
O próprio legado se contradiz: o "Select All" do módulo de impressão respeita o filtro.

🔑 **A seleção é um `BTreeSet`, e no legado é um `HashSet`** — e a diferença **vaza para a folha de
impressão**: entrar na Impressão com dez fotos selecionadas monta a coleção em ordem de hash, e a
posição na lista decide em qual célula cada foto cai. É o terceiro lugar em que a ordem de um
`HashSet` chega à tela (os outros dois: o `Invert` da impressão e este).

🚨 **O sinalizador em lote decide pelo grupo; no legado decide foto a foto.** Lá o
`handle_flag_shortcuts` calcula a alternância **dentro do laço**: com três selecionadas e uma já
escolhida, `P` **desmarca aquela** e marca as outras duas — uma tecla, dois desfechos opostos no mesmo
gesto. Aqui vale a regra que a cor já tinha (`all_already_have_color`): só desmarca se todas já
estiverem. Com uma foto só — o caso comum — as duas regras dão o mesmo resultado.

⚠️ **Duas forças de marca na grade**: a principal (a que a Revelação abre) com a borda cheia, as
outras da seleção com 45% dela. Marcando as dez igual, apertar "Revelação" com dez selecionadas
abriria uma delas sem que nada na tela tivesse dito qual.

⚠️ **E clicar numa das selecionadas encolhe a seleção para ela**, em vez de limpar tudo — só desmarca
quando ela já era a única. Desmarcar cinco por engano ao tentar escolher uma delas é o desfecho que
ninguém quer, e desfazer isso é reselecionar tudo.

#### A segunda tela ✅ — e o `ViewportDeferred` que virou janela de verdade

[`cliente.rs`](../crates/ui-gpui/src/cliente.rs): a janela que o fotógrafo vira para o cliente. Tela
cheia no outro monitor, fundo preto, sem barra de título e sem controle — só a foto selecionada na
janela principal, com o nome e as estrelas num rodapé que `I` liga e desliga, e `Esc` para fechar.

🔑 **O que muda do egui para o GPUI é o caminho de volta.** Lá é um `ViewportDeferred` dentro do
mesmo `Context`, e as duas telas conversam por `ctx.data_mut` com chaves de texto: o `Esc` da segunda
janela **não fecha nada** — grava `secondary_window_close_req` na memória global do egui, e o quadro
seguinte da janela principal lê, consome e fecha. Aqui a janela tem entidade própria: `Esc` chama
`window.remove_window()`, e acabou.

⚠️ **E some junto o repaint incondicional.** O legado chama `ctx.request_repaint()` nos dois lados
enquanto a segunda tela estiver aberta, para ela não mostrar imagem velha. Aqui a foto chega por
chamada de método, quando muda.

🚨 **A inscrição que mantém a segunda tela em dia é a peça que some sem avisar.** É um `cx.observe`
na Biblioteca; descartado, a janela abre, mostra a primeira foto e **congela ali** — e do outro lado
do monitor não há como perceber. Conferido quebrando de propósito: o teste falha.

🔑 **O `DisplayId` do GPUI não pode ser construído de fora do crate** (o campo é `pub(crate)`), então
a regra de qual monitor usar é genérica no tipo do id. Com assinatura concreta, ela só poderia ser
conferida numa máquina com dois monitores plugados — que é o mesmo que não conferir. A regra é a do
legado: o primeiro que não for o principal, e o principal se não houver outro (é o que permite ver a
segunda tela funcionando sem um segundo monitor).

⚠️ **Contexto de teclado próprio.** As duas janelas existem ao mesmo tempo, e `Esc` na principal
volta para a Biblioteca enquanto `Esc` aqui fecha a janela. Com um contexto só, a tecla faria as duas
coisas conforme quem estivesse com o foco.

⚠️ **As instruções não dependem do `I`.** Desligar o rodapé e perder junto a única pista de como
fechar a janela deixaria uma tela preta sem saída visível, num monitor que costuma estar de costas
para quem a abriu.

#### 🚨 O docking, lido antes de portar: metade dele não tem como ser aberta

O `egui_dock` **é a tela inteira** do legado — Biblioteca e Revelação são dois `DockState`
persistidos (`eframe::set_value`), e o que se vê são abas arrastáveis. A leitura de 17/ago encontrou
três coisas que mudam o que "portar o docking" quer dizer:

1. 🚨 **9 das 19 abas nunca são criadas.** As duas funções de leiaute
   (`create_library_layout`, `create_develop_layout`) instanciam 10 abas, e **não há UI nenhuma para
   acrescentar aba** (`grep DockTab:: crates/ui/src` fora do módulo de docking: zero ocorrências).
   `Collections`, `BasicAdjustments`, `ToneCurve`, `HSLColor`, `HSLHue`, `HSLLuminance`,
   `LensCorrections`, `Detail` e `CropTool` têm código de desenho e nenhum caminho até a tela.
2. 🚨 **Fechar uma aba é irreversível** a menos de "Reset Docking Layout", nas Configurações — que
   joga fora o arranjo inteiro das duas telas. Fechar "AllAdjustments" na Revelação tira os 42
   controles, e a única volta custa todo o resto.
3. 🚨 **`views/develop_view.rs` (1.301 LOC) e `views/library_view.rs` (175 LOC) são código morto.**
   O `app.rs` não os menciona; quem desenha é o dock. O `library_view.rs:117` que o STATUS cita como
   "Coleções: UI é um TODO" está **dentro do arquivo morto** — o TODO é sobre uma tela que não abre.
   É a quarta vez que aparecem duas versões da mesma decisão com só uma viva (as outras: o JPEG com
   alfa, o `GpuEditParams::default`, o `print_dialog.rs`).

⚠️ **O que isso decide**: "portar o docking" vira duas coisas separadas.

| Parte | Estado |
|---|---|
| **O conteúdo das abas vivas** — as 10 que os leiautes criam | ✅ todas |
| **O rearranjo em si** (arrastar, redimensionar, persistir) | 🔄 **o dono pediu em 17/ago** — a Biblioteca já está no dock (abaixo) |

#### O dock da Biblioteca ✅ — e o truque que o fez caber num commit

Os quatro painéis (pastas, grade, informações, filmstrip) agora são `Entity` própria com a `trait
Panel`, dentro de um `DockArea` — arrastáveis e redimensionáveis, no arranjo do
`create_library_layout` de lá.

🔑 **Os painéis não têm estado próprio: eles chamam métodos da `Biblioteca`.** O desenho de cada um já
existia lá, e os `cx.listener` de lá esperam `Context<Biblioteca>` — mover o desenho para dentro de
views novas trocaria **todos** eles por `entidade.update(…)`, umas 600 linhas reescritas só para
mudar de lugar, com os testes por baixo. Assim o dock custou um arquivo pequeno
([`paineis.rs`](../crates/ui-gpui/src/biblioteca/paineis.rs)) e **nenhum teste precisou mudar**.

🚨 **A referência de volta é fraca, e não por elegância.** A `Biblioteca` guarda o `DockArea`, o dock
guarda os painéis, e os painéis apontam para a `Biblioteca`: com `Entity` nos dois sentidos isso é um
ciclo de contagem de referência — memória que não volta ao fechar a janela. Com `WeakEntity` na volta
o ciclo se abre, e quando ela não puder ser lida (a janela fechando) o painel desenha vazio em vez de
derrubar o app.

⚠️ **O dock é montado depois do construtor**, e é uma consequência da mesma coisa: os painéis precisam
de uma referência à entidade, e dentro de `Biblioteca::nova` ela ainda não foi entregue ao `cx`.

⚠️ **Nenhum painel fecha** — e no legado todos fecham. Voltar de um fechamento lá exige o "Reset
Docking Layout", que joga fora o arranjo das duas telas de uma vez: fechar a grade por engano custa
tudo o que se arrumou. Aqui eles se movem e se redimensionam, e some a única forma de perder um painel
sem querer.

🚨 **Os nomes dos painéis são o que o arranjo gravado guarda** (`biblioteca:grade` e companhia), e
mudá-los depois faz um leiaute salvo apontar para um painel que não existe. O `gpui-component` avisa
disso na própria `trait`; há teste para a mudança falhar aqui, e não na máquina de quem usa.

#### O arranjo sobrevive a fechar o app ✅

[`biblioteca/arranjo.rs`](../crates/ui-gpui/src/biblioteca/arranjo.rs): o `dump()` do dock vira JSON
**ao lado do catálogo** (`arranjo-biblioteca.json`), e volta na abertura. Arrumar a tela e perder a
arrumação ao fechar é o mesmo que não poder arrumar — e o legado grava o `DockState` das duas telas
(`eframe::set_value`), então isto é paridade.

🔑 **No catálogo, e não numa pasta de configuração do sistema.** O `VLB_CATALOG` é o que separa o
catálogo real do de medição; com o arranjo junto, rodar o app contra um catálogo descartável não mexe
na arrumação de quem trabalha.

⚠️ **Ler dali nunca derruba o app.** Arquivo corrompido, de outra versão, ou inexistente — os três
viram `None`, e `None` é o arranjo padrão. É o oposto da leitura do catálogo, onde falhar alto é o que
impede escrever em cima do dado de alguém; aqui o pior que um arquivo ruim custa é a arrumação da
tela. O número de versão existe para o dia em que um painel for dividido: o arranjo salvo passa a
descrever uma tela que não existe, e restaurá-lo daria uma Biblioteca sem grade.

⚠️ **A gravação é adiada em 500 ms**, a mesma espera dos ajustes da Revelação. O próprio
`gpui-component` avisa que `LayoutChanged` *"may be emitted too frequently"* — um arrasto de divisória
emite dezenas por segundo, e cada um seria um arquivo escrito.

🚨 **E o teste desta volta nasceu cego.** A primeira versão gravava, restaurava e comparava os dois
retratos — e **passava com o `register_panel` removido inteiro**. O motivo está no
`InvalidPanel::dump`: ele devolve *o estado antigo*, com o nome original dentro. O painel quebrado
mente no retrato e só se denuncia vivo, então a conferência passou a ser pelos painéis **dentro do
dock** (`items()`), onde ele responde `InvalidPanel`. Com o registro removido, o teste agora acusa
`["InvalidPanel", "InvalidPanel", "InvalidPanel", "InvalidPanel"]`.

🚨 **E montar o dock nos testes revelou que eles desenhavam outra tela.** Até aqui o `tela_com` dos
testes criava a Biblioteca **sem** dock — ou seja, sem painel nenhum embaixo da barra, uma tela que o
app nunca tem. Qualquer defeito de dock passaria despercebido por 15 testes. Foi o primeiro teste a
tocar no dock que acusou.

#### A Revelação no dock ✅ — e o caminho de gravação que escapou para o catálogo real

Cinco painéis, no arranjo do `create_develop_layout`: presets à esquerda, a foto no centro com o
filmstrip embaixo, gráficos e ajustes à direita.

⚠️ **Histograma e curva de tons ficam no mesmo painel**, e no legado são lugares diferentes (o
histograma é aba própria; a curva vive dentro de "AllAdjustments"). Os dois são desenho da mesma
medida, nenhum tem controle, e separá-los daria uma aba de 120px de altura para um gráfico só.

🚨 **E aqui a gravação do arranjo escapou para o catálogo real.** A troca do caminho fixo por um
parâmetro **não pegou** numa das substituições, e o que ficou no arquivo continuava chamando
`arranjo::caminho("biblioteca")` — o catálogo de verdade. O teste que eu tinha acabado de escrever
apontava para um `TempDir` e falhava dizendo "o arquivo não existe"; a explicação não era a gravação
não acontecer, era **ela estar acontecendo no lugar errado**. O `arranjo-biblioteca.json` apareceu em
`~/Pictures/VintageLightbox/`, escrito por `cargo test`.

🔑 **É o defeito da fase 0 de novo** — `PreviewManager::new()` num teste de UI escrevendo no cache do
fotógrafo —, e a defesa é a mesma: **o caminho entra por parâmetro** (`montar_o_dock_em`), e os testes
montam num arquivo descartável. A suíte inteira roda agora sem tocar no catálogo real, e isso é
conferível com um `ls`.

⚠️ **E o susto veio de um diagnóstico que quase acusou o inocente**: as três primeiras hipóteses foram
sobre o `advance_clock` não disparar o `timer` nos testes. Comparar com a espera da Revelação — que
dispara — foi o que mostrou que o problema não era o relógio.

⬜ **Falta**: a barra de cima da Biblioteca (busca, filtros, colunas) fica **fora** do dock de
propósito — um dock que pudesse fechá-la deixaria a tela sem busca e sem filtro.

#### O painel de informações ✅ — as duas abas vivas que faltavam

[`biblioteca/informacoes.rs`](../crates/ui-gpui/src/biblioteca/informacoes.rs) (as contas, sem tela) e
a coluna da direita da Biblioteca: dados da foto selecionada (nome, data, câmera, exposição, nota,
cor), a distribuição por nota e as câmeras mais usadas. É o `Metadata` do dock mais a parte legível
do `Quick Develop`.

🚨 **O `Quick Develop` do legado promete mais do que faz.** Ele desenha a fileira de cores com
`ColorLabels::show(ui, &None, false)`: a cor da foto **não é passada** e o `false` desliga o clique.
A fileira aparece sempre vazia e não responde a nada, num painel cujo nome diz ser para revelar
rápido. Aqui a cor é a da foto — e continua sem clique, porque quem marca cor são as teclas `6`–`9`.

🚨 **E o gráfico de câmeras de lá tem cinco barras e três nomes.** `show_camera_usage` desenha as
cinco mais usadas com `show_axes([false, true])` e lista o nome de **três** embaixo: as duas últimas
barras são anônimas. Aqui as cinco aparecem com nome e contagem.

🔑 **O empate entre câmeras é desempatado pelo nome, e no legado não é.** Lá a ordenação é só por
contagem, sobre um `HashMap`: duas câmeras com o mesmo número de fotos trocam de lugar entre
execuções, e a sexta colocada entra ou não na lista por sorteio. Painel de estatística que muda de
resposta sem o acervo mudar não é conferível. (Quarto lugar em que a ordem de um `HashMap` chega à
tela neste porte.)

⚠️ **A conta lê o acervo inteiro, e não a lista filtrada** — é o que o legado faz, e é o que faz
sentido: a distribuição existe para responder "como está o acervo". Sobre o filtrado, filtrar por
★★★★ desenharia sempre uma barra só.

⚠️ **E a largura útil da grade passou a descontar as duas colunas.** Contar só a das pastas faria
`colunas_que_cabem` responder mais colunas do que cabem — a última sairia cortada pela borda, que é o
mesmo defeito que a árvore causou quando entrou. Tem teste.

#### O filmstrip da Revelação ✅ — revelar é uma sequência

A Revelação abria **uma foto solta**: para ir à seguinte era voltar à grade, clicar e entrar de novo.
Revelar um casamento assim vira 400 trocas de tela para 200 ajustes.

Agora a lista filtrada vai junto (a mesma que a Impressão recebe), as setas andam nela e o filmstrip
do rodapé mostra a vizinhança da atual — a mesma decisão do filmstrip da Biblioteca, e a mesma
pergunta: *o que vem antes e depois desta*.

🔑 **A posição é a de dentro da lista filtrada**, e não o índice no acervo: com filtro ativo, o
índice do acervo apontaria para outra foto. São os dois espaços de índice que a grade já separava.

⚠️ **Andar grava a foto que sai.** A espera de 500 ms é uma janela de perda e a seta cai bem no meio
dela — arrastar um slider e apertar `→` é a sequência normal de quem revela em série. É a quarta
porta da gravação (as outras: o fim da espera, trocar de foto pela grade, e sair com `Esc`), e tem
teste conferido quebrando de propósito.

⚠️ **O filmstrip some com uma foto só.** Uma faixa de um item ocupa espaço da foto para não dizer
nada — e é o que acontece ao abrir a Revelação sem lista.

✅ **`Grid Settings` — decidido pelo dono em 17/ago: as duas coisas.** A barra tem **auto** (o padrão,
quantas couberem na janela) mais 1 a 5. O legado só tem o fixo (`state.grid_columns`, 4 por padrão) e
a grade nova só tinha o automático; manter os dois é o único arranjo que responde aos dois casos —
"aproveite a janela toda" e "quero ver estas quatro grandes".

⚠️ **É feature nova**, e a regra §7.1 só a permite assim: com decisão de dono registrada. O que o
teste prende é que a escolha **ganha da janela** — sem isso o botão acende, o número muda na barra e a
grade continua com as colunas que cabem.


### ✅ Fase 5 — desligamento, 17/ago/2026

`crates/ui` saiu do workspace em commit isolado (`8c7df32`): **21.352 LOC e 146 testes**, num commit
só, para o `git revert` ser um comando e trazer tudo de volta inteiro.

| A porta que a regra 4 exigia | Como foi cumprida |
|---|---|
| Nenhum teste apagado antes de virar linha | [PARIDADE-UI.md](PARIDADE-UI.md) — os 146, comportamento a comportamento |
| A lista tinha de zerar | Zerou: os últimos cinco itens entraram em 17/ago, e os dois que eram decisão de dono foram decididos |
| O app novo não podia depender dele | O WGSL já era **cópia** (a fase 2 escolheu copiar em vez de `include_str!` cruzando crates, por causa deste dia), e o teste que compara os dois byte a byte se desliga sozinho quando o outro lado some |

**O que mudou no dia seguinte ao desligamento:**

- `cargo run -p ui` deixou de existir. O app é `cargo run -p ui-gpui`.
- 🔑 **`cargo clippy --workspace --all-targets -- -D warnings` passa limpo** — o que nunca tinha
  acontecido. O job do CI cobria só as quatro camadas internas porque o `crates/ui` tinha centenas de
  avisos, e o gatilho quebrado da fase 0 escondia isso.
- A referência de paridade passou a ser este documento, o `PARIDADE-UI.md` e o histórico do git.

**Placar**: 668 passando, 0 falhando, 2 ignorados — e o app abre, revela, importa, imprime, tria e
mostra ao cliente.

**Total: 11–15 semanas** de trabalho focado de uma pessoa era a estimativa.

---

## 6. O que vai doer

1. 🚨 **146 testes de UI morrem** — 47 unitários e 99 E2E `egui_kittest` com snapshot, em 18
   arquivos. São a única especificação executável do comportamento da interface hoje.
   **Antes de apagar qualquer um, extrair dele a lista de comportamentos** para `docs/PARIDADE-UI.md`.
   ✅ O substituto existe e é decente: **`gpui::TestAppContext`** — é como o Zed testa a própria
   interface. Não é snapshot visual, mas dirige janela e afirma sobre estado.
   ✅ **E não é mais promessa**: os dois primeiros rodam desde 15/ago (`544a0cb`, fase 2), com o
   `test-support` ligado em `[dev-dependencies]`.
2. ⚠️ **`gpui` é pre-1.0** (0.2.2), com quebras assumidas entre versões. Não é pior que o egui, que
   já obrigou 0.28 → 0.31 aqui — mas também não é melhor.
3. ⚠️ **Windows é alpha** no Zed, com relatos de *DirectX device removal*. Hoje isso não bloqueia
   nada — o projeto não tem usuário —, mas vira decisão real antes de distribuir para Windows.
4. ⚠️ **Metal Toolchain** na máquina e no CI (§1).
5. ⚠️ **A reescrita de ~15.000 LOC.** Qualquer troca de framework cobra isso.

**O modo de falha mais provável não é técnico**: é cansar na fase 2 e ficar com dois frontends pela
metade. Por isso `crates/ui` fica vivo e rodando até a fase 5 — abandonar precisa custar zero.

---

## 7. Regras durante a migração — ⚠️ **revogadas em 17/ago/2026**

> 🚨 **Não siga esta seção.** Ela valia enquanto o alvo era o app de egui; hoje o alvo é o Lightroom
> ([`00-OBJETIVO.md`](00-OBJETIVO.md)) e as três primeiras regras dizem o **contrário** do que agora é
> certo. Ficam registradas porque explicam decisões que estão no código — não porque orientem o
> próximo commit.
>
> A que sobrevive é a 5 (*um incremento por commit, contando o que foi encontrado*), e ela nunca
> dependeu da migração.

1. ❌ ~~**Nenhuma feature nova.**~~ A conferência é paridade; feature nova torna impossível saber se a
   diferença é defeito ou escopo.
2. ✅ **O `crates/ui` compila e roda até a fase 5.** É o rollback e a referência de paridade.
3. ❌ **Nunca traduzir componente egui linha a linha.** Modo imediato traduzido para retido vira o
   pior dos dois. Portar é reescrever com a regra entendida.
4. 🚨 **Nenhum teste do `ui` é apagado antes de virar linha em `docs/PARIDADE-UI.md`.**
5. ✅ **Um incremento por commit**, com mensagem contando o que foi **encontrado**.

---

## 8. Depois do desligamento — o que a migração adiou de propósito

A fase 2 encontrou quatro defeitos e não consertou nenhum, sempre pela mesma razão: **conserto
durante o porte mistura "portei errado" com "estava errado"**. Enquanto os dois apps liam o mesmo
shader e os mesmos presets, qualquer mudança neles tirava a régua do lugar.

Com a fase 5, essa razão acabou — há um app só. Esta seção é a fila que sobrou, e o que já saiu dela.

### ✅ O `uniform` de 28 campos — alinhado em 17/ago/2026

O `struct Params` do WGSL passou a declarar os **46** campos do `Ajustes`, na mesma ordem, sem a
duplicata de `nr_luminance`. O que isso mudou, medido:

| Antes | Depois |
|---|---|
| 5 sliders de matiz aplicavam **outra coisa** (o do vermelho borrava a foto) | não aplicam nada — e não mentem mais |
| os **4 controles de Detalhe** não chegavam ao shader | ✅ **funcionam** — ruído (luminância e cor) e nitidez, que já tinham código no corpo |
| 19 ajustes fora do `uniform` | 19 ajustes **dentro** dele, ainda sem código que os use |

🔑 **Chegar e ser aplicado são duas coisas, e só a primeira estava quebrada.** O corpo do shader
sempre soube o que fazer com `nr_luminance`, `nr_color` e `sharpen_amount`; faltava o valor chegar.
Matiz, luminância e lente chegam agora e continuam inertes porque **não há código para eles** — que é
um estado honesto, e visível, em vez de um controle que faz o avesso do rótulo.

🚨 **E apareceu uma regra de alinhamento que ninguém precisava saber antes.** No endereço `uniform` do
WGSL a struct é arredondada para múltiplo de 16 bytes: 46 `f32` são 184, que **não** é — e o buffer
precisa ter 192, senão o `bind group` recusa. Com 28 campos (112 bytes) o problema não existia. É a
`TAMANHO_DO_UNIFORM` em `processador.rs`, e os 8 bytes de sobra nunca são escritos nem lidos.

**As duas razões do adiamento caíram juntas:**

1. *"Nos dois lados ao mesmo tempo"* — não há dois lados. O `crates/ui` saiu do workspace na fase 5.
2. *"Decisão de dono sobre o acervo existente"* — o acervo é **zero**. `select count(*) from photos`
   no catálogo real responde `0` desde que ele foi recriado limpo na fase 0, e nenhuma foto tem
   `hsl_*_hue` gravado. Não há aparência para mudar retroativamente.

⚠️ **A segunda razão volta a valer no dia em que houver acervo revelado**, e o número acima é como se
confere — não a lembrança de que estava vazio.

Os testes de `processador.rs` acompanharam a virada, e é para isso que existiam:

| Teste | O que ele fixa agora |
|---|---|
| `o_wgsl_declara_os_mesmos_46_campos_na_mesma_ordem` | lê o `.wgsl` e compara com o `Ajustes`, campo a campo — sem GPU |
| `o_basico_e_o_detalhe_chegam_ao_shader` | os 23 primeiros **e** os 4 de Detalhe mudam a foto |
| `os_dezenove_ajustes_sem_codigo_no_shader_nao_mudam_nenhum_pixel` | matiz (8), luminância (8) e lente (3) — o que falta |
| `o_matiz_do_vermelho_nao_borra_mais_a_foto` | o contraste local voltou ao do neutro |

O terceiro **tem de falhar** conforme cada família ganhar código, e some quando a última entrar.

### A fila que continua aberta

| O que | Onde mora | Por que ainda não |
|---|---|---|
| **Matiz e luminância do HSL** (16 sliders) | `image_adjustments.wgsl` | chegam ao shader, sem código — o bloco de HSL já tem a ponderação por faixa de matiz para a saturação, e é onde entram |
| **Lente** — distorção e vinheta (3) | idem | idem; distorção precisa reamostrar coordenada, e não é uma linha |
| **A exportação descarta 31 dos 46 ajustes** | `infrastructure/image_exporter.rs` | é outro caminho, na CPU, com a matemática duplicada — trabalho próprio |
| **A exportação ignora o crop** | idem | idem |
| **Os 5 presets de sistema fora de escala** | `use-cases`, `ListPresetsUseCase` | preso em `o_preset_bw_do_legado_nao_da_preto_e_branco`, que falha no dia em que alguém arrumar |
| **O corte fora do histórico de undo/redo** | `revelacao/historico.rs` | dívida própria do porte |
| **O preset recém-salvo aparece com id local** | `revelacao/presets.rs` | nada depende do id hoje |

---

## 9. Referências

| Assunto | Onde |
|---------|------|
| O tema Vintage Dark, e por que ele não é enfeite | [crates/ui-gpui/src/tema.rs](../crates/ui-gpui/src/tema.rs) |
| Spike compilado (fora do repositório) | `scratchpad/gpui-spike/src/main.rs` |
| O slider de 290 linhas que vira um componente | [advanced_slider.rs](../crates/ui/src/components/advanced_slider.rs) |
| Pipeline GPU que sobrevive | [gpu_processor.rs](../crates/ui/src/gpu_processor.rs) · [image_adjustments.wgsl](../crates/ui/src/shaders/image_adjustments.wgsl) |
| Estado de edição que sobrevive | [state.rs:20-78](../crates/ui/src/state.rs#L20-L78) |
| Caminho fixo do catálogo (fase 0) | [paths.rs](../crates/infrastructure/src/paths.rs) |
| Os 99 E2E que viram lista de paridade | [crates/ui/tests/](../crates/ui/tests/) |
| Avaliação do Tauri, descartada | [09-MIGRACAO-TAURI.md](09-MIGRACAO-TAURI.md) |
| `gpui-component` | https://github.com/longbridge/gpui-component |
