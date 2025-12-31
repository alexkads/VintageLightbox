# Arquitetura do Sistema - VintageLightbox

**Última atualização**: 31 de dezembro de 2025  
**Status**: MVP Completo com 360+ testes ✅

## 1. Visão Geral da Arquitetura

VintageLightbox segue os princípios da **Clean Architecture** (Arquitetura Limpa) proposta por Robert C. Martin, combinada com práticas de **Test-Driven Development (TDD)**. Esta abordagem garante:

- ✅ **Independência de Frameworks**: A lógica de negócio não depende de egui ou outras bibliotecas externas
- ✅ **Testabilidade**: Todas as camadas são facilmente testáveis de forma isolada (360+ testes)
- ✅ **Independência de UI**: egui pode ser substituído sem afetar regras de negócio
- ✅ **Independência de Banco de Dados**: SQLite com migrations versionadas
- ✅ **GPU Acceleration**: wgpu compute shaders para edição em tempo real
- ✅ **Princípios SOLID**: Código modular, extensível e de fácil manutenção

### Camadas da Clean Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│              UI (Camada 5 - Presentation)                          │
│   egui 0.31 • 4 Views • 25+ Components • 5 Themes                  │
│   Immediate Mode GUI • GPU-accelerated rendering                   │
├────────────────────────────────────────────────────────────────────┤
│              Infrastructure (Camada 4 - Frameworks & Drivers)      │
│   SQLite + rusqlite • wgpu compute shaders • LibRaw                │
│   Multi-level Cache • 16 migrations • 48 testes                    │
├────────────────────────────────────────────────────────────────────┤
│              Adapters (Camada 3 - Interface Adapters)              │
│   6 Controllers • Presenters • ViewModels • State Management       │
│   EditHistory • PhotoFilters • 27 testes                           │
├────────────────────────────────────────────────────────────────────┤
│              Use Cases (Camada 2 - Application Business Rules)     │
│   20+ Use Cases: Import, Edit, Organize, Export, Print, Presets    │
│   65 testes                                                        │
├────────────────────────────────────────────────────────────────────┤
│              Domain (Camada 1 - Enterprise Business Rules)         │
│   4 Entities • 15 Value Objects • Repository Traits • Domain Errors│
│   220 testes • 100% cobertura                                      │
└────────────────────────────────────────────────────────────────────┘

