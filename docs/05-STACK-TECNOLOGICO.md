# Stack Tecnológico - VintageLightbox

## Visão Geral

Este documento detalha todas as tecnologias, bibliotecas e ferramentas utilizadas no desenvolvimento do VintageLightbox.

**Princípios Fundamentais**:
- 🏗️ **Clean Architecture**: Separação clara de responsabilidades em camadas
- 🧪 **Test-Driven Development (TDD)**: Testes primeiro, código depois
- 📐 **SOLID Principles**: Design orientado a objetos de qualidade
- ♻️ **Refatoração Contínua**: Código limpo e evolutivo

**Status**: MVP Completo ✅ | 360+ testes | Todas as camadas implementadas

---

## 1. Linguagem de Programação

### Rust (Stable - Latest)

**Por que Rust?**
- ✅ **Performance**: Comparável a C/C++, essencial para processamento de imagens
- ✅ **Segurança de Memória**: Elimina classes inteiras de bugs (use-after-free, data races)
- ✅ **Zero-cost Abstractions**: Alto nível sem sacrificar performance
- ✅ **Concorrência Segura**: Ownership system previne data races em compile-time
- ✅ **Cross-platform**: Excelente suporte para macOS e Windows
- ✅ **Ecossistema**: Crates.io com bibliotecas de qualidade
- ✅ **Tooling**: Cargo, rustfmt, clippy - ferramentas de primeira classe

**Versão Mínima**: Rust 1.75+ (ou latest stable)

---

## 2. Interface Gráfica

### egui 0.31 (Immediate Mode GUI) ✅ IMPLEMENTADO

**Características**:
- ✅ Immediate Mode - simples e performático
- ✅ Nativo e responsivo
- ✅ Cross-platform (macOS, Windows, Linux)
- ✅ Suporte a HiDPI/Retina
- ✅ Integração com wgpu para GPU
- ✅ Hot-reload durante desenvolvimento

**Crates Utilizados**:
```toml
egui = "0.31"
eframe = "0.31"
egui_plot = "0.31"      # Gráficos interativos
egui-notify = "0.19"    # Sistema de notificações toast
egui_phosphor = "0.9"   # Ícones profissionais
```

**Exemplo de Código**:
```rust
fn ui(&mut self, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("VintageLightbox");
        
        if ui.button("Import Photos").clicked() {
            self.show_import_dialog = true;
        }
        
        PhotoGrid::new(&self.photos)
            .columns(self.grid_columns)
            .show(ui);
    });
}
```

**Design System Implementado**:
- **5 Temas**: Vintage Dark, Mocha Dark, Macchiato Dark, Frappe Dark, Latte Light
- **Phosphor Icons**: Ícones profissionais em toda a UI
- **Sistema de Notificações**: Toast notifications não-intrusivas
- **Gráficos Interativos**: Histograma RGB, Tone Curve, Rating Distribution

---

## 3. Processamento de Imagens RAW

### 3.1 Rawler (Rust puro) ✅ IMPLEMENTADO

**Crate**: `rawler`

**Características**:
- ✅ 100% Rust (sem dependências C)
- ✅ Rápido e seguro
- ✅ Suporta principais formatos: CR2, NEF, ARW, DNG, RAF
- ✅ Mais fácil de distribuir (sem deps nativas)

### 3.2 LibRaw (Fallback)

**Uso**: Fallback para formatos não suportados pelo rawler

---

## 4. Aceleração GPU

### wgpu (Compute Shaders) ✅ IMPLEMENTADO

**Crate**: `wgpu`

**Características**:
- ✅ Aceleração GPU cross-platform (Vulkan, Metal, DX12)
- ✅ Compute shaders para processamento de imagem
- ✅ Fallback automático para CPU
- ✅ Performance: < 16ms para 24MP (60fps)

**Ajustes GPU Suportados** (single-pass shader):
1. Exposure
2. Contrast
3. Temperature
4. Tint
5. Highlights
6. Shadows
7. Whites
8. Blacks
9. Clarity
10. Vibrance
11. Saturation
12. Noise Reduction (Luminance + Color)
13. Sharpening

---

## 5. Processamento de Imagens

### 5.1 Image Crate

**Crate**: `image`

