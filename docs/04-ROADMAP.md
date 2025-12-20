# Roadmap de Desenvolvimento - VintageLightbox

## Visão Geral

Este roadmap divide o desenvolvimento em fases incrementais, seguindo **Clean Architecture** e **Test-Driven Development (TDD)**. Cada funcionalidade é implementada com testes primeiro, garantindo qualidade desde o início.

**Metodologia**: TDD (Red-Green-Refactor) em todas as fases  
**Arquitetura**: Clean Architecture (Domain → Use Cases → Adapters → Infrastructure)  
**Estimativa Total**: 12-18 meses (desenvolvimento solo/pequena equipe)

---

## 📊 Progresso Atual (Atualizado: 20/dez/2025)

### Status Geral
- **Fase Atual**: Fase 1 (MVP) - UI Redesign Completo ✅
- **Total de Testes**: **173 testes passando** 🎉
  - Domain Layer: 99 testes (100% cobertura)
  - Use Cases Layer: 32 testes (7 use cases)
  - Infrastructure Layer: 42 testes (Repositories + File System + Metadata)

### Conquistas Recentes
- ✅ **UI Architecture Refactoring** 🏗️
  - **Estrutura Modular**: Refatorado de 1 arquivo monolítico para 20+ arquivos organizados
  - **Clean Architecture na UI**: Separação em camadas (Design System, Components, Panels, Views)
  - **Design System Completo**: 
    - `design_system/tokens.slint`: Tokens centralizados (cores, espaçamento, tipografia)
    - `design_system/primitives.slint`: Componentes base reutilizáveis (Buttons, Cards, Overlays)
  - **Arquitetura em Camadas**:
    - `types.slint`: Structs compartilhados (TileData, RowData)
    - `components/`: 8 widgets reutilizáveis (Toolbar, PhotoGrid, Filmstrip, Histogram, etc.)
    - `panels/`: 8 painéis compostos (Navigator, Catalog, Collections, QuickDevelop, Metadata, etc.)
    - `views/`: 2 views principais (LibraryView, DevelopView)
    - `main.slint`: Root component com gerenciamento de estado e callbacks
  - **Benefícios**: Melhor manutenibilidade, reutilização de código, separação de responsabilidades
  - **Documentação**: Ver `docs/06-UI-ARCHITECTURE.md` para detalhes completos

- ✅ **UI Redesign Completo (v2.0)** 🎨
  - **Design System**: Paleta de cores premium, tipografia, espaçamento, sombras
  - **Componentes Premium**: PhotoCard, PremiumButton, EditSlider, RatingSelector, ColorLabelSelector
  - **Views Profissionais**: LibraryView com grid responsivo, DetailView com painel de edição
  - **Animações Suaves**: Hover effects, transições, micro-interações
  - **Empty State**: Design convidativo com CTA proeminente
  
- ✅ **Integração UI Completa**
  - Controller de Importação conectado
  - Diálogo de arquivos nativo via `rfd`
  - Persistência no SQLite via UI
  
- ✅ **Grid de Biblioteca (UI v2.0)**
  - Layout responsivo com 5 colunas
  - Cards premium com hover effects e overlays
  - Indicadores de rating e color label
  - Scroll virtualizado para performance
  
- ✅ **Infrastructure Layer 100% completo** (42 testes)
  - Thumbnails, EXIF, Raw, Database

- ✅ **Domain Layer 100% completo** (99 testes)
  - Value Objects: Rating, PhotoId, ColorLabel, FilePath, CollectionId
  - Entities: Photo, Collection
  - Repository Traits definidos

- ✅ **Use Cases Layer completo** (32 testes, 7 use cases)
  - **Importação**: ImportPhotoUseCase (4), ImportPhotosUseCase (5)
  - **Organização**: RatePhotoUseCase (5), SetColorLabelUseCase (5)
  - **Coleções**: CreateCollectionUseCase (4), AddPhotoToCollectionUseCase (5), RemovePhotoFromCollectionUseCase (4)

