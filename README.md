# VintageLightbox 📷

Clone profissional do Adobe Lightroom desenvolvido em Rust com interface GPUI.

## 🎯 Por que ele existe

O fluxo do estúdio passa pelo Lightroom, e o Lightroom **não conversa com o
`recordarfotos.com.br`** — onde o cliente baixa o ensaio que comprou e compra as fotos que ficaram
para trás. Entre a revelação e a galeria há um vão que hoje se atravessa na mão: exportar, separar o
comprado do não comprado, subir, montar a galeria. Cada passo manual é um lugar onde a foto errada
vai para a galeria errada.

**Esta ferramenta existe para fechar esse vão.** O objetivo inteiro, com o teste de alinhamento e os
critérios de "funcional", está em **[docs/00-OBJETIVO.md](docs/00-OBJETIVO.md)**.

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
- 📊 **Domain**: 205 testes, incluindo property-based testing
- ⚠️ **Adapters**: sem testes — é o vão de cobertura conhecido

## 🚀 Tecnologias

- **Linguagem**: Rust (performance e segurança)
- **Interface**: GPUI + gpui-component (nativa, na GPU)
- **RAW Processing**: LibRaw/rawler
- **Database**: SQLite
- **Testing**: cargo test, mockall, proptest, criterion, **gpui::TestAppContext** (interface)
- **Plataformas**: macOS e Windows

## 🎯 Status do Desenvolvimento

**Fase Atual**: Fase 2 — Funcionalidades Essenciais (importação avançada, edição RAW completa,
presets, flags, crop, impressão)

> ✅ **Compila, suíte verde (676 testes), app sobe** — depois de dois consertos e da reescrita da
> tela de importação, feitos em 15/ago/2026 e **ainda não commitados**. O quadro completo, incluindo
> 4 migrations aplicadas em catálogos existentes que não estão no repositório, está em
> **[docs/STATUS.md](docs/STATUS.md)**.

- [x] Workspace com Clean Architecture (5 crates)
- [x] **Domain** — 4 entidades, 13 value objects, **205 testes passando**
- [x] **Use Cases** — 22 módulos (importação, organização, edição, presets, export, print)
- [x] **Infrastructure** — SQLite (15 migrations), cache L1/L2/L3, RAW via LibRaw, EXIF, thumbnails
- [x] **Adapters** — 6 controllers (⚠️ sem testes)
- [x] **UI** — GPUI: Biblioteca, Revelação, Importação, Impressão, segunda tela, docking com arranjo gravado
- [ ] Commitar os consertos e reativar o CI ← **próximo passo**

**676 testes passando · 0 falhas** · `fmt` e `clippy -D warnings` limpos

**Rodando os testes**:
```bash
cargo test --workspace

cargo test -p domain

# Testes da interface (gpui::TestAppContext)
cargo test -p ui-gpui --lib revelacao   # a Revelação
cargo test -p ui-gpui --lib importacao  # a importação

# Atualizar snapshots (quando necessário)
cargo test -p ui-gpui                   # a interface inteira
```

## 📚 Documentação

A documentação completa está em `docs/` — **comece por
[00-OBJETIVO.md](docs/00-OBJETIVO.md)** (o alvo) e
**[PARIDADE-LIGHTROOM.md](docs/PARIDADE-LIGHTROOM.md)** (a fila de trabalho, medida do código):

- **[00-OBJETIVO.md](docs/00-OBJETIVO.md)** - O alvo, o teste de alinhamento, os critérios de "funcional"
- **[PARIDADE-LIGHTROOM.md](docs/PARIDADE-LIGHTROOM.md)** - O que funciona, o que promete e não faz, o que não existe
- **[06-UI-ARCHITECTURE.md](docs/06-UI-ARCHITECTURE.md)** - A interface em GPUI, do código ✅ reescrito em 17/ago
- **[01-REQUISITOS.md](docs/01-REQUISITOS.md)** - Requisitos funcionais e não-funcionais detalhados
- **[02-ARQUITETURA.md](docs/02-ARQUITETURA.md)** - Arquitetura do sistema, módulos e padrões de design
- **[03-FUNCIONALIDADES.md](docs/03-FUNCIONALIDADES.md)** - Especificação detalhada de cada funcionalidade
- **[04-ROADMAP.md](docs/04-ROADMAP.md)** - Planejamento de desenvolvimento em fases
- **[05-STACK-TECNOLOGICO.md](docs/05-STACK-TECNOLOGICO.md)** - Stack completo e dependências
- **[07-E2E-TESTING.md](docs/07-E2E-TESTING.md)** - Como este projeto testa (`gpui::TestAppContext`) ✅ reescrito em 17/ago

