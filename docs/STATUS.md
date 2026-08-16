# Status do Projeto - VintageLightbox

**Última atualização**: 15 de agosto de 2026
**Último commit**: `318178d` — 25/jan/2026, _"feat: implement embedded preview extraction for RAW files"_
**Branch de trabalho**: `dev` (com 3 arquivos alterados e **não commitados** — veja abaixo)
**Estado**: ✅ compila, suíte verde, app sobe — depois de dois consertos feitos hoje

> ⚠️ **Este documento foi reescrito em 15/ago/2026 a partir do código, não do histórico.**
> A versão anterior datava de 20/dez/2025 e descrevia um projeto muito menor do que o que existe
> hoje (dizia "Adapters: não iniciado" — há 6 controllers; dizia 173 testes — há 481 declarados).
> Os números abaixo foram medidos rodando `cargo test`/`cargo check`, não copiados.

---

## ✅ O que foi consertado em 15/ago/2026 (mudanças locais, **não commitadas**)

O HEAD de `dev` não compilava. Dois erros independentes, ambos consertados:

### 1. `infrastructure` (lib) — quebrava o app inteiro

```
error[E0609]: no field `thumbnails` on type `rawloader::RawImage`
  --> crates/infrastructure/src/raw_processing.rs:108
```

`extract_embedded_preview()` (último commit, 25/jan) lia `raw.thumbnails`, campo que **não existe**
em `rawloader 0.37.1`. Esse commit já tinha sido revertido uma vez (`a2a3e60` reverteu `6f0c1c8`) e
voltou no mesmo dia sem correção.

**Conserto**: reescrito sobre `rsraw::RawImage::extract_thumbs()` — a mesma LibRaw que
`load_raw_as_dynamic_image` já usava. A função agora recebe `min_height` e devolve o **menor**
preview JPEG que atende, em vez de um índice arbitrário (o `[0]` do código anterior seria o menor,
apesar do comentário dizer "geralmente o maior"). Previews não-JPEG são descartados, porque quem
chama passa o resultado por `image::load_from_memory`. `unpack()` não é chamado de propósito — o
preview sai do arquivo sem demosaic, que é o ponto do caminho rápido.

### 2. `use-cases` (lib test) — quebrava a suíte, não o app

```
error[E0061]: this method takes 55 arguments but 47 arguments were supplied
  --> crates/use-cases/src/save_photo_edits.rs:186
```

O Crop & Rotate (27/dez/2025) acrescentou 8 parâmetros a `SavePhotoEditsUseCase::execute` e o teste
no próprio arquivo não foi atualizado. A suíte estava quebrada **desde 27/dez/2025**, e o CI
vermelho junto, nas três plataformas.

**Conserto**: os 8 `None` que faltavam. ⚠️ **É remendo, não solução** — a assinatura de 55
parâmetros posicionais é a causa, e vai quebrar de novo no próximo ajuste de edição. Trocar por um
struct `PhotoEdits` continua sendo o conserto de verdade.

---

## 📊 Métricas medidas

| Métrica | Valor |
|---------|-------|
| `cargo check --workspace --all-targets` | ✅ **limpo** |
| `cargo test --workspace` | ✅ **612 passando, 0 falhas, 3 ignorados** (16/ago) |
| App | ✅ **sobe** — janela 1352×848, `GPU: Initialized successfully with Apple M2 Pro` |
| Migrations SQLite no repositório | 15 (`001` … `015`) |
| Crates | 6 (domain, use-cases, adapters, infrastructure, ui, **ui-gpui**) |

### Testes por camada

| Camada | Testes | Situação |
|--------|-------:|----------|
| Domain | 202 | ✅ passando |
| Use Cases | 65 | ✅ passando |
| Adapters | 0 | ⚠️ nenhum teste escrito |
| Infrastructure | 65 (34 unit + 31 integração em 7 arquivos) | ✅ passando (1 ignorado) |
| UI (egui) | 146 (47 unit + 99 E2E `egui_kittest` em 18 arquivos) | ✅ passando (2 ignorados) |
| UI (GPUI) | 86 (83 unit + 3 de integração com banco) | ✅ passando — fase 2 em andamento |