**Uso**:
- Encoding/decoding JPEG, PNG, TIFF
- Redimensionamento
- Conversões de formato de pixel
- Operações básicas de imagem

### 5.2 Fast Image Resize

**Crate**: `fast_image_resize`

**Uso**:
- Redimensionamento high-quality e rápido
- Múltiplos algoritmos (Lanczos, Mitchell, etc.)
- SIMD optimizations

---

## 6. Metadados

### Kamadak-exif ✅ IMPLEMENTADO

**Crate**: `kamadak-exif`

**Uso**:
- Leitura de metadados EXIF
- Suporta TIFF, JPEG, RAW (via TIFF headers)
- Acesso a tags padrão e customizadas

**Metadados Extraídos**:
- Câmera, Lente
- ISO, Apertura, Velocidade do obturador
- Data de captura
- Dimensões
- GPS (quando disponível)

---

## 7. Banco de Dados

### SQLite via Rusqlite ✅ IMPLEMENTADO

**Crate**: `rusqlite`

**Características**:
- ✅ Embarcado (sem servidor)
- ✅ Zero-configuration
- ✅ ACID compliant
- ✅ Rápido para leitura
- ✅ Cross-platform

**Features Usadas**:
- Bundled (SQLite compilado junto)
- BLOB storage para thumbnails/previews
- Migrations automáticas

**Schema Atual** (16 migrations):
- `photos` - Dados principais das fotos
- `collections` - Coleções de fotos
- `collection_photos` - Relação N:N
- `presets` - Presets de edição salvos
preview_threads = 2

[ui]
theme = "dark"
thumbnail_size = 200
```

---

## 10. Cache

### 10.1 LRU Cache

**Crate**: `lru = "0.12"`

**Uso**:
- Cache de thumbnails em memória
- Cache de previews
- Eviction automática

### 10.2 File-based Cache

**Custom Implementation**

**Estrutura**:
```
~/.cache/VintageLightbox/
├── thumbnails/
│   ├── abc123.jpg
│   └── def456.jpg
└── previews/
    ├── abc123.jpg
    └── def456.jpg
```

---

## 11. File System

### 11.1 Walkdir

**Crate**: `walkdir = "2.4"`

**Uso**:
- Scanning recursivo de diretórios
- Filtrar por extensão
- Eficiente e cross-platform

### 11.2 Notify

**Crate**: `notify = "6.1"`

**Uso**:
- File system watcher
- Detectar mudanças externas
- Auto-refresh de fotos

---

## 12. Hashing e Integridade

### Blake3

**Crate**: `blake3 = "1.5"`

**Uso**:
- Hash de arquivos para detecção de duplicatas
- Checksum para integridade
- Mais rápido que SHA-256

**Exemplo**:
```rust
let hash = blake3::hash(file_contents);
```

---

## 13. Logging e Debugging

### 13.1 Tracing

**Crate**: 
```toml
tracing = "0.1"
tracing-subscriber = "0.3"
```

**Uso**:
- Structured logging
- Performance tracing
- Debug information
- Multiple log levels

### 13.2 Env Logger

**Crate**: `env_logger = "0.11"`

**Uso**:
- Simple logging para desenvolvimento
- Configurável via variável de ambiente

---

## 14. Test-Driven Development (TDD) - Ferramentas de Teste

### Metodologia TDD

O VintageLightbox segue rigorosamente o ciclo **Red-Green-Refactor**:

```
1. 🔴 RED: Escrever teste que falha
   ├─> Definir comportamento esperado
   └─> Criar interface/assinatura de função

2. 🟢 GREEN: Implementar código mínimo
   ├─> Fazer o teste passar
   └─> Não se preocupar com otimização ainda

3. 🔵 REFACTOR: Melhorar código
   ├─> Eliminar duplicação
   ├─> Melhorar design
   └─> Manter testes verdes
```

### 14.1 Cargo Test (Built-in)

**Framework de teste padrão do Rust**

**Características**:
- ✅ Integrado ao Cargo
- ✅ Testes unitários e integração
- ✅ Execução paralela
- ✅ Filtering e organização

**Estrutura de Testes**:
```rust
// Em cada módulo: tests unitários
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_photo_rating_valid() {
        let rating = Rating::try_from(5).unwrap();
        assert_eq!(rating.value(), 5);
    }
    
    #[test]
    #[should_panic(expected = "invalid rating")]
    fn test_photo_rating_invalid() {
        Rating::try_from(6).unwrap(); // Deve falhar
    }
}

