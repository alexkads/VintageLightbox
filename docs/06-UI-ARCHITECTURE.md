# Arquitetura da Interface - VintageLightbox

Este documento descreve a arquitetura da interface de usuário do VintageLightbox, construída com Slint UI seguindo princípios de Clean Architecture e Design System.

## Visão Geral

A UI foi refatorada de um arquivo monolítico para uma estrutura modular com 20+ arquivos, organizados em camadas com responsabilidades bem definidas.

## Estrutura de Arquivos

```
crates/ui/ui/
├── main.slint                      # Root component (composição)
├── types.slint                     # Structs compartilhados
├── design_system/
│   ├── tokens.slint                # Design tokens
│   └── primitives.slint            # Componentes base
├── components/
│   ├── toolbar.slint               # Barra de navegação
│   ├── photo_grid.slint            # Grid de fotos
│   ├── filmstrip.slint             # Thumbnails
│   ├── histogram.slint             # Histograma
│   ├── rating_widget.slint         # Estrelas de rating
│   ├── color_labels.slint          # Labels de cor
│   ├── slider_control.slint        # Slider com label
│   └── image_viewer.slint          # Viewer com zoom/pan
├── panels/
│   ├── navigator_panel.slint       # Painel Navigator
│   ├── catalog_panel.slint         # Painel Catalog
│   ├── collections_panel.slint     # Painel Collections
│   ├── quick_develop_panel.slint   # Painel Quick Develop
│   ├── metadata_panel.slint        # Painel Metadata
│   ├── presets_panel.slint         # Painel Presets
│   ├── history_panel.slint         # Painel History
│   └── basic_adjustments_panel.slint # Painel de ajustes
└── views/
    ├── library_view.slint          # View da biblioteca
    └── develop_view.slint          # View de edição
```

## Design System

### Design Tokens (`design_system/tokens.slint`)

Tokens centralizados garantem consistência visual em toda a aplicação:

```slint
export global Theme {
    // Cores de Background
    out property <color> bg-app: #1a1a1a;
    out property <color> bg-surface: #252525;
    out property <color> bg-elevated: #2d2d2d;
    out property <color> bg-hover: #353535;

    // Cores de Texto
    out property <color> text-primary: #e0e0e0;
    out property <color> text-secondary: #b0b0b0;
    out property <color> text-muted: #808080;

    // Cores de Accent
    out property <color> accent-primary: #4a9eff;

    // Espaçamentos
    out property <length> space-sm: 8px;
    out property <length> space-md: 12px;
    out property <length> space-lg: 16px;

    // Tipografia
    out property <length> font-sm: 11px;
    out property <length> font-md: 12px;
    out property <length> font-lg: 14px;
}
```

### Componentes Primitivos (`design_system/primitives.slint`)

Componentes base reutilizáveis:

| Componente | Descrição |
|------------|-----------|
| `PrimaryButton` | Botão principal com cor de accent |
| `SecondaryButton` | Botão secundário/ghost |
| `IconButton` | Botão circular para ícones |
| `NavButton` | Botão de navegação com estado ativo |
| `PanelHeader` | Cabeçalho de painel colapsável |
| `MenuItem` | Item de menu com hover |
| `Card` | Container com background |
| `Overlay` | Overlay modal para estados busy |

## Arquitetura de Camadas

### 1. Types (`types.slint`)

Structs de dados compartilhados:

```slint
export struct TileData {
    id: string,
    name: string,
    image: image,
    rating: int,
    color_label: string,
}

export struct RowData {
    tiles: [TileData],
}
```

### 2. Components (`components/`)

Widgets reutilizáveis com lógica encapsulada:

- **RatingWidget**: Widget interativo de estrelas
- **ColorLabels**: Seletor de labels de cor
- **SliderControl**: Slider com label e valor
- **PhotoGrid**: Grid de thumbnails com seleção
- **ImageViewer**: Viewer com zoom/pan por gestos
- **Filmstrip**: Strip horizontal de thumbnails

### 3. Panels (`panels/`)

Painéis compostos para sidebars:

- **NavigatorPanel**: Preview da foto selecionada
- **CatalogPanel**: Navegação do catálogo
- **CollectionsPanel**: Lista de coleções
- **QuickDevelopPanel**: Controles rápidos de edição
- **MetadataPanel**: Exibição de metadados
- **BasicAdjustmentsPanel**: Controles de edição completos

### 4. Views (`views/`)

Views completas que compõem panels e components:

- **LibraryView**: Grid de fotos + sidebars + filmstrip
- **DevelopView**: Editor de foto + panels de ajustes

### 5. Main (`main.slint`)

Root component que:
- Gerencia estado da aplicação
- Compõe views baseado em `current_view`
- Define callbacks para bridge com Rust
- Gerencia overlay de busy state

## Fluxo de Dados

```
┌─────────────────────────────────────────────────────────────┐
│  main.slint                                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  State: grid_model, detail_*, active_*, is_busy     │   │
│  └─────────────────────────────────────────────────────┘   │
│                           │                                 │
│                           ▼                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Views: LibraryView / DevelopView                   │   │
│  │  ┌─────────────────────────────────────────────┐   │   │
│  │  │  Panels: composed from components           │   │   │
│  │  │  ┌─────────────────────────────────────┐   │   │   │
│  │  │  │  Components: atomic UI elements     │   │   │   │
│  │  │  └─────────────────────────────────────┘   │   │   │
│  │  └─────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────┘   │
│                           │                                 │
│                           ▼                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Callbacks → Rust Backend                           │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Callbacks (Bridge para Rust)

```slint
callback import_clicked();
callback tile_clicked(string);
callback back_clicked();
callback rate_photo(string, int);
callback navigate(int);
callback apply_edits(float, float);
callback save_edits(string, float, float);
callback export_clicked(string);
```

## Atalhos de Teclado

| Tecla | Ação | View |
|-------|------|------|
| `→` | Próxima foto | Develop |
| `←` | Foto anterior | Develop |
| `Esc` | Voltar para Library | Develop |

## Adicionando Novos Componentes

### 1. Criar componente

```slint
// components/my_component.slint
import { Theme } from "../design_system/tokens.slint";

export component MyComponent inherits Rectangle {
    in property <string> value;
    callback value-changed(string);

    background: Theme.bg-surface;
    // ...
}
```

### 2. Usar em panel ou view

```slint
import { MyComponent } from "../components/my_component.slint";

export component MyPanel {
    MyComponent {
        value: "test";
        value-changed(v) => { /* handle */ }
    }
}
```

### 3. Propagar callback se necessário

Se o callback precisa chegar ao Rust, propague até `main.slint`:

```
Component → Panel → View → main.slint → Rust callback
```

## Boas Práticas

1. **Use tokens**: Nunca hardcode cores ou espaçamentos
2. **Componentes pequenos**: Cada arquivo ~50-100 linhas
3. **Props explícitas**: Use `in property` e `callback` para interface clara
4. **Nomes consistentes**: Use kebab-case para propriedades Slint
5. **Documentação**: Comente a intenção do componente no topo do arquivo

## Build

O sistema de build não requer mudanças - Slint resolve imports automaticamente:

```rust
// build.rs
slint_build::compile("ui/main.slint").unwrap();
```

```rust
// main.rs
slint::include_modules!();
```