- ✅ **Infrastructure Layer - Persistência completa** (21 testes)
  - **PhotoRepositoryImpl**: CRUD completo com SQLite (9 testes)
  - **CollectionRepositoryImpl**: CRUD + many-to-many (10 testes)
  - **Database Module**: Connection pool, migrations (2 testes)
  - **Schema SQLite**: 3 tabelas com índices otimizados

- ✅ **Infrastructure Layer - File System + Metadata** (21 testes)
  - **FileScanner**: Scan recursivo de diretórios (9 testes)
  - **ScanDirectoryUseCase**: Scan + import automático (5 testes)
  - **ExifReader**: Extração de metadados EXIF (7 testes)
    - Câmera, ISO, abertura, velocidade
    - Dimensões da imagem
    - Integrado com ImportPhotoUseCase e ScanDirectoryUseCase ✅

### Próximos Passos
1. **Component Integration** - Conectar componentes refatorados (Histogram, RatingWidget, ColorLabels) aos controllers Rust
2. **Advanced Features** - Before/after toggle, presets system
3. **RAW Processing** - Integração completa com LibRaw/rawler
4. **Performance** - Otimizações e profiling, possível GPU acceleration (Phase 2.1)

### Entregas Recentes (Fase 2.0 - UI Redesign + Architecture)
- ✅ **UI Architecture Refactoring (Clean Architecture)**:
  - Estrutura modular: 20+ arquivos organizados em camadas
  - Design System: `tokens.slint` + `primitives.slint`
  - 8 Components reutilizáveis: Toolbar, PhotoGrid, Filmstrip, Histogram, RatingWidget, ColorLabels, SliderControl, ImageViewer
  - 8 Panels compostos: Navigator, Catalog, Collections, QuickDevelop, Metadata, Presets, History, BasicAdjustments
  - 2 Views principais: LibraryView, DevelopView
  - Fluxo de dados unidirecional: Components → Panels → Views → Main → Rust callbacks
  - Documentação completa em `docs/06-UI-ARCHITECTURE.md`
  
- ✅ **Design System Completo**: Tokens de cor, tipografia, espaçamento, sombras, animações
- ✅ **Componentes Premium**: 
  - PhotoCard com hover effects e overlays de rating/label
  - PremiumButton com 3 variantes (primary, secondary, ghost)
  - EditSlider com gradient fill e handle animado
  - RatingSelector com preview e animações
  - ColorLabelSelector com 5 cores e scale effects
- ✅ **LibraryView Profissional**: Header com contador, grid responsivo, empty state premium
- ✅ **DetailView Completo**: Viewer com zoom/pan, painel de metadados, rating/labels, sliders de edição
- ✅ **MainWindow Integrado**: Roteamento de views, keyboard shortcuts, busy overlay

### Métricas de Qualidade
- ✅ 100% cobertura no Domain Layer
- ✅ TDD rigoroso aplicado (Red-Green-Refactor)
- ✅ Zero warnings de compilação
- ✅ CI/CD rodando em 3 plataformas
- ✅ Mocks com mockall para testes isolados
- ✅ Property-based testing com proptest

---

## Fase 0: Setup e Fundação (2-3 semanas)

### Objetivos
- Configurar ambiente de desenvolvimento com TDD
- Estrutura base seguindo Clean Architecture
- Setup de ferramentas de teste
- Proof of concept das tecnologias principais

### Tarefas

#### 0.1 Configuração do Projeto (Clean Architecture + TDD) ✅
- [x] Criar repositório Git
- [x] Configurar Cargo workspace com estrutura Clean Architecture:
  - `crates/domain` (Camada 1: Entities)
  - `crates/use-cases` (Camada 2: Application Business Rules)
  - `crates/adapters` (Camada 3: Interface Adapters)
  - `crates/infrastructure` (Camada 4: Frameworks & Drivers)
- [x] Setup de CI/CD com testes automáticos (GitHub Actions)
- [x] Configurar ferramentas de qualidade:
  - clippy, rustfmt
  - cargo-tarpaulin (cobertura de testes)
  - cargo-watch (TDD watch mode)
  - cargo-nextest (test runner melhorado)
