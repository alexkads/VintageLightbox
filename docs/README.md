# Índice da Documentação - VintageLightbox

Bem-vindo à documentação do VintageLightbox! Este índice organiza todos os documentos do projeto.

## 📖 Visão Geral

VintageLightbox é **um clone profissional do Adobe Lightroom em Rust**, com interface **GPUI 0.2 +
gpui-component 0.5**, para importar, organizar, triar, revelar e **entregar** fotos.

🔑 **E ele existe por um motivo específico**: o fluxo do estúdio hoje passa pelo Lightroom, e o
Lightroom não conversa com o `recordarfotos.com.br` — onde o cliente baixa o ensaio que comprou e
compra as fotos que ficaram para trás. Entre a revelação e a galeria há um vão atravessado na mão. A
ferramenta existe para fechá-lo. Está inteiro em [`00-OBJETIVO.md`](00-OBJETIVO.md).

⚠️ **Entre fev e ago/2026 o objetivo foi outro** — migrar a interface de egui para GPUI **com
paridade**. Ele foi alcançado e virou [história](historico/10-MIGRACAO-GPUI.md). A diferença prática
é grande: a regra *"nenhuma feature nova"* caiu, e com ela o motivo de 19 sliders da Revelação
existirem sem fazer nada.

⚠️ **Alguns documentos abaixo ainda descrevem a UI em Slint** — tecnologia avaliada e **nunca usada**.
Cada um traz no topo o que está errado nele. Os que foram reescritos do código estão marcados.

**Estado (6/set/2026)**: ✅ compila, app sobe | **947 testes passando, 0 falhas** | `clippy -D
warnings` limpo

🆕 **O app é sessão-primeiro desde 6/set/2026.** Logado, ele abre na lista de sessões e nada acontece
fora de uma — ver [`09-A-SESSAO-FOTOGRAFICA.md`](09-A-SESSAO-FOTOGRAFICA.md).

## 📚 Documentos Principais

### 0. [O Objetivo](00-OBJETIVO.md) 🆕
**Conteúdo**: o alvo do projeto, o teste de alinhamento, e o que "funcional" quer dizer em critérios
mensuráveis. Também explica **qual objetivo ele substituiu e por quê**.

**Leia se você quer**: saber se o trabalho que você vai começar é o trabalho certo. **Comece por
aqui.**

### 0.1. [O que falta para ser um Lightroom](PARIDADE-LIGHTROOM.md) 🆕
**Conteúdo**: a lista **medida** do que funciona, do que a tela promete e não faz, e do que não
existe — com a fila de trabalho em ordem.

**Leia se você quer**: escolher a próxima tarefa. É a fila.

### 0.2. [Status do Projeto](STATUS.md)
**Conteúdo**: o estado do código camada por camada, testes medidos, e as lacunas conhecidas com a
razão de cada uma.

**Leia se você quer**: entender o estado técnico antes de mexer numa camada.

### 1. [Requisitos do Sistema](01-REQUISITOS.md)
Os 62 requisitos funcionais e não-funcionais. ⚠️ **É o alvo, não o estado** — para saber o que
existe, [`PARIDADE-LIGHTROOM.md`](PARIDADE-LIGHTROOM.md).

### 2. [Arquitetura do Sistema](02-ARQUITETURA.md)
Clean Architecture, as quatro camadas, a regra de dependência. ✅ **O miolo confere com o código** —
`domain`, `use-cases` e `adapters` atravessaram a migração de framework com zero linha alterada.
⚠️ Onde diz "Slint", leia GPUI.

### 3. [Funcionalidades](03-FUNCIONALIDADES.md)
Catálogo detalhado do que o produto pretende fazer. ⚠️ Escrito antes do código; não distingue pronto
de pretendido.

### 4. [Roadmap](04-ROADMAP.md) 🚫 encerrado
O plano de construção e de migração, que terminou em 17/ago/2026. **A fila de agora é a
[`PARIDADE-LIGHTROOM.md`](PARIDADE-LIGHTROOM.md).**

