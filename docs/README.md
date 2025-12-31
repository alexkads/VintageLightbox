# Índice da Documentação - VintageLightbox

Bem-vindo à documentação do VintageLightbox! Este índice organiza todos os documentos do projeto.

## 📖 Visão Geral

VintageLightbox é um clone profissional do Adobe Lightroom desenvolvido em Rust com interface egui, focado em fotógrafos profissionais que precisam de uma solução completa para importação, edição, organização e venda de fotos.

**Status Atual**: 360+ testes passando | MVP Completo ✅ | Todas as camadas implementadas  
**Nota**: A UI (egui) está 100% livre de regras de negócio; toda decisão passa por controllers → use cases.
**Veja**: [STATUS.md](STATUS.md) para progresso detalhado

## 📚 Documentos Principais

### 0. [Status do Projeto](STATUS.md) 🆕
**Conteúdo**: Status consolidado da implementação
- 360+ testes passando
- Progresso por camada (Todas completas ✅)
- Métricas e conquistas
- Próximas milestones

**Leia se você quer**: Ver o progresso atual e próximos passos.

### 1. [Requisitos do Sistema](01-REQUISITOS.md)
**Conteúdo**: Requisitos funcionais e não-funcionais completos
- 62 requisitos funcionais detalhados
- Requisitos de performance, usabilidade e compatibilidade
- Critérios de aceitação para MVP
- Restrições técnicas

**Leia se você quer**: Entender o que o sistema deve fazer e suas limitações.

### 2. [Arquitetura do Sistema](02-ARQUITETURA.md)
**Conteúdo**: Design técnico e estrutura do código
- Clean Architecture em 5 camadas
- Estrutura de módulos detalhada
- Sistema de cache multi-nível
- GPU processing pipeline
- Diagramas e exemplos de código

**Leia se você quer**: Entender como o sistema é construído internamente.

### 3. [Especificação de Funcionalidades](03-FUNCIONALIDADES.md)
**Conteúdo**: Detalhamento de cada funcionalidade
- Módulos funcionais explicados em detalhe
- Interfaces de usuário
- Fluxos de trabalho típicos
- Atalhos de teclado
- Exemplos de uso

**Leia se você quer**: Entender como cada feature funciona do ponto de vista do usuário.

### 4. [Roadmap de Desenvolvimento](04-ROADMAP.md)
**Conteúdo**: Planejamento e progresso do desenvolvimento
- Conquistas recentes detalhadas
- Features implementadas
- Próximos passos
- Histórico de desenvolvimento

**Leia se você quer**: Entender o cronograma e prioridades de desenvolvimento.

### 5. [Stack Tecnológico](05-STACK-TECNOLOGICO.md)
**Conteúdo**: Tecnologias, bibliotecas e ferramentas
- Rust como linguagem base
- egui para UI
- wgpu para GPU acceleration
- SQLite para banco de dados
- 20+ crates detalhados

**Leia se você quer**: Entender as escolhas técnicas e dependências.

### 6. [Arquitetura da UI](06-UI-ARCHITECTURE.md)
**Conteúdo**: Design da interface gráfica
- Estrutura de componentes
- Design system (5 temas)
- Sistema de atalhos
- Performance da UI

### 7. [Testes E2E](07-E2E-TESTING.md)
**Conteúdo**: Guia de testes End-to-End
- egui_kittest setup
- Snapshot testing
- Boas práticas

### 8. [Arquitetura de Cache](08-CACHE-ARCHITECTURE.md)
**Conteúdo**: Sistema de cache multi-nível
- ProcessedCache (L0) - 0.01ms
- ImageCache (L1) - 15 imagens
- Preview Cache (SQLite BLOB)
- Prefetch paralelo

## 🗺️ Guia de Leitura

### Para Desenvolvedores
1. [STATUS.md](STATUS.md) - Estado atual do projeto
2. [02-ARQUITETURA.md](02-ARQUITETURA.md) - Entenda a estrutura
3. [05-STACK-TECNOLOGICO.md](05-STACK-TECNOLOGICO.md) - Tecnologias usadas
4. [06-UI-ARCHITECTURE.md](06-UI-ARCHITECTURE.md) - Arquitetura da UI

### Para Avaliadores Técnicos
1. [02-ARQUITETURA.md](02-ARQUITETURA.md) - Decisões de design
2. [05-STACK-TECNOLOGICO.md](05-STACK-TECNOLOGICO.md) - Justificativa das escolhas
3. [08-CACHE-ARCHITECTURE.md](08-CACHE-ARCHITECTURE.md) - Sistema de performance

## 📊 Estatísticas do Projeto

| Métrica | Valor |
|---------|-------|
| Testes | 360+ |
| Cobertura Domain | ~100% |
| Componentes UI | 25+ |
| Temas | 5 |
| Use Cases | 20+ |
| Migrations DB | 16 |

## ✅ Features Implementadas

- [x] Importação de fotos (RAW + JPEG)
- [x] Edição não-destrutiva (13 ajustes)
- [x] GPU acceleration (wgpu)
- [x] Rating (0-5 estrelas)
- [x] Color labels (5 cores)
- [x] Flags (Pick/Reject)
- [x] Coleções
- [x] Exportação JPEG/PNG
- [x] Sistema de cache multi-nível
- [x] 5 temas visuais
- [x] Atalhos de teclado
- [x] Histograma interativo
- [x] Tone curve visualization
- [x] Crop & Straighten
- [x] Noise Reduction
- [x] Sharpening

---

**Última Revisão**: 31 de dezembro de 2025