Regra de Dependência: As dependências apontam sempre para DENTRO
(Camadas externas dependem de camadas internas, nunca o contrário)
```

### Metodologia de Desenvolvimento: TDD (Test-Driven Development)

Todos os componentes foram desenvolvidos seguindo o ciclo **Red-Green-Refactor**:

1. **🔴 Red**: Escrever um teste que falha
2. **🟢 Green**: Implementar código mínimo para passar o teste
3. **🔵 Refactor**: Refatorar mantendo os testes verdes

**Resultado**: 360+ testes passando em todas as camadas

## 2. Estrutura de Módulos (Clean Architecture)

A estrutura de diretórios reflete as camadas da Clean Architecture, com a UI separada em seu próprio crate para garantir o desacoplamento.

```
crates/
├── domain/              # Camada 1: Entities (220 testes)
├── use-cases/           # Camada 2: Application Business Rules (65 testes)
├── adapters/            # Camada 3: Interface Adapters (27 testes)
├── infrastructure/      # Camada 4: Frameworks & Drivers (48 testes)
└── ui/                  # Camada 5: User Interface (egui 0.31)
```

### 2.1 Domain Layer (Camada 1 - Entities) ✅ COMPLETO

**Responsabilidade**: Regras de negócio empresariais puras, independentes de qualquer framework ou tecnologia.

**Status**: 220 testes, ~100% cobertura

#### `crates/domain/src/`

```rust
domain/
├── entities/
│   ├── photo.rs          // Entidade Photo (rating, flags, edits)
│   ├── collection.rs     // Entidade Collection
│   ├── preset.rs         // Entidade Preset
│   └── print_job.rs      // Entidade PrintJob
├── value_objects/
│   ├── rating.rs         // 0-5 estrelas com validação
│   ├── photo_id.rs       // UUID único
│   ├── color_label.rs    // Red, Yellow, Green, Blue, Purple
│   ├── flag.rs           // Flagged, Rejected, Unflagged
│   ├── file_path.rs      // Caminho validado
│   ├── photo_edits.rs    // Ajustes de edição (13 parâmetros)
│   ├── crop_settings.rs  // Configurações de corte
│   ├── aspect_ratio.rs   // Proporções predefinidas
│   ├── rotation_fill_mode.rs // Modos de preenchimento
│   ├── photo_metadata.rs // Metadados EXIF
│   ├── import_options.rs // Opções de importação
│   ├── print_settings.rs // Configurações de impressão
│   ├── print_layout.rs   // Layouts de impressão
│   └── ...
├── repositories.rs       // Repository Traits (interfaces)
├── ports/                // Ports para inversão de dependência
├── services/             // Domain Services
└── errors.rs             // DomainError enum
```

### 2.2 Use Cases Layer (Camada 2 - Application Business Rules) ✅ COMPLETO

**Responsabilidade**: Orquestração de fluxos de trabalho e regras de negócio específicas da aplicação.

**Status**: 65 testes, 20+ use cases implementados

#### `crates/use-cases/src/`

```rust
use-cases/
├── import/
│   ├── import_photo.rs
│   ├── import_photos.rs (batch)
│   ├── preview_before_import.rs
│   ├── check_duplicates.rs
│   ├── import_with_options.rs
│   └── get_import_sources.rs
├── edit/
│   ├── save_photo_edits.rs
│   └── apply_preset.rs
├── organize/
│   ├── rate_photo.rs
│   ├── set_color_label.rs
│   ├── set_flag.rs
│   └── delete_photo.rs
├── collections/
│   ├── create_collection.rs
│   ├── add_photo_to_collection.rs
│   └── remove_photo_from_collection.rs
├── export/
│   └── export_photo.rs
├── print/
│   └── configure_print_job.rs
└── presets/
    ├── create_preset.rs
    ├── list_presets.rs
    └── delete_preset.rs
```

### 2.3 Adapters Layer (Camada 3 - Interface Adapters) ✅ COMPLETO

**Responsabilidade**: Adaptar dados entre Use Cases e UI/Infrastructure. Controllers recebem input da UI e chamam Use Cases. Presenters formatam output para a UI.

**Status**: 27 testes

#### `crates/adapters/src/`

```rust
adapters/
├── controllers/
│   ├── library_controller.rs   // Gerencia biblioteca
│   ├── editor_controller.rs    // Gerencia edição
│   ├── photo_controller.rs     // CRUD de fotos
│   ├── import_controller.rs    // Fluxo de importação
│   ├── export_controller.rs    // Fluxo de exportação
│   └── preset_controller.rs    // Gerencia presets
├── presenters.rs              // Formatação de dados para UI
├── view_models.rs             // ViewModels para egui
├── services/
│   ├── editor_service.rs      // Serviço de edição
│   └── navigation_service.rs  // Navegação entre fotos
└── state/
    ├── application_state.rs   // Estado global
    ├── edit_history.rs        // Histórico Undo/Redo
    └── photo_filters.rs       // Filtros de biblioteca