---

## 📥 Tela de importação reescrita em 15/ago/2026

A tela existia mas **não importava**: o botão "Import N Photos" era um `// TODO: call controller`
seguido de volta para a biblioteca. Escolher fotos, na prática, só dava pelo botão "Advanced
Import" — seletor de **um arquivo por vez** e uma lista de texto, sem miniatura nenhuma, apesar de
o `ImportPreviewItemViewModel` já carregar os bytes do thumbnail.

Agora é o formato do Lightroom, num **modal** sobre a biblioteca: **DE** (cartões, recentes,
escolher pasta, incluir subpastas) · **grade de miniaturas marcáveis** · **PARA** (modo, destino,
organização, renomeação, duplicatas). Importar é tarefa que começa e termina, não lugar onde se
fica — por isso `CurrentView::Import` **deixou de existir**, e o estado virou
`ImportViewState::open`.

**O que faz a tela abrir rápido é a ordem das leituras**, cada uma assíncrona e independente:

1. `ScanSource` lista só caminhos — a grade aparece cheia na hora;
2. `DescribeCandidates` lê EXIF em paralelo e as células vão se completando;
3. miniaturas são geradas **só para as células visíveis** (`show_rows` + `AsyncThumbnailLoader`),
   com chave `import::<caminho>` para não colidir com id de foto no cache;
4. `CheckDuplicates` confere por hash e desmarca o que já está no catálogo.

Emendar 3 em 1 é o que faria um cartão de 2.000 RAWs travar a janela por minutos.

**`ImportOptions` ganhou o que a tela precisa decidir**: `mode` (`Add`/`Copy`/`Move`),
`destination`, `source_root` e `include_subfolders`. Os campos novos têm `#[serde(default)]` —
catálogo gravado antes deles continua lendo (há teste). `PreserveStructure` passou a preservar
mesmo a hierarquia (usando `source_root`); `IntoOneFolder` é o comportamento antigo, agora
nomeado.

⚠️ **`Move` apaga o original** — e só depois de a foto estar no catálogo e as previews gravadas.
Falha ao apagar não invalida a importação: sobra uma cópia órfã na origem, e isso vai para o log.
Coberto por 7 testes E2E com JPEGs reais em disco (`import_modes_e2e.rs`).

⚠️ **O modal forçou trocar o seletor de pastas.** `egui_file::FileDialog` é uma `Window` do egui
(`Order::Middle`); o backdrop do modal fica em `Order::Foreground` e `set_modal_layer` bloqueia a
entrada das camadas abaixo — o seletor apareceria escurecido e sem responder ao clique. Passou a ser
o **seletor nativo do sistema** (`rfd::AsyncFileDialog`, dependência nova em `ui`), que é janela do
SO e não disputa camada com o egui. A lupa, pelo mesmo motivo, virou modal aninhado em vez de
`Window`.

O seletor abre nas **Imagens do usuário** (`AppPaths::default_browse_dir`), ou onde a escolha
anterior parou — antes abria em `/`, obrigando a descer `Users` → nome → Pictures toda vez.

**A aparência foi refeita numa segunda passada**, depois de a primeira versão ficar com cara de
protótipo: painéis com fundo próprio (`BG_ELEVATED` no cabeçalho/rodapé, `BG_SURFACE` nas laterais,
`BG_APP` na grade) para as três regiões se separarem; **controle segmentado** no lugar da fileira de
`selectable_label` que parecia três links soltos; linhas de origem com ícone, nome, caminho e barra
de acento; marcador de seleção maior, com ✓ de verdade; estados vazios com ícone e uma saída
("Tente ligar Incluir subpastas"); e **prévia do destino** — "a primeira foto vai para
2026/08/15/photo-2026-08-15-001.cr2" —, que transforma três combos abstratos numa decisão
conferível antes de apertar o botão.