// Em tests/: testes de integração
#[test]
fn test_import_workflow() {
    let catalog = Catalog::new();
    catalog.import_from("test_fixtures/photos").unwrap();
    assert!(catalog.count() > 0);
}
```

**Comandos**:
```bash
# Rodar todos os testes
cargo test

# Testes específicos
cargo test test_photo_rating

# Com output detalhado
cargo test -- --nocapture

# Testes de um crate específico
cargo test -p domain

# Rodar testes em série (não paralelo)
cargo test -- --test-threads=1
```

### 14.2 Mockall

**Crate**: `mockall = "0.12"`

**Uso**: Mock objects para testar Use Cases isoladamente

**Características**:
- ✅ Mocking de traits
- ✅ Expectativas e verificações
- ✅ Controle total sobre retornos

**Exemplo TDD**:
```rust
use mockall::mock;
use mockall::predicate::*;

// 1. RED: Definir o contrato (trait)
pub trait PhotoRepository {
    fn find_by_id(&self, id: PhotoId) -> Result<Option<Photo>>;
    fn save(&self, photo: &Photo) -> Result<()>;
}

// 2. RED: Escrever teste com mock
#[cfg(test)]
mod tests {
    use super::*;
    
    mock! {
        pub PhotoRepo {}
        impl PhotoRepository for PhotoRepo {
            fn find_by_id(&self, id: PhotoId) -> Result<Option<Photo>>;
            fn save(&self, photo: &Photo) -> Result<()>;
        }
    }
    
    #[test]
    fn test_rate_photo_updates_repository() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        let test_photo = Photo::new_test();
        
        mock_repo.expect_find_by_id()
            .with(eq(PhotoId::from(123)))
            .times(1)
            .returning(move |_| Ok(Some(test_photo.clone())));
        
        mock_repo.expect_save()
            .times(1)
            .returning(|_| Ok(()));
        
        // Act
        let use_case = RatePhotoUseCase::new(mock_repo);
        let result = use_case.execute(RatePhotoInput {
            photo_id: PhotoId::from(123),
            rating: Rating::Five,
        });
        
        // Assert
        assert!(result.is_ok());
    }
}

// 3. GREEN: Implementar o Use Case
// 4. REFACTOR: Melhorar código mantendo testes verdes
```

### 14.3 Proptest (Property-based Testing)

**Crate**: `proptest = "1.4"`

**Uso**: Testes baseados em propriedades, gera casos de teste automaticamente

**Características**:
- ✅ Geração automática de casos de teste
- ✅ Shrinking - encontra caso mínimo que falha
- ✅ Testa invariantes e propriedades

**Exemplo**:
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_rating_roundtrip(rating in 0..=5i32) {
        // Property: Rating deve fazer roundtrip através de conversão
        let r = Rating::try_from(rating).unwrap();
        prop_assert_eq!(r.value(), rating);
    }
    
    #[test]
    fn test_exposure_adjustment_symmetric(delta in -5.0..5.0f32) {
        // Property: aplicar +delta e depois -delta deve voltar ao original
        let mut photo = Photo::new_test();
        let original_exposure = photo.exposure();
        
        photo.adjust_exposure(delta).unwrap();
        photo.adjust_exposure(-delta).unwrap();
        
        prop_assert!((photo.exposure() - original_exposure).abs() < 0.001);
    }
}
```

### 14.4 Criterion (Benchmarking)

**Crate**: `criterion = "0.5"`

**Uso**: Benchmarks estatisticamente rigorosos

**Características**:
- ✅ Medição precisa de performance
- ✅ Detecção de regressões
- ✅ Geração de gráficos
- ✅ Comparação entre versões

**Exemplo**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_thumbnail_generation(c: &mut Criterion) {
    let photo = load_test_photo();
    
    c.bench_function("generate_thumbnail_200px", |b| {
        b.iter(|| {
            generate_thumbnail(black_box(&photo), black_box(200))
        })
    });
}

