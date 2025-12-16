# Arquitetura do Sistema - VintageLightbox

## 1. Visão Geral da Arquitetura

VintageLightbox segue uma arquitetura em camadas modular, aproveitando os pontos fortes de Rust (segurança de memória, performance) e Slint (interface declarativa e reativa).

```
┌─────────────────────────────────────────────────────────┐
│                   Interface Gráfica                     │
│                    (Slint UI)                           │
├─────────────────────────────────────────────────────────┤
│              Camada de Apresentação                     │
│         (View Models, State Management)                 │
├─────────────────────────────────────────────────────────┤
│               Camada de Aplicação                       │
│    (Controllers, Use Cases, Business Logic)             │
├─────────────────────────────────────────────────────────┤
│               Camada de Domínio                         │
│       (Entities, Value Objects, Services)               │
├─────────────────────────────────────────────────────────┤
│            Camada de Infraestrutura                     │
│  (Repositories, External Services, File System)         │
└─────────────────────────────────────────────────────────┘
```

## 2. Estrutura de Módulos

### 2.1 Core Modules

#### `core/`
Módulo central com tipos e funcionalidades fundamentais.

```rust
core/
├── domain/
│   ├── photo.rs          // Entity: Photo com metadados
│   ├── catalog.rs        // Entity: Catálogo de fotos
│   ├── collection.rs     // Entity: Coleções/Álbuns
│   ├── rating.rs         // Value Object: Sistema de classificação
│   ├── color_label.rs    // Value Object: Marcações por cor
│   └── edit_history.rs   // Value Object: Histórico de edições
├── errors.rs             // Definições de erros do sistema
├── config.rs             // Configurações da aplicação
└── events.rs             // Sistema de eventos da aplicação
```

### 2.2 Import Module

Responsável pela importação e indexação de fotos.

```rust
import/
├── scanner.rs            // Escaneia diretórios
├── importer.rs           // Coordena processo de importação
├── metadata_reader.rs    // Extrai metadados EXIF/XMP
├── thumbnail_generator.rs // Gera thumbnails
├── duplicate_detector.rs  // Detecta duplicatas
└── progress.rs           // Tracking de progresso
```

### 2.3 RAW Processing Module

Processamento e edição de arquivos RAW/DNG.

```rust
raw/
├── decoder.rs            // Decodifica RAW usando LibRaw/rawler
├── processor.rs          // Aplica ajustes não-destrutivos
├── adjustments/
│   ├── basic.rs         // Exposição, contraste, temperatura
│   ├── tone_curve.rs    // Curvas de tom
│   ├── hsl.rs           // Ajustes HSL
│   ├── lens.rs          // Correção de lente
│   ├── noise.rs         // Redução de ruído
│   └── sharpening.rs    // Nitidez
├── preset.rs            // Gerenciamento de presets
└── color_management.rs  // Perfis de cor e conversão
```

### 2.4 Database Module

Persistência de dados usando SQLite.

```rust
database/
├── connection.rs         // Pool de conexões SQLite
├── schema.rs            // Definição de schema
├── migrations.rs        // Migrações de banco
├── repositories/
│   ├── photo_repo.rs    // Repository de fotos
│   ├── collection_repo.rs // Repository de coleções
│   ├── preset_repo.rs   // Repository de presets
│   └── purchase_repo.rs // Repository de vendas
└── query_builder.rs     // Builder para queries complexas
```

### 2.5 UI Module

Interface gráfica com Slint.

```rust
ui/
├── main_window.slint     // Janela principal
├── components/
│   ├── photo_grid.slint  // Grade de thumbnails
│   ├── photo_viewer.slint // Visualizador ampliado
│   ├── edit_panel.slint  // Painel de edição
│   ├── sidebar.slint     // Navegação lateral
│   ├── toolbar.slint     // Barra de ferramentas
│   └── dialogs/
│       ├── import.slint  // Dialog de importação
│       ├── export.slint  // Dialog de exportação
│       └── print.slint   // Dialog de impressão
├── viewmodels/
│   ├── library_vm.rs     // ViewModel do modo biblioteca
│   ├── develop_vm.rs     // ViewModel do modo edição
│   ├── compare_vm.rs     // ViewModel do modo comparação
│   └── slideshow_vm.rs   // ViewModel da apresentação
└── state.rs              // State management global
```

### 2.6 Export Module

Exportação de fotos processadas.

```rust
export/
├── exporter.rs           // Coordena exportação
├── formats/
│   ├── jpeg.rs          // Exportação JPEG
│   └── png.rs           // Exportação PNG
├── resize.rs            // Redimensionamento
├── watermark.rs         // Aplicação de marca d'água
└── batch.rs             // Exportação em lote
```

### 2.7 Print Module

Sistema de impressão.