Na interação: duplo clique abre a lupa (antes alternava a marcação, o que contradizia o clique
simples), ↑↓ navegam pelas linhas **levando a rolagem junto** (sem isso o foco saía da tela e a
seta parecia não fazer nada), Shift+clique marca intervalo, ⌘A marca tudo e Enter importa.

**Removido junto**: o botão "Advanced Import", o `ImportPreviewDialog` e o `preview_import` do
controller. Ficaram sem chamador quando a tela nova passou a fazer o trabalho inteiro — e manter
dois caminhos de importação, um deles pior, é convite a usar o errado.

---

## 🚨 Bloqueio que restou: 4 migrations aplicadas que não existem no repositório

O app subiu só depois de encostar o catálogo local. Ele morria no start-up:

```
panicked at crates/ui/src/main.rs:59:
Failed to run database migrations: Migrate(VersionMissing(16))
```

A tabela `_sqlx_migrations` do catálogo em `~/Pictures/VintageLightbox/` registra **19** migrations
aplicadas; o repositório tem **15**. As quatro que faltam:

| Versão | Descrição | Aplicada em | Onde está o arquivo |
|-------:|-----------|-------------|---------------------|
| 16 | add crop fill mode | 28/dez/2025 | só nas branches `feature/refactur_arc` e `Diffusion-CNN-Content-Aware` |
| 17 | create print jobs table | 01/jan/2026 | **em nenhuma branch** |
| 18 | update fill mode default | 01/jan/2026 | **em nenhuma branch** |
| 19 | add preset hsl fields | 25/jan/2026 | **em nenhuma branch** |

O banco local tem a tabela `print_jobs` criada; o repositório não sabe criá-la. E o código atual em
`dev` **não referencia** `fill_mode`, `print_jobs` nem campos HSL de preset — zero ocorrências. Ou
seja: essas migrations vieram de trabalho que ficou fora de `dev`, e o `PrintJob`/`print_view` que
existem no código hoje trabalham sem a tabela que alguém já criou no banco.

**Duas consequências práticas**:
1. Quem clonar o repositório hoje monta um catálogo com 15 migrations — e nunca vai bater com este.
2. Qualquer migration nova em `dev` vai nascer como `016` e colidir com a `016` das branches
   laterais.

**Decidido em 15/ago/2026**: o catálogo antigo virou
`~/Pictures/VintageLightbox/VintageLightbox Catalog/vintage_lightbox.db.bak-20260815` e o app criou
um novo, limpo, com as 15 migrations do repositório. Voltar atrás é renomear de volta.

---

## 🎯 Progresso por camada

### 1️⃣ Domain ✅ saudável

**202 testes passando, 0 falhas.** É a única camada verificável hoje.

- **Entidades**: `Photo`, `Collection`, `Preset`, `PrintJob`
- **Value Objects** (13): `PhotoId`, `CollectionId`, `PrintJobId`, `Rating`, `ColorLabel`, `Flag`,
  `FilePath`, `PhotoMetadata`, `CropSettings`, `AspectRatio`, `ImportOptions`, `PrintLayout`,
  `PrintSettings`
- **Serviços**: `FileOrganizer`, `PreviewStorage` (traits)
- **Repositórios** (traits): `PhotoRepository`, `CollectionRepository`, `PresetRepository`
- **Erros**: `DomainError` / `DomainResult` com `thiserror`
- **Property-based testing**: 5 blocos `proptest!`

`Photo` cresceu muito além do documentado em dez/2025: além de rating/color label/flag, carrega
**~50 campos de edição** — básicos, tone curve (4 zonas), HSL (8 canais × hue/sat/lum = 24),
correção de lente, redução de ruído, nitidez e crop.