criterion_group!(benches, benchmark_thumbnail_generation);
criterion_main!(benches);
```

**Rodar benchmarks**:
```bash
cargo bench
```

### 14.5 Insta (Snapshot Testing)

**Crate**: `insta = "1.34"`

**Uso**: Testes de snapshot para outputs complexos

**Características**:
- ✅ Captura output e compara com snapshot salvo
- ✅ Útil para testar serialização, formatação, etc.
- ✅ Review de mudanças com `cargo insta review`

**Exemplo**:
```rust
use insta::assert_debug_snapshot;

#[test]
fn test_photo_metadata_serialization() {
    let photo = Photo::new_test();
    assert_debug_snapshot!(photo.metadata());
}

#[test]
fn test_adjustment_chain_output() {
    let adjustments = vec![
        Adjustment::Exposure(1.5),
        Adjustment::Contrast(0.2),
    ];
    assert_debug_snapshot!(adjustments);
}
```

### 14.6 Fake (Test Data Generation)

**Crate**: `fake = "2.9"`

**Uso**: Geração de dados realistas para testes

**Exemplo**:
```rust
use fake::{Fake, Faker};

#[test]
fn test_with_fake_data() {
    let photo = Photo {
        id: PhotoId::new(),
        path: Faker.fake(),
        camera: Faker.fake(),
        lens: Faker.fake(),
        // ...
    };
    // Testar com dados realistas
}
```

### 14.7 Wiremock (HTTP Mocking)

**Crate**: `wiremock = "0.6"`

**Uso**: Mock de serviços HTTP (útil se integrar com serviços externos)

### 14.8 Tempfile (Arquivos Temporários)

**Crate**: `tempfile = "3.8"`

**Uso**: Criar arquivos temporários para testes

**Exemplo**:
```rust
use tempfile::tempdir;

#[test]
fn test_import_photos_from_directory() {
    let dir = tempdir().unwrap();
    let test_file = dir.path().join("test.jpg");
    
    // Criar arquivo de teste
    std::fs::write(&test_file, TEST_JPEG_DATA).unwrap();
    
    // Testar importação
    let catalog = Catalog::new();
    catalog.import_from(dir.path()).unwrap();
    
    assert_eq!(catalog.count(), 1);
    // tempdir é automaticamente deletado ao sair do escopo
}
```

### 14.9 Cobertura de Testes

**Ferramenta**: `cargo-tarpaulin` ou `cargo-llvm-cov`

**Instalação**:
```bash
cargo install cargo-tarpaulin
# ou
cargo install cargo-llvm-cov
```

**Uso**:
```bash
# Gerar relatório de cobertura
cargo tarpaulin --out Html --output-dir coverage

# Com llvm-cov
cargo llvm-cov --html
```

**Meta de Cobertura**:
- **Domain Layer**: 100% (regras de negócio críticas)
- **Use Cases Layer**: ≥ 95%
- **Adapters Layer**: ≥ 85%
- **Infrastructure Layer**: ≥ 70% (muitos testes de integração)

### 14.10 Estrutura de Testes

```
crates/
├── domain/
│   ├── src/
│   │   ├── entities/
│   │   │   └── photo.rs
│   │   └── lib.rs
│   └── tests/              # Testes de integração do domínio
│       └── photo_tests.rs
├── use-cases/
│   ├── src/
│   │   └── import/
│   │       └── import_photos.rs
│   └── tests/              # Testes com mocks
│       └── import_tests.rs
└── infrastructure/
    └── tests/              # Testes de integração reais
        └── sqlite_repository_tests.rs

tests/                      # Testes E2E da aplicação
├── fixtures/               # Dados de teste
│   └── photos/
└── integration/
    └── full_workflow_test.rs
```

### Comandos Úteis TDD

```bash
# Watch mode - roda testes automaticamente ao salvar
cargo install cargo-watch
cargo watch -x test

# Rodar apenas testes rápidos (unitários)
cargo test --lib

# Rodar testes de integração
cargo test --test '*'

# Continuous testing com feedback visual
cargo install cargo-nextest
cargo nextest run