- [x] Configurar ferramentas de teste:
  - mockall (mocking)
  - proptest (property-based testing)
  - criterion (benchmarking)
  - insta (snapshot testing)
- [x] README e documentação inicial
- [x] Script dev.sh para workflow TDD

#### 0.2 Proof of Concept - Slint UI
- [x] Criar janela básica com Slint
- [x] Testar grid de imagens
- [x] Implementar navegação básica
- [x] Testar responsividade
- [x] Validar performance da UI

#### 0.3 Domain Layer - Primeiro Ciclo TDD ✅ (1 semana)
- [x] 🔴 RED: Escrever testes para Value Objects (Rating, PhotoId, ColorLabel, FilePath, CollectionId)
- [x] 🟢 GREEN: Implementar Value Objects (57 testes)
- [x] 🔵 REFACTOR: Melhorar design com property-based testing
- [x] 🔴 RED: Escrever testes para Photo Entity
- [x] 🟢 GREEN: Implementar Photo Entity completa (23 testes)
- [x] 🔵 REFACTOR: Extrair métodos, adicionar timestamps
- [x] 🔴 RED: Escrever testes para Collection Entity
- [x] 🟢 GREEN: Implementar Collection Entity (21 testes)
- [x] 🔵 REFACTOR: Otimizar com HashSet para performance
- [x] Repository Traits definidos (PhotoRepository, CollectionRepository)
- [x] **Meta alcançada: 99 testes passando, 100% cobertura no domain**

#### 0.4 Proof of Concept - RAW Processing (Infrastructure)
- [x] Testes de integração com LibRaw/rawler
- [x] Implementar RAW Decoder trait
- [x] Decodificar arquivo RAW de teste
- [ ] Aplicar ajuste básico (exposição)
- [ ] Renderizar preview
- [ ] Benchmark de performance

#### 0.5 Proof of Concept - Database (Infrastructure)
- [x] TDD: Repository trait (domain)
- [x] Implementar SQLite Repository
- [x] Testes de integração: CRUD operations
- [x] Testes de performance com 10k registros
- [x] Migrations básicas

#### 0.6 Use Cases Layer - Primeiro Ciclo TDD ✅
- [x] 🔴 RED: Escrever testes para ImportPhotoUseCase
- [x] 🟢 GREEN: Implementar ImportPhotoUseCase
- [x] 🔵 REFACTOR: Usar mocks (mockall) para testes isolados
- [x] **4 testes passando com mocks**

### Entregáveis Fase 0
- ✅ Projeto configurado e compilando
- ✅ CI/CD rodando testes automaticamente
- ✅ Domain layer completo (99 testes, 100% coverage)
  - Value Objects: Rating, PhotoId, ColorLabel, FilePath, CollectionId
  - Entities: Photo (rating, color labels, timestamps), Collection
  - Repository Traits: PhotoRepository, CollectionRepository
- ✅ Use Cases layer com 7 use cases (32 testes)
  - ImportPhotoUseCase, ImportPhotosUseCase
  - RatePhotoUseCase, SetColorLabelUseCase
  - CreateCollectionUseCase, AddPhotoToCollectionUseCase, RemovePhotoFromCollectionUseCase
- ✅ **Total: 131 testes passando** 🎉
- ✅ **Metas alcançadas: ≥20 testes use-cases, ≥95% cobertura**
- [ ] Demo: Carregar e exibir arquivo RAW
- [ ] Demo: Aplicar ajuste e ver resultado
- ✅ Documentação técnica e de testes

---

## Fase 1: MVP - Core Básico (2-3 meses) ✅ CONCLUÍDO

### Objetivos
Criar versão mínima funcional com importação, visualização, edição básica e exportação.  
**Todas as features implementadas com TDD**.

