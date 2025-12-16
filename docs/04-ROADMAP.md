# Roadmap de Desenvolvimento - VintageLightbox

## Visão Geral

Este roadmap divide o desenvolvimento em fases incrementais, seguindo **Clean Architecture** e **Test-Driven Development (TDD)**. Cada funcionalidade é implementada com testes primeiro, garantindo qualidade desde o início.

**Metodologia**: TDD (Red-Green-Refactor) em todas as fases  
**Arquitetura**: Clean Architecture (Domain → Use Cases → Adapters → Infrastructure)  
**Estimativa Total**: 12-18 meses (desenvolvimento solo/pequena equipe)

---

## Fase 0: Setup e Fundação (2-3 semanas)

### Objetivos
- Configurar ambiente de desenvolvimento com TDD
- Estrutura base seguindo Clean Architecture
- Setup de ferramentas de teste
- Proof of concept das tecnologias principais

### Tarefas

#### 0.1 Configuração do Projeto (Clean Architecture + TDD)
- [ ] Criar repositório Git
- [ ] Configurar Cargo workspace com estrutura Clean Architecture:
  - `crates/domain` (Camada 1: Entities)
  - `crates/use-cases` (Camada 2: Application Business Rules)
  - `crates/adapters` (Camada 3: Interface Adapters)
  - `crates/infrastructure` (Camada 4: Frameworks & Drivers)
- [ ] Setup de CI/CD com testes automáticos (GitHub Actions)
- [ ] Configurar ferramentas de qualidade:
  - clippy, rustfmt
  - cargo-tarpaulin (cobertura de testes)
  - cargo-watch (TDD watch mode)
  - cargo-nextest (test runner melhorado)
- [ ] Configurar ferramentas de teste:
  - mockall (mocking)
  - proptest (property-based testing)
  - criterion (benchmarking)
  - insta (snapshot testing)
- [ ] README e documentação inicial
- [ ] Template de PR com checklist TDD

#### 0.2 Proof of Concept - Slint UI
- [ ] Criar janela básica com Slint
- [ ] Testar grid de imagens
- [ ] Implementar navegação básica
- [ ] Testar responsividade
- [ ] Validar performance da UI

#### 0.3 Domain Layer - Primeiro Ciclo TDD (1 semana)
- [ ] 🔴 RED: Escrever testes para Value Objects (Rating, PhotoId)
- [ ] 🟢 GREEN: Implementar Value Objects
- [ ] 🔵 REFACTOR: Melhorar design
- [ ] 🔴 RED: Escrever testes para Photo Entity
- [ ] 🟢 GREEN: Implementar Photo Entity básica
- [ ] 🔵 REFACTOR: Extrair comportamentos
- [ ] Meta: 100% cobertura de testes no domain

#### 0.4 Proof of Concept - RAW Processing (Infrastructure)
- [ ] Testes de integração com LibRaw/rawler
- [ ] Implementar RAW Decoder trait
- [ ] Decodificar arquivo RAW de teste
- [ ] Aplicar ajuste básico (exposição)
- [ ] Renderizar preview
- [ ] Benchmark de performance

#### 0.5 Proof of Concept - Database (Infrastructure)
- [ ] TDD: Repository trait (domain)
- [ ] Implementar SQLite Repository
- [ ] Testes de integração: CRUD operations
- [ ] Testes de performance com 10k registros
- [ ] Migrations básicas

### Entregáveis
- ✅ Projeto configurado e compilando
- ✅ CI/CD rodando testes automaticamente
- ✅ Domain layer testado (100% coverage)
- ✅ Demo: Carregar e exibir arquivo RAW
- ✅ Demo: Aplicar ajuste e ver resultado
- ✅ Documentação técnica e de testes

---

## Fase 1: MVP - Core Básico (2-3 meses)

### Objetivos
Criar versão mínima funcional com importação, visualização, edição básica e exportação.  
**Todas as features implementadas com TDD**.