# Verificar testes sem compilar código
cargo check --tests
```

---

## 14. Testes

### 14.1 Cargo Test (Built-in)

**Uso**:
- Unit tests
- Integration tests
- Doc tests

### 14.2 Criterion

**Crate**: `criterion = "0.5"`

**Uso**:
- Benchmarking
- Performance regression detection
- Statistical analysis

### 14.3 Proptest

**Crate**: `proptest = "1.4"`

**Uso**:
- Property-based testing
- Fuzzing inputs
- Edge case discovery

---

## 15. Error Handling

### 15.1 Thiserror

**Crate**: `thiserror = "1.0"`

**Uso**:
- Derivar Error trait
- Error messages customizados

**Exemplo**:
```rust
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid RAW file")]
    InvalidRaw,
    
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
}
```

### 15.2 Anyhow (Opcional)

**Crate**: `anyhow = "1.0"`

**Uso**:
- Error handling simplificado para aplicação
- Context adicional
- Backtraces

---

## 16. Build e Distribuição

### 16.1 Cargo

**Ferramentas**:
- `cargo build --release` - Build otimizado
- `cargo clippy` - Linter
- `cargo fmt` - Formatter
- `cargo test` - Test runner

### 16.2 Cross-Compilation

**Targets**:
```toml
# .cargo/config.toml
[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-mmacosx-version-min=10.15"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-mmacosx-version-min=11.0"]
```

### 16.3 Bundlers

**macOS**:
- `cargo-bundle` - Criar .app bundle
- `create-dmg` - Criar instalador DMG

**Windows**:
- WiX Toolset - MSI installer
- Inno Setup - EXE installer

---

## 17. CI/CD

### GitHub Actions

**Workflows**:

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches-ignore:
      - dev
  pull_request:
    branches-ignore:
      - dev

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - run: cargo test --all-features
      - run: cargo clippy -- -D warnings
```

**Nota**: CI/CD não executa na branch `dev` para permitir desenvolvimento experimental sem overhead de testes automáticos.

---

## 18. Dependências Completas

### Cargo.toml Principal

```toml
[package]
name = "vintage-lightbox"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[workspace]
members = [
    "crates/vintage-core",
    "crates/vintage-raw",
    "crates/vintage-ui",
    "crates/vintage-import",
    "crates/vintage-export",
]

[dependencies]
# UI
slint = "1.3"

# Image Processing
image = "0.24"
imageproc = "0.23"
fast_image_resize = "3.0"
rawler = "0.6"

# Color Management
lcms2 = "6.0"

# Metadata
kamadak-exif = "0.5"

# Database
rusqlite = { version = "0.30", features = ["bundled", "backup"] }

# Async/Concurrency
tokio = { version = "1.35", features = ["full"] }
rayon = "1.8"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# File System
walkdir = "2.4"
notify = "6.1"

# Caching
lru = "0.12"

# Hashing
blake3 = "1.5"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

# Utilities
chrono = "0.4"
uuid = { version = "1.6", features = ["v4"] }

[dev-dependencies]
criterion = "0.5"
proptest = "1.4"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true

[profile.dev]
opt-level = 1  # Faster dev builds
```

---

## 19. Requisitos do Sistema

### Desenvolvimento

**macOS**:
- macOS 10.15+ (Catalina)
- Xcode Command Line Tools
- Homebrew (para dependências)

**Windows**:
- Windows 10+
- Visual Studio Build Tools 2019+
- MSVC toolchain

**Ambos**:
- Rust 1.75+
- 8GB RAM mínimo
- 10GB espaço em disco

### Runtime (Usuário Final)

**macOS**:
- macOS 10.15+
- 4GB RAM mínimo (8GB recomendado)
- 500MB espaço em disco

**Windows**:
- Windows 10+
- 4GB RAM mínimo (8GB recomendado)
- 500MB espaço em disco

---

## 20. Estrutura de Projeto

```
VintageLightbox/
├── Cargo.toml                 # Workspace root
├── Cargo.lock
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── crates/
│   ├── vintage-core/          # Core domain logic
│   ├── vintage-raw/           # RAW processing
│   ├── vintage-ui/            # Slint UI
│   ├── vintage-import/        # Import module
│   ├── vintage-export/        # Export module
│   └── vintage-database/      # Database layer
├── assets/
│   ├── icons/
│   ├── presets/
│   └── icc-profiles/
├── docs/
│   ├── 01-REQUISITOS.md
│   ├── 02-ARQUITETURA.md
│   ├── 03-FUNCIONALIDADES.md
│   ├── 04-ROADMAP.md
│   └── 05-STACK-TECNOLOGICO.md
├── tests/
│   ├── integration/
│   └── fixtures/              # Sample images for tests
├── benches/                   # Benchmarks
└── README.md
```

