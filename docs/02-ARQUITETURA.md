# Arquitetura do Sistema - VintageLightbox

## 1. Visão Geral da Arquitetura

VintageLightbox segue os princípios da **Clean Architecture** (Arquitetura Limpa) proposta por Robert C. Martin, combinada com práticas de **Test-Driven Development (TDD)**. Esta abordagem garante:

- ✅ **Independência de Frameworks**: A lógica de negócio não depende de egui ou outras bibliotecas externas
- ✅ **Testabilidade**: Todas as camadas são facilmente testáveis de forma isolada
- ✅ **Independência de UI**: A interface pode ser substituída sem afetar regras de negócio
- ✅ **Independência de Banco de Dados**: SQLite pode ser trocado por outra solução
- ✅ **Princípios SOLID**: Código modular, extensível e de fácil manutenção

### Camadas da Clean Architecture

```
┌───────────────────────────────────────────────────────────────┐
│              UI (Camada 5 - Presentation)                     │
│   - egui Views, Components, App State                         │
│   - Sem lógica de negócio, apenas visualização e eventos      │
├───────────────────────────────────────────────────────────────┤
│              Frameworks & Drivers (Camada 4)                  │
│   - SQLite, LibRaw, Sistema de Arquivos                       │
│   - Implementações concretas de Repositories e Gateways       │
├───────────────────────────────────────────────────────────────┤
│           Interface Adapters (Camada 3)                       │
│   - Controllers, Presenters, View Models                      │
│   - Converte dados para formato conveniente para UI e DB      │
├───────────────────────────────────────────────────────────────┤
│              Use Cases (Camada 2)                             │
│   - Regras de negócio específicas da aplicação               │
│   - Orquestração de fluxos de trabalho                       │
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

## 2. Estrutura de Módulos (Clean Architecture)

A estrutura de diretórios reflete as camadas da Clean Architecture, com a UI separada em seu próprio crate para garantir o desacoplamento.

```
crates/
├── domain/              # Camada 1: Entities (Core Domain)
├── use-cases/           # Camada 2: Application Business Rules
├── adapters/            # Camada 3: Interface Adapters
├── infrastructure/      # Camada 4: Frameworks & Drivers
└── ui/                  # Camada 5: User Interface (egui)
```

### 2.1 Domain Layer (Camada 1 - Entities)

**Responsabilidade**: Regras de negócio empresariais puras, independentes de qualquer framework ou tecnologia.

#### `crates/domain/` ✅ **IMPLEMENTADO**

Módulo com entidades e lógica de domínio central.

```rust
domain/
├── entities/
│   ├── photo.rs          // Entidade Photo
│   ├── collection.rs     // Entidade Collection
│   └── ...
├── value_objects/
│   ├── rating.rs         // Value Objects (básicos e imutáveis)
│   ├── photo_id.rs
│   ├── crop_settings.rs  // Configurações de corte
│   └── ...
├── repositories.rs       // Repository Traits (Interfaces abstratas)
└── errors.rs             // Erros de domínio
```

### 2.2 Use Cases Layer (Camada 2 - Application Business Rules)

**Responsabilidade**: Orquestração de fluxos de trabalho e regras de negócio específicas da aplicação.

#### `crates/use-cases/` 🚧 **EM ANDAMENTO**

Define *o que* o sistema faz.

```rust
use-cases/
├── import/
│   ├── import_photo.rs
│   └── ...
├── edit/
│   ├── save_edit.rs
│   └── ...
└── ...
```

### 2.3 Interface Adapters Layer (Camada 3)

**Responsabilidade**: Adaptar dados entre Use Cases e Frameworks/Drivers. Controllers recebem input da UI e chamam Use Cases. Presenters formatam output dos Use Cases para a UI.

#### `crates/adapters/`

```rust
adapters/
├── controllers/          // Recebem ações da UI
├── presenters/           // Preparam dados para exibição
├── gateways/             // Implementações abstratas de acesso a dados
└── models/               // Modelos de dados adaptados
```

### 2.4 Infrastructure Layer (Camada 4 - Frameworks & Drivers)

**Responsabilidade**: Detalhes de implementação técnica, banco de dados, acesso a sistema de arquivos, processamento de imagem de baixo nível.

#### `crates/infrastructure/`

```rust
infrastructure/
├── database/             // Implementação SQLite (sqlx)
├── file_system/          // Operações de disco
├── image_processing/     // Wrapper para LibRaw/ImageMagick
├── native_dialog/        // Diálogos de sistema
└── ...
```

### 2.5 UI Layer (Camada 5 - Presentation)

**Responsabilidade**: Renderização da interface gráfica usando **egui**. Esta camada deve ser **MUITO BURRA**. Não deve conter regras de negócio, Apenas lógica de visualização e captura de eventos.

#### `crates/ui/` (Atualizado para egui)

```rust
ui/
├── src/
│   ├── main.rs           // Entry point
│   ├── app.rs            // Loop principal da aplicação
│   ├── state.rs          // Estado global da UI (ViewState)
│   ├── views/            // Telas principais (Library, Develop)
│   ├── components/       // Widgets reutilizáveis (PhotoGrid, Toolbar)
│   ├── panels/           // Painéis laterais
│   ├── async_loader.rs   // Carregamento assíncrono de imagens p/ UI
│   └── image_processing.rs // Processamento visual simples p/ UI (crop display)
```

**Regras Críticas da UI**:
1.  **Zero Lógica de Negócio**: Validações e regras devem estar no Domain/Use Cases.
2.  **PreviewManager**: A UI nunca carrega arquivos diretamente do disco (`std::fs`). Ela solicita imagens ao `PreviewManager` (via `infrastructure` ou `adapters`).
3.  **State Management**: O estado da UI (`AppState`) armazena apenas o necessário para a visualização (seleção atual, modo de visualização, zoom). O estado persistente dos dados vive no Banco de Dados.

## 3. Fluxo de Dados

### 3.1 Edição e Persistência

```
1. [UI] Usuário move slider de exposição
2. [UI] Evento atualiza `PhotoEdits` temporário na memória
3. [UI] `ImageProcessor` atualiza preview na tela (GPU/Async)
4. [UI] Usuário clica em "Salvar" ou troca de foto
5. [UI] Chama `EditorController.save_edits(edits)`
6. [Adapters] Controller converte `PhotoEdits` para Input Model
7. [Use Cases] `SavePhotoEditsUseCase` valida e orquestra
8. [Domain] Entidade `Photo` é atualizada
9. [Infrastructure] `SqlitePhotoRepository` salva no DB
```

### 3.2 Processamento de Imagem Otimizado

Para performance (60fps), o processamento pesado é separado:

1.  **Thumbnails/Previews**: Gerados no `infrastructure` e cacheados.
2.  **Ajustes em Tempo Real**: Aplicados via GPU (shaders) ou processamento leve na UI para feedback instantâneo.
3.  **Exportação**: Processamento Full-Res acontece em background no `infrastructure`.

## 4. Tecnologias Chave

*   **Linguagem**: Rust
*   **UI Framework**: `egui` (Immediate Mode GUI)
*   **Database**: SQLite com `sqlx` (async)
*   **RAW Processing**: `LibRaw` / `rawler`
*   **GPU Acceleration**: `wgpu` / `vulkan` (via egui/infrastructure)
*   **Testing**: `mockall`, `proptest`, `egui_kittest`

## 5. Padrões Adotados

*   **Repository Pattern**: Para abstração de banco de dados.
*   **Dependency Injection**: Use Cases recebem traits, não structs concretas.
*   **Observer/Event Bus**: Para comunicação desacoplada entre modulos (ex: atualização de progresso).
*   **Async/Await**: I/O non-blocking em todo o sistema.