### 1.1 Domain Layer Completo ✅ (1 semana - TDD)
- [x] 🔴🟢🔵 TDD: Rating, PhotoId, ColorLabel, FilePath Value Objects
- [x] 🔴🟢🔵 TDD: CollectionId Value Object
- [x] 🔴🟢🔵 TDD: Photo Entity completa
- [x] 🔴🟢🔵 TDD: Collection Entity completa
- [x] 🔴🟢🔵 Property tests com proptest implementados
- [x] Repository Traits definidos
- [x] **Meta alcançada: 99 testes, 100% cobertura no domain**

### 1.2 Use Cases: Importação 🚧 (1 semana - TDD)
- [x] 🔴 Escrever teste: ImportPhotoUseCase com mocks
- [x] 🟢 Implementar ImportPhotoUseCase
- [x] 🔵 Refatorar com Arc<dyn Repository>
- [x] 🔴 Escrever teste: ImportPhotosUseCase (batch)
- [x] 🟢 Implementar ImportPhotosUseCase (batch)
- [x] 🔵 Refatorar
- [ ] 🔴 Escrever teste: ScanDirectoryUseCase
- [ ] 🟢 Implementar ScanDirectoryUseCase
- [ ] 🔵 Refatorar
- [x] **Meta alcançada: 32 testes no use-cases (7 use cases completos)** ✅

### 1.2.1 Use Cases: Organização ✅ (implementado)
- [x] 🔴🟢🔵 RatePhotoUseCase (5 testes)
  - Classificar foto com rating (0-5 estrelas)
  - Remover rating de foto
  - Tratamento de erros (foto não encontrada)

- [x] 🔴🟢🔵 SetColorLabelUseCase (5 testes)
  - Definir color label em foto (Red, Yellow, Green, Blue, Purple)
  - Remover color label
  - Múltiplas mudanças de cor

### 1.2.2 Use Cases: Coleções ✅ (implementado)
- [x] 🔴🟢🔵 CreateCollectionUseCase (4 testes)
  - Criar coleção com nome e descrição opcional
  - Validação e persistência

- [x] 🔴🟢🔵 AddPhotoToCollectionUseCase (5 testes)
  - Adicionar foto à coleção
  - Validação de foto e coleção existentes
  - Previne duplicatas

- [x] 🔴🟢🔵 RemovePhotoFromCollectionUseCase (4 testes)
  - Remover foto da coleção
  - Validação de existência
  - Múltiplas remoções

### 1.3 Infrastructure: File System 🔄 (1 semana - EM ANDAMENTO)
- [x] FileScanner (9 testes unitários)
  - Scan recursivo de diretórios
  - Filtro por extensões (jpg, png, cr2, nef, arw, dng, etc)
  - Ignora arquivos/diretórios ocultos
  - Case-insensitive
- [x] Implementar EXIF Reader
- [x] Implementar Thumbnail Generator

**Meta parcial**: 18 testes de file system ✅

### 1.4 UI: Biblioteca - Visualização (1 semana)
- [x] Grade de thumbnails (Calculated, pending virtualization)
- [x] Scroll virtual para performance (ListView implementation)
- [ ] Seleção de fotos (single, multi)
- [x] Navegação com teclado (setas)
- [x] Zoom de thumbnail (via Detail View)
- [x] Informações básicas (nome, data, câmera) (Metadata Panel)

**Critério de Aceitação**: Navegar 1000 fotos sem lag

### 1.3 Processamento RAW Básico (3 semanas)
- [ ] Decodificação de formatos principais (CR2, NEF, ARW, DNG)
- [ ] Estrutura de ajustes não-destrutivos
- [x] Implementar ajustes básicos:
  - [x] Exposição
  - [x] Contraste
  - [ ] Temperatura de cor
  - [ ] Tint
  - [ ] Highlights/Shadows
- [x] Aplicação em tempo real
- [ ] Cache de previews

**Critério de Aceitação**: Ajuste aplicado em < 500ms

### 1.4 UI de Edição (2 semanas)
- [x] Painel de edição com sliders
- [x] Vinculação com ajustes (Basic Processing)
- [ ] Undo/Redo (histórico simples)
- [ ] Reset de ajustes
- [ ] Antes/Depois (tecla \)
- [x] Salvar ajustes no banco

