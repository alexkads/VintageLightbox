# Migração da UI para GPUI

**Escrito em**: 15 de agosto de 2026
**Estado**: ✅ **decidido** — GPUI. O plano de Tauri ([09](09-MIGRACAO-TAURI.md)) foi avaliado e descartado.
**Base**: código em `dev` na data acima, mais um **spike compilado e rodando** (§1)

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
| ⬜ Carregamento assíncrono das fotos (hoje bloqueia a abertura) | |

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

⚠️ **Fica preso em teste, e não consertado** — mesma razão do exportador: conserto durante o porte
mistura "portei errado" com "estava errado", e aqui há um agravante. As fotos já reveladas têm
`hsl_*_hue` gravado no banco; arrumar o alinhamento muda **retroativamente** a aparência delas —
o que era borrão vira giro de matiz. É trabalho próprio, nos dois lados ao mesmo tempo, com decisão
de dono sobre o acervo existente.

O que roda hoje, em [`processador.rs`](../crates/ui-gpui/src/revelacao/processador.rs):

| Teste | O que ele fixa |
|---|---|
| `o_wgsl_declara_28_campos_para_os_46_que_o_rust_manda` | lê o `.wgsl` e cobra a tabela acima, posição a posição — sem GPU |
| `os_ajustes_a_partir_do_campo_28_nao_mudam_nenhum_pixel` | a contraprova medida na imagem: os 23 primeiros mudam, os 18 últimos não |
| `o_matiz_do_vermelho_borra_a_foto_em_vez_de_girar_a_cor` | o contraste local **cai** — assinatura de borrão, que nenhum giro de matiz produz |
| `detalhe_e_lente_nao_chegam_ao_shader` | os 7 sliders de Detalhe e Lente, um a um |

O primeiro **tem de falhar** no dia em que o WGSL for consertado. É o lembrete de que a tabela, este
documento e o `crates/ui` mudam juntos.

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

⚠️ **O que fica de dívida própria**: o preset recém-salvo com id local, o corte fora do histórico, e o
carregamento síncrono da foto na abertura (o mesmo que a fase 1 deixou).

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

⬜ **Falta**: as miniaturas da grade sob demanda (com a chave `import::<caminho>`, que separa estas
entradas das fotos catalogadas), o lado "PARA" (modo, destino, organização, renomeação) e os atalhos
da grade (↑↓ com rolagem, Shift+clique na tela, ⌘A, Enter).

### Fase 4 — Impressão, multi-monitor, docking, atalhos (2–3 semanas)

### Fase 5 — Testes e desligamento (1–2 semanas)

`crates/ui` sai do workspace **por último**, em commit isolado, para o `git revert` ser um comando.

**Total: 11–15 semanas** de trabalho focado de uma pessoa.

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

## 7. Regras durante a migração

1. ❌ **Nenhuma feature nova.** A conferência é paridade; feature nova torna impossível saber se a
   diferença é defeito ou escopo.
2. ✅ **O `crates/ui` compila e roda até a fase 5.** É o rollback e a referência de paridade.
3. ❌ **Nunca traduzir componente egui linha a linha.** Modo imediato traduzido para retido vira o
   pior dos dois. Portar é reescrever com a regra entendida.
4. 🚨 **Nenhum teste do `ui` é apagado antes de virar linha em `docs/PARIDADE-UI.md`.**
5. ✅ **Um incremento por commit**, com mensagem contando o que foi **encontrado**.

---

## 8. Referências

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
