# Arquitetura do Sistema - VintageLightbox

## 1. Visão Geral da Arquitetura

VintageLightbox segue os princípios da **Clean Architecture** (Arquitetura Limpa) proposta por Robert C. Martin, combinada com práticas de **Test-Driven Development (TDD)**. Esta abordagem garante:

- ✅ **Independência de Frameworks**: A lógica de negócio não depende de Slint ou outras bibliotecas externas
- ✅ **Testabilidade**: Todas as camadas são facilmente testáveis de forma isolada
- ✅ **Independência de UI**: A interface pode ser substituída sem afetar regras de negócio
- ✅ **Independência de Banco de Dados**: SQLite pode ser trocado por outra solução
- ✅ **Princípios SOLID**: Código modular, extensível e de fácil manutenção

### Camadas da Clean Architecture

```
┌───────────────────────────────────────────────────────────────┐
│              Frameworks & Drivers (Camada 4)                  │
│   - Slint UI, SQLite, LibRaw, Sistema de Arquivos            │
│   - Dependências externas e detalhes de implementação        │
├───────────────────────────────────────────────────────────────┤
│           Interface Adapters (Camada 3)                       │
│   - Controllers, Presenters, View Models                      │
│   - Gateways, Repository Implementations                      │
│   - Adaptadores para conversão de dados                      │
├───────────────────────────────────────────────────────────────┤
│              Use Cases (Camada 2)                             │
│   - Regras de negócio específicas da aplicação               │
│   - Orquestração de fluxos de trabalho                       │
│   - Input/Output Boundaries                                   │
├───────────────────────────────────────────────────────────────┤
│              Entities (Camada 1 - Core)                       │
│   - Regras de negócio empresariais                           │
│   - Entities, Value Objects, Domain Services                 │
│   - Independente de qualquer detalhe externo                 │
└───────────────────────────────────────────────────────────────┘

Regra de Dependência: As dependências apontam sempre para DENTRO
(Camadas externas dependem de camadas internas, nunca o contrário)
```

### Metodologia de Desenvolvimento: TDD (Test-Driven Development)

Todos os componentes serão desenvolvidos seguindo o ciclo **Red-Green-Refactor**:

1. **🔴 Red**: Escrever um teste que falha
2. **🟢 Green**: Implementar código mínimo para passar o teste
3. **🔵 Refactor**: Refatorar mantendo os testes verdes

**Benefícios do TDD**:
- Design emergente e evolutivo
- Cobertura de testes desde o início
- Código mais simples e focado
- Documentação viva através dos testes
- Refatoração segura

## 2. Estrutura de Módulos (Clean Architecture)

A estrutura de diretórios reflete as camadas da Clean Architecture:

```
crates/
├── domain/              # Camada 1: Entities (Core Domain)
├── use-cases/           # Camada 2: Application Business Rules
├── adapters/            # Camada 3: Interface Adapters
└── infrastructure/      # Camada 4: Frameworks & Drivers
```

### 2.1 Domain Layer (Camada 1 - Entities)

**Responsabilidade**: Regras de negócio empresariais puras, independentes de qualquer framework ou tecnologia.

#### `domain/` (crates/domain/)
Módulo com entidades e lógica de domínio central.

```rust
domain/
├── entities/
│   ├── photo.rs          // Entity: Photo com metadados e validações
│   ├── catalog.rs        // Entity: Catálogo de fotos
│   ├── collection.rs     // Entity: Coleções/Álbuns
│   └── adjustment.rs     // Entity: Ajustes de edição
├── value_objects/
│   ├── rating.rs         // Value Object: Sistema de classificação (0-5)
│   ├── color_label.rs    // Value Object: Marcações por cor
│   ├── photo_id.rs       // Value Object: Identificador único
│   ├── file_path.rs      // Value Object: Caminho de arquivo validado
│   └── color_space.rs    // Value Object: Espaço de cor
├── services/
│   ├── duplicate_detection.rs  // Domain Service: Detecção de duplicatas
│   └── color_management.rs     // Domain Service: Gerenciamento de cor
├── repositories/          // Repository Traits (Interfaces)
│   ├── photo_repository.rs
│   ├── collection_repository.rs
│   └── preset_repository.rs
├── errors.rs             // Domain-specific errors
└── lib.rs

// Exemplo de Entity
pub struct Photo {
    id: PhotoId,
    file_path: FilePath,
    metadata: PhotoMetadata,
    rating: Rating,
    color_label: Option<ColorLabel>,
    adjustments: Vec<Adjustment>,
}

impl Photo {
    pub fn rate(&mut self, rating: Rating) -> Result<(), DomainError> {
        rating.validate()?;
        self.rating = rating;
        Ok(())
    }
}
```