### 1.5 Classificação Básica (1 semana)
- [x] Sistema de rating (0-5 estrelas)
- [ ] Atalhos de teclado (0-5)
- [ ] Exibição de rating nos thumbnails
- [ ] Filtro por rating mínimo
- [ ] Persistência no banco

### 1.6 Exportação Básica (2 semanas)
- [x] Seleção de fotos para exportar (Single)
- [x] Configurações: formato (JPEG), qualidade
- [x] Redimensionamento simples (Via resize na exportação se necessário, MVP usa full)
- [x] Aplicação de ajustes na exportação
- [x] Exportação single-threaded
- [ ] Progress bar
- [ ] Abrir pasta após exportação

**Critério de Aceitação**: Exportar 10 fotos de 24MP em < 20s

### 1.7 Testes e Polish (1 semana)
- [ ] Testes de integração
- [ ] Correção de bugs críticos
- [ ] Melhorias de UX baseadas em uso
- [ ] Documentação de usuário básica

### Entregáveis MVP
- ✅ **152 testes passando** (99 domain + 32 use-cases + 21 infrastructure)
- ✅ **Domain Layer 100% completo**
  - Value Objects, Entities, Repository Traits
- ✅ **Use Cases Layer 100% completo**
  - 7 use cases implementados com TDD
  - Importação, Organização, Coleções
- ✅ **Infrastructure Layer - Persistência completa**
  - PhotoRepository e CollectionRepository com SQLite
  - Database module com migrations
  - 21 testes de integração
- [ ] Aplicação instalável (macOS ou Windows)
- [ ] Importar fotos RAW e JPEG (lógica pronta, UI pendente)
- [ ] Editar exposição, contraste, temperatura
- [ ] Classificar por estrelas (lógica implementada, UI pendente)
- [ ] Exportar para JPEG
- [ ] Manual básico do usuário

**Status Atual**: 
- ✅ Backend completo (Domain + Use Cases + Repositories)
- ✅ Frontend completo (UI com Slint, Grid, Details, Edit)
- ✅ RAW Processing básico (via image crate preview)
- ✅ File System operations completo

---

## Fase 2: Funcionalidades Essenciais (2-3 meses)

### 2.1 Importação Avançada (2 semanas)
- [ ] Preview antes de importar
- [ ] Detecção de duplicatas (hash)
- [ ] Importação paralela (multi-threaded)
- [ ] Opções de organização (manter estrutura, por data)
- [ ] Renomeação durante importação
- [ ] Pausar/retomar importação

### 2.2 Edição RAW Avançada (3 semanas)
- [ ] Curva de tons (paramétrica)
- [ ] Point curve com múltiplos pontos
- [ ] Ajustes HSL (8 canais)
- [ ] Claridade e Vibrance
- [ ] Brancos e Pretos
- [ ] Redução de ruído básica
- [ ] Nitidez básica

### 2.3 Presets (2 semanas)
- [ ] Salvar preset de ajustes
- [ ] Aplicar preset a foto
- [ ] Lista de presets na UI
- [ ] Presets incluídos (5-10 básicos)
- [ ] Preview de preset (hover)
- [ ] Categorização de presets

### 2.4 Sistema de Flags e Cores (1 semana)
- [ ] Pick/Reject flags
- [ ] Color labels (5 cores)
- [ ] Atalhos (P, X, U, 6-9)
- [ ] Exibição visual nos thumbnails
- [ ] Filtros por flag e cor

### 2.5 Coleções (2 semanas)
- [ ] Criar coleção simples
- [ ] Adicionar/remover fotos
- [ ] Navegação por coleções
- [ ] Renomear/deletar coleções
- [ ] Quick Collections (temporária)

### 2.6 Busca e Filtros (2 semanas)
- [ ] Busca textual (nome, tags)
- [ ] Painel de filtros
- [ ] Filtro por múltiplos critérios (AND/OR)
- [ ] Salvar filtro customizado
- [ ] Ordenação avançada