```

### 2.4 Infrastructure Layer (Camada 4 - Frameworks & Drivers) ✅ COMPLETO

**Responsabilidade**: Detalhes de implementação técnica, banco de dados, acesso a sistema de arquivos, processamento de imagem, cache.

**Status**: 48 testes, 16 migrations SQLite

#### `crates/infrastructure/src/`

```rust
infrastructure/
├── database/
│   ├── mod.rs                    // Pool de conexões
│   ├── photo_repository_impl.rs  // SQLite repository
│   ├── collection_repository_impl.rs
│   └── preset_repository_impl.rs
├── cache/
│   ├── processed_cache.rs        // L0: Cache processado (0.01ms)
│   ├── image_cache.rs            // L1: Cache de imagens (15 imgs)
│   └── preview_cache.rs          // Preview SQLite BLOB
├── image_processing/
│   ├── gpu_processor.rs          // wgpu compute shaders
│   ├── adjustments.rs            // 13 ajustes GPU
│   └── pipeline.rs               // Pipeline single-pass
├── thumbnail_generator.rs        // Geração de thumbnails
├── raw_processing.rs             // LibRaw/rawler
├── exif_reader.rs                // Leitura de metadados
├── file_organizer.rs             // Organização de arquivos
├── file_system.rs                // Operações de disco
├── scan_directory.rs             // Scanner recursivo
├── image_exporter.rs             // Exportação JPG/PNG/TIFF
├── content_hash.rs               // Hash para duplicatas
└── paths.rs                      // Gerenciamento de paths
```

### 2.5 UI Layer (Camada 5 - Presentation) ✅ COMPLETO

**Responsabilidade**: Renderização da interface gráfica usando **egui 0.31**. Esta camada é "burra" - não contém regras de negócio, apenas lógica de visualização e captura de eventos. Após a extração das regras de negócio, a UI apenas emite eventos/AppAction e os controllers (Adapters) chamam os use cases para validação e persistência.

#### `crates/ui/src/`

```rust
ui/
├── main.rs                    // Entry point
├── app.rs                     // Loop principal, estado global
├── state.rs                   // ViewState, seleção, zoom
├── views/
│   ├── library_view.rs        // Grid de fotos + sidebars
│   ├── develop_view.rs        // Editor de foto
│   ├── print_view.rs          // Módulo de impressão
│   └── import_view.rs         // Wizard de importação
├── components/
│   ├── toolbar.rs             // Barra de navegação
│   ├── photo_grid.rs          // Grid com seleção
│   ├── filmstrip.rs           // Thumbnails horizontais
│   ├── histogram.rs           // Histograma RGB
│   ├── rating_widget.rs       // Estrelas interativas
│   ├── color_labels.rs        // Seletor de cores
│   ├── flag_widget.rs         // Flags (pick/reject)
│   ├── slider_control.rs      // Sliders de ajuste
│   ├── image_viewer.rs        // Viewer com zoom/pan
│   ├── tone_curve.rs          // Curva de tons
│   ├── crop_overlay.rs        // Overlay de corte
│   └── ... (25+ componentes)
├── panels/
│   ├── navigator_panel.rs     // Preview da foto
│   ├── catalog_panel.rs       // Navegação do catálogo
│   ├── collections_panel.rs   // Lista de coleções
│   ├── quick_develop_panel.rs // Controles rápidos
│   ├── metadata_panel.rs      // Metadados EXIF
│   ├── presets_panel.rs       // Presets salvos
│   ├── history_panel.rs       // Histórico de edição
│   └── basic_adjustments_panel.rs
├── design_system/
│   ├── tokens.rs              // Design tokens (cores, spacing)
│   ├── themes.rs              // 5 temas (Light, Dark, Nord, etc)
│   └── icons.rs               // Phosphor Icons
├── async_loader.rs            // Carregamento assíncrono
└── gpu_processor.rs           // Interface com wgpu
```

## 3. Fluxo de Dados

### 3.1 Edição e Persistência

```
1. [UI] Usuário move slider de exposição
2. [UI] Evento atualiza PhotoEdits temporário em memória
3. [Infrastructure] GPU (wgpu) aplica ajustes em tempo real (~0.01ms)
4. [UI] Preview atualizado instantaneamente
5. [UI] Usuário clica em "Salvar" ou troca de foto
6. [Adapters] EditorController.save_edits(edits)
7. [Use Cases] SavePhotoEditsUseCase valida e orquestra
8. [Domain] Entidade Photo é atualizada com novos edits
9. [Infrastructure] SqlitePhotoRepository persiste no DB
```

### 3.2 Sistema de Cache Multi-nível

Para performance (60fps), o sistema usa cache em 3 níveis:

```
┌─────────────────────────────────────────────────────────────┐
│  L0: ProcessedCache (HashMap)                               │
│  • Lookup: 0.01ms                                           │
│  • Imagem processada com ajustes atuais                     │
│  • Invalidado quando PhotoEdits muda                        │
├─────────────────────────────────────────────────────────────┤
│  L1: ImageCache (LRU 15 imagens)                            │
│  • Lookup: 1-5ms                                            │
│  • Imagem base decodificada (sem ajustes)                   │
│  • Prefetch de próximas 5 fotos                             │
├─────────────────────────────────────────────────────────────┤
│  L2: Preview SQLite BLOB                                    │
│  • Lookup: 10-50ms                                          │
│  • Preview JPEG em resolução média                          │
│  • Persistido no banco de dados                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.3 Pipeline GPU (wgpu Compute Shaders)