**Características**:
- ✅ Sem dependências externas (apenas Rust std)
- ✅ Lógica de negócio pura e testável
- ✅ Imutabilidade quando possível
- ✅ 100% de cobertura de testes unitários

### 2.2 Use Cases Layer (Camada 2 - Application Business Rules)

**Responsabilidade**: Orquestração de fluxos de trabalho e regras de negócio específicas da aplicação.

#### `use-cases/` (crates/use-cases/)

```rust
use-cases/
├── import/
│   ├── import_photos.rs       // Use Case: Importar fotos
│   ├── scan_directory.rs      // Use Case: Escanear diretório
│   └── detect_duplicates.rs   // Use Case: Detectar duplicatas
├── edit/
│   ├── apply_adjustment.rs    // Use Case: Aplicar ajuste a foto
│   ├── save_preset.rs         // Use Case: Salvar preset
│   └── batch_edit.rs          // Use Case: Edição em lote
├── organize/
│   ├── rate_photo.rs          // Use Case: Classificar foto
│   ├── add_to_collection.rs   // Use Case: Adicionar a coleção
│   └── filter_photos.rs       // Use Case: Filtrar fotos
├── export/
│   ├── export_photo.rs        // Use Case: Exportar foto
│   └── batch_export.rs        // Use Case: Exportação em lote
├── ports/                      // Input/Output Boundaries
│   ├── photo_repository.rs    // Port: Repository interface
│   ├── file_system.rs         // Port: File system interface
│   ├── raw_decoder.rs         // Port: RAW decoder interface
│   └── presenter.rs           // Port: Presenter interface
└── lib.rs

// Exemplo de Use Case
pub struct RatePhotoUseCase<R: PhotoRepository> {
    photo_repository: R,
}

impl<R: PhotoRepository> RatePhotoUseCase<R> {
    pub fn execute(&self, input: RatePhotoInput) -> Result<RatePhotoOutput, UseCaseError> {
        // 1. Buscar foto
        let mut photo = self.photo_repository
            .find_by_id(input.photo_id)?
            .ok_or(UseCaseError::PhotoNotFound)?;
        
        // 2. Aplicar regra de negócio
        photo.rate(input.rating)?;
        
        // 3. Persistir
        self.photo_repository.save(&photo)?;
        
        // 4. Retornar output
        Ok(RatePhotoOutput { photo })
    }
}
```

**Características**:
- ✅ Depende apenas da camada de Domain
- ✅ Define interfaces (ports) para camadas externas
- ✅ Orquestra chamadas a Entities e Services
- ✅ Testado com mocks/stubs dos repositories

### 2.3 Interface Adapters Layer (Camada 3)

**Responsabilidade**: Adaptar dados entre Use Cases e Frameworks/Drivers.

#### `adapters/` (crates/adapters/)

```rust
adapters/
├── controllers/
│   ├── library_controller.rs   // Controller do modo biblioteca
│   ├── develop_controller.rs   // Controller do modo edição
│   └── export_controller.rs    // Controller de exportação
├── presenters/
│   ├── photo_presenter.rs      // Formata dados para UI
│   ├── grid_presenter.rs       // Presenter da grade
│   └── histogram_presenter.rs  // Presenter do histograma
├── view_models/
│   ├── library_vm.rs           // ViewModel do modo biblioteca
│   ├── develop_vm.rs           // ViewModel do modo edição
│   └── slideshow_vm.rs         // ViewModel da apresentação
├── gateways/                    // Implementação dos Ports
│   ├── sqlite_photo_repo.rs    // Implementação SQLite do Repository
│   ├── file_system_impl.rs     // Implementação File System
│   └── raw_decoder_impl.rs     // Implementação RAW Decoder
└── lib.rs

// Exemplo de Controller
pub struct LibraryController {
    rate_photo_use_case: RatePhotoUseCase<SqlitePhotoRepository>,
}

impl LibraryController {
    pub fn rate_selected_photo(&self, rating: i32) -> Result<()> {
        let input = RatePhotoInput {
            photo_id: self.get_selected_photo_id(),
            rating: Rating::try_from(rating)?,
        };
        
        let output = self.rate_photo_use_case.execute(input)?;
        self.update_ui(output);
        Ok(())
    }
}
```

