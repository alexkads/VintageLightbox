# VintageLightbox 📷

Clone profissional do Adobe Lightroom desenvolvido em Rust com interface egui.

**Status**: ✅ MVP Completo | **Testes**: 360+ | **Data**: 31 de dezembro de 2025

## 📸 Sobre o Projeto

VintageLightbox é uma aplicação multiplataforma de gerenciamento e edição de fotos RAW, projetada para fotógrafos profissionais que precisam de:

- 📥 **Importação eficiente** de grandes volumes de fotos com detecção de duplicatas
- 🎨 **Edição não-destrutiva** com 13 ajustes GPU-accelerated em tempo real
- 🖥️ **Interface moderna** com 5 temas e atalhos de teclado profissionais
- ⭐ **Organização avançada** com classificação, flags, cores e coleções
- 💾 **Presets personalizados** para workflow consistente
- 📤 **Exportação otimizada** para JPEG/PNG/TIFF
- 🖨️ **Sistema de impressão** com layouts variados

## 🏗️ Arquitetura e Metodologia

Este projeto segue **Clean Architecture** e **Test-Driven Development (TDD)**:

```
┌─────────────────────────────────────────────────────┐
│  UI Layer (egui 0.31)                               │
│  Render-only (sem regras de negócio) • 4 Views • 25+ Components • 5 Temas │
├─────────────────────────────────────────────────────┤
│  Infrastructure (wgpu • SQLite • LibRaw)            │
│  GPU Compute • Multi-level Cache • 48 testes        │
├─────────────────────────────────────────────────────┤
│  Adapters (Controllers • Presenters • State)        │
│  6 Controllers • EditHistory • 27 testes            │
├─────────────────────────────────────────────────────┤
│  Use Cases (20+ casos de uso)                       │
│  Import • Edit • Organize • Export • 65 testes      │
├─────────────────────────────────────────────────────┤
│  Domain (Entities • Value Objects • Traits)         │
│  4 Entities • 15 Value Objects • 220 testes         │
└─────────────────────────────────────────────────────┘
```

## 🚀 Tecnologias

| Categoria | Tecnologia | Versão |
|-----------|------------|--------|
| **Linguagem** | Rust | 1.75+ |
| **UI Framework** | egui | 0.31 |
| **GPU Compute** | wgpu | 24.0 |
| **Database** | SQLite + rusqlite | 0.32 |
| **RAW Processing** | LibRaw / rawler | - |
| **Image Codecs** | image-rs | 0.25 |
| **Testing** | mockall, proptest | - |

## 🎯 Status do Desenvolvimento

**Fase Atual**: MVP Completo ✅

### Camadas Implementadas

| Camada | Status | Testes |
|--------|--------|--------|
| Domain | ✅ Completo | 220 |
| Use Cases | ✅ Completo | 65 |
| Infrastructure | ✅ Completo | 48 |
| Adapters | ✅ Completo | 27 |
| UI | ✅ Completo | - |
| **Total** | **✅** | **360+** |

### Features Implementadas

- [x] **Value Objects**: Rating, PhotoId, ColorLabel, Flag, PhotoEdits, CropSettings, etc.
- [x] **Entities**: Photo, Collection, Preset, PrintJob
- [x] **20+ Use Cases**: Import, Edit, Organize, Export, Print, Presets
- [x] **6 Controllers**: Library, Editor, Photo, Import, Export, Preset
- [x] **GPU Processing**: 13 ajustes em single-pass shader (~2ms para 4K)
- [x] **Multi-level Cache**: L0 (0.01ms) → L1 (15 imgs) → L2 (SQLite BLOB)
- [x] **UI Completa**: 4 views, 25+ componentes, 5 temas

## 🧪 Rodando os Testes

```bash
# Todos os testes
cargo test --workspace

# Por camada
cargo test -p domain        # 220 testes
cargo test -p use-cases     # 65 testes
cargo test -p infrastructure # 48 testes
cargo test -p adapters      # 27 testes

# Com output detalhado
cargo test --workspace -- --nocapture
```

## 🖥️ Executando a Aplicação

```bash
# Desenvolvimento
cargo run -p ui

# Release otimizado
cargo build -p ui --release
./target/release/ui
```

## 📚 Documentação

A documentação completa do projeto está organizada na pasta `docs/`:

| Documento | Descrição |
|-----------|-----------|
| [01-REQUISITOS.md](docs/01-REQUISITOS.md) | Requisitos funcionais e não-funcionais |
| [02-ARQUITETURA.md](docs/02-ARQUITETURA.md) | Clean Architecture, camadas e padrões |
| [03-FUNCIONALIDADES.md](docs/03-FUNCIONALIDADES.md) | Especificação detalhada de features |
| [04-ROADMAP.md](docs/04-ROADMAP.md) | Planejamento de desenvolvimento |
| [05-STACK-TECNOLOGICO.md](docs/05-STACK-TECNOLOGICO.md) | Stack completo e dependências |
| [06-UI-ARCHITECTURE.md](docs/06-UI-ARCHITECTURE.md) | Arquitetura da UI egui |
| [STATUS.md](docs/STATUS.md) | Status atual do projeto |

## ✨ Funcionalidades Principais

### 🎨 Edição GPU-Accelerated

13 ajustes em tempo real via wgpu compute shaders:

| Categoria | Ajustes |
|-----------|---------|
| **Básicos** | Exposure, Contrast, Highlights, Shadows, Whites, Blacks |
| **Presença** | Clarity, Vibrance, Saturation |
| **White Balance** | Temperature, Tint |
| **Tone Curve** | RGB channels, Parametric |
| **HSL** | 8 cores × Hue/Saturation/Luminance |
| **Detalhe** | Sharpening, Noise Reduction |
| **Transformação** | Crop, Rotation, Flip |

### 📥 Importação Inteligente

- Suporte a múltiplos formatos RAW (CR2, NEF, ARW, DNG, etc.)
- Detecção automática de duplicatas via hash SHA-256
- Geração paralela de thumbnails
- Preview antes de importar
- Organização automática por data

### ⭐ Organização Avançada

- Classificação por estrelas (0-5)
- Flags: Pick, Reject, Unflagged
- Color Labels: Red, Yellow, Green, Blue, Purple
- Coleções com fotos organizadas
- Filtros e busca avançada

### 🖨️ Impressão Profissional

- Layouts variados
- Preview de impressão
- Configurações de papel e qualidade

## 🎹 Atalhos de Teclado

### Globais
| Tecla | Ação |
|-------|------|
| `G` | Library View |
| `D` | Develop View |
| `1-5` | Rating |
| `6-9` | Color Labels |
| `P` / `X` / `U` | Flag |
| `Cmd+Z` | Undo |

### Develop View
| Tecla | Ação |
|-------|------|
| `←` / `→` | Foto anterior/próxima |
| `Space` | Antes/Depois |
| `F` | Fullscreen |
| `R` | Crop Tool |
| `0` / `1` / `2` | Zoom Fit/100%/200% |

## 🛠️ Estrutura do Projeto

```
VintageLightbox/
├── crates/
│   ├── domain/          # Camada 1: Entities, Value Objects (220 testes)
│   ├── use-cases/       # Camada 2: Application Business Rules (65 testes)
│   ├── adapters/        # Camada 3: Controllers, Presenters (27 testes)
│   ├── infrastructure/  # Camada 4: SQLite, wgpu, Cache (48 testes)
│   └── ui/              # Camada 5: egui views e componentes
├── docs/                # Documentação completa
└── target/              # Build output
```

## 📋 Requisitos do Sistema

### Para Desenvolvedores
- Rust 1.75 ou superior
- macOS 10.15+ ou Windows 10+
- 8GB RAM mínimo
- GPU com suporte Vulkan/Metal (para wgpu)

### Para Usuários Finais
- macOS 10.15+ ou Windows 10+
- 4GB RAM (8GB recomendado)
- GPU dedicada ou integrada recente
- 500MB espaço em disco

## 🎯 Diferenciais

- **100% Rust**: Segurança de memória e performance nativa
- **GPU-Accelerated**: Edição em tempo real via wgpu compute shaders
- **Clean Architecture**: 5 camadas bem definidas, 360+ testes
- **Interface Moderna**: egui com 5 temas e atalhos profissionais
- **Multi-level Cache**: Performance otimizada (0.01ms lookup)
- **Cross-Platform**: Funciona nativamente em macOS e Windows

## 📖 Para Começar

### 1. Clone o Repositório
```bash
git clone https://github.com/alexkads/VintageLightbox.git
cd VintageLightbox
```

### 2. Instale o Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 3. Execute os Testes
```bash
cargo test --workspace
```

### 4. Execute a Aplicação
```bash
cargo run -p ui
```

## 🤝 Contribuindo

Contribuições são bem-vindas! Por favor:

1. Leia a documentação em `docs/`
2. Siga TDD: escreva testes primeiro
3. Use `cargo fmt` e `cargo clippy`
4. Atualize a documentação quando necessário

## 📄 Licença

MIT License - Veja [LICENSE](LICENSE) para detalhes.

## 🙏 Inspiração

Este projeto é inspirado em:
- Adobe Lightroom
- DarkTable
- RawTherapee

---

**Status**: ✅ MVP Completo | **Versão**: 0.1.0 | **Última Atualização**: 31 de dezembro de 2025