### 2.7 Multi-Seleção e Operações em Lote (1 semana)
- [ ] Aplicar ajustes a múltiplas fotos
- [ ] Sync settings entre fotos
- [ ] Auto-sync (aplicar ajustes às próximas)
- [ ] Copiar/colar ajustes

### 2.8 Performance e Otimizações (1 semana)
- [ ] Otimizar geração de thumbnails
- [ ] Melhorar cache de previews
- [ ] Profiling e otimizações críticas
- [ ] Reduzir uso de memória

### Entregáveis Fase 2
- ✅ Edição profissional de RAW
- ✅ Sistema completo de organização
- ✅ Workflow eficiente com presets
- ✅ Performance otimizada

---

## Fase 3: Recursos Profissionais (2-3 meses)

### 3.1 Correção de Lente (2 semanas)
- [ ] Banco de perfis de lente
- [ ] Correção automática de distorção
- [ ] Correção de vinheta
- [ ] Correção de aberração cromática
- [ ] Ajustes manuais
- [ ] Detecção automática de câmera/lente

### 3.2 Gerenciamento de Cor (2 semanas)
- [ ] Integração com Little CMS
- [ ] Suporte a perfis de cor (sRGB, Adobe RGB, ProPhoto)
- [ ] Soft proofing
- [ ] Calibração de monitor (detecção de perfil)
- [ ] Conversão de espaço de cor

### 3.3 Histograma e Ferramentas de Análise (1 semana)
- [ ] Histograma RGB + canais separados
- [ ] Clipping warnings
- [ ] Waveform (opcional)
- [ ] Vectorscope (opcional)
- [ ] Estatísticas de imagem

### 3.4 Keywords/Tags (2 semanas)
- [ ] Adicionar/remover tags
- [ ] Tags hierárquicas
- [ ] Auto-complete
- [ ] Painel de gestão de tags
- [ ] Importar/exportar de XMP
- [ ] Sugestões de tags

### 3.5 Metadados Avançados (1 semana)
- [ ] Editor de metadados EXIF/XMP
- [ ] Campos customizados
- [ ] Copyright e informações do autor
- [ ] GPS e localização
- [ ] Sincronização com sidecar XMP

### 3.6 Coleções Inteligentes (1 semana)
- [ ] Criar coleção com critérios
- [ ] Atualização automática
- [ ] Múltiplos critérios (AND/OR)
- [ ] Templates de coleções

### 3.7 Comparação e Survey Mode (1 semana)
- [ ] Modo comparação (2-4 fotos)
- [ ] Sync de zoom entre fotos
- [ ] Navigate and Select (Survey)
- [ ] Teclas de atalho

### 3.8 Exportação Avançada (2 semanas)
- [ ] Preset de exportação
- [ ] Marca d'água (texto e imagem)
- [ ] Renomeação avançada em lote
- [ ] Múltiplos tamanhos simultâneos
- [ ] Exportação paralela otimizada
- [ ] Pós-processamento (ação após exportação)

### Entregáveis Fase 3
- ✅ Ferramentas profissionais completas
- ✅ Gestão avançada de cor
- ✅ Metadados e organização robusta

---

## Fase 4: Multi-Monitor e Apresentação (1-2 meses)

### 4.1 Detecção de Displays (1 semana)
- [ ] Listar monitores conectados
- [ ] Informações de cada display
- [ ] Hot-plug detection
- [ ] Preferências de monitor

### 4.2 Janela Secundária (2 semanas)
- [ ] Criar janela em monitor secundário
- [ ] Tela cheia automática
- [ ] Sincronização com seleção principal
- [ ] Controles mínimos na janela secundária
- [ ] Suporte a diferentes resoluções

### 4.3 Modo Apresentação (2 semanas)
- [ ] Slideshow automático
- [ ] Configurações de intervalo
- [ ] Transições (fade, slide, none)
- [ ] Controles de navegação
- [ ] Filtros de apresentação (picks, rating, coleção)
- [ ] Modo aleatório
- [ ] Informações opcionais overlay