**Características**:
- ✅ Converte dados entre formatos externos e domínio
- ✅ Implementa interfaces definidas pelos Use Cases
- ✅ Coordena interação entre UI e Use Cases

### 2.4 Infrastructure Layer (Camada 4 - Frameworks & Drivers)

**Responsabilidade**: Detalhes de implementação, frameworks, bibliotecas externas.

#### `infrastructure/` (crates/infrastructure/)

```rust
infrastructure/
├── ui/                          // Slint UI Framework
│   ├── main_window.slint
│   ├── components/
│   │   ├── photo_grid.slint
│   │   ├── photo_viewer.slint
│   │   └── edit_panel.slint
│   └── bridge.rs                // Bridge Rust ↔ Slint
├── database/                     // SQLite Database
│   ├── connection.rs
│   ├── schema.rs
│   ├── migrations.rs
│   └── repositories/
│       ├── sqlite_photo_repo.rs  // Implementa PhotoRepository trait
│       ├── sqlite_collection_repo.rs
│       └── sqlite_preset_repo.rs
├── raw_processing/               // LibRaw/Rawler
│   ├── libraw_decoder.rs
│   ├── rawler_decoder.rs
│   └── processor.rs
├── file_system/
│   ├── scanner.rs
│   ├── watcher.rs               // File system watcher
│   └── thumbnail_generator.rs
├── cache/
│   ├── thumbnail_cache.rs
│   ├── preview_cache.rs
│   └── lru_cache.rs
├── multimonitor/
│   ├── display_manager.rs
│   └── secondary_window.rs
└── lib.rs

// Exemplo de Implementação de Repository
pub struct SqlitePhotoRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl PhotoRepository for SqlitePhotoRepository {
    fn save(&self, photo: &Photo) -> Result<(), RepositoryError> {
        let conn = self.pool.get()?;
        // SQL específico do SQLite
        conn.execute(
            "INSERT OR REPLACE INTO photos (...) VALUES (...)",
            params![photo.id(), photo.path()],
        )?;
        Ok(())
    }
    
    fn find_by_id(&self, id: PhotoId) -> Result<Option<Photo>, RepositoryError> {
        // Implementação específica SQLite
    }
}
```

**Características**:
- ✅ Pode ser trocada sem afetar regras de negócio
- ✅ Contém todos os detalhes técnicos
- ✅ Integração com bibliotecas externas (Slint, SQLite, LibRaw)

## 3. Fluxo de Dados seguindo Clean Architecture

### 3.1 Importação de Fotos (seguindo camadas)

**Fluxo TDD**: Primeiro escrevemos testes para cada camada, depois implementamos.

```
┌─────────────────────────────────────────────────────────────┐
│  UI (Infrastructure)                                         │
│  - User clica em "Importar"                                 │
│  - Slint emite evento                                       │
└────────────────────┬────────────────────────────────────────┘
                     │ (1) UI Event
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  Controller (Adapters)                                       │
│  - LibraryController.import_photos()                        │
│  - Converte evento UI para Input do Use Case               │
└────────────────────┬────────────────────────────────────────┘
                     │ (2) ImportPhotosInput
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  Use Case (Application Business Rules)                      │
│  - ImportPhotosUseCase.execute(input)                       │
│  - Orquestra o processo de importação                       │
│  - Chama repositories e services                            │
└────┬────────────────────┬────────────────────┬──────────────┘
     │ (3a) scan()        │ (3b) find_by_hash()│ (3c) save()
     ▼                    ▼                    ▼
┌─────────────┐  ┌──────────────────┐  ┌────────────────┐
│FileSystem   │  │PhotoRepository   │  │DuplicateService│
│Port/Gateway │  │Port/Gateway      │  │(Domain)        │
└─────────────┘  └──────────────────┘  └────────────────┘
     │                    │                    │
     │ (4) Read files     │ (5) SQL queries   │ (6) Hash check
     ▼                    ▼                    ▼
┌─────────────────────────────────────────────────────────────┐
│  Infrastructure (External Dependencies)                      │
│  - Disk I/O                                                 │
│  - SQLite Database                                          │
│  - LibRaw/Rawler                                            │
└────────────────────┬────────────────────────────────────────┘
                     │ (7) Return results
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  Presenter (Adapters)                                        │
│  - PhotoPresenter.present(output)                           │
│  - Formata dados para exibição                              │
└────────────────────┬────────────────────────────────────────┘
                     │ (8) ViewModel
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  UI (Infrastructure)                                         │
│  - Atualiza grid com thumbnails                             │
│  - Mostra progresso                                         │
└─────────────────────────────────────────────────────────────┘
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

### 3.2 Edição de Foto RAW (seguindo Clean Architecture)

```
UI → Controller → Use Case → Domain Entities → Repositories → Infrastructure
                    ↓
              Apply business rules
                    ↓
              Domain validation
                    ↓
              Persistence
