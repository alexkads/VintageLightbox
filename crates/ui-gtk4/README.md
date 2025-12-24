# VintageLightbox GTK4/Relm4 UI

> ⚠️ **STATUS: EXPERIMENTAL - EM DESENVOLVIMENTO ATIVO**
>
> Esta é uma implementação **experimental** da interface de usuário usando GTK4/Relm4.
> O objetivo é migrar da UI egui atual para uma interface nativa GTK4, mas o processo
> está em andamento e **NÃO está pronto para uso em produção**.

## 🚧 Status da Migração

**Progresso Atual: ~40% (Fases 1, 2 e 3 parcial)**

### ✅ Completado

- **Fase 1: Sistema de Docking** (2 arquivos, 364 linhas)
  - `docking/dock_manager.rs` - Sistema customizado de docking com GtkPaned + GtkNotebook
  - Layout configurável para Library e Develop views

- **Fase 2: Async Image Loading** (4 arquivos, 141 linhas)
  - `workers/thumbnail_worker.rs` - Worker background com Rayon para thumbnails
  - `utils/image_conversion.rs` - Conversão thread-safe DynamicImage ↔ Pixbuf ↔ Texture

- **Fase 3: Integração (Parcial)**
  - ✅ **3.1 PhotoGrid Integration**: Convertido para Component, ThumbnailWorker integrado, Cache LRU
  - ✅ **3.2 LibraryView Refactoring**: DockManager integrado, FolderTree, FilterPanel, MetadataPanel

### ⏳ Em **Progresso:** 65%
**Status:** Fase 3 (Integração e Refatoração) - Em andamentorar DevelopView com DockManager
  - Implementar ImageViewer com Cairo
  - Criar painéis de ajuste e presets

### 📋 Pendente

- Fase 4: Componentes Faltantes (SliderControl, Histogram, Filmstrip)
- Fase 5: Auto-Save com Debouncing
- Fase 6: Dialogs (Import, Export, Settings)
- Fase 7: Polish (CSS, Performance, Error Handling)