### 5. [Stack Tecnológico](05-STACK-TECNOLOGICO.md)
⚠️ **Defende Slint, que nunca foi usado.** O cabeçalho traz o stack de verdade, medido do
`Cargo.toml`: GPUI, wgpu, sqlx, rsraw, image, tokio.

### 6. [Arquitetura da Interface](06-UI-ARCHITECTURE.md) ✅ reescrito 17/ago
**GPUI, do código.** O mapa dos módulos, o modelo de entidade/`Render`, as portas para o mundo
assíncrono, a ponte de imagem em BGRA, a virtualização, o dock e o tema — com as armadilhas que
custaram commit.

### 7. [Como este projeto testa](07-E2E-TESTING.md) ✅ reescrito 17/ago
`gpui::TestAppContext` no lugar do `egui_kittest`. E a regra que vale mais que todas: **todo teste é
conferido quebrando de propósito**.

### 8. [Arquitetura de Cache](08-CACHE-ARCHITECTURE.md)
Miniaturas e previews em SQLite. ✅ Confere com o código.

### 9. [A sessão fotográfica](09-A-SESSAO-FOTOGRAFICA.md) 🆕
**Conteúdo**: o eixo do app depois de 6/set/2026 — **logado, tudo acontece dentro de uma sessão**.
As quatro telas, o fluxo do estúdio em onze passos, como uma foto pertence a um ensaio, a porta do
app, e a tela de sessão desenhada contra a rota `[id]` do site.

**Leia se você quer**: mexer em qualquer coisa que fale com o `recordarfotos.com.br`, ou entender
por que a "Biblioteca" deixou de ser o lugar onde se escolhe foto. Traz também os defeitos que este
trabalho encontrou e a lista do que ainda falta para a paridade com a tela de lá.

### 10. [A rota do site, da lista até a Revelação](10-A-ROTA-DO-SITE-ATE-A-REVELACAO.md) 🆕
**Conteúdo**: o que `/dashboard/sessoes-fotograficas` faz na web, passo a passo — a porta, a lista,
a sessão (grade em wasm, área temporária, painel, os proxies de imagem), a passagem para o editor e
o que a Revelação faz por baixo (fonte dos pixels, os 53 ajustes, salvar por bilhete, sincronizar).
Com a rota da API de cada gesto e os comentários do dono que explicam cada decisão.

**Leia se você quer**: conferir uma tela de cá contra a de lá sem abrir o site — é a referência de
"mesmo gesto, mesmo resultado".

### 11. [Offline com sincronização](11-OFFLINE-E-SINCRONIZACAO.md) 🆕
**Conteúdo**: o modelo que o desktop e o navegador já seguem — os bytes esperam localmente, a
classificação autoriza a subida —, e o problema que ele ainda tem: **a chave da sincronização é o
nome do arquivo**. Duas `DSC_2571.jpg` de dois cartões não cabem na mesma galeria, e a idempotência
da retomada depende de um dado que não é único. Traz a proposta (chave UUID gerada pelo cliente) em
três fases, e o que cada uma custa.

**Leia se você quer**: mexer no envio de qualquer um dos dois clientes, ou entender por que um
`409 já existe uma foto chamada X` aparece no meio de uma sessão. ⚠️ **É plano, não é código** —
nada dele foi implementado.

## 🗄️ História — [`historico/`](historico/)

Documentos que descrevem decisões tomadas, e que não orientam o próximo commit. **Continuam valendo
como explicação de por que o código é como é.**

| | |
|---|---|
| [10-MIGRACAO-GPUI.md](historico/10-MIGRACAO-GPUI.md) | A migração de egui para GPUI, fase a fase. **O melhor registro das armadilhas do framework** — BGRA, o `uniform` que casa por posição, o foco que não se concede, a fluidez que só se mede em `--release`. ⚠️ As regras da §7 estão **revogadas** |
| [09-MIGRACAO-TAURI.md](historico/09-MIGRACAO-TAURI.md) | A alternativa avaliada e **descartada** em ago/2026 |
| [PARIDADE-UI.md](historico/PARIDADE-UI.md) | Os 146 testes do app de egui virados em lista de comportamentos, antes de ele ser apagado |

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

**Última Revisão**: 15 de agosto de 2026