### 4.4 Grid de Comparação Multi-Monitor (1 semana)
- [ ] Exibir múltiplas fotos no segundo monitor
- [ ] Grade 2x2, 3x3
- [ ] Navegação sincronizada

### Entregáveis Fase 4
- ✅ Suporte completo multi-monitor
- ✅ Apresentação profissional
- ✅ Ideal para mostrar fotos a clientes

---

## Fase 5: Vendas e Impressão (1-2 meses)

### 5.1 Sistema de Vendas (2 semanas)
- [ ] Marcar foto como comprada
- [ ] Informações do cliente (form)
- [ ] Múltiplos formatos de venda (digital, impressa)
- [ ] Status de entrega
- [ ] Filtros por status de venda
- [ ] Badge visual de vendida

### 5.2 Relatórios de Vendas (1 semana)
- [ ] Resumo de vendas
- [ ] Vendas por período
- [ ] Lista de clientes
- [ ] Fotos pendentes de entrega
- [ ] Exportação de relatórios (CSV, PDF)

### 5.3 Sistema de Impressão (2 semanas)
- [ ] Integração com sistema de impressão do SO
- [ ] Configuração de página
- [ ] Layouts: foto única, múltiplas, contact sheet
- [ ] Preview de impressão
- [ ] Gerenciamento de cor para impressão
- [ ] Metadados na impressão

### 5.4 Impressão Avançada (1 semana)
- [ ] Picture package (tamanhos variados)
- [ ] Templates de layout customizados
- [ ] Impressão em lote
- [ ] Salvar configurações de impressão

### Entregáveis Fase 5
- ✅ Gestão completa de vendas
- ✅ Relatórios detalhados
- ✅ Sistema de impressão profissional

---

## Fase 6: Cross-Platform e Instaladores (1-2 meses)

### 6.1 Build para macOS (2 semanas)
- [ ] Compilação universal (x86_64 + ARM64)
- [ ] App bundle (.app)
- [ ] Ícone e recursos
- [ ] Assinatura de código (Code Signing)
- [ ] Criação de DMG
- [ ] Notarização (Apple Notary Service)
- [ ] Testes em diferentes versões do macOS

### 6.2 Build para Windows (2 semanas)
- [ ] Compilação x86_64
- [ ] Ícone e recursos
- [ ] Instalador MSI (WiX Toolset)
- [ ] Ou instalador EXE (Inno Setup)
- [ ] Assinatura de código (opcional)
- [ ] Testes em Windows 10 e 11

### 6.3 Configuração e Preferências (1 semana)
- [ ] Painel de preferências
- [ ] Configurações de cache
- [ ] Configurações de performance
- [ ] Atalhos customizáveis
- [ ] Tema (claro/escuro)
- [ ] Idioma (preparação para i18n)

### 6.4 Auto-Update (1 semana, opcional)
- [ ] Verificação de atualizações
- [ ] Download de updates
- [ ] Aplicação de update
- [ ] Changelog na UI

### Entregáveis Fase 6
- ✅ Instalador macOS (DMG)
- ✅ Instalador Windows (MSI/EXE)
- ✅ Aplicação polida e profissional
- ✅ Experiência nativa em cada plataforma

---

## Fase 7: Polish e Preparação para Lançamento (1-2 meses)

### 7.1 Otimizações Finais (2 semanas)
- [ ] Profiling completo da aplicação
- [ ] Otimizações de memória
- [ ] Otimizações de CPU
- [ ] Redução de tempo de startup
- [ ] Melhorias de responsividade da UI

### 7.2 Testes Extensivos (2 semanas)
- [ ] Testes em diferentes hardware
- [ ] Testes com catálogos grandes (50k+ fotos)
- [ ] Testes de stress
- [ ] Testes de usabilidade
- [ ] Beta testing com usuários reais
- [ ] Correção de bugs reportados

### 7.3 Documentação (1 semana)
- [ ] Manual do usuário completo
- [ ] Tutoriais em vídeo (opcional)
- [ ] FAQ
- [ ] Guia de início rápido
- [ ] Documentação de atalhos
- [ ] Troubleshooting guide

