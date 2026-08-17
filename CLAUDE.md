# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Test Commands

```bash
# Build the entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p domain
cargo test -p use-cases
cargo test -p infrastructure

# Run a single test by name
cargo test test_name --workspace

# Run the application
cargo run -p ui-gpui

# Check code (faster than build)
cargo check --workspace

# Format and lint
cargo fmt --all
cargo clippy --workspace
```

## Architecture

VintageLightbox follows **Clean Architecture** with 4 layers as separate crates:

```
┌─────────────────────────────────────────────────────┐
│  ui-gpui (GPUI)                                     │
│  - main.rs: entrada, tema, teclas, janela           │
│  - app.rs: a raiz — qual tela está no ar            │
│  - biblioteca/ revelacao/ importacao/ impressao/    │
│  - cliente.rs: a segunda tela; configuracoes.rs     │
├─────────────────────────────────────────────────────┤
│  adapters                                           │
│  - controllers/: ImportController, EditorController │
│  - presenters.rs, view_models.rs                    │
├─────────────────────────────────────────────────────┤
│  use-cases                                          │
│  - Business logic orchestration                     │
│  - ImportPhotoUseCase, SavePhotoEditsUseCase, etc.  │
├─────────────────────────────────────────────────────┤
│  infrastructure                                     │
│  - database/: SQLite repositories (sqlx)            │
│  - exif_reader.rs, thumbnail_generator.rs           │
│  - raw_processing.rs, image_exporter.rs             │
│  - migrations/: SQL migration files                 │
├─────────────────────────────────────────────────────┤
│  domain (innermost - no external dependencies)      │
│  - entities/: Photo, Collection                     │
│  - value_objects/: PhotoId, Rating, ColorLabel      │
│  - repositories.rs: Trait definitions               │
│  - errors.rs: DomainError, DomainResult             │
└─────────────────────────────────────────────────────┘
```

**Dependency Rule**: Inner layers never depend on outer layers. The `domain` crate has no dependencies on other crates; `use-cases` depends only on `domain`; etc.

## Key Patterns

- **Repository Pattern**: Traits defined in `domain/src/repositories.rs`, implemented in `infrastructure/src/database/`
- **Async-First**: All repository methods use `async_trait` and `async fn`
- **TDD**: Tests use `mockall` for mocking repositories, `proptest` for property-based testing
- **Dependency Injection**: Use cases receive repository implementations via constructor

## Database

- SQLite with `sqlx` (runtime-tokio-native-tls)
- Migrations in `crates/infrastructure/migrations/`
- Uses `sqlx::migrate!` macro for embedded migrations
- Local database file: `vintage_lightbox.db`

## UI Framework

**GPUI 0.2 + gpui-component 0.5** — o egui saiu em 17/ago/2026, com a migração
concluída (`docs/10-MIGRACAO-GPUI.md`). Quem procura o app antigo o encontra no
histórico do git; o que ele fazia está listado, comportamento a comportamento,
em `docs/PARIDADE-UI.md`.

- As telas ficam em `crates/ui-gpui/src/{biblioteca,revelacao,importacao,impressao}/`
- As duas telas grandes vivem num **dock**: os painéis se arrastam e se
  redimensionam, e o arranjo é gravado ao lado do catálogo (`arranjo-*.json`)
- Imagem: `DynamicImage → RgbaImage → Frame → RenderImage` (`imagem.rs`)
  ⚠️ **em BGRA** — o GPUI espera essa ordem e o crate `image` produz RGBA
- Seletor de arquivos nativo via `rfd`; o motor de revelação é wgpu próprio,
  numa thread de fundo (`revelacao/processador.rs`)
- Teclas: as da raiz em `app.rs`, e sempre com contexto — ligação sem `!Input`
  come a letra de quem está digitando na busca

⚠️ **Toda medida de desempenho é em `--release`**: em `debug` uma miniatura
custa 56× mais, e a fase 1 quase condenou o framework por medir no perfil
errado. As réguas estão em `cargo run --release -p ui-gpui --bin medir-miniaturas`
e `--bin medir-abertura`.

## Adding New Features

1. Define domain entities/value objects in `domain` with tests
2. Create use case in `use-cases` that depends only on domain traits
3. Implement infrastructure (repository, file system) in `infrastructure`
4. Add controller in `adapters` to bridge use case and UI
5. Wire up in `ui/src/app.rs` and create/update UI components in `components/` or `views/`
