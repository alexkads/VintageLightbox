# Arquitetura da Interface - VintageLightbox

**Última atualização**: 31 de dezembro de 2025  
**Framework**: egui 0.31 (Immediate Mode GUI)

Este documento descreve a arquitetura da interface de usuário do VintageLightbox, construída com **egui** seguindo princípios de Clean Architecture e Design System.

## Visão Geral

A UI foi desenvolvida com egui (Immediate Mode GUI), oferecendo:
- **4 Views principais**: Library, Develop, Print, Import
- **25+ componentes** reutilizáveis
- **5 temas** (Light, Dark, Nord, Monokai, Solarized)
- **GPU-accelerated** rendering via wgpu
- **Design System** com tokens e ícones Phosphor

## Responsabilidades (UI sem regras de negócio)

- Renderizar estado vindo de ViewModels e presenters.
- Transformar interações do usuário em `AppAction` encaminhadas para os controllers.
- Não realizar validações ou mutações de domínio; todas as regras vivem em Use Cases e Domain.

## Estrutura de Arquivos

```
crates/ui/src/
├── main.rs                         # Entry point
├── app.rs                          # Loop principal, estado global
├── state.rs                        # ViewState, seleção, zoom, modo
├── design_system/
│   ├── mod.rs                      # Re-exports
│   ├── tokens.rs                   # Design tokens (cores, spacing)
│   ├── themes.rs                   # 5 temas predefinidos
│   └── icons.rs                    # Phosphor Icons integration
├── views/
│   ├── mod.rs                      # Re-exports
│   ├── library_view.rs             # Grid de fotos + sidebars
│   ├── develop_view.rs             # Editor de foto + ajustes
│   ├── print_view.rs               # Módulo de impressão
│   └── import_view.rs              # Wizard de importação
├── components/
│   ├── mod.rs                      # Re-exports
│   ├── toolbar.rs                  # Barra de navegação principal
│   ├── photo_grid.rs               # Grid de thumbnails com seleção
│   ├── filmstrip.rs                # Thumbnails horizontais
│   ├── histogram.rs                # Histograma RGB
│   ├── rating_widget.rs            # Estrelas interativas (0-5)
│   ├── color_labels.rs             # Seletor de 5 cores
│   ├── flag_widget.rs              # Pick/Reject/Unflagged
│   ├── slider_control.rs           # Slider com label e valor
│   ├── image_viewer.rs             # Viewer com zoom/pan
│   ├── tone_curve.rs               # Editor de curva de tons
│   ├── crop_overlay.rs             # Overlay de corte interativo
│   ├── aspect_ratio_selector.rs    # Proporções predefinidas
│   ├── progress_bar.rs             # Barra de progresso
│   ├── busy_overlay.rs             # Overlay de carregamento
│   ├── search_bar.rs               # Busca de fotos
│   ├── filter_bar.rs               # Filtros de biblioteca
│   └── ... (25+ componentes)
├── panels/
│   ├── mod.rs                      # Re-exports
│   ├── navigator_panel.rs          # Preview da foto selecionada
│   ├── catalog_panel.rs            # Navegação do catálogo
│   ├── collections_panel.rs        # Lista de coleções
│   ├── quick_develop_panel.rs      # Controles rápidos de edição
│   ├── metadata_panel.rs           # Exibição de metadados EXIF
│   ├── presets_panel.rs            # Presets salvos
│   ├── history_panel.rs            # Histórico de edição
│   └── basic_adjustments_panel.rs  # Controles completos de edição
├── async_loader.rs                 # Carregamento assíncrono de imagens
└── gpu_processor.rs                # Interface com wgpu para ajustes
```

## Design System

### Design Tokens (`design_system/tokens.rs`)

Tokens centralizados garantem consistência visual em toda a aplicação:

```rust
pub struct DesignTokens {
    // Cores de Background
    pub bg_app: Color32,         // #1a1a1a (dark)
    pub bg_surface: Color32,     // #252525
    pub bg_elevated: Color32,    // #2d2d2d
    pub bg_hover: Color32,       // #353535

    // Cores de Texto
    pub text_primary: Color32,   // #e0e0e0
    pub text_secondary: Color32, // #b0b0b0
    pub text_muted: Color32,     // #808080

    // Cores de Accent
    pub accent_primary: Color32, // #4a9eff

    // Espaçamentos
    pub space_xs: f32,           // 4.0
    pub space_sm: f32,           // 8.0
    pub space_md: f32,           // 12.0
    pub space_lg: f32,           // 16.0
    pub space_xl: f32,           // 24.0

    // Tipografia
    pub font_sm: f32,            // 11.0
    pub font_md: f32,            // 12.0
    pub font_lg: f32,            // 14.0
    pub font_xl: f32,            // 18.0

    // Raios de borda
    pub radius_sm: f32,          // 4.0
    pub radius_md: f32,          // 8.0
    pub radius_lg: f32,          // 12.0
}
```