### 1.1 Domain Layer Completo (1 semana - TDD)
- [ ] 🔴🟢🔵 TDD: Adjustment Value Objects
- [ ] 🔴🟢🔵 TDD: Collection Entity
- [ ] 🔴🟢🔵 TDD: Domain Services (DuplicateDetection)
- [ ] 🔴🟢🔵 Property tests com proptest
- [ ] Meta: 100% cobertura no domain

### 1.2 Use Cases: Importação (1 semana - TDD)
- [ ] 🔴 Escrever teste: ImportPhotosUseCase com mocks
- [ ] 🟢 Implementar ImportPhotosUseCase
- [ ] 🔵 Refatorar orquestração
- [ ] 🔴 Escrever teste: ScanDirectoryUseCase
- [ ] 🟢 Implementar ScanDirectoryUseCase
- [ ] 🔵 Refatorar
- [ ] Meta: ≥95% cobertura

### 1.3 Infrastructure: Importação (1 semana)
- [ ] Implementar File Scanner (testes de integração)
- [ ] Implementar EXIF Reader
- [ ] Implementar Thumbnail Generator
- [ ] SQLite Photo Repository
- [ ] Testes de performance: importar 100 fotos

**Critério de Aceitação**: Importar 100 fotos em < 2 minutos

### 1.4 UI: Biblioteca - Visualização (1 semana)
- [ ] Grade de thumbnails
- [ ] Scroll virtual para performance
- [ ] Seleção de fotos (single, multi)
- [ ] Navegação com teclado (setas)
- [ ] Zoom de thumbnail (hover/click)
- [ ] Informações básicas (nome, data, câmera)

**Critério de Aceitação**: Navegar 1000 fotos sem lag

### 1.3 Processamento RAW Básico (3 semanas)
- [ ] Decodificação de formatos principais (CR2, NEF, ARW, DNG)
- [ ] Estrutura de ajustes não-destrutivos
- [ ] Implementar ajustes básicos:
  - [ ] Exposição
  - [ ] Contraste
  - [ ] Temperatura de cor
  - [ ] Tint
  - [ ] Highlights/Shadows
- [ ] Aplicação em tempo real
- [ ] Cache de previews

**Critério de Aceitação**: Ajuste aplicado em < 500ms

### 1.4 UI de Edição (2 semanas)
- [ ] Painel de edição com sliders
- [ ] Vinculação com ajustes RAW
- [ ] Undo/Redo (histórico simples)
- [ ] Reset de ajustes
- [ ] Antes/Depois (tecla \)
- [ ] Salvar ajustes no banco

### 1.5 Classificação Básica (1 semana)
- [ ] Sistema de rating (0-5 estrelas)
- [ ] Atalhos de teclado (0-5)
- [ ] Exibição de rating nos thumbnails
- [ ] Filtro por rating mínimo
- [ ] Persistência no banco

### 1.6 Exportação Básica (2 semanas)
- [ ] Seleção de fotos para exportar
- [ ] Configurações: formato (JPEG), qualidade
- [ ] Redimensionamento simples
- [ ] Aplicação de ajustes na exportação
- [ ] Exportação single-threaded
- [ ] Progress bar
- [ ] Abrir pasta após exportação

**Critério de Aceitação**: Exportar 10 fotos de 24MP em < 20s

### 1.7 Testes e Polish (1 semana)
- [ ] Testes de integração
- [ ] Correção de bugs críticos
- [ ] Melhorias de UX baseadas em uso
- [ ] Documentação de usuário básica

### Entregáveis MVP
- ✅ Aplicação instalável (macOS ou Windows)
- ✅ Importar fotos RAW e JPEG
- ✅ Editar exposição, contraste, temperatura
- ✅ Classificar por estrelas
- ✅ Exportar para JPEG
- ✅ Manual básico do usuário

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
