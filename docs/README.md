# Índice da Documentação - VintageLightbox

Bem-vindo à documentação do VintageLightbox! Este índice organiza todos os documentos do projeto.

## 📖 Visão Geral

VintageLightbox é um clone profissional do Adobe Lightroom desenvolvido em Rust com interface Slint, focado em fotógrafos profissionais que precisam de uma solução completa para importação, edição, organização e venda de fotos.

**Status Atual**: 103 testes passando | Domain Layer completo | Use Cases iniciado  
**Veja**: [STATUS.md](STATUS.md) para progresso detalhado

## 📚 Documentos Principais

### 0. [Status do Projeto](STATUS.md) 🆕
**Conteúdo**: Status consolidado da implementação
- 103 testes passando (99 domain + 4 use-cases)
- Progresso por camada (Domain ✅, Use Cases 🚧)
- Métricas e conquistas
- Próximas milestones
- Guia de contribuição

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
- Arquitetura em camadas
- Estrutura de módulos (10+ módulos)
- Padrões de design aplicados
- Fluxo de dados
- Concorrência e gerenciamento de memória
- Diagramas e exemplos de código

**Leia se você quer**: Entender como o sistema é construído internamente.

### 3. [Especificação de Funcionalidades](03-FUNCIONALIDADES.md)
**Conteúdo**: Detalhamento de cada funcionalidade
- 9 módulos funcionais explicados em detalhe
- Interfaces de usuário mockadas
- Fluxos de trabalho típicos
- Atalhos de teclado
- Exemplos de uso

**Leia se você quer**: Entender como cada feature funciona do ponto de vista do usuário.

### 4. [Roadmap de Desenvolvimento](04-ROADMAP.md)
**Conteúdo**: Planejamento de desenvolvimento em 7 fases
- Timeline: 12-18 meses
- Fase 0: Setup e POC (2-3 semanas)
- Fase 1: MVP (2-3 meses)
- Fases 2-7: Features incrementais até v1.0
- Métricas de sucesso
- Gerenciamento de riscos

**Leia se você quer**: Entender o cronograma e prioridades de desenvolvimento.

### 5. [Stack Tecnológico](05-STACK-TECNOLOGICO.md)
**Conteúdo**: Tecnologias, bibliotecas e ferramentas
- Rust como linguagem base
- Slint para UI
- LibRaw/rawler para processamento RAW
- SQLite para banco de dados
- 20+ crates detalhados
- Alternativas consideradas
- Requisitos de sistema

**Leia se você quer**: Entender as escolhas técnicas e dependências.

## 🗺️ Guia de Leitura

### Para Usuários e Fotógrafos
1. Comece pelo [README principal](../README.md)
2. Leia [Funcionalidades](03-FUNCIONALIDADES.md) - foco nas seções de interface
3. Veja o [Roadmap](04-ROADMAP.md) - seção "Fluxo de Trabalho Típico"

### Para Desenvolvedores Novatos no Projeto
1. [README principal](../README.md) - visão geral
2. [Requisitos](01-REQUISITOS.md) - entenda o que construir
3. [Arquitetura](02-ARQUITETURA.md) - entenda a estrutura
4. [Stack Tecnológico](05-STACK-TECNOLOGICO.md) - entenda as ferramentas
5. [Roadmap](04-ROADMAP.md) - veja onde estamos e para onde vamos

### Para Contribuidores
1. [Roadmap](04-ROADMAP.md) - identifique fase atual e próximas tarefas
2. [Arquitetura](02-ARQUITETURA.md) - entenda onde seu código se encaixa
3. [Requisitos](01-REQUISITOS.md) - valide que sua contribuição atende requisitos

### Para Avaliadores Técnicos
1. [Arquitetura](02-ARQUITETURA.md) - decisões de design
2. [Stack Tecnológico](05-STACK-TECNOLOGICO.md) - justificativa das escolhas
3. [Roadmap](04-ROADMAP.md) - viabilidade e planejamento
4. [Requisitos](01-REQUISITOS.md) - completude da especificação

## 📊 Estatísticas da Documentação

- **Total de Páginas**: ~100 páginas equivalentes
- **Requisitos Funcionais**: 62
- **Requisitos Não-Funcionais**: 30+
- **Módulos de Código**: 10
- **Fases de Desenvolvimento**: 7
- **Estimativa de Tempo**: 12-18 meses
- **Crates/Dependências**: 20+

## 🎯 Destaques por Documento

### 01-REQUISITOS.md
- ✅ 62 requisitos funcionais numerados
- ✅ Requisitos de performance quantificados
- ✅ Critérios claros de aceitação

### 02-ARQUITETURA.md
- ✅ Arquitetura em camadas bem definida
- ✅ 10 módulos principais especificados
- ✅ Padrões de design documentados
- ✅ Diagramas de componentes e fluxo

### 03-FUNCIONALIDADES.md
- ✅ 9 módulos funcionais detalhados
- ✅ Mockups de interface em ASCII art
- ✅ Fluxos de trabalho realistas
- ✅ 50+ atalhos de teclado

### 04-ROADMAP.md
- ✅ 7 fases de desenvolvimento
- ✅ Estimativas de tempo por fase
- ✅ Critérios de sucesso mensuráveis
- ✅ Análise de riscos

### 05-STACK-TECNOLOGICO.md
- ✅ 20+ crates documentados
- ✅ Justificativas técnicas
- ✅ Alternativas consideradas
- ✅ Exemplos de código

## 🔗 Links Úteis

### Tecnologias Principais
- [Rust Language](https://www.rust-lang.org/)
- [Slint UI](https://slint.dev/)
- [LibRaw](https://www.libraw.org/)
- [SQLite](https://www.sqlite.org/)

### Inspiração
- [Adobe Lightroom](https://www.adobe.com/products/photoshop-lightroom.html)
- [DarkTable](https://www.darktable.org/)
- [RawTherapee](https://rawtherapee.com/)

### Comunidades
- [Rust Users Forum](https://users.rust-lang.org/)
- [Slint Discussions](https://github.com/slint-ui/slint/discussions)
- [r/rust](https://www.reddit.com/r/rust/)

## 📝 Notas de Atualização

- **v0.1.0** (Dezembro 2025): Documentação inicial completa
  - 5 documentos principais criados
  - Planejamento completo de 7 fases
  - Arquitetura definida
  - Stack tecnológico selecionado

## ✅ Checklist de Completude

- [x] Requisitos funcionais definidos
- [x] Requisitos não-funcionais definidos
- [x] Arquitetura documentada
- [x] Módulos especificados
- [x] Funcionalidades detalhadas
- [x] Roadmap criado
- [x] Stack tecnológico definido
- [x] README principal criado
- [ ] Proof of concept implementado
- [ ] MVP desenvolvido
- [ ] Versão 1.0 lançada

## 🚀 Próximos Passos

Após completar a documentação, os próximos passos são:

1. **Setup do Projeto** (Fase 0)
   - Criar estrutura de diretórios
   - Configurar Cargo workspace
   - Setup CI/CD

2. **Proof of Concept** (Fase 0)
   - Validar Slint UI
   - Validar processamento RAW
   - Validar performance

3. **Desenvolvimento MVP** (Fase 1)
   - Importação básica
   - Edição RAW básica
   - Exportação JPEG

Consulte o [Roadmap](04-ROADMAP.md) para detalhes completos.

---

**Mantenedores**: Adicione seu nome aqui quando contribuir significativamente para a documentação.

**Última Revisão**: Dezembro 2025