### 2️⃣ Use Cases ⚠️ implementado, suíte quebrada

**20 módulos** (a versão anterior deste doc listava 7):

| Área | Use Cases |
|------|-----------|
| Importação | `ImportPhoto`, `ImportPhotos`, `ImportWithOptions`, `PreviewBeforeImport`, `CheckDuplicates`, `GetImportSources`, `ScanSource`, `DescribeCandidates` |
| Organização | `RatePhoto`, `SetColorLabel`, `SetFlag`, `DeletePhoto`, `Organize` |
| Coleções | `CreateCollection`, `AddPhotoToCollection`, `RemovePhotoFromCollection` |
| Edição | `SavePhotoEdits`, `Edit` |
| Presets | `SavePreset`, `ListPresets`, `DeletePreset` |
| Saída | `ExportPhoto`, `Export`, `ConfigurePrintJob` |

⚠️ **`SavePhotoEditsUseCase::execute` recebe 55 parâmetros posicionais.** O erro 2 é sintoma disso:
a assinatura cresce a cada feature de edição e o call site quebra em silêncio. É candidato natural a
um struct `PhotoEdits` — e o conserto do teste sem essa mudança só adia a próxima quebra.

### 3️⃣ Adapters 🔄 existe, sem teste

6 controllers (`Import`, `Library`, `Editor`, `Export`, `Photo`, `Preset`), mais `presenters.rs` e
`view_models.rs`. **Zero testes** — é o único vão de cobertura estrutural do projeto.
`LibraryController` está praticamente vazio (só `new`).

### 4️⃣ Infrastructure ❌ bloqueada

- **Database (sqlx/SQLite)**: `PhotoRepositoryImpl`, `CollectionRepositoryImpl`,
  `SqlitePresetRepository` + 15 migrations
- **Cache**: `preview_manager` — hierarquia L1 RAM (LRU 15 imagens) / L2 Smart Previews (BLOB
  SQLite) / L3 disco, documentada em [08-CACHE-ARCHITECTURE.md](08-CACHE-ARCHITECTURE.md)
- **RAW**: `raw_processing` com `rsraw` (LibRaw: demosaic, white balance, cor) e `rawloader` como
  fallback ← **onde está o erro 1**
- **Arquivos**: `file_scanner`, `file_organizer`, `source_scanner`, `content_hash`, `paths`
  (`AppPaths` resolve catálogo por SO), `exif_reader`, `thumbnail_generator`, `image_exporter`
- **Dispositivos**: `devices/` — detecção de fontes de importação (cartões) + histórico

### 5️⃣ UI ❌ bloqueada (transitivo)

egui **0.31** com eframe sobre **wgpu** — não glow/OpenGL.

- **4 views**: `library_view`, `develop_view`, `import_view`, `print_view`
- **26 componentes**, incluindo `crop_panel`/`crop_overlay`/`crop_toolbar`, `thumbnail_renderer`,
  `filmstrip` (+ filtro e janelas secundárias), `histogram_plot`, `tone_curve`, `metadata_charts`,
  `print_dialog`, `settings_dialog`, `import_dialogs`
- **Design system**: 5 temas, tokens, Phosphor Icons, `theme_selector`
- **Docking** (`egui_dock` 0.16), **multi-monitor** (`monitors.rs`, janelas secundárias)
- **`gpu_processor.rs`**: pipeline wgpu com shader single-pass (NR + sharpening 5×5)
- **`async_loader.rs`**: `ProcessedCache` + prefetch paralelo de vizinhos

---

## 📦 O que entrou desde a última atualização real (dez/2025 → jan/2026)

- ✅ **Crop & Rotate completo** (27/dez): `CropSettings`, `AspectRatio`, painel + overlay + toolbar,
  rotação e flip, renderização por mesh com UV, persistência (migration `015`), auto-apply na
  navegação, aplicação nos thumbnails via `thumbnail_renderer`, 4 arquivos de teste E2E