### Temas (`design_system/themes.rs`)

5 temas disponíveis:

| Tema | Descrição |
|------|-----------|
| `Dark` | Tema escuro padrão (como Lightroom) |
| `Light` | Tema claro |
| `Nord` | Tons azul-acinzentados |
| `Monokai` | Tons quentes, inspirado no editor |
| `Solarized` | Tema Solarized Dark |

```rust
pub enum Theme {
    Dark,
    Light,
    Nord,
    Monokai,
    Solarized,
}

impl Theme {
    pub fn tokens(&self) -> DesignTokens {
        match self {
            Theme::Dark => dark_tokens(),
            Theme::Light => light_tokens(),
            // ...
        }
    }
}
```

### Componentes Base

| Componente | Descrição |
|------------|-----------|
| `primary_button` | Botão principal com cor de accent |
| `secondary_button` | Botão secundário/ghost |
| `icon_button` | Botão circular para ícones |
| `nav_button` | Botão de navegação com estado ativo |
| `panel_header` | Cabeçalho de painel colapsável |
| `menu_item` | Item de menu com hover |
| `card` | Container com background |
| `overlay` | Overlay modal para estados busy |
| `slider` | Slider customizado com precisão |
| `dropdown` | Dropdown com busca |

## Arquitetura de Camadas

### 1. State (`state.rs`)

Estado global da aplicação:

```rust
pub struct AppState {
    pub current_view: ViewMode,        // Library, Develop, Print, Import
    pub selected_photo_id: Option<PhotoId>,
    pub selected_photo_ids: HashSet<PhotoId>, // Multi-seleção
    pub zoom_level: f32,
    pub pan_offset: Vec2,
    pub current_theme: Theme,
    pub is_busy: bool,
    pub busy_message: String,
    
    // Filtros e ordenação
    pub filters: PhotoFilters,
    pub sort_by: SortField,
    pub sort_order: SortOrder,
    
    // Estado de edição
    pub current_edits: PhotoEdits,
    pub edit_history: EditHistory,
}

pub enum ViewMode {
    Library,
    Develop,
    Print,
    Import,
}
```

### 2. Views (`views/`)

Views completas que compõem panels e components:

- **LibraryView**: Grid de fotos + sidebars + filmstrip + filtros
- **DevelopView**: Editor de foto + painéis de ajustes + histograma
- **PrintView**: Configuração de impressão + preview de layout
- **ImportView**: Wizard de importação com preview

### 3. Components (`components/`)

Widgets reutilizáveis com lógica encapsulada:

- **RatingWidget**: Widget interativo de estrelas (0-5)
- **ColorLabels**: Seletor de 5 labels de cor
- **FlagWidget**: Toggle de flags (Flagged/Rejected/Unflagged)
- **SliderControl**: Slider com label e valor numérico
- **PhotoGrid**: Grid de thumbnails com seleção múltipla
- **ImageViewer**: Viewer com zoom/pan por gestos e teclado
- **Filmstrip**: Strip horizontal de thumbnails com scroll
- **ToneCurve**: Editor de curva de tons interativo
- **CropOverlay**: Overlay de corte com handles arrastáveis
- **Histogram**: Histograma RGB em tempo real

### 4. Panels (`panels/`)

Painéis compostos para sidebars:

- **NavigatorPanel**: Preview da foto selecionada com zoom miniatura
- **CatalogPanel**: Árvore de navegação do catálogo
- **CollectionsPanel**: Lista de coleções com drag-and-drop
- **QuickDevelopPanel**: Controles rápidos (exposure, WB, etc)
- **MetadataPanel**: Exibição de metadados EXIF/IPTC
- **PresetsPanel**: Lista de presets com preview hover
- **HistoryPanel**: Histórico de edições com Undo/Redo
- **BasicAdjustmentsPanel**: Controles completos de edição

## Fluxo de Dados

