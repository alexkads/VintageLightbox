# Stack Tecnológico - VintageLightbox

## Visão Geral

Este documento detalha todas as tecnologias, bibliotecas e ferramentas utilizadas no desenvolvimento do VintageLightbox.

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

### Slint UI 1.x

**Características**:
- ✅ Declarativa e reativa (similar a QML/SwiftUI)
- ✅ Nativa e performática
- ✅ Cross-platform (macOS, Windows, Linux)
- ✅ Suporte a HiDPI/Retina
- ✅ Temas customizáveis
- ✅ Hot-reload durante desenvolvimento

**Exemplo de Código**:
```slint
component PhotoGrid {
    in property <[PhotoItem]> photos;
    
    GridView {
        for photo in photos: Rectangle {
            Image {
                source: photo.thumbnail;
            }
        }
    }
}
```

**Website**: https://slint.dev/

**Alternativa (Plano B)**: egui - se Slint não atender requisitos

---

## 3. Processamento de Imagens RAW

### 3.1 LibRaw (via Rust binding)

**Crate**: `libraw-rs` ou FFI customizado

**Características**:
- ✅ Suporta 90+ formatos RAW
- ✅ Maduro e battle-tested
- ✅ Usado por DarkTable, RawTherapee
- ✅ Extração de thumbnail embutido
- ✅ Metadados completos

**Instalação Nativa**:
- macOS: `brew install libraw`
- Windows: Compilar ou usar binários pré-compilados

### 3.2 Rawler (Rust puro)

**Crate**: `rawler`

**Características**:
- ✅ 100% Rust (sem dependências C)
- ✅ Rápido e seguro
- ✅ Menos formatos que LibRaw
- ✅ Mais fácil de distribuir

**Decisão**: Começar com rawler, avaliar LibRaw se precisar de mais formatos.

---

## 4. Processamento de Imagens

### 4.1 Image Crate

**Crate**: `image = "0.24"`

**Uso**:
- Encoding/decoding JPEG, PNG, TIFF
- Redimensionamento
- Conversões de formato de pixel
- Operações básicas de imagem

### 4.2 ImageProc

**Crate**: `imageproc = "0.23"`

**Uso**:
- Filtros (blur, sharpen)
- Transformações
- Operações morfológicas
- Detecção de features

### 4.3 Fast Image Resize

**Crate**: `fast_image_resize = "3.0"`

**Uso**:
- Redimensionamento high-quality e rápido
- Múltiplos algoritmos (Lanczos, Mitchell, etc.)
- SIMD optimizations

---

## 5. Gerenciamento de Cores

### Little CMS 2 (LCMS2)

**Crate**: `lcms2 = "6.0"` (binding)

**Características**:
- ✅ Engine de gerenciamento de cor profissional
- ✅ Suporta ICC profiles
- ✅ Conversões de espaço de cor precisas
- ✅ Usado por Photoshop, GIMP, etc.

**Perfis Incluídos**:
- sRGB IEC61966-2.1
- Adobe RGB (1998)
- ProPhoto RGB
- Display P3

---

## 6. Metadados

### 6.1 Kamadak-exif

**Crate**: `kamadak-exif = "0.5"`

**Uso**:
- Leitura de metadados EXIF
- Suporta TIFF, JPEG, RAW (via TIFF headers)
- Acesso a tags padrão e customizadas

### 6.2 XMP Toolkit (Opcional)

**Crate**: Binding customizado ou `xmp-toolkit-rs`

**Uso**:
- Leitura/escrita de sidecar XMP
- Sincronização de metadados
- Histórico de edições

---

## 7. Banco de Dados

### SQLite via Rusqlite

**Crate**: `rusqlite = "0.30"`

**Características**:
- ✅ Embarcado (sem servidor)
- ✅ Zero-configuration
- ✅ ACID compliant
- ✅ Rápido para leitura
- ✅ Cross-platform

**Features Usadas**:
- Bundled (SQLite compilado junto)
- Backup API
- Full-Text Search (FTS5)
- JSON1 extension

**Schema Base**:
```sql
CREATE TABLE photos (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    file_hash TEXT,
    import_date DATETIME,
    capture_date DATETIME,
    camera TEXT,
    lens TEXT,
    iso INTEGER,
    aperture REAL,
    shutter_speed TEXT,
    focal_length REAL,
    width INTEGER,
    height INTEGER,
    rating INTEGER DEFAULT 0,
    color_label INTEGER,
    pick_flag INTEGER DEFAULT 0, -- 0: none, 1: pick, -1: reject
    is_edited BOOLEAN DEFAULT 0,
    is_purchased BOOLEAN DEFAULT 0
);

CREATE TABLE adjustments (
    id INTEGER PRIMARY KEY,
    photo_id INTEGER NOT NULL,
    exposure REAL DEFAULT 0,
    contrast REAL DEFAULT 0,
    temperature INTEGER DEFAULT 5500,
    tint REAL DEFAULT 0,
    highlights REAL DEFAULT 0,
    shadows REAL DEFAULT 0,
    -- ... outros ajustes
    FOREIGN KEY (photo_id) REFERENCES photos(id)
);
```

---

## 8. Concorrência e Paralelização

### 8.1 Rayon

**Crate**: `rayon = "1.8"`

**Uso**:
- Data parallelism
- Geração de thumbnails em paralelo
- Processamento batch
- Fork-join parallelism

**Exemplo**:
```rust
use rayon::prelude::*;

photos.par_iter()
    .map(|photo| generate_thumbnail(photo))
    .collect()
```

### 8.2 Tokio (Async Runtime)

**Crate**: `tokio = { version = "1.35", features = ["full"] }`

**Uso**:
- I/O assíncrono (file system)
- Background tasks
- Event loop
- Timers

**Características**:
- Multi-threaded runtime
- Work-stealing scheduler
- Async file I/O

---

## 9. Serialização e Configuração

### 9.1 Serde

**Crate**: 
```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

**Uso**:
- Serialização de presets
- Configurações da aplicação
- Comunicação entre módulos
- Formato de dados estruturados

### 9.2 Toml (Configuração)

**Crate**: `toml = "0.8"`

**Uso**:
- Arquivo de configuração do usuário
- Settings e preferências

**Exemplo**:
```toml
# config.toml
[cache]
max_size_mb = 2048
location = "~/Library/Caches/VintageLightbox"

[performance]
thumbnail_threads = 4
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

## 25. Próximos Passos Técnicos

1. **Setup Inicial**
   ```bash
   cargo new --bin vintage-lightbox
   cd vintage-lightbox
   cargo init --lib crates/vintage-core
   cargo init --lib crates/vintage-raw
   # ...
   ```

2. **Primeiro Protótipo**
   - Janela Slint básica
   - Carregar um arquivo RAW
   - Aplicar ajuste de exposição
   - Exibir resultado

3. **Validação Tecnológica**
   - Benchmark de performance
   - Testes de compatibilidade
   - Validação de UX

---

## Conclusão

O stack escolhido oferece:
- ✅ **Performance**: Rust + otimizações adequadas
- ✅ **Segurança**: Memory safety + validações
- ✅ **Cross-platform**: Funciona nativamente em macOS e Windows
- ✅ **Manutenibilidade**: Código limpo, modular, testável
- ✅ **Ecossistema**: Crates de qualidade disponíveis
- ✅ **Futuro**: Tecnologias em desenvolvimento ativo

Este stack é sólido o suficiente para construir uma aplicação profissional competitiva.