```

**Testes TDD por camada**:
1. **Domain**: Testar lógica de ajustes (exposure, contrast, etc.)
2. **Use Cases**: Testar orquestração com mocks de repositories
3. **Adapters**: Testar conversão de dados
4. **Infrastructure**: Testes de integração com SQLite real

## 4. Padrões de Design (Clean Architecture)

### 4.1 Dependency Inversion Principle (DIP)

Use Cases definem interfaces (traits), Infrastructure implementa.

```rust
// Use Cases Layer - Define a interface
pub trait PhotoRepository {
    fn save(&self, photo: &Photo) -> Result<(), RepositoryError>;
    fn find_by_id(&self, id: PhotoId) -> Result<Option<Photo>, RepositoryError>;
    fn find_all(&self) -> Result<Vec<Photo>, RepositoryError>;
}

// Infrastructure Layer - Implementa
pub struct SqlitePhotoRepository { /* ... */ }

impl PhotoRepository for SqlitePhotoRepository {
    // Implementação específica SQLite
}
```

**Benefício**: Use Case não conhece SQLite, pode ser testado com mock.

### 4.2 Repository Pattern

Abstração do acesso a dados.

```rust
// Test com Mock
#[cfg(test)]
mod tests {
    use mockall::mock;
    
    mock! {
        pub PhotoRepo {}
        impl PhotoRepository for PhotoRepo {
            fn save(&self, photo: &Photo) -> Result<(), RepositoryError>;
            fn find_by_id(&self, id: PhotoId) -> Result<Option<Photo>, RepositoryError>;
        }
    }
    
    #[test]
    fn test_rate_photo_use_case() {
        let mut mock_repo = MockPhotoRepo::new();
        mock_repo.expect_find_by_id()
            .returning(|_| Ok(Some(Photo::new_test())));
        
        let use_case = RatePhotoUseCase::new(mock_repo);
        // Test without database!
    }
}
```

### 4.3 Command Pattern (Domain Layer)

Todas as edições são comandos imutáveis e reversíveis.

```rust
pub trait Command {
    fn execute(&self, photo: &mut Photo) -> Result<(), DomainError>;
    fn undo(&self, photo: &mut Photo) -> Result<(), DomainError>;
    fn description(&self) -> String;
}

pub struct AdjustExposureCommand {
    delta: f32,
}

impl Command for AdjustExposureCommand {
    fn execute(&self, photo: &mut Photo) -> Result<(), DomainError> {
        photo.adjust_exposure(self.delta)
    }
    
    fn undo(&self, photo: &mut Photo) -> Result<(), DomainError> {
        photo.adjust_exposure(-self.delta)
    }
}
```

### 4.4 Strategy Pattern (Use Cases Layer)

Diferentes estratégias de exportação, processamento.

```rust
pub trait ExportStrategy {
    fn export(&self, photo: &Photo, config: &ExportConfig) 
        -> Result<Vec<u8>, ExportError>;
}

pub struct JpegExportStrategy;
pub struct PngExportStrategy;
pub struct TiffExportStrategy;
```

### 4.5 Builder Pattern (Domain Layer)

Construção de Value Objects e Entities complexas.

```rust
// TDD: Primeiro o teste
#[test]
fn test_export_config_builder() {
    let config = ExportConfig::builder()
        .format(ImageFormat::Jpeg)
        .quality(90)
        .build()
        .unwrap();
    
    assert_eq!(config.quality(), 90);
}

