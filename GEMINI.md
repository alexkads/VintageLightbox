# VintageLightbox Context for Gemini

## Project Overview
**VintageLightbox** is a professional RAW photo management and editing application (an Adobe Lightroom clone) built in **Rust**. It is designed for cross-platform use (macOS, Windows) and prioritizes performance, non-destructive editing, and a clean user interface.

## Architecture
The project strictly follows **Clean Architecture** and **Test-Driven Development (TDD)** principles. The codebase is organized as a Rust Workspace with four main layers, ensuring separation of concerns and testability.

### Layers (Dependency Rule: Inner layers never depend on outer layers)
1.  **Domain (`crates/domain`)**:
    *   **Role**: The core business logic and entities.
    *   **Dependencies**: None (Pure Rust).
    *   **Contents**: Entities (`Photo`, `Collection`), Value Objects (`Rating`, `PhotoId`, `ColorLabel`), Repository Traits (interfaces), and Domain Errors.
    *   **Coverage**: 100% test coverage required.

2.  **Use Cases (`crates/use-cases`)**:
    *   **Role**: Application specific business rules and orchestration.
    *   **Dependencies**: Depends only on `domain`.
    *   **Contents**: Interactors like `ImportPhotoUseCase`, `SavePhotoEditsUseCase`. Implements business logic using domain objects and repository interfaces.

3.  **Adapters (`crates/adapters`)**:
    *   **Role**: Interface Adapters. Converts data between the format most convenient for the use cases and entities, and the format most convenient for some external agency (Web, UI, DB).
    *   **Dependencies**: Depends on `use-cases` and `domain`.
    *   **Contents**: Controllers (`LibraryController`, `EditorController`), Presenters, View Models.

4.  **Infrastructure (`crates/infrastructure`)**:
    *   **Role**: Frameworks and Drivers. Details of the system.
    *   **Dependencies**: Depends on `adapters`, `use-cases`, and `domain`.
    *   **Contents**: SQLite Database implementation (`sqlx`), File System operations, RAW processing (`LibRaw`/`rawler`), `egui` integration code.

5.  **UI (`crates/ui`)**:
    *   **Role**: The user interface layer.
    *   **Dependencies**: Uses `adapters` to interact with the system.
    *   **Contents**: Main application entry point (`main.rs`, `app.rs`), `egui` components, views (`LibraryView`, `DevelopView`), and state management.

## Key Technologies
*   **Language**: Rust
*   **UI Framework**: `egui` (0.28+) with `eframe`
*   **Database**: SQLite with `sqlx` (async)
*   **RAW Processing**: `LibRaw` / `rawler`
*   **Testing**: `mockall` (mocking), `proptest` (property testing), `egui_kittest` (E2E)

## Development Conventions
*   **Strict TDD**: All features must be implemented using the **Red-Green-Refactor** cycle. Write the test first.
*   **Async-First**: Repository traits and implementations are async (`async_trait`).
*   **Dependency Injection**: Use cases receive abstract repository implementations (traits) via their constructors.
*   **Code Style**: Follow standard Rust idioms (`rustfmt`, `clippy`).

## Common Commands

### Building and Running
*   **Build Workspace**: `cargo build --workspace`
*   **Run Application**: `cargo run -p ui`
*   **Check Code**: `cargo check --workspace`

### Testing
*   **Run All Tests**: `cargo test --workspace`
*   **Run Specific Crate Tests**: `cargo test -p domain` (or `use-cases`, `infrastructure`, `ui`)
*   **Run Single Test**: `cargo test <test_name> --workspace`
*   **Update Snapshots**: `UPDATE_SNAPSHOTS=true cargo test -p ui`

### Maintenance
*   **Format Code**: `cargo fmt --all`
*   **Lint Code**: `cargo clippy --workspace`

## Database
*   **Migration Management**: `sqlx` migrations are located in `crates/infrastructure/migrations/`.
*   **Local DB File**: `vintage_lightbox.db`

## Documentation References
*   `CLAUDE.md`: Quick reference for commands and architecture.
*   `docs/02-ARQUITETURA.md`: Detailed architectural guidelines and TDD examples.
*   `docs/04-ROADMAP.md`: Project status and future plans.