```rust
print/
├── printer.rs           // Interface com sistema de impressão
├── layout.rs            // Layouts de impressão
├── preview.rs           // Preview de impressão
└── color_profile.rs     // Gerenciamento de cor para impressão
```

### 2.8 Multi-Monitor Module

Suporte para múltiplos monitores.

```rust
multimonitor/
├── display_manager.rs   // Detecta e gerencia displays
├── secondary_window.rs  // Janela do segundo monitor
└── sync.rs              // Sincronização entre janelas
```

### 2.9 Cache Module

Sistema de cache para performance.

```rust
cache/
├── thumbnail_cache.rs   // Cache de thumbnails
├── preview_cache.rs     // Cache de previews
├── lru.rs               // Implementação LRU
└── disk_cache.rs        // Cache em disco
```

### 2.10 Utils Module

Utilitários gerais.

```rust
utils/
├── file_system.rs       // Operações de arquivo
├── image_utils.rs       // Utilitários de imagem
├── keyboard.rs          // Atalhos de teclado
└── async_tasks.rs       // Tarefas assíncronas
```

## 3. Fluxo de Dados

### 3.1 Importação de Fotos

```
1. User seleciona diretório
   ↓
2. Scanner escaneia recursivamente
   ↓
3. MetadataReader extrai EXIF/XMP
   ↓
4. DuplicateDetector verifica duplicatas
   ↓
5. ThumbnailGenerator cria thumbnails
   ↓
6. PhotoRepository persiste no SQLite
   ↓
7. UI atualiza com novas fotos
```

### 3.2 Edição de Foto RAW

```
1. User seleciona foto
   ↓
2. RAWDecoder carrega arquivo RAW
   ↓
3. User ajusta parâmetros na UI
   ↓
4. RAWProcessor aplica ajustes em tempo real
   ↓
5. ColorManagement aplica perfil de cor
   ↓
6. Preview atualizado na tela
   ↓
7. EditHistory salvo no banco
```

### 3.3 Exportação

```
1. User seleciona fotos e configurações
   ↓
2. Exporter processa cada foto em paralelo
   ↓
3. RAWProcessor aplica edições salvas
   ↓
4. Resize redimensiona se necessário
   ↓
5. Watermark aplica marca d'água se configurado
   ↓
6. JPEGEncoder/PNGEncoder salva arquivo
   ↓
7. Progress feedback para UI
```

## 4. Padrões de Design

### 4.1 Repository Pattern
Abstrai acesso a dados, permitindo trocar implementação do banco sem afetar lógica de negócio.

```rust
pub trait PhotoRepository {
    fn save(&self, photo: &Photo) -> Result<()>;
    fn find_by_id(&self, id: PhotoId) -> Result<Option<Photo>>;
    fn find_all(&self) -> Result<Vec<Photo>>;
    fn find_by_filter(&self, filter: PhotoFilter) -> Result<Vec<Photo>>;
}
```

### 4.2 Command Pattern
Todas as edições são comandos que podem ser desfeitos/refeitos.

```rust
pub trait Command {
    fn execute(&self) -> Result<()>;
    fn undo(&self) -> Result<()>;
    fn description(&self) -> String;
}
```

### 4.3 Observer Pattern
Sistema de eventos para comunicação entre módulos.

```rust
pub enum AppEvent {
    PhotoImported(Photo),
    PhotoEdited(PhotoId),
    PhotoDeleted(PhotoId),
    RatingChanged(PhotoId, Rating),
    // ...
}
```

### 4.4 Strategy Pattern
Diferentes estratégias de exportação, processamento, etc.

```rust
pub trait ExportStrategy {
    fn export(&self, photo: &Photo, config: &ExportConfig) -> Result<Vec<u8>>;
}
```

### 4.5 Builder Pattern
Para construção de objetos complexos como configurações de exportação.

```rust
ExportConfig::builder()
    .format(ImageFormat::Jpeg)
    .quality(90)
    .resize(Some(ResizeConfig::new(1920, 1080)))
    .watermark(Some(watermark_config))
    .build()
```

## 5. Concorrência e Paralelização

### 5.1 Thread Pools
- **Importação**: Pool de threads para processar múltiplas fotos em paralelo
- **Thumbnail Generation**: Paralelo usando `rayon`
- **Exportação**: Batch paralelo com limite de threads

### 5.2 Async/Await
- I/O não-bloqueante para operações de arquivo
- UI permanece responsiva durante operações longas

### 5.3 Message Passing
- Canais (mpsc) para comunicação entre threads
- Event bus para propagação de eventos

## 6. Gerenciamento de Memória

### 6.1 Smart Pointers
- `Arc<T>` para dados compartilhados entre threads
- `Rc<T>` para referências contadas single-thread
- `RefCell<T>` para mutabilidade interior quando necessário

### 6.2 Cache Management
- LRU cache para thumbnails e previews
- Limite de memória configurável
- Liberação automática sob pressão de memória