### 7.4 Website e Marketing (1 semana)
- [ ] Website do produto
- [ ] Screenshots e vídeo demo
- [ ] Página de download
- [ ] Documentação online
- [ ] Blog/changelog

### 7.5 Preparação de Lançamento (1 semana)
- [ ] Versão 1.0.0 final
- [ ] Changelog detalhado
- [ ] Release notes
- [ ] Assets de marketing
- [ ] Plano de comunicação

### Entregáveis Fase 7
- ✅ VintageLightbox v1.0.0
- ✅ Aplicação estável e polida
- ✅ Documentação completa
- ✅ Pronto para usuários finais

---

## Roadmap Futuro (Pós v1.0)

### Features Avançadas (v1.x)
- [ ] Edição local (brush adjustments)
- [ ] Healing e cloning (retoque)
- [ ] Máscaras de ajuste
- [ ] Panoramas e HDR
- [ ] Focus stacking
- [ ] **Phase 2.1: GPU Acceleration**
  - Implement Custom Slint Renderer or integrate `wgpu`.
  - Migrate image processing from CPU (`image` crate) to Metal Compute Shaders.
  - Target: Real-time processing of 24MP+ RAW files.
- [ ] GPU acceleration (WGPU)

### Integração e Extensibilidade (v2.x)
- [ ] Plugin system
- [ ] Integração com serviços cloud
- [ ] Sincronização de catálogo
- [ ] Mobile companion app
- [ ] API para automação
- [ ] Integração com Lightroom (importar catálogos)

### Recursos Colaborativos (v3.x)
- [ ] Compartilhamento de coleções
- [ ] Comentários e anotações
- [ ] Aprovação de cliente online
- [ ] Galeria web integrada

---

## Métricas de Sucesso

### Performance
- [ ] Importação: 100 fotos/min
- [ ] Navegação: 60fps constante
- [ ] Ajuste RAW: < 500ms
- [ ] Exportação: < 2s por foto 24MP
- [ ] Startup: < 3s
- [ ] Memória: < 500MB em idle

### Qualidade
- [ ] Zero crashes em uso normal
- [ ] Nenhuma perda de dados de usuário
- [ ] < 10 bugs críticos no lançamento
- [ ] > 90% satisfação em testes beta

### Usabilidade
- [ ] Novo usuário consegue importar e editar em < 5min
- [ ] Todas as ações principais acessíveis via teclado
- [ ] Documentação cobre 95% dos casos de uso

---

## Gerenciamento de Riscos

### Riscos Técnicos
1. **Performance de RAW Processing**
   - Mitigação: POC na Fase 0, otimizações contínuas

2. **Slint Limitations**
   - Mitigação: Avaliar cedo, plano B com egui

3. **Cross-platform Issues**
   - Mitigação: Testes contínuos em ambas plataformas

4. **Memory Leaks**
   - Mitigação: Profiling regular, Rust ownership ajuda

### Riscos de Escopo
1. **Feature Creep**
   - Mitigação: Roadmap claro, disciplina de MVP

2. **Over-engineering**
   - Mitigação: Implementar o mínimo que funciona primeiro

### Riscos de Tempo
1. **Subestimação de Tarefas**
   - Mitigação: Buffer de 20% em cada fase

2. **Bloqueios Técnicos**
   - Mitigação: Identificar riscos cedo, ter alternativas

---

## Conclusão

Este roadmap é um guia vivo e deve ser ajustado conforme o desenvolvimento avança. 

**Prioridades**:
1. **Fase 1 (MVP)**: Fundamental - sem isso não há produto
2. **Fases 2-3**: Essencial - torna o produto competitivo
3. **Fases 4-5**: Diferenciadores - features únicas de valor
4. **Fases 6-7**: Profissional - torna o produto comercializável

**Recomendação**: Começar com Fase 0 imediatamente, validar tecnologias, e então comprometer-se com o desenvolvimento completo.
