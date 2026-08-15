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

### Fase 0 — Pré-condições (≈ 1 semana)

1. **Commitar a árvore** — 25 arquivos modificados e 5 não rastreados em `dev`, incluindo a
   reescrita da tela de importação. Migrar por cima disso torna impossível separar "quebrou agora"
   de "já estava quebrado".
2. **Resolver as migrations 16–19** ([STATUS](STATUS.md)) — o app novo roda as mesmas migrations.
3. **Catálogo por variável de ambiente** — hoje `AppPaths::catalog_root()` é caminho fixo; sem isso
   não há teste automatizável contra catálogo descartável.
4. ✅ **`image` 0.24 → 0.25** (§3.2) — **feito em 15/ago**. Quatro pontos de código, 522 testes
   verdes, e um defeito de alfa no cache de preview que só apareceu porque a versão nova reclama.
5. **CI verde**, sem o Ubuntu (§1), com o passo da Metal Toolchain.

### Fase 1 — Biblioteca, num crate ao lado (2 semanas)

`crates/ui-gpui` entra no workspace **sem tirar `crates/ui`**. Os dois compilam, os dois rodam:
`cargo run -p ui` e `cargo run -p ui-gpui`.

Entrega: grade virtualizada com miniaturas de verdade, filmstrip, árvore de pastas, filtros,
nota/cor/sinalizador. Inclui a ponte de imagem da §3.1 — sem ela não há miniatura.

**Critério de saída**: abrir o catálogo real e navegar 2.000 fotos a 60fps.
**É aqui que você decide se gosta de morar nisso.** Duas semanas jogadas fora se não gostar, em vez
de cinco meses.

### Fase 2 — Revelação (3–4 semanas)

Sliders, histograma, curva de tons, HSL nos 8 canais, detalhe, lente, crop overlay, undo/redo,
presets. O `gpu_processor.rs` e o WGSL **não mudam** — só o último passo, que hoje devolve
`ColorImage`.

**Critério de saída**: editar um RAW e exportar com **o mesmo resultado de pixel** do app egui,
conferido por comparação automatizada de imagem.

⚠️ Aqui se consertam as duas lacunas que o STATUS registra: a exportação ignora o crop, e o
undo/redo ignora o crop. Reproduzir defeito conhecido de propósito custa mais do que arrumar.

### Fase 3 — Importação (2–3 semanas)

O modal de 4 etapas assíncronas (`import_view.rs`, 2.107 LOC), reescrito em 15/ago e ainda fresco.
A ordem das leituras — escanear, descrever, miniaturar o visível, conferir duplicatas — é regra
conquistada e se preserva.

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
| Spike compilado (fora do repositório) | `scratchpad/gpui-spike/src/main.rs` |
| O slider de 290 linhas que vira um componente | [advanced_slider.rs](../crates/ui/src/components/advanced_slider.rs) |
| Pipeline GPU que sobrevive | [gpu_processor.rs](../crates/ui/src/gpu_processor.rs) · [image_adjustments.wgsl](../crates/ui/src/shaders/image_adjustments.wgsl) |
| Estado de edição que sobrevive | [state.rs:20-78](../crates/ui/src/state.rs#L20-L78) |
| Caminho fixo do catálogo (fase 0) | [paths.rs](../crates/infrastructure/src/paths.rs) |
| Os 99 E2E que viram lista de paridade | [crates/ui/tests/](../crates/ui/tests/) |
| Avaliação do Tauri, descartada | [09-MIGRACAO-TAURI.md](09-MIGRACAO-TAURI.md) |
| `gpui-component` | https://github.com/longbridge/gpui-component |