// Depois a implementação
impl ExportConfig {
    pub fn builder() -> ExportConfigBuilder {
        ExportConfigBuilder::default()
    }
}
```

### 4.6 Observer Pattern (Cross-Cutting)

Eventos de domínio para comunicação desacoplada.

```rust
// Domain Events
pub enum DomainEvent {
    PhotoRated { photo_id: PhotoId, rating: Rating },
    PhotoEdited { photo_id: PhotoId, adjustment: Adjustment },
    CollectionCreated { collection_id: CollectionId },
}

// Event Bus (Infrastructure)
pub trait EventBus {
    fn publish(&self, event: DomainEvent);
    fn subscribe<H: EventHandler>(&mut self, handler: H);
}
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

### 8.2 Estratégia de Testes (TDD)

**Pirâmide de Testes**:
```
              /\
             /  \    E2E Tests (poucos)
            /____\   
           /      \  
          / Integr.\  Integration Tests (médio)
         /  Tests   \
        /____________\
       /              \
      /   Unit Tests   \  Unit Tests (muitos)
     /__________________\
```

**Tipos de Testes por Camada**:

#### Domain Layer (Entities)
- **Unit Tests**: Testar lógica de negócio isoladamente
- **Property-Based Tests**: Invariantes e propriedades
- **Coverage**: 100% obrigatório (regras de negócio críticas)

```rust
// Exemplo de teste de domínio
#[test]
fn test_photo_rating_cannot_exceed_five() {
    let mut photo = Photo::new_test();
    let result = photo.rate(Rating::try_from(6));
    assert!(result.is_err());
}

// Property test
proptest! {
    #[test]
    fn test_rating_always_within_bounds(rating in 0..=5i32) {
        let r = Rating::try_from(rating);
        prop_assert!(r.is_ok());
        prop_assert!(r.unwrap().value() <= 5);
    }
}
```

#### Use Cases Layer
- **Unit Tests com Mocks**: Testar orquestração sem dependências externas
- **Coverage**: ≥95%

```rust
#[test]
fn test_import_photos_use_case() {
    let mut mock_repo = MockPhotoRepository::new();
    let mut mock_fs = MockFileSystem::new();
    
    mock_fs.expect_scan_directory()
        .returning(|_| Ok(vec![PathBuf::from("photo1.jpg")]));
    
    mock_repo.expect_save()
        .times(1)
        .returning(|_| Ok(()));
    
    let use_case = ImportPhotosUseCase::new(mock_repo, mock_fs);
    let result = use_case.execute(ImportInput { path: "test/" });
    
    assert!(result.is_ok());
}
```

#### Interface Adapters Layer
- **Unit Tests**: Controllers e Presenters
- **Snapshot Tests**: Verificar formato de dados para UI
- **Coverage**: ≥85%

```rust
#[test]
fn test_photo_presenter_format() {
    let photo = Photo::new_test();
    let presenter = PhotoPresenter::new();
    let view_model = presenter.present(&photo);
    
    assert_debug_snapshot!(view_model);
}
```

#### Infrastructure Layer
- **Integration Tests**: Testar com recursos reais (SQLite, file system)
- **Contract Tests**: Verificar implementação de traits
- **Coverage**: ≥70%

```rust
#[test]
fn test_sqlite_repository_save_and_find() {
    let db = setup_test_database();
    let repo = SqlitePhotoRepository::new(db);
    
    let photo = Photo::new_test();
    repo.save(&photo).unwrap();
    
    let found = repo.find_by_id(photo.id()).unwrap();
    assert_eq!(found.unwrap().id(), photo.id());
}
```

#### End-to-End Tests
- **UI Tests**: Fluxos completos de usuário
- **Smoke Tests**: Funcionalidades críticas
- **Poucos mas críticos**

```rust
#[test]
fn test_full_import_workflow() {
    let app = setup_test_app();
    
    // Simular ação do usuário
    app.select_import_directory("tests/fixtures/photos");
    app.click_import_button();
    
    // Verificar resultado
    wait_for_import_completion();
    assert_eq!(app.photo_count(), 5);
}
```