Veja o [ROADMAP completo](../../docs/04-ROADMAP.md#fase-22-migração-para-gtk4relm4) para detalhes.

## 🎯 Por Que GTK4/Relm4?

A migração da UI egui para GTK4 visa:

1. **Interface Nativa**: Melhor integração com o sistema operacional
2. **Acessibilidade**: Suporte nativo a leitores de tela e tecnologias assistivas
3. **Temas do Sistema**: Integração automática com temas GTK do usuário
4. **Widgets Nativos**: File dialogs, menus e componentes seguem as guidelines da plataforma
5. **Maturidade**: GTK4 é um toolkit maduro e bem testado

## 📚 Arquitetura

A UI GTK4 segue o **Elm Architecture** através do Relm4:

```
App (root component)
├── Model (estado da aplicação)
├── Messages (Input/Output/CommandOutput)
├── Components (widgets reutilizáveis)
│   ├── PhotoGrid
│   ├── ImageViewer
│   ├── SliderControl
│   └── ...
├── Views (telas principais)
│   ├── LibraryView (com DockManager)
│   └── DevelopView (com DockManager)
└── Workers (processamento assíncrono)
    ├── ThumbnailWorker (Rayon paralelo)
    └── PreviewWorker (planejado)
```

### Padrões Implementados

- **Component Pattern**: Relm4 Components com `impl Component`
- **Message Passing**: Unidirecional (Input → Update → Output)
- **Async Workers**: `impl Worker` para operações custosas (thumbnails, I/O)
- **Thread Safety**: Transferência de dados via `Vec<u8>` (PNG bytes) para evitar tipos não-Send
- **Custom Docking**: GtkPaned + GtkNotebook para replicar `egui_dock`

## 🏗️ Estrutura de Arquivos

```
ui-gtk4/
├── src/
│   ├── main.rs              # Entry point, setup de runtime
│   ├── app.rs               # Componente raiz, routing, keyboard shortcuts
│   ├── model.rs             # AppModel (estado global)
│   ├── messages.rs          # AppMsg e CommandOutput
│   │
│   ├── components/          # Componentes reutilizáveis
│   │   ├── photo_grid.rs    # ✅ Grid de thumbnails (Integrado com Worker)
│   │   ├── image_viewer.rs  # Visualizador com zoom/pan
│   │   ├── slider_control.rs
│   │   ├── rating_widget.rs
│   │   └── ...
│   │
│   ├── views/               # Views principais (telas)
│   │   ├── library_view.rs  # ✅ View de biblioteca (DockManager integrado)
│   │   └── develop_view.rs  # View de edição (⏳ refatoração)
│   │
│   ├── docking/             # ✅ Sistema de docking customizado
│   │   └── dock_manager.rs
│   │
│   ├── workers/             # ✅ Workers assíncronos
│   │   └── thumbnail_worker.rs
│   │
│   ├── utils/               # ✅ Utilidades
│   │   └── image_conversion.rs
│   │
│   └── dialogs/             # 📋 Dialogs (planejado)
│       ├── import_dialog.rs
│       ├── export_dialog.rs
│       └── settings_dialog.rs
```

## 🚀 Como Rodar (Experimental)

```bash
# Build
cargo build -p ui-gtk4

# Run (ainda não funcional completamente)
cargo run -p ui-gtk4

# Ou com o binário nomeado
cargo run --bin vintage-lightbox-gtk
```

**⚠️ AVISO**: A aplicação **não está funcional** ainda. Muitos componentes são placeholders.

## 🔄 Comparação: egui vs GTK4

| Aspecto              | UI egui (atual)       | UI GTK4 (experimental) |
|----------------------|-----------------------|------------------------|
| **Status**           | ✅ Funcional          | ⚠️ Em desenvolvimento  |
| **Framework**        | egui 0.28 + eframe    | relm4 0.9 + GTK4 0.9   |
| **Rendering**        | Immediate mode (GPU)  | Retained mode (GTK)    |
| **Docking**          | egui_dock             | Custom (Paned+Notebook)|
| **Async**            | Channels + polling    | Relm4 Workers          |
| **Temas**            | Custom CSS            | GTK themes nativos     |
| **Acessibilidade**   | Limitada              | Nativa (AT-SPI)        |
| **File Dialogs**     | rfd (custom)          | GtkFileChooserDialog   |
| **Integração OS**    | Boa                   | Excelente              |

## 📝 Decisões Técnicas

### 1. Thread Safety com Pixbuf

**Problema**: `gdk_pixbuf::Pixbuf` não implementa `Send`, mas `Worker::Output` requer `Send`.

**Solução**: Transferir imagens como PNG bytes entre threads:
```rust
// Worker thread (background)
dynamic_image.write_to(&mut bytes, ImageFormat::Png); // Vec<u8>
sender.output(ThumbnailResult { bytes });

// UI thread (main)
let pixbuf = bytes_to_pixbuf(&bytes, 300, 300);
let texture = gdk4::Texture::for_pixbuf(&pixbuf);
```

### 2. Custom Docking System

**Problema**: GTK4 não tem docking nativo como `egui_dock`.

**Solução**: Implementar `DockManager` com:
- `GtkPaned`: Painéis redimensionáveis
- `GtkNotebook`: Tabs para múltiplos widgets no mesmo painel
- Layout configurável por view (Library vs Develop)

### 3. Debounced Sliders (planejado)

**Solução**: Usar `glib::timeout_add_local_once` para auto-save 500ms após última mudança.

## 🧪 Testes

**Status**: Ainda não implementados.

**Planejado**:
- Unit tests para componentes isolados
- Integration tests para views
- UI tests (GTK Test framework ou snapshot tests)

## 🤝 Contribuindo

⚠️ **ATENÇÃO**: Esta UI está em desenvolvimento ativo. Contribuições são bem-vindas, mas:

1. **Leia o [Plano de Implementação](../../docs/04-ROADMAP.md#fase-22-migração-para-gtk4relm4)** antes de começar
2. **Verifique o status** de cada fase no ROADMAP
3. **Coordene** para evitar trabalho duplicado
4. **Siga os padrões** Relm4 estabelecidos nos componentes existentes

## 📖 Recursos

- [Relm4 Book](https://relm4.org/book/stable/)
- [GTK4 Documentation](https://docs.gtk.org/gtk4/)
- [Rust GTK4 Bindings](https://gtk-rs.org/)
- [Elm Architecture](https://guide.elm-lang.org/architecture/)

## ⚖️ Licença

Mesma licença do projeto principal VintageLightbox (a ser definida).

---

**Última Atualização**: Dezembro 2024
**Versão**: 0.1.0-experimental
**Progresso**: 13% (Fases 1-2 completas de 7 planejadas)