---

## 21. Alternativas Consideradas

### UI Frameworks
- **egui**: Immediate mode, mais simples mas menos nativo
- **Iced**: Declarativo, inspirado em Elm
- **Tauri**: Web-based (HTML/CSS/JS), mais pesado
- **GTK-rs**: Bindings para GTK, complexo
- **Qt for Rust**: Experimental, bindings incompletos

**Escolha**: Slint - melhor balanço nativo/declarativo

### RAW Processing
- **dcraw**: Antigo, C, difícil de integrar
- **LibRaw**: Maduro, muitos formatos, C++
- **rawler**: Rust puro, menos formatos
- **Fazer do zero**: Muito complexo

**Escolha**: rawler + LibRaw como fallback

---

## 22. Considerações de Segurança

### Memory Safety
- ✅ Rust garante por design
- Validação de inputs de usuário
- Bounds checking automático

### File System
- Sanitização de paths
- Validação de permissões
- Limites de tamanho de arquivo

### Database
- Prepared statements (SQL injection protection)
- Validação de schema
- Backups regulares

### Atualizações
- HTTPS para updates
- Signature verification
- Rollback em caso de falha

---

## 23. Licenciamento

### Aplicação
- **Recomendado**: GPL-3.0 ou MIT
- Depende da estratégia de monetização

### Dependências
Verificar compatibilidade:
- Slint: GPL-3.0 ou Commercial
- LibRaw: LGPL-2.1 ou Commercial
- SQLite: Public Domain
- Outros crates: Maioria MIT/Apache-2.0

**Nota**: Se usar Slint com GPL, aplicação deve ser GPL também.

---

## 24. Recursos de Aprendizado

### Rust
- The Rust Book: https://doc.rust-lang.org/book/
- Rust by Example: https://doc.rust-lang.org/rust-by-example/

### Slint
- Slint Documentation: https://slint.dev/docs
- Slint Examples: https://github.com/slint-ui/slint/tree/master/examples

### Image Processing
- Digital Image Processing (Gonzalez & Woods)
- rawler documentation
- LibRaw API docs

### RAW Photography
- Understanding Digital RAW Capture
- Color Science for Digital Photography

---

## 25. Próximos Passos Técnicos (Abordagem TDD)

### 1. Setup Inicial com Clean Architecture

```bash
# Criar workspace
cargo new --bin vintage-lightbox
cd vintage-lightbox

# Criar crates seguindo Clean Architecture
cargo init --lib crates/domain              # Camada 1: Entities
cargo init --lib crates/use-cases           # Camada 2: Application Business Rules
cargo init --lib crates/adapters            # Camada 3: Interface Adapters
cargo init --lib crates/infrastructure      # Camada 4: Frameworks & Drivers

# Configurar workspace
cat > Cargo.toml << 'EOF'
[workspace]
members = [
    "crates/domain",
    "crates/use-cases",
    "crates/adapters",
    "crates/infrastructure",
]

[workspace.dependencies]
# Dependências compartilhadas
serde = { version = "1.0", features = ["derive"] }
thiserror = "1.0"

# Ferramentas de teste
mockall = "0.12"
proptest = "1.4"
criterion = "0.5"
insta = "1.34"
fake = "2.9"
tempfile = "3.8"
EOF
```

### 2. Primeiro Ciclo TDD: Rating de Fotos

**Passo 1: 🔴 RED - Escrever o teste primeiro**

```bash
# crates/domain/src/entities/photo.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_photo_with_valid_rating() {
        // Arrange
        let mut photo = Photo::new_test();
        let rating = Rating::Five;
        
        // Act
        let result = photo.rate(rating);
        
        // Assert
        assert!(result.is_ok());
        assert_eq!(photo.rating(), Some(Rating::Five));
    }
}

# Rodar teste (vai falhar - RED)
cargo test -p domain
```

**Passo 2: 🟢 GREEN - Implementar código mínimo**