**Ferramentas de Teste**:
- `cargo test`: Framework padrão
- `mockall`: Mocking de traits
- `proptest`: Property-based testing
- `criterion`: Benchmarking
- `insta`: Snapshot testing
- `fake`: Geração de dados de teste
- `tempfile`: Arquivos temporários
- `cargo-tarpaulin` / `cargo-llvm-cov`: Cobertura de código

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

## 15. Fluxo de Trabalho TDD (Test-Driven Development)

### 15.1 Ciclo Red-Green-Refactor

Todo desenvolvimento segue estritamente o ciclo TDD:

```
┌─────────────────────────────────────────────────────────────┐
│ CICLO TDD: Red → Green → Refactor                           │
└─────────────────────────────────────────────────────────────┘

1. 🔴 RED (Teste Falha)
   ├─> Escrever teste que define comportamento desejado
   ├─> Teste deve falhar (código ainda não existe)
   └─> Executar: cargo test --lib <nome_do_teste>

2. 🟢 GREEN (Teste Passa)
   ├─> Implementar código MÍNIMO para passar o teste
   ├─> Não se preocupar com qualidade ainda
   ├─> Objetivo: fazer o teste passar o mais rápido possível
   └─> Executar: cargo test --lib <nome_do_teste>

3. 🔵 REFACTOR (Melhorar Código)
   ├─> Eliminar duplicação
   ├─> Aplicar padrões de design
   ├─> Melhorar nomes e estrutura
   ├─> Testes devem continuar passando
   └─> Executar: cargo test --lib

4. ♻️ REPETIR
   └─> Voltar ao passo 1 para próxima funcionalidade
```

### 15.2 Exemplo Prático: Implementando Rating

**Iteração 1: Caso básico**

```rust
// 🔴 RED: Escrever teste primeiro
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_photo_with_five_stars() {
        let mut photo = Photo::new(PhotoId::new(), FilePath::new("test.jpg"));
        let result = photo.rate(Rating::Five);
        
        assert!(result.is_ok());
        assert_eq!(photo.rating(), Some(Rating::Five));
    }
}

// Executar: cargo test test_rate_photo_with_five_stars
// ❌ FALHA: método `rate` não existe

// 🟢 GREEN: Implementação mínima
impl Photo {
    pub fn rate(&mut self, rating: Rating) -> Result<(), DomainError> {
        self.rating = Some(rating);
        Ok(())
    }
    
    pub fn rating(&self) -> Option<Rating> {
        self.rating
    }
}

// Executar: cargo test test_rate_photo_with_five_stars
// ✅ PASSA

// 🔵 REFACTOR: Nada a refatorar ainda (código simples)
```

**Iteração 2: Validação**

```rust
// 🔴 RED: Teste de edge case
#[test]
fn test_rating_must_be_valid() {
    let mut photo = Photo::new_test();
    let result = photo.rate(Rating::try_from(10).unwrap_or_default());
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), DomainError::InvalidRating);
}

// ❌ FALHA: teste não falha como esperado

// 🟢 GREEN: Adicionar validação
impl Photo {
    pub fn rate(&mut self, rating: Rating) -> Result<(), DomainError> {
        if rating.is_valid() {
            self.rating = Some(rating);
            Ok(())
        } else {
            Err(DomainError::InvalidRating)
        }
    }
}

// ✅ PASSA

// 🔵 REFACTOR: Mover validação para Value Object
impl Rating {
    pub fn new(value: u8) -> Result<Self, DomainError> {
        if value <= 5 {
            Ok(Rating(value))
        } else {
            Err(DomainError::InvalidRating)
        }
    }
}

impl Photo {
    pub fn rate(&mut self, rating: Rating) -> Result<(), DomainError> {
        // Validação já foi feita na construção de Rating
        self.rating = Some(rating);
        Ok(())
    }
}

// Executar todos os testes
// cargo test --lib
// ✅ TODOS PASSAM
```

### 15.3 TDD por Camada

**Domain Layer (Bottom-Up TDD)**:
1. Começar pelos Value Objects mais simples
2. Depois Entities
3. Por último Domain Services

**Use Cases Layer (Outside-In TDD)**:
1. Definir interface do Use Case (Input/Output)
2. Escrever teste com mocks
3. Implementar orquestração

**Infrastructure Layer (Integration TDD)**:
1. Escrever teste de integração
2. Implementar adapter/gateway
3. Testar com recurso real (SQLite, file system)

