# VintageLightbox 📷

Clone profissional do Adobe Lightroom desenvolvido em Rust com interface GPUI.

> 🖥️ **Um app, um instalador.** O **VintageLightbox (Zed GPUI)** se instala com
> `instalar-vintagelightbox-gpui.cmd` (**[docs/INSTALAR-GPUI.md](docs/INSTALAR-GPUI.md)**). Veja a
> seção **🖥️ Instalar**, mais abaixo.

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
- Windows 10/11, macOS ou Linux (`apt`, `dnf` ou `pacman`)
- 8GB RAM (mais para compilar o GPUI com folga)
- ~10GB livres em disco: o instalador compila o app na própria máquina (veja **🖥️ Instalar**)

## 🎯 Diferenciais

- **100% Rust**: Segurança de memória e performance nativa
- **Interface Nativa**: UI responsiva e fluida
- **Multi-Monitor**: Recurso essencial para fotógrafos
- **Gestão de Vendas**: Integrada diretamente no workflow
- **Open Source**: Transparência e comunidade
- **Cross-Platform**: Funciona nativamente em macOS, Windows e Linux
- **Atualiza sozinho**: fora das lojas, com pacote assinado e conferido antes de instalar

## 🖥️ Instalar — um arquivo, que instala tudo

**Cada máquina compila o próprio app**, com um arquivo só, que roda em Windows, macOS e Linux e
instala sozinho o que falta.

