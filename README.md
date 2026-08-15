# VintageLightbox 📷

Clone profissional do Adobe Lightroom desenvolvido em Rust com interface egui.

## 📸 Sobre o Projeto

VintageLightbox é uma aplicação multiplataforma de gerenciamento e edição de fotos RAW, projetada para fotógrafos profissionais que precisam de:

- 📥 **Importação eficiente** de grandes volumes de fotos
- 🎨 **Edição não-destrutiva** de arquivos RAW/DNG
- 🖥️ **Suporte multi-monitor** para apresentação a clientes
- ⭐ **Organização avançada** com classificação, tags e coleções
- ✅ **Pré-seleção rápida** de fotos
- 💰 **Gestão de vendas** e marcação de fotos compradas
- 🖨️ **Sistema de impressão** profissional
- 💾 **Presets personalizados** para workflow consistente
- 📤 **Exportação otimizada** para JPEG/PNG

## 🏗️ Arquitetura e Metodologia

Este projeto segue **Clean Architecture** e **Test-Driven Development (TDD)**:

- 🏛️ **4 Camadas**: Domain → Use Cases → Adapters → Infrastructure
- 🧪 **TDD**: Ciclo Red-Green-Refactor em todo o código
- 📊 **Domain**: 202 testes, incluindo property-based testing
- ⚠️ **Adapters**: sem testes — é o vão de cobertura conhecido

## 🚀 Tecnologias

- **Linguagem**: Rust (performance e segurança)
- **Interface**: egui 0.31 (nativa e multiplataforma)
- **RAW Processing**: LibRaw/rawler
- **Database**: SQLite
- **Testing**: cargo test, mockall, proptest, criterion, **egui_kittest** (E2E)
- **Plataformas**: macOS e Windows

## 🎯 Status do Desenvolvimento

**Fase Atual**: Fase 2 — Funcionalidades Essenciais (importação avançada, edição RAW completa,
presets, flags, crop, impressão)

> ✅ **Compila, suíte verde (478 testes), app sobe** — depois de dois consertos feitos em
> 15/ago/2026 e **ainda não commitados**. O quadro completo, incluindo 4 migrations aplicadas em
> catálogos existentes que não estão no repositório, está em **[docs/STATUS.md](docs/STATUS.md)**.

- [x] Workspace com Clean Architecture (5 crates)
- [x] **Domain** — 4 entidades, 13 value objects, **202 testes passando**
- [x] **Use Cases** — 20 módulos (importação, organização, edição, presets, export, print)
- [x] **Infrastructure** — SQLite (15 migrations), cache L1/L2/L3, RAW via LibRaw, EXIF, thumbnails
- [x] **Adapters** — 6 controllers (⚠️ sem testes)
- [x] **UI** — egui 0.31 sobre wgpu: 4 views, 26 componentes, docking, multi-monitor, 5 temas
- [ ] Commitar os consertos e reativar o CI ← **próximo passo**

**478 testes passando · 0 falhas · 3 ignorados**

**Rodando os testes**:
```bash
cargo test --workspace

cargo test -p domain

# Testes E2E da UI (egui_kittest)
cargo test -p ui --test crop_feature_e2e_test

# Atualizar snapshots (quando necessário)
UPDATE_SNAPSHOTS=true cargo test -p ui
```

## 📚 Documentação

A documentação completa do projeto está organizada na pasta `docs/`:

- **[01-REQUISITOS.md](docs/01-REQUISITOS.md)** - Requisitos funcionais e não-funcionais detalhados
- **[02-ARQUITETURA.md](docs/02-ARQUITETURA.md)** - Arquitetura do sistema, módulos e padrões de design
- **[03-FUNCIONALIDADES.md](docs/03-FUNCIONALIDADES.md)** - Especificação detalhada de cada funcionalidade
- **[04-ROADMAP.md](docs/04-ROADMAP.md)** - Planejamento de desenvolvimento em fases
- **[05-STACK-TECNOLOGICO.md](docs/05-STACK-TECNOLOGICO.md)** - Stack completo e dependências
- **[07-E2E-TESTING.md](docs/07-E2E-TESTING.md)** - Guia de testes E2E com egui_kittest

## ✨ Principais Funcionalidades

### Importação
- Suporte a múltiplos formatos RAW (CR2, NEF, ARW, DNG, etc.)
- Detecção automática de duplicatas
- Geração paralela de thumbnails
- Organização automática por data