### 6.3 Streaming
- Processamento de imagens grandes por chunks
- Evita carregar imagens completas na memória quando possível

## 7. Tratamento de Erros

### 7.1 Hierarquia de Erros

```rust
pub enum AppError {
    Io(io::Error),
    Database(DatabaseError),
    RawProcessing(RawError),
    Import(ImportError),
    Export(ExportError),
    // ...
}

impl From<io::Error> for AppError { /* ... */ }
```

### 7.2 Result Type
Todas as operações que podem falhar retornam `Result<T, AppError>`

### 7.3 Error Recovery
- Transações de banco com rollback
- Fallbacks para operações críticas
- Mensagens de erro amigáveis na UI

## 8. Testes

### 8.1 Estrutura de Testes

```
tests/
├── unit/               // Testes unitários por módulo
├── integration/        // Testes de integração
├── fixtures/          // Dados de teste (imagens exemplo)
└── performance/       // Benchmarks
```

### 8.2 Estratégia de Testes
- **Unit Tests**: Cada módulo tem seus próprios testes
- **Integration Tests**: Fluxos completos end-to-end
- **Property Tests**: Usando `proptest` para testes baseados em propriedades
- **Benchmarks**: Usando `criterion` para medição de performance

## 9. Build e Deployment

### 9.1 Cargo Workspaces
```toml
[workspace]
members = [
    "vintage-core",
    "vintage-raw",
    "vintage-ui",
    "vintage-import",
    "vintage-export",
]
```

### 9.2 Feature Flags
```toml
[features]
default = ["ui"]
ui = ["slint"]
cli = []  # Interface linha de comando opcional
gpu-acceleration = ["wgpu"]  # GPU para processamento futuro
```

### 9.3 Cross-Compilation
- macOS: Target `x86_64-apple-darwin` e `aarch64-apple-darwin`
- Windows: Target `x86_64-pc-windows-msvc`

### 9.4 Packaging
- **macOS**: App bundle (.app) + DMG installer
- **Windows**: MSI installer usando WiX ou Inno Setup

## 10. Segurança

### 10.1 Validação de Input
- Validação de paths de arquivo
- Sanitização de nomes de arquivo
- Validação de formato de imagem antes de processar

### 10.2 Isolamento
- Sandbox para processamento de RAW
- Limites de recursos (memória, CPU)

### 10.3 Dados Sensíveis
- Dados de vendas criptografados
- Não armazenar informações de pagamento

## 11. Extensibilidade

### 11.1 Plugin System (Futuro)
Sistema de plugins para:
- Novos formatos de exportação
- Ajustes customizados
- Integração com serviços externos

### 11.2 API Interna
- API documentada para módulos
- Traits bem definidos para extensão

## 12. Monitoramento e Logging

### 12.1 Logging
- Uso de `log` crate com níveis apropriados
- Arquivos de log rotativos
- Não logar informações sensíveis

### 12.2 Métricas
- Tempo de importação
- Tempo de processamento RAW
- Uso de memória e cache

## 13. Diagramas

### 13.1 Diagrama de Componentes

```
┌──────────────┐
│  Slint UI    │
└──────┬───────┘
       │
┌──────▼────────────────────────────────┐
│      Application Layer                │
│  ┌──────────┐  ┌─────────────────┐   │
│  │ Library  │  │ Develop Manager │   │
│  │ Manager  │  │                 │   │
│  └──────────┘  └─────────────────┘   │
└──────┬────────────────────────────────┘
       │
┌──────▼────────────────────────────────┐
│         Domain Layer                  │
│  ┌────────┐ ┌──────────┐ ┌─────────┐ │
│  │ Photo  │ │Collection│ │ Preset  │ │
│  └────────┘ └──────────┘ └─────────┘ │
└──────┬────────────────────────────────┘
       │
┌──────▼────────────────────────────────┐
│    Infrastructure Layer               │
│  ┌─────────┐ ┌──────┐  ┌──────────┐  │
│  │Database │ │ RAW  │  │FileSystem│  │
│  │(SQLite) │ │Proc. │  │          │  │
│  └─────────┘ └──────┘  └──────────┘  │
└───────────────────────────────────────┘
```

## 14. Considerações de Performance

### 14.1 Otimizações Prioritárias
1. Geração de thumbnails em background
2. Cache agressivo de previews
3. Lazy loading de metadados
4. Processamento SIMD quando possível
5. GPU acceleration para ajustes futuros

### 14.2 Targets de Performance
- Importação: 100 fotos/minuto
- Geração de thumbnail: < 200ms por foto
- Ajuste RAW em tempo real: < 500ms
- Exportação JPEG: < 2s por foto (24MP)

## 15. Próximos Passos

- Implementar proof-of-concept do processamento RAW
- Prototipar UI principal no Slint
- Definir schema completo do banco de dados
- Criar projeto base com estrutura de módulos