```rust
impl Photo {
    pub fn rate(&mut self, rating: Rating) -> Result<(), DomainError> {
        self.rating = Some(rating);
        Ok(())
    }
}

# Rodar teste novamente (deve passar - GREEN)
cargo test -p domain
```

**Passo 3: 🔵 REFACTOR - Melhorar código**

```rust
// Adicionar validação, melhorar design
impl Photo {
    pub fn rate(&mut self, rating: Rating) -> Result<(), DomainError> {
        rating.validate()?;
        self.rating = Some(rating);
        self.emit_event(DomainEvent::PhotoRated { 
            photo_id: self.id, 
            rating 
        });
        Ok(())
    }
}

# Testes ainda passam após refatoração
cargo test -p domain
```

### 3. Configurar CI/CD com TDD

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - name: Run tests
        run: cargo test --all-features
      - name: Check coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --all-features --workspace --out Xml
      - name: Upload coverage
        uses: codecov/codecov-action@v3

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run clippy
        run: cargo clippy -- -D warnings

  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Check formatting
        run: cargo fmt -- --check
```

### 4. Roteiro de Desenvolvimento TDD

**Semana 1-2: Domain Layer**
- ✅ TDD: Entities (Photo, Collection, Adjustment)
- ✅ TDD: Value Objects (Rating, ColorLabel, PhotoId)
- ✅ TDD: Domain Services
- ✅ Meta: 100% cobertura de testes

**Semana 3-4: Use Cases Layer**
- ✅ TDD: ImportPhotosUseCase com mocks
- ✅ TDD: RatePhotoUseCase
- ✅ TDD: ApplyAdjustmentUseCase
- ✅ Meta: ≥95% cobertura

**Semana 5-6: Infrastructure Layer**
- ✅ Testes de integração: SQLite Repository
- ✅ Testes de integração: File System
- ✅ Testes de integração: RAW Decoder
- ✅ Meta: ≥70% cobertura + todos os testes de integração passando

**Semana 7-8: Adapters + UI**
- ✅ Controllers com testes
- ✅ Presenters com testes de snapshot
- ✅ Integração Slint (testes manuais + E2E)

### 5. Checklist de Qualidade

Antes de cada commit:
```bash
# 1. Rodar todos os testes
cargo test --workspace

# 2. Verificar cobertura
cargo tarpaulin --workspace

# 3. Rodar linter
cargo clippy --all-targets --all-features -- -D warnings

# 4. Verificar formatação
cargo fmt --all -- --check

# 5. Rodar benchmarks (se mudou código de performance)
cargo bench

# 6. Verificar documentação
cargo doc --no-deps --workspace
```

Script automatizado:
```bash
#!/bin/bash
# scripts/pre-commit.sh
set -e

echo "🧪 Running tests..."
cargo test --workspace

echo "📊 Checking coverage..."
cargo tarpaulin --workspace --ignore-tests

echo "📎 Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

echo "🎨 Checking formatting..."
cargo fmt --all -- --check

echo "✅ All checks passed!"
```

---

## Conclusão

O stack escolhido, combinado com **Clean Architecture** e **TDD**, oferece:

- ✅ **Performance**: Rust + otimizações adequadas
- ✅ **Segurança**: Memory safety + validações + testes abrangentes
- ✅ **Cross-platform**: Funciona nativamente em macOS e Windows
- ✅ **Manutenibilidade**: Código limpo, modular, testável, bem documentado
- ✅ **Qualidade**: TDD garante código correto desde o início
- ✅ **Arquitetura**: Clean Architecture permite evolução independente de camadas
- ✅ **Testabilidade**: 100% das regras de negócio testadas isoladamente
- ✅ **Ecossistema**: Crates de qualidade + ferramentas de teste de primeira classe
- ✅ **Futuro**: Tecnologias em desenvolvimento ativo
- ✅ **Refatoração Segura**: Testes garantem que mudanças não quebram funcionalidades

**Princípios Seguidos**:
- 🏗️ Clean Architecture: Separação clara de responsabilidades
- 🧪 TDD: Testes primeiro, código depois
- 📐 SOLID: Design de qualidade
- ♻️ Refatoração Contínua: Sempre melhorando
- 🎯 Domain-Driven Design: Foco nas regras de negócio

Este stack e metodologia são sólidos o suficiente para construir uma aplicação profissional competitiva e sustentável a longo prazo.