```
┌─────────────────────────────────────────────────────────────┐
│  app.rs (Loop Principal)                                    │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  AppState: view_mode, selected_photos, edits, zoom  │   │
│  └─────────────────────────────────────────────────────┘   │
│                           │                                 │
│                           ▼                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Views: LibraryView / DevelopView / etc             │   │
│  │  ┌─────────────────────────────────────────────┐   │   │
│  │  │  Panels: sidebar panels with state          │   │   │
│  │  │  ┌─────────────────────────────────────┐   │   │   │
│  │  │  │  Components: atomic UI elements     │   │   │   │
│  │  │  └─────────────────────────────────────┘   │   │   │
│  │  └─────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────┘   │
│                           │                                 │
│                           ▼                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Controllers (Adapters) → Use Cases → Domain        │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Atalhos de Teclado

### Globais

| Tecla | Ação |
|-------|------|
| `G` | Ir para Library |
| `D` | Ir para Develop |
| `Cmd/Ctrl + I` | Importar fotos |
| `Cmd/Ctrl + E` | Exportar foto(s) |
| `Cmd/Ctrl + Z` | Desfazer |
| `Cmd/Ctrl + Shift + Z` | Refazer |
| `Cmd/Ctrl + ,` | Preferências |
| `1-5` | Definir rating |
| `6-9` | Definir color label |
| `P` | Flag como Pick |
| `X` | Flag como Rejected |
| `U` | Remover flag |

### Library View

| Tecla | Ação |
|-------|------|
| `↑ ↓ ← →` | Navegar grid |
| `Enter` | Abrir em Develop |
| `Space` | Toggle seleção |
| `Cmd/Ctrl + A` | Selecionar todas |
| `Delete` | Remover foto(s) |
| `+` / `-` | Zoom grid |

### Develop View

| Tecla | Ação |
|-------|------|
| `→` | Próxima foto |
| `←` | Foto anterior |
| `Esc` | Voltar para Library |
| `F` | Fullscreen |
| `R` | Ferramenta de corte |
| `Space` | Antes/Depois |
| `0` | Fit to screen |
| `1` | Zoom 100% |
| `2` | Zoom 200% |

## Comunicação com Backend

### Via Controllers (Adapters Layer)

```rust
// Em DevelopView
fn on_slider_change(&mut self, ctx: &Context, new_exposure: f32) {
    // 1. Atualiza estado local para feedback instantâneo
    self.state.current_edits.exposure = new_exposure;
    
    // 2. Solicita reprocessamento GPU
    ctx.request_repaint();
    
    // 3. (Opcional) Salva automaticamente após debounce
    if self.auto_save_enabled {
        self.editor_controller.save_edits(&self.state.current_edits);
    }
}

// Em LibraryView
fn on_photo_click(&mut self, photo_id: PhotoId) {
    // 1. Atualiza seleção
    self.state.selected_photo_id = Some(photo_id);
    
    // 2. Solicita preview via controller
    self.library_controller.load_preview(photo_id);
}
```

### Callbacks para Ações

```rust
pub enum AppAction {
    ImportPhotos(Vec<PathBuf>),
    ExportPhoto(PhotoId, ExportSettings),
    RatePhoto(PhotoId, Rating),
    SetColorLabel(PhotoId, ColorLabel),
    SetFlag(PhotoId, Flag),
    SaveEdits(PhotoId, PhotoEdits),
    CreateCollection(String),
    AddToCollection(CollectionId, PhotoId),
    DeletePhoto(PhotoId),
    ApplyPreset(PhotoId, PresetId),
}
```

## Processamento GPU

### Pipeline de Ajustes

```rust
pub struct GpuProcessor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuProcessor {
    /// Aplica ajustes à imagem usando compute shaders
    pub fn process(&self, image: &Image, edits: &PhotoEdits) -> Image {
        // 1. Upload da imagem para GPU
        let input_texture = self.create_texture(image);
        
        // 2. Cria buffer de uniforms com parâmetros de edição
        let uniforms = self.create_uniforms(edits);
        
        // 3. Dispatch do compute shader (13 ajustes em single-pass)
        self.dispatch_compute(&input_texture, &uniforms);
        
        // 4. Download do resultado
        self.download_result()
    }
}
```

### Ajustes Suportados

| Categoria | Ajustes |
|-----------|---------|
| **Básicos** | Exposure, Contrast, Highlights, Shadows, Whites, Blacks |
| **Presença** | Clarity, Vibrance, Saturation |
| **White Balance** | Temperature, Tint |
| **Tone Curve** | RGB channels, Parametric |
| **HSL** | 8 cores × Hue/Saturation/Luminance |
| **Detalhe** | Sharpening, Noise Reduction |
| **Efeitos** | Vignette, Grain |
| **Correções** | Lens Distortion, Chromatic Aberration |
| **Transformação** | Crop, Rotation, Flip |

## Boas Práticas

1. **Use tokens**: Nunca hardcode cores ou espaçamentos
2. **Componentes pequenos**: Cada arquivo ~100-200 linhas
3. **Props explícitas**: Funções recebem apenas o necessário
4. **Estado mínimo**: Só armazene o que precisa para renderizar
5. **Feedback instantâneo**: GPU para ajustes em tempo real
6. **Atalhos de teclado**: Todas as ações têm atalhos
7. **Async loading**: Imagens carregam em background
8. **Prefetching**: Próximas fotos pré-carregadas

## Build

```bash
# Desenvolvimento com hot-reload
cargo run -p ui

# Release otimizado
cargo build -p ui --release

# Testes de UI (se houver)
cargo test -p ui
```