### 15.4 Regras de Ouro do TDD

1. **Nunca escrever código de produção sem um teste falhando**
2. **Escrever apenas teste suficiente para falhar** (incluindo erros de compilação)
3. **Escrever apenas código suficiente para passar o teste**
4. **Refatorar imediatamente após teste passar**
5. **Rodar testes frequentemente** (a cada 1-2 minutos)
6. **Commits pequenos e frequentes** (após cada ciclo Green)

### 15.5 Boas Práticas

```rust
// ✅ BOM: Teste específico e focado
#[test]
fn test_exposure_adjustment_increases_brightness() {
    let mut photo = Photo::new_test();
    photo.adjust_exposure(1.0).unwrap();
    
    assert!(photo.exposure() > 0.0);
}

// ❌ RUIM: Teste muito abrangente
#[test]
fn test_all_adjustments() {
    let mut photo = Photo::new_test();
    photo.adjust_exposure(1.0).unwrap();
    photo.adjust_contrast(0.5).unwrap();
    photo.adjust_saturation(0.3).unwrap();
    // Muito coisa sendo testada de uma vez
}

// ✅ BOM: Setup claro e reutilizável
impl Photo {
    #[cfg(test)]
    pub fn new_test() -> Self {
        Photo::new(
            PhotoId::new(),
            FilePath::new("test.jpg").unwrap(),
        )
    }
}

// ✅ BOM: Uso de builders para testes complexos
#[cfg(test)]
pub struct PhotoBuilder {
    rating: Option<Rating>,
    adjustments: Vec<Adjustment>,
    // ...
}

impl PhotoBuilder {
    pub fn with_rating(mut self, rating: Rating) -> Self {
        self.rating = Some(rating);
        self
    }
    
    pub fn build(self) -> Photo {
        // Construir Photo com dados de teste
    }
}
```

### 15.6 Ferramentas de Apoio

**Watch Mode** (testes automáticos ao salvar):
```bash
cargo install cargo-watch
cargo watch -x "test --lib"
```

**Test Explorer** (VS Code):
- Extensão: `Rust Test Explorer`
- Rodar testes individuais com um clique

**Coverage em Tempo Real**:
```bash
cargo install cargo-watch cargo-tarpaulin
cargo watch -s "cargo tarpaulin --lib"
```

## 16. Próximos Passos

### Fase 1: Setup e Domain Layer (Semanas 1-2)
1. ✅ Configurar workspace com Clean Architecture
2. ✅ Setup CI/CD com testes automáticos
3. ✅ TDD: Implementar Value Objects (Rating, ColorLabel, PhotoId)
4. ✅ TDD: Implementar Entities (Photo, Collection, Adjustment)
5. ✅ TDD: Implementar Domain Services
6. ✅ Meta: 100% cobertura de testes do domínio

### Fase 2: Use Cases Layer (Semanas 3-4)
1. ✅ Definir Repository traits
2. ✅ TDD: ImportPhotosUseCase com mocks
3. ✅ TDD: RatePhotoUseCase
4. ✅ TDD: ApplyAdjustmentUseCase
5. ✅ TDD: ExportPhotoUseCase
6. ✅ Meta: ≥95% cobertura

### Fase 3: Infrastructure (Semanas 5-6)
1. ✅ Implementar SQLite Repository (testes de integração)
2. ✅ Implementar File System Scanner
3. ✅ Integrar LibRaw/Rawler (testes de integração)
4. ✅ Implementar Cache (testes de integração)

### Fase 4: Adapters & UI (Semanas 7-8)
1. ✅ Implementar Controllers
2. ✅ Implementar Presenters (snapshot tests)
3. ✅ Prototipar UI Slint básica
4. ✅ Integração completa

### Fase 5: E2E & Refinamento (Semanas 9-10)
1. ✅ Testes E2E dos fluxos principais
2. ✅ Benchmarks e otimizações
3. ✅ Documentação
4. ✅ Beta testing

**Princípios a seguir em cada fase**:
- 🧪 TDD em todas as camadas
- 🏗️ Clean Architecture sempre respeitada
- 📊 Monitorar cobertura de testes
- ♻️ Refatoração contínua
- 📝 Documentação inline e testes como documentação