## ✨ Principais Funcionalidades

### Importação
- Modal no formato do Lightroom: origem à esquerda, grade de miniaturas ao centro, opções à direita
- Cartões e pastas recentes detectados automaticamente; "incluir subpastas" opcional
- Grade com miniaturas marcáveis, ordenação por captura/nome/tamanho/tipo e lupa (`L`)
- Modos **Add** (catalogar onde está), **Copy** e **Move**
- Suporte a múltiplos formatos RAW (CR2, NEF, ARW, DNG, etc.)
- Detecção automática de duplicatas por hash de conteúdo, com desmarcação na grade
- Miniaturas geradas sob demanda, só para o que está visível
- Organização por data, preservando subpastas, ou numa pasta só

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
│   └── ui-gpui/             # GPUI: telas, painéis do dock, motor de revelação (wgpu)
├── crates/infrastructure/migrations/   # 15 migrations SQLite
├── docs/                    # Documentação (comece por 00-OBJETIVO.md)
└── dev.sh                   # Helper de TDD
```

## 📋 Requisitos do Sistema

### Para Desenvolvedores
- Rust 1.98 ou superior
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
- **Cross-Platform**: Funciona nativamente em macOS, Windows e Linux
- **Atualiza sozinho**: fora das lojas, com pacote assinado e conferido antes de instalar

## ⬇️ Baixar

**https://alexkads.github.io/VintageLightbox/** — macOS (Intel e Apple Silicon), Windows e Linux.

O app **não passa por loja nenhuma** e, a partir da primeira instalação, **se atualiza sozinho** —
cada atualização é conferida por assinatura antes de ser instalada.

> ⚠️ **No macOS**, a primeira abertura pede um passo a mais: o sistema dirá que não pôde verificar o
> app e oferecerá só *Mover para o Lixo*. Clique em **OK**, abra **Ajustes do Sistema → Privacidade e
> Segurança**, role até o fim e clique em **Abrir Assim Mesmo**. É uma vez só.
>
> O truque antigo de *botão direito → Abrir* **não funciona a partir do macOS 15**.

### Ou compile na sua máquina — e o aviso do macOS não aparece

```bash
curl -fsSL https://alexkads.github.io/VintageLightbox/instalar.sh | sh
```

🔑 **Por que isso resolve.** O Gatekeeper interroga o que chegou pela rede **com a marca de
quarentena** (`com.apple.quarantine`), que quem põe é o navegador. Um app que saiu do compilador da
própria máquina nunca teve essa marca: abre no primeiro duplo-clique, sem passar por Ajustes do
Sistema. O aviso acima existe porque o `.dmg` não é assinado com um **Developer ID** — que custa
US$ 99 por ano à Apple, e que este projeto decidiu não pagar (8/set/2026).

O script está em [`docs/instalar.sh`](docs/instalar.sh) — no `docs/`, que é o que o Pages publica, e
é por isso que ele tem um endereço curto. Ele baixa o código da versão publicada, compila só para a
arquitetura desta máquina, monta o `.app` com o mesmo `Info.plist` do instalador oficial e o instala
em `/Applications`.

| | |
|---|---|
| **Exige** | Xcode (grátis) com o componente Metal — o GPUI compila os shaders na build |
| **Instala sozinho** | o Rust, pelo `rustup`, em `~/.cargo`, sem `sudo` |
| **Custa** | 15 a 40 minutos na primeira vez, ~10 GiB em `~/.vintagelightbox` |
| **Opções** | `sh -s -- --versao main`, `--destino <pasta>`, `--seco`, `--ajuda` |

⚠️ **Só macOS, e é de propósito.** No Linux o `.deb` e o `.AppImage` instalam sem interrogatório
nenhum, e no Windows o `.msi` pede *Executar assim mesmo* uma vez — nenhum dos dois tem o problema
que compilar resolve. O script **recusa** fora do macOS, em vez de gastar meia hora para chegar ao
mesmo lugar.

Depois da primeira vez não é preciso recompilar: o app se atualiza sozinho, e a atualização continua
sendo conferida por assinatura minisign — essa parte nunca dependeu da Apple.

### Como os instaladores são gerados

Por **GitHub Actions**, e cada plataforma no sistema dela: `macos-14`, `ubuntu-22.04` e
`windows-latest`. O princípio nunca foi "não usar CI" — era **não gerar de uma plataforma para
outra**, porque o que sai assim ninguém abre para conferir.

Lançar é empurrar uma tag:

```bash
make lancar     # confere a versão, marca vX.Y.Z e empurra
```

O CI compila as três, cria o Release com os instaladores e publica o `latest.json` no Pages.

Para gerar na sua própria máquina — útil para testar antes de lançar:

| Onde você está | O comando |
|---|---|
| macOS | `make mac` → `.app` + `.dmg` universal |
| Linux | `make linux` → `.deb` + `.AppImage` |
| Windows 11 | `.\scripts\empacotar.ps1` → `.msi` + `.exe` |

⚠️ **Gerar de uma máquina para outra funciona — e foi recusado.** Em 7/set/2026 o `cargo-xwin` gerou
um `.exe` válido a partir do Mac, e um contêiner Docker gerou o `.deb`. As duas saíram de cena pelo
mesmo motivo: **o que sai de uma máquina que não é a de destino, ninguém abre para conferir.** O
registro está em [`empacotamento/README.md`](empacotamento/README.md).

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
Comece por:
- [O objetivo](docs/00-OBJETIVO.md) — por que a ferramenta existe, e o que "funcional" quer dizer
- [A fila de trabalho](docs/PARIDADE-LIGHTROOM.md) — medida do código
- [A interface em GPUI](docs/06-UI-ARCHITECTURE.md) — e as armadilhas que já custaram commit

### 4. Setup do Desenvolvimento (Em breve)
```bash
# Será disponibilizado na Fase 0
cargo build
cargo test
cargo run --release -p ui-gpui   # 🚨 --release não é opcional: debug é 57× mais lento por miniatura
```

## 🤝 Contribuindo

Contribuições são bem-vindas! Por favor:

1. Leia a documentação completa
2. Abra uma issue para discutir mudanças grandes
3. Siga os padrões de código Rust (rustfmt, clippy)
4. Escreva testes para novas funcionalidades
5. Atualize a documentação quando necessário

## 📄 Licença

**MIT** — veja [LICENSE](LICENSE). Use, modifique e redistribua, inclusive comercialmente; só
mantenha o aviso de copyright.

Feito pelo **Recordar Fotos Estúdio**, em Gramado e Canela (RS). Nasceu da necessidade de um estúdio
de verdade, e é aberto para quem tiver a mesma.

## 🙏 Agradecimentos

Este projeto é inspirado em:
- Adobe Lightroom
- DarkTable
- RawTherapee

Agradecimentos às comunidades de Rust, GPUI e processamento de imagens open source.

## 📞 Contato

- **Issues**: Use o GitHub Issues para bugs e sugestões
- **Discussões**: Use o GitHub Discussions para perguntas
- **Documentação**: Consulte a pasta `docs/`

---

**Status**: 🟢 A migração para GPUI terminou; o alvo agora é a paridade com o Lightroom — veja [docs/PARIDADE-LIGHTROOM.md](docs/PARIDADE-LIGHTROOM.md)

**Versão**: 0.1.0

**Última Atualização**: 15 de agosto de 2026
