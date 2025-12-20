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
cargo run -p ui

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
│  ui (Slint)                                         │
│  - main.rs: Application entry, UI callbacks         │
│  - ui/main.slint: UI definitions                    │
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

- **Slint 1.8.0** with `renderer-winit-femtovg`
- UI definitions in `crates/ui/ui/main.slint`
- Build script: `crates/ui/build.rs` compiles `.slint` files
- Async file dialogs via `rfd` crate
- Image processing offloaded from main thread via debounced background processor

## Adding New Features

1. Define domain entities/value objects in `domain` with tests
2. Create use case in `use-cases` that depends only on domain traits
3. Implement infrastructure (repository, file system) in `infrastructure`
4. Add controller in `adapters` to bridge use case and UI
5. Wire up in `ui/src/main.rs` and add UI elements to `.slint` file