- ✅ **HSL 8 canais** (hue/sat/lum) e **correção de lente** — migrations `011` e `014`
- ✅ **Redução de ruído e nitidez** em single-pass no shader
- ✅ **Presets** com persistência (migration `010`) e painel na UI
- ✅ **Print**: `PrintJob`, `PrintLayout`, `PrintSettings`, `print_view`, `print_dialog`
- ❌ **Extração de preview embutido em RAW** (25/jan) — **não compila**; é o erro 1

✅ O roadmap ([04-ROADMAP.md](04-ROADMAP.md)) foi corrigido junto com este documento: a seção 2.9
(Crop & Rotate) estava marcada "📋 PLANEJADO" e o HSL da 2.2 como pendente — ambos implementados
desde dez/2025.

---

## ⚠️ Lacunas encontradas na leitura do código

Não são erros de compilação; são features que a UI mostra como prontas e que não fecham o ciclo.

0. 🚨 **O painel de revelação tem 18 sliders que não fazem nada, e 5 que fazem outra coisa.** O
   `struct Params` do WGSL (`crates/ui/src/shaders/image_adjustments.wgsl`) declara **28** campos
   para os **46** que `GpuEditParams` manda, e o `uniform` casa por posição. Do campo 23 em diante o
   shader lê o do vizinho — "HSL / matiz — Vermelho" **borra a foto**, porque ali o shader espera
   `nr_luminance`; amarelo e verde aplicam ruído de cor e nitidez. Do 28 em diante nada chega:
   HSL/luminância inteiro, três matizes, os **4 controles de Detalhe** e os **3 de Lente**. Nada
   falha: o buffer é maior que o mínimo do binding, então o wgpu ignora a sobra, e a duplicata de
   `nr_luminance` no WGSL o naga aceita. Medido em 16/ago/2026 e preso por quatro testes em
   `crates/ui-gpui/src/revelacao/processador.rs`; a tabela posição a posição está em
   [docs/10-MIGRACAO-GPUI.md](10-MIGRACAO-GPUI.md), §"Fase 2".
   ⚠️ **Consertar é decisão de dono, não de migração**: as fotos já reveladas têm `hsl_*_hue` gravado
   no banco, e alinhar o shader muda a aparência delas retroativamente.
1. 🚨 **A exportação ignora o crop.** `ImageExporterImpl::export` abre o arquivo original, aplica os
   ajustes tonais e grava — sem nenhuma referência a crop, rotação ou flip. O usuário corta a foto,
   vê o corte no viewer e nos thumbnails, exporta e recebe a imagem inteira.
   🚨 **E é bem maior que o crop** (medido em 15/ago/2026, ao portar o motor para GPUI):
   `ImageExporterImpl::process_image` aplica **15** ajustes; o shader que desenha a tela aplica
   **46**. A exportação descarta em silêncio a **curva de tons** inteira (4), o **HSL inteiro** —
   saturação, matiz e luminância nos 8 canais (24) — e a **lente** (3). Quem revela mexendo em HSL vê
   o resultado na tela, exporta e recebe outra imagem. A conta está em
   [docs/10-MIGRACAO-GPUI.md](10-MIGRACAO-GPUI.md), §"Fase 2", com o script que a refaz.
2. 🚨 **O undo/redo ignora o crop.** `EditSnapshot` (`crates/ui/src/state.rs:20`) lista os ~50
   campos de edição, mas nenhum de crop. Cortar não entra no histórico, e desfazer um ajuste
   posterior não restaura o corte anterior.
3. ⚠️ **Crop no shader GPU foi revertido** (`305466e` → `10dda3f`). O corte roda por mesh/UV no
   viewer e por CPU (`ImageProcessing::apply_crop`) nos thumbnails. Funciona, mas é caminho
   diferente do resto do pipeline de edição, que é GPU.