### Edição RAW
- Ajustes básicos: exposição, contraste, temperatura, matiz
- Ajustes avançados: curva de tons, HSL, correção de lente
- Redução de ruído e nitidez
- Efeitos criativos: vinheta, split toning, grain
- Histórico completo com undo/redo ilimitado

### Organização
- Classificação por estrelas (0-5)
- Flags de cor personalizáveis
- Sistema de tags hierárquico
- Coleções simples e inteligentes
- Busca e filtros avançados

### Multi-Monitor
- Exibição em tela cheia no segundo monitor
- Modo apresentação com slideshow
- Sincronização automática
- Comparação lado a lado

### Vendas
- Marcação de fotos compradas
- Registro de informações do cliente
- Relatórios de vendas
- Controle de entregas

### Exportação e Impressão
- Exportação JPEG/PNG com ajuste de qualidade
- Redimensionamento e marca d'água
- Renomeação em lote
- Layouts de impressão variados
- Gerenciamento de cor para impressão

## 🛠️ Estrutura do Projeto

```
VintageLightbox-Rust/
├── crates/
│   ├── domain/              # Entidades, value objects, traits (sem dependências externas)
│   ├── use-cases/           # Orquestração de regras de negócio
│   ├── adapters/            # Controllers, presenters, view models
│   ├── infrastructure/      # SQLite, cache, RAW, EXIF, arquivos, dispositivos
│   └── ui/                  # egui/eframe (wgpu): views, componentes, design system
├── crates/infrastructure/migrations/   # 15 migrations SQLite
├── docs/                    # Documentação (comece por STATUS.md)
└── dev.sh                   # Helper de TDD
```

## 📋 Requisitos do Sistema

### Para Desenvolvedores
- Rust 1.75 ou superior
- macOS 10.15+ ou Windows 10+
- 8GB RAM mínimo
- Xcode Command Line Tools (macOS) ou Visual Studio Build Tools (Windows)

### Para Usuários Finais
- macOS 10.15+ ou Windows 10+
- 4GB RAM (8GB recomendado)
- 500MB espaço em disco

## 🎯 Diferenciais

- **100% Rust**: Segurança de memória e performance nativa
- **Interface Nativa**: UI responsiva e fluida
- **Multi-Monitor**: Recurso essencial para fotógrafos
- **Gestão de Vendas**: Integrada diretamente no workflow
- **Open Source**: Transparência e comunidade
- **Cross-Platform**: Funciona nativamente em macOS e Windows

## 📖 Para Começar

### 1. Clone o Repositório
```bash
git clone https://github.com/seu-usuario/VintageLightbox.git
cd VintageLightbox
```

### 2. Instale o Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 3. Leia a Documentação
Comece pelos documentos na pasta `docs/` para entender o projeto:
- [Requisitos](docs/01-REQUISITOS.md)
- [Arquitetura](docs/02-ARQUITETURA.md)
- [Roadmap](docs/04-ROADMAP.md)

### 4. Setup do Desenvolvimento (Em breve)
```bash
# Será disponibilizado na Fase 0
cargo build
cargo test
cargo run
```

## 🤝 Contribuindo

Contribuições são bem-vindas! Por favor:

1. Leia a documentação completa
2. Abra uma issue para discutir mudanças grandes
3. Siga os padrões de código Rust (rustfmt, clippy)
4. Escreva testes para novas funcionalidades
5. Atualize a documentação quando necessário

## 📄 Licença

A ser definida. Opções consideradas:
- GPL-3.0 (para projeto completamente open source)
- MIT (para maior permissividade)
- Dual License (GPL + Commercial)

## 🙏 Agradecimentos

Este projeto é inspirado em:
- Adobe Lightroom
- DarkTable
- RawTherapee

Agradecimentos às comunidades de Rust, egui e processamento de imagens open source.

## 📞 Contato

- **Issues**: Use o GitHub Issues para bugs e sugestões
- **Discussões**: Use o GitHub Discussions para perguntas
- **Documentação**: Consulte a pasta `docs/`

---

**Status**: 🟢 Fase 2 em andamento — compila e roda (veja [docs/STATUS.md](docs/STATUS.md))

**Versão**: 0.1.0

**Última Atualização**: 15 de agosto de 2026