13 ajustes aplicados em single-pass no shader:

```wgsl
// Ordem de processamento no shader
1. White Balance (temperatura, matiz)
2. Exposure (exposição, contraste)
3. Highlights/Shadows (luzes, sombras)
4. Whites/Blacks (brancos, pretos)
5. Clarity (clareza)
6. Vibrance/Saturation (vibração, saturação)
7. HSL Adjustments (8 cores × 3 canais)
8. Tone Curve (RGB individual)
9. Sharpening (nitidez, raio, mascaramento)
10. Noise Reduction (luminância, cor, detalhe)
11. Lens Corrections (vignette, distorção)
12. Crop/Rotation (com fill modes)
13. Output Transform (gamma, color space)
```

**Performance**: ~2ms para imagem 4K no GPU integrado

## 4. Tecnologias Chave

| Categoria | Tecnologia | Versão |
|-----------|------------|--------|
| **Linguagem** | Rust | 1.75+ |
| **UI Framework** | egui | 0.31 |
| **GPU Compute** | wgpu | 24.0 |
| **Database** | SQLite + rusqlite | 0.32 |
| **RAW Processing** | LibRaw / rawler | - |
| **Image Codecs** | image-rs | 0.25 |
| **Async Runtime** | tokio | 1.x |
| **Error Handling** | thiserror | 2.x |
| **Mocking** | mockall | 0.13 |
| **Property Testing** | proptest | 1.x |

## 5. Padrões Adotados

### Arquiteturais
- **Clean Architecture**: 5 camadas com dependências de fora para dentro
- **Repository Pattern**: Abstração de persistência via traits
- **Dependency Injection**: Use Cases recebem traits, não implementações concretas
- **CQRS-lite**: Separação de leitura (queries) e escrita (commands) nos use cases

### Técnicos
- **Async/Await**: I/O non-blocking em todo o sistema
- **Multi-level Cache**: L0 (processado) → L1 (decodificado) → L2 (preview)
- **GPU Compute**: wgpu shaders para processamento em tempo real
- **Content Addressing**: Hash SHA-256 para detecção de duplicatas

### Testing
- **TDD**: Red-Green-Refactor em todas as camadas
- **Unit Tests**: Isolados com mocks (mockall)
- **Property-based Tests**: Fuzzing com proptest
- **Integration Tests**: Banco de dados em memória

## 6. Regras Críticas

### UI Layer
1. **Zero Lógica de Negócio**: Validações no Domain/Use Cases
2. **Sem I/O Direto**: Nunca usar `std::fs` - sempre via Infrastructure
3. **Estado Mínimo**: Apenas o necessário para visualização
4. **GPU para Ajustes**: Todos os ajustes visuais via wgpu

### Domain Layer
1. **Puro**: Zero dependências externas (exceto std)
2. **Imutabilidade**: Value Objects imutáveis
3. **Validação**: Toda entrada validada na construção
4. **Testável**: 100% de cobertura possível sem mocks complexos

### Infrastructure Layer
1. **Implementações Concretas**: Implementa traits do Domain
2. **Migrations Versionadas**: SQLite schema via migrations
3. **Cache Inteligente**: Invalidação correta baseada em mudanças
4. **Error Handling**: Erros específicos convertidos para DomainError