4. ⚠️ **Coleções: backend pronto, UI é um TODO** (`library_view.rs:117`).
5. ⚠️ **Adapters sem nenhum teste**, e `LibraryController` só tem `new`.

---

## 🔜 Próximos passos, em ordem

1. **Commitar os dois consertos** — hoje só existem na árvore de trabalho. Enquanto não forem
   commitados, o CI segue vermelho e um `git stash` os perde.
2. **Decidir o destino das migrations 16-19.** As opções reais são recriar os arquivos a partir do
   schema do banco antigo (o `.bak-20260815` ainda tem tudo), ou assumir que aquele trabalho ficou
   nas branches laterais e seguir de 016 em `dev` sabendo da colisão.
3. **Trocar os 55 parâmetros de `SavePhotoEditsUseCase::execute` por um struct** — a quebra de
   27/dez foi sintoma, não causa.
4. **Fechar o ciclo do crop**: exportação e undo/redo (veja "Lacunas" abaixo).
5. **Testar a camada Adapters** (0 testes hoje).
6. **Coleções na UI**: backend pronto e testado, mas `library_view.rs:117` ainda é
   `// TODO: Create new collection`.

---

## 🛠️ Ferramentas

| Área | Estado |
|------|--------|
| `cargo test` + `mockall` + `proptest` | ✅ configurado |
| `egui_kittest` (E2E de UI, com snapshots) | ✅ 18 arquivos, 101 testes |
| `criterion` / `insta` | ✅ configurados |
| CI GitHub Actions (Ubuntu/macOS/Windows, fmt, clippy `-D warnings`, tarpaulin) | ⚠️ vermelho desde 27/dez/2025 — verde quando os consertos forem commitados |
| `dev.sh` (`test`, `test:watch`, `coverage`, `check`) | ✅ |

---

## 📚 Estado da documentação

| Documento | Situação |
|-----------|----------|
| [01-REQUISITOS.md](01-REQUISITOS.md) | ✅ |
| [02-ARQUITETURA.md](02-ARQUITETURA.md) | ⚠️ revisar (fala em Slint em partes) |
| [03-FUNCIONALIDADES.md](03-FUNCIONALIDADES.md) | ✅ |
| [04-ROADMAP.md](04-ROADMAP.md) | ⚠️ 2.9 e HSL desatualizados |
| [05-STACK-TECNOLOGICO.md](05-STACK-TECNOLOGICO.md) | ⚠️ revisar (Slint × egui) |
| [06-UI-ARCHITECTURE.md](06-UI-ARCHITECTURE.md) | ❌ descreve UI em Slint; a UI é egui |
| [07-E2E-TESTING.md](07-E2E-TESTING.md) | ✅ |
| [08-CACHE-ARCHITECTURE.md](08-CACHE-ARCHITECTURE.md) | ✅ confere com o código |
| STATUS.md | ✅ este documento |

`CLAUDE.md` na raiz também está desatualizado: diz egui 0.28 com backend glow (é 0.31 com wgpu) e
cita 2 views (são 4).

---

## 🚀 Como rodar os testes

```bash
cargo test --workspace          # 478 passando, 0 falhas, 3 ignorados
cargo test -p domain            # 202 testes, ~0.01s

# E2E de UI (egui_kittest)
cargo test -p ui --test crop_feature_e2e_test
UPDATE_SNAPSHOTS=true cargo test -p ui   # atualizar snapshots

cargo run -p ui                 # sobe o app
```

⚠️ **O app usa um caminho fixo de catálogo** (`~/Pictures/VintageLightbox/VintageLightbox Catalog/`,
via `AppPaths::catalog_root()`), sem variável de ambiente para redirecionar. Não dá para rodar
contra um catálogo descartável sem mexer no do usuário — o que atrapalha teste manual e vale como
melhoria.

---

**Última execução de testes**: 15/ago/2026
**Resultado**: ✅ 478 passando, 0 falhas, 3 ignorados · app sobe e renderiza