| | **VintageLightbox (Zed GPUI)** |
|---|---|
| **Windows** (baixe e dê dois cliques) | **[instalar-vintagelightbox-gpui.cmd](https://github.com/alexkads/VintageLightbox/releases/download/instalador-tauri/instalar-vintagelightbox-gpui.cmd)** |
| **macOS e Linux** (cole no Terminal) | `curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/instalar-vintagelightbox-gpui.cmd \| sh` |
| **No macOS precisa de** | Command Line Tools (os shaders Metal são compilados na abertura; o Xcode não é preciso) |
| **Primeira vez** | 15 a 40 minutos |
| **Guia completo** | [docs/INSTALAR-GPUI.md](docs/INSTALAR-GPUI.md) |

O mesmo passo a passo, com as telas e os atalhos, está em **https://alexkads.github.io/VintageLightbox/**.

**Antes de começar:** internet ligada, alguns GiB livres no disco (a compilação usa ~10 GiB) e o tempo
da tabela acima. O computador pode ser usado enquanto isso.

### Windows 10 e 11

1. Baixe o `.cmd`, na tabela acima. Se o navegador disser que o arquivo pode ser perigoso,
   escolha **Manter** (no Edge: **…** › Manter).
2. Na pasta **Downloads**, dê dois cliques no arquivo.
3. Se aparecer *"O Windows protegeu o computador"*, clique em **Mais informações** e depois em
   **Executar assim mesmo**. Só acontece na primeira vez.
4. Se o Windows perguntar se permite que um programa faça alterações, clique em **Sim**: são as
   ferramentas que o instalador põe na máquina.
5. Espere a janela preta terminar e aperte qualquer tecla.
6. Abra pelo **Menu Iniciar**: *VintageLightbox (Zed GPUI)*.

O instalador põe sozinho, pelo `winget`: o **Rust** `-gnu`, o **MSYS2** com o compilador, a libclang
e o `windres`, e o **Windows SDK**, de onde vem o `fxc.exe` que compila os shaders. O Visual Studio
**não** é preciso. O arquivo baixado não
envelhece: ao rodar, ele busca a versão mais nova de si mesmo antes de começar.

### macOS

1. Abra o **Terminal** (⌘ Espaço, digite *Terminal*, Enter).
2. Cole a linha do app (tabela acima) e aperte Enter.
3. Se ele disser que faltam as **Command Line Tools**, rode `xcode-select --install`, clique em
   **Instalar**, espere terminar e cole a linha de novo.
4. Espere aparecer *"instalado em /Applications"* e abra pelo Launchpad ou pela pasta Aplicativos.
   **Abre no primeiro clique, sem aviso de segurança** — o app saiu do compilador da própria máquina.

O Rust o instalador põe sozinho, pelo `rustup`, na pasta pessoal e sem senha. O do GPUI instala o
`VintageLightbox (Zed GPUI).app` com o mesmo identificador dos pacotes prontos, então as permissões já
concedidas continuam valendo; um `VintageLightbox.app` antigo não é apagado.

### Linux

1. Abra o Terminal, cole a linha do app (tabela acima) e aperte Enter.
2. Quando pedir, digite a sua senha (ela não aparece enquanto se digita): serve para instalar as
   bibliotecas do sistema.
3. Espere aparecer *"instalado"* e abra pelo menu de aplicativos.

Funciona com **`apt`** (Ubuntu, Debian, Mint), **`dnf`** (Fedora) e **`pacman`** (Arch, Manjaro): o
instalador põe o compilador, a libclang, o X11/Wayland/Vulkan e companhia, e o Rust — um Rust antigo
da distribuição fica de lado. Em outra distribuição, ele lista o que instalar
à mão. Sem `curl`, instale-o antes (`sudo apt install curl` ou `sudo dnf install curl`).

Para importar diretamente de câmeras conectadas por **PTP** (`camera:/` ou
`gphoto2://`), instale também o `gphoto2`, o backend GVfs e as regras USB da
distribuição:

```bash
# Ubuntu/Debian/Mint
sudo apt install gphoto2 gvfs-backends libgphoto2-6

# Fedora
sudo dnf install gphoto2 gvfs-gphoto2 libgphoto2

# Arch/Garuda/Manjaro
sudo pacman -S gphoto2 gvfs libgphoto2
```

Depois de conectar a câmera, ela aparece no menu **Do cartão ou pasta…** como
PTP. O app monta a câmera pelo GVfs, lista as fotos sem baixar o cartão inteiro,
permite escolher somente parte delas e copia apenas as selecionadas para a
sessão. Se a distribuição não usar esses nomes de pacotes, instale os
equivalentes `gphoto2`/`libgphoto2`, `gio`/GVfs e as regras `udev` da câmera.

O `gphoto2` direto é mantido como fallback para sessões sem GVfs; nesse caso a
origem pode precisar ser preparada localmente antes da seleção.

- Sem um driver **Vulkan** (Mesa) a janela não abre, mesmo com tudo compilado.
- **Fedora Workstation (GNOME):** o instalador põe e liga a extensão AppIndicator para o ícone da
  bandeja. O GNOME abre um diálogo pedindo para instalá-la: clique em **Instalar** e o ícone aparece
  na hora — sem o diálogo, saia e entre de novo na sessão.
- **Fedora Silverblue, Kinoite, Bazzite:** o sistema não aceita `dnf install`. O instalador para e
  mostra um `sudo rpm-ostree install …`: rode-o, reinicie e cole a linha de novo.
- **Pouca memória (8 GiB ou menos):** feche os outros programas antes, principalmente para o GPUI.

### Atualizar

**Repita o passo da instalação** — dois cliques no arquivo no Windows, ou a mesma linha no Terminal.
Feche o app antes (pela bandeja, **Sair**). A partir da segunda vez leva poucos minutos, e o catálogo
continua onde estava.

### Opções

| O que | macOS e Linux | Windows (Prompt de Comando) |
|---|---|---|
| Só mostrar o que faria | `… \| sh -s -- --seco` | `set VLB_SECO=1` |
| Outra versão (branch ou tag) | `… \| sh -s -- --versao v0.2.0` | `set VLB_VERSAO=v0.2.0` |
| Outra pasta de instalação | `… \| sh -s -- --destino ~/Apps` | `set VLB_DESTINO=D:\Apps\VintageLightbox` |
| Uma compilação de cada vez (pouca memória) | `… \| CARGO_BUILD_JOBS=1 sh` | `set CARGO_BUILD_JOBS=1` |
| Ver as opções | `… \| sh -s -- --ajuda` | — |

`…` é o `curl -fsSL …` da instalação. **No Windows**, as variáveis só valem se o arquivo for rodado da
mesma janela em que foram definidas (dois cliques não as enxergam): abra o **Prompt de Comando**, digite
o `set` e depois `"%USERPROFILE%\Downloads\instalar-vintagelightbox-gpui.cmd"`.

Para ler o instalador antes de rodar:

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/instalar-vintagelightbox-gpui.cmd -o instalar.cmd
less instalar.cmd
sh instalar.cmd
```

### Onde as coisas ficam

| | macOS | Linux | Windows |
|---|---|---|---|
| O app | `/Applications` | `~/.local/bin/vintagelightbox-gpui` | `%LOCALAPPDATA%\Programs\VintageLightbox-GPUI` |
| O catálogo | Imagens › VintageLightbox, na pasta pessoal (`VintageLightbox Catalog`) | idem | idem |
| Cache do compilador (alguns GiB) | `~/.vintagelightbox/target-gpui` | idem | `%USERPROFILE%\.vintagelightbox\target-gpui` |
| Código baixado | `~/.vintagelightbox/fonte-gpui` | idem | `%USERPROFILE%\.vintagelightbox\fonte-gpui` |

Apagar a pasta `.vintagelightbox` é seguro: a próxima atualização só demora mais.

### Desinstalar

- **macOS:** arraste o app de Aplicativos para o Lixo.
- **Windows:** apague a pasta `%LOCALAPPDATA%\Programs\VintageLightbox-GPUI` e o atalho
  do Menu Iniciar (botão direito › Abrir local do arquivo › apagar).
- **Linux:**
  ```bash
  rm ~/.local/bin/vintagelightbox-gpui \
     ~/.local/share/applications/vintagelightbox-gpui.desktop \
     ~/.local/share/icons/hicolor/256x256/apps/vintagelightbox-gpui.png
  ```

A pasta `.vintagelightbox` pode ir junto. ⚠️ **O catálogo, só depois de conferir que a bandeja diz que
tudo subiu**: fotos que ainda não subiram só existem nele. O Rust e o MSYS2 ficam instalados e podem ser
removidos à parte.

### Quando algo dá errado

| O que aparece | O que fazer |
|---|---|
| O navegador bloqueou o download (Windows) | Na lista de downloads, escolha **Manter**. |
| `o winget … não existe nesta máquina` (Windows) | Instale o [Instalador de Aplicativo](https://apps.microsoft.com/detail/9NBLGGH4NNS1) pela Microsoft Store e repita. |
| `o g++ do MinGW continua faltando` (Windows) | Instale o [MSYS2](https://www.msys2.org) e, no terminal dele, rode `pacman -S mingw-w64-x86_64-gcc`. Repita. |
| `o g++ do MinGW nao compilou um arquivo de teste` (Windows) | O instalador atualiza o MSYS2 e reinstala o compilador sozinho. Se continuar, abra o **MSYS2 MINGW64**, rode `pacman -Syu` duas vezes e repita. |
| `a libclang continua faltando` (Windows) | No terminal do MSYS2, rode `pacman -S mingw-w64-x86_64-clang` e repita. |
| `a compilacao falhou de novo` (Windows) | O instalador já tentou duas vezes. Se a mensagem fala em acesso negado, é o antivírus: em **Segurança do Windows › Proteção contra vírus e ameaças › Exclusões**, adicione `%USERPROFILE%\.vintagelightbox` e `C:\msys64`, e repita. |
| A janela do Windows fechou sozinha | Rode o arquivo de dentro do **Prompt de Comando** para ler a mensagem. |
| `faltam as Command Line Tools do Xcode` (macOS) | `xcode-select --install`, espere terminar e repita. |
| `falta o compilador Metal` (macOS) | É uma cópia antiga do instalador do GPUI: cole a linha de novo — a atual não precisa do Xcode. |
| `command not found: curl` (Linux) | `sudo apt install curl` ou `sudo dnf install curl`, e repita. |
| `não reconheci o gerenciador de pacotes` (Linux) | Instale à mão os pacotes que a mensagem lista e repita. |
| `a instalação dos pacotes falhou ou não há 'sudo'` (Linux) | Peça a quem administra a máquina para rodar o comando mostrado, e repita. |
| `Unable to find libclang` (Linux) | Instale `clang` e `libclang-dev` (Fedora: `clang` e `clang-devel`) e repita. |
| `o Rust continua ausente ou anterior ao 1.89` | `rustup update stable` e repita. |
| Falta de espaço no meio da compilação | Libere alguns GiB e repita. |
| `signal: 9` ou `SIGKILL` no meio da compilação | Faltou memória: feche outros programas e repita com `CARGO_BUILD_JOBS=1` (veja **Opções**). |
| O app não muda depois de atualizar | O antigo continuava aberto: saia pela bandeja (**Sair**) e abra de novo. |
| A tela do cliente não vai para o outro monitor | Confira se o monitor está **estendido**, e não espelhado. No Wayland, arraste a janela uma vez. |
| A lista aparece vazia ou dá erro ao entrar | Confira a internet: os dados vêm da RecordarFotos. As fotos importadas continuam no catálogo. |

### O nome antigo e o gerador

⚠️ **O nome antigo, `instalar-vintagelightbox.cmd`, continua funcionando**: ele só baixa o
`-gpui.cmd` e o roda.

🔧 O `.cmd` é **gerado** de `scripts/instalador-modelo.cmd.in` por
`python3 scripts/gerar-instaladores.py` (`--conferir` só diz se está em dia); os testes são
`python3 scripts/testar-instalador.py`. O link do Windows aponta para um Release — cuja tag ainda se
chama `instalador-tauri`, de quando havia outra interface, e renomeá-la quebraria os links já
copiados — que é atualizado à mão depois de mudar o modelo:

```bash
gh release upload instalador-tauri scripts/instalar-vintagelightbox*.cmd --clobber -R alexkads/VintageLightbox
```

Mesmo um `.cmd` antigo do Release roda a versão nova: ao abrir, ele baixa a do branch `main` e só usa a
própria cópia se estiver sem internet (ou com `VLB_SECO=1`).

## ⬇️ Baixar os pacotes publicados

> ⚠️ **O caminho recomendado é o `instalar-vintagelightbox-gpui.cmd`, na seção acima**, que roda nos
> três sistemas. O que segue são os pacotes publicados e o `instalar.sh`, que é só do macOS.

**https://alexkads.github.io/VintageLightbox/** — a versão publicada hoje só tem o `.dmg` do macOS
(Intel e Apple Silicon).

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
git clone https://github.com/alexkads/VintageLightbox.git
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

### 4. Setup do Desenvolvimento

Para executar o app contra a pilha local do e-commerce, é possível usar o
atalho `make rodar-local`. Ele prepara os contêineres, confere a API e o site e
inicia o app em debug.

Para levantar tudo manualmente — inclusive em modo release — suba primeiro a
pilha local e depois execute o app a partir da raiz deste repositório:

```bash
cd /Users/alexkads/Projects/RecordarFotos/recordarfotos-e-commerce
docker compose -f docker-compose.dev.yml up -d

cd /Users/alexkads/Projects/RecordarFotos/VintageLightbox-Rust
VLB_POS_VENDA_URL=http://localhost:8080 VLB_SITE_URL=http://localhost:8001 cargo run --release -p ui-gpui
```

Se a pilha já estiver ativa, basta executar o app diretamente:

```bash
VLB_POS_VENDA_URL=http://localhost:8080 \
VLB_SITE_URL=http://localhost:8001 \
cargo run --release -p ui-gpui
```

A API local responde em `http://localhost:8080` e o site em
`http://localhost:8001`. O usuário de desenvolvimento é
`admin@recordarfotos.com`, com a senha `admin123`.

Para desenvolvimento iterativo, o script equivalente em debug é:

```bash
./crates/ui-gpui/rodar-local.sh
```

🚨 `--release` não é opcional para medir desempenho: debug é 57× mais lento por
miniatura.

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
