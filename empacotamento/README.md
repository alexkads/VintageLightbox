# Empacotamento e distribuição — tudo nesta máquina

O VintageLightbox **não passa pela App Store nem pela Microsoft Store**. Ele é gerado aqui,
publicado em `recordarfotos.com.br/vintageLightbox`, e a partir da primeira instalação **se atualiza
sozinho**.

```bash
make mac         # .app + .dmg universal (Intel e Apple Silicon)
make linux       # .deb + .AppImage, num contêiner Docker
make tudo        # os dois
make publicar    # sobe dist/ para o site
make             # a lista inteira
```

Por baixo do `make` estão os scripts, que aceitam mais opções:

```bash
./scripts/empacotar.sh mac-universal --publicar   # gera e publica de uma vez
./scripts/empacotar.sh --help
./scripts/publicar.py --seco --notas "corrige o magenta da tonalização"
```

## 🔑 São dois scripts, e isso é desenho

| Script | Alvos | Onde roda |
|---|---|---|
| `scripts/empacotar.sh` | macOS (`.app`, `.dmg`) e Linux (`.deb`, `.AppImage`) | neste Mac |
| `scripts/empacotar.ps1` | Windows (`.msi`, `.exe`) | **numa máquina Windows** |

Decisão do dono, 7/set/2026. Não é divisão por gosto: é o único jeito de cada alvo ser gerado onde
pode ser gerado **e conferido**. Um Windows produzido por cross-compilação seria um binário que
ninguém aqui consegue abrir antes do cliente — e o renderizador inteiro do app depende justamente da
parte que a cross-compilação não alcança.

Os dois leem a **mesma** `empacotamento/packager.toml` e assinam com a **mesma** chave. Publicar é
sempre daqui, porque é aqui que fica a `service_role`.

## O caminho inteiro, de uma vez

```
scripts/empacotar.sh                 scripts/publicar.py            o app instalado
  ├─ cargo build (por alvo)            ├─ sobe para o bucket          ├─ pergunta ao site
  ├─ lipo (o universal do macOS)       │   versoes/<versao>/…         │   /api/vintagelightbox/
  ├─ cargo packager                    └─ escreve ultima.json         │      atualizacao
  │   .app .dmg .deb .AppImage                                        ├─ confere a assinatura
  │   .msi .exe                        Supabase Storage               └─ instala e reabre
  └─ minisign: .sig de cada um         (bucket público)
                                              ▲
                                              │ lê
                          site: /vintageLightbox  e  /api/…/atualizacao
```

## O que é o quê

| Arquivo | Serve a |
|---|---|
| `packager.toml` | **A configuração, única.** Nome, identificador, ícones, entitlements, dependências do `.deb`, geometria do `.dmg`, idiomas do instalador do Windows |
| `chave-publica.txt` | A chave que **confere** a assinatura. É compilada dentro do app (`atualizacao/porta.rs`) |
| `icones/icone-mestre.png` | **A fonte do ícone**, 1024×1024. O `.icns`, o `.ico`, os PNGs e o do site são derivados por `scripts/gerar-icones.sh` — nenhum se edita à mão |
| `macos/entitlements.plist` | Os entitlements do Hardened Runtime, um por um com o motivo |
| `linux/Dockerfile` | A toolchain Linux inteira, para o `.deb` e o `.AppImage` saírem daqui |

O empacotador é o [`cargo-packager`](https://github.com/crabnebula-dev/cargo-packager) — o
`tauri-bundler` extraído para servir app que não é Tauri —, e o updater do app é o
`cargo-packager-updater`, do mesmo autor. É por isso que o `.sig` que sai daqui é exatamente o que o
app sabe conferir.

```bash
cargo install cargo-packager --locked
brew install librsvg imagemagick   # só para regerar os ícones
```

## 🔑 A chave de atualização — o que a segura

Cada pacote é assinado com **minisign** (ed25519). A chave **privada** está em
`~/.vintagelightbox/atualizacao.key`, **fora do repositório**; a pública está aqui e vai compilada
dentro do app.

Isso é o que torna a distribuição fora das lojas segura: o app baixa, confere a assinatura e **só
então** instala. Quem tomasse o servidor de download conseguiria *negar* atualizações — não
conseguiria instalar nada.

⚠️ **Perder a chave privada quebra a atualização de todo app já instalado**, e não há conserto pelo
software: os que estão na rua só aceitam pacote assinado por ela. Guarde uma cópia.

⚠️ **Trocar `chave-publica.txt` por uma que não corresponda** transforma toda atualização em
"assinatura inválida", em silêncio, na máquina do cliente.

Gerar uma (só se ainda não houver):

```bash
cargo packager signer generate --path ~/.vintagelightbox/atualizacao.key
cp ~/.vintagelightbox/atualizacao.key.pub empacotamento/chave-publica.txt
```

## 🚨 Três armadilhas que já custaram uma rodada inteira

Todas descobertas gerando a 0.1.0, em 7/set/2026. Nenhuma delas falha de um jeito que aponte para a
causa.

1. **`--password` sempre, mesmo vazio.** Sem ele o minisign pede a senha no terminal, e num shell não
   interativo a leitura de `/dev/tty` responde `Device not configured (os error 6)`. O efeito é pior
   que um erro: **os pacotes saem, e só o `.sig` não sai** — instalador pronto, atualização morta. O
   script já passa `--password "${VLB_SENHA_DA_CHAVE:-}"`.

2. **O `.dmg` deixa o volume montado quando falha.** Montar a imagem é a última etapa e ela falha por
   fora do nosso controle (o Spotlight indexa o volume e o `hdiutil detach` responde *Resource
   busy*). Quando isso acontece `/Volumes/VintageLightbox` **fica montado**, e a tentativa seguinte
   falha por outro motivo. `desmontar_restos()` limpa antes de cada tentativa.

3. **Não edite `scripts/empacotar.sh` enquanto ele roda.** O bash lê o script por deslocamento de
   bytes, conforme executa: uma edição no meio da execução desloca tudo e ele passa a executar
   fragmentos de linha. O sintoma foi `line 309: --publicar: command not found`, numa linha que não
   tem nada disso.

## Lançar

**Lançar é empurrar uma tag.** O GitHub Actions compila as três plataformas — `macos-14`,
`ubuntu-22.04`, `windows-latest` —, cria o Release com os instaladores e publica o `latest.json` no
GitHub Pages.

```bash
make lancar     # confere a versão, marca vX.Y.Z e empurra
```

🔑 **Não há credencial de nuvem no caminho.** Os arquivos vão para o Releases e o manifesto para o
Pages, os dois do próprio repositório. A única chave que o CI toca é a **minisign**, que assina a
atualização — e ela não dá acesso a nada além disso.

Até 7/set/2026 isto passava pelo Supabase Storage e exigia a `SUPABASE_SERVICE_ROLE_KEY` na máquina
que publicava. A mudança para o GitHub tirou essa chave do caminho por completo, e foi o motivo dela.

⚠️ **Suba a versão antes**, nos **dois** lugares: `[workspace.package]` do `Cargo.toml` e
`packager.toml`. O app compara a própria `CARGO_PKG_VERSION` com a do manifesto — lançar sem subir a
versão não atualiza ninguém. O `make lancar` recusa se os dois divergirem, e recusa também com a
árvore suja: uma tag marcando um estado que não existe no repositório é pior que nenhuma tag.

### O secret que o CI precisa

Um só: **`VLB_CHAVE_ATUALIZACAO`**, com o conteúdo de `~/.vintagelightbox/atualizacao.key`.

```bash
gh secret set VLB_CHAVE_ATUALIZACAO < ~/.vintagelightbox/atualizacao.key
```

🚨 **Tem de ser a mesma chave nas três plataformas e em todo lançamento.** Uma chave diferente faz
todo app já instalado recusar a atualização por assinatura inválida — e o operador vê só "não
consegui atualizar". Perder a privada não tem conserto pelo software.

⚠️ Os gatilhos do workflow são **tag** e **disparo manual**, nunca `pull_request`. É isso que torna
seguro o repositório ser público: PR de fork não alcança secret nenhum.

## Uma máquina por plataforma — e isso é o desenho

| Onde você está | O comando | O que sai |
|---|---|---|
| **macOS** | `make mac` | `.app` + `.dmg` universal (Intel e Apple Silicon) |
| **Linux** | `make linux` | `.deb` + `.AppImage`, nativo |
| **Windows 11** | `.\scripts\empacotar.ps1` | `.msi` + `.exe` |

Um alvo de outro sistema **recusa de imediato** e diz onde rodar — antes de compilar nada.

🚨 **Não é falta de saída: as duas alternativas foram construídas, e as duas funcionaram.** Em
7/set/2026 o `cargo-xwin` gerou um `.exe` de 34,9 MB a partir do Mac, e um contêiner Docker gerou o
`.deb`. As duas foram recusadas pelo dono — *"quero deixar tudo nativo mesmo"* — e o motivo é o
mesmo nos dois casos, e é o que decide:

> **O que sai de uma máquina que não é a de destino, ninguém abre para conferir.**

Um contêiner compila Linux e não tem X11, Wayland nem GPU: ele não abre o app. Um `.exe` cruzado não
roda no Mac. Uma máquina de verdade **gera e confere** — e conferir é metade do trabalho, porque o
que se distribui aqui é um app gráfico, não uma biblioteca.

⏳ **Publicar é sempre do Mac**, porque é lá que fica a `service_role` do Storage. Traga
`dist/<plataforma>/` da máquina que gerou e rode `make publicar` — o `ultima.json` é regerado a
partir do que houver em `dist/`, então acrescentar plataforma é republicar.

### Por que o Windows exige uma máquina Windows### Por que o Windows exige uma máquina Windows

O impedimento é um só, e é preciso: **o `fxc.exe`**.

Em release, o `build.rs` do `gpui 0.2.2` compila os sete shaders HLSL com o `fxc.exe` — o compilador
de shader da Microsoft, que existe **só como binário do Windows** — e grava o resultado num
`shaders_bytes.rs` que o crate inclui. Sem ele o gpui não compila para Windows. E a chamada está
dentro de `#[cfg(target_os = "windows")]`, que num build script é a máquina que **compila**: do macOS
o trecho nem chega a rodar. `cargo-xwin` e `cross` não mudam nada disso — não é falta de toolchain, é
uma dependência de build que só roda no Windows.

Um segundo detalhe **muda o alvo**, e vale no Windows também: o `rsraw-sys` monta os ~200 `.cpp` do
LibRaw e faz `panic!("MSVC is not supported")`. O alvo é **`x86_64-pc-windows-gnu`**, com o g++ do
MSYS2 — nunca o `-msvc`.

#### 🚫 Cross-compilação: **funciona, e foi recusada** — não refazer

Em 7/set/2026 as três saídas foram testadas de verdade. As duas primeiras falharam; **a terceira
funcionou** — e o resultado foi recusado pelo dono.

| Tentativa | Resultado |
|---|---|
| `mingw-w64` + `x86_64-pc-windows-gnu` | ⛔ para em `couldn't read .../shaders_bytes.rs` |
| `cargo-zigbuild` 0.23.4 + zig 0.16 | ⛔ o mesmo erro, byte por byte |
| `ghcr.io/cross-rs/…-windows-msvc` | ⛔ a imagem **não existe** (só a `-gnu`), e a `-gnu` é um contêiner Linux — mesmo erro |
| **`cargo-xwin` 0.23.1 + LLVM + 2 remendos** | ✅ **gerou um `ui-gpui.exe` de 34,9 MB**, `PE32+ x86-64`, zero erros |

O que o `cargo-xwin` exigiu para chegar lá: `brew install llvm` (pelo `llvm-lib`), um remendo no
`rsraw-sys` (tirar o `panic!("MSVC is not supported")`, trocar `-pthread` por `-DLIBRAW_NODLL`) e um
remendo no `gpui 0.2.2` (compilar o HLSL **na abertura do app**, a partir do fonte embutido, em vez
de ler bytes que o `fxc.exe` gera em tempo de build).

🚨 **E é exatamente por isso que foi recusado.** Decisão do dono, 7/set/2026: *"quero deixar tudo
nativo mesmo"*. As razões que a tentativa deixou claras:

- **Dois crates bifurcados para manter** — um deles o framework da interface inteira. Todo upgrade
  do gpui exigiria reaplicar o remendo dos shaders.
- **O caminho de shader remendado é o que o upstream usava só em desenvolvimento.** Passar a entregar
  por ele muda o renderizador do app — a parte de que tudo depende.
- **E nada disso pode ser conferido aqui.** O `.exe` saiu; ninguém neste Mac consegue abri-lo. Um
  defeito sutil nos shaders só apareceria na máquina do cliente.

Uma máquina Windows resolve as três de uma vez: sem fork, sem remendo, e **o build é conferido na
mesma máquina que o gerou**.

#### Conferir o `.ps1` **daqui**, sem Windows

O script do Windows é o único pedaço que não se testa rodando. Mas ele se **valida** no Mac — o
PowerShell 7 roda aqui:

```bash
brew install --cask powershell
pwsh -NoProfile -File scripts/empacotar.ps1 -Conferir    # roda até a lista de pendências
```

🔑 **Faça isso a cada mudança no `.ps1`.** Em 7/set/2026 essa validação achou dois defeitos que só
apareceriam na máquina Windows, e nenhum dos dois daria erro claro:

1. **`where.exe` derrubava o script inteiro.** Com `$ErrorActionPreference = "Stop"`, um `where.exe`
   que falha vira erro terminante em vez de "falta o fxc.exe" — que é o que o operador precisa ler.
2. **🚨 A função de pré-requisitos vazava o pipeline.** Em PowerShell uma função devolve **tudo** que
   escreve, não só o `return`. Um `& rustup target add` solto fazia `Conferir-Prerequisitos` devolver
   `@("saída do rustup…", $false)` — e `-not` de array não vazio é `$false`. **O script compilaria
   meia hora com um pré-requisito faltando**, para morrer no fim. A correção são os `| Out-Host` e o
   `[bool]` na chamada.

**Como fazer**: uma máquina Windows — VM neste Mac ([UTM](https://mac.getutm.app) é gratuito;
Parallels e VMware Fusion servem) ou qualquer PC. Compartilhe o repositório com ela e rode o
PowerShell, que confere cada pré-requisito e diz o comando que instala o que faltar:

```powershell
.\scripts\empacotar.ps1 -Conferir    # só diz o que falta
.\scripts\empacotar.ps1              # gera o .msi e o .exe
```

Ele usa a **mesma** `packager.toml` e a **mesma** chave de assinatura. Três coisas precisam existir
lá; o WiX e o NSIS **não** estão na lista porque o próprio cargo-packager os baixa:

| O quê | Para quê | Como |
|---|---|---|
| Rust | compila o app | `winget install Rustlang.Rustup` |
| g++ do MinGW-w64 | compila o C++ do LibRaw | `winget install MSYS2.MSYS2`, depois `pacman -S mingw-w64-x86_64-gcc` e o `mingw64\bin` no PATH |
| `fxc.exe` (Windows SDK) | os shaders do gpui | `winget install Microsoft.WindowsSDK`, ou `GPUI_FXC_PATH` apontando para ele |

🚨 **A chave de assinatura tem de ser a mesma.** Copie `~/.vintagelightbox/atualizacao.key` do Mac
para `%USERPROFILE%\.vintagelightbox\` na máquina Windows. Uma chave diferente faz todo Windows
instalado recusar a atualização por assinatura inválida — e o operador vê só "não consegui
atualizar".

Depois, traga `dist\windows-x86_64\` de volta para o Mac e rode `./scripts/publicar.py` — publicar
é sempre daqui, porque é aqui que fica a `service_role`.

## Assinatura do macOS — e por que ela não é "passar pela loja"

Hoje o app sai **sem assinatura da Apple**, por decisão do dono. O efeito prático é o Gatekeeper
recusando a primeira abertura com *"não pôde verificar se o item está livre de malware"* — e
oferecendo **só "Mover para o Lixo"**.

🚨 **O caminho de escape mudou, e a instrução antiga engana.** Até o macOS 14 bastava botão direito →
Abrir; **a partir do 15 isso não funciona mais**. Hoje é: clicar em **OK**, ir a **Ajustes do Sistema
→ Privacidade e Segurança**, rolar até o fim e clicar em **Abrir Assim Mesmo**. A página de download
explica assim, com os três passos.

⚠️ **Não há como evitar isso sem pagar a Apple.** O app está apenas com assinatura *ad-hoc* — a que o
linker do Rust põe sozinha, com `TeamIdentifier=not set`. Tirar o aviso exige **Developer ID +
notarização**, que exigem o Apple Developer Program (US$ 99/ano). Qualquer outro "jeito" é trabalho
que o **cliente** tem de fazer, não você.

⚠️ **"Developer ID Application" não é App Store.** É o certificado de distribuição **fora** da loja —
o mesmo que Zed, Docker e Figma usam. Quando existir, é um flag:

```bash
./scripts/empacotar.sh mac-universal --assinar --publicar
```

O script confere o chaveiro e aborta com a lista do que encontrou se não houver um. Para notarizar,
uma destas três no ambiente:

```bash
xcrun notarytool store-credentials --apple-id "..." --team-id "..."
export APPLE_KEYCHAIN_PROFILE="o-nome-que-você-deu"
# ou: APPLE_API_KEY + APPLE_API_ISSUER
# ou: APPLE_ID + APPLE_PASSWORD + APPLE_TEAM_ID
```

A assinatura minisign da atualização é **independente disso** e continua valendo: são duas
assinaturas com propósitos diferentes — a da Apple diz ao Gatekeeper quem publicou; a nossa diz ao
app instalado que o pacote é o mesmo que saiu daqui.

## Trocar o ícone

Troque `icones/icone-mestre.png` (quadrado, mínimo 1024×1024) e rode `./scripts/gerar-icones.sh`. O
`.icns` (10 medidas), o `.ico` (6), os PNGs do Linux e o do site saem todos dele.

⚠️ O script **recusa** mestre não-quadrado ou menor que 1024: ampliar entrega um ícone borrado
justamente no tamanho em que ele mais aparece.

## Subir a versão

A versão está em **dois** lugares — `Cargo.toml` (`[workspace.package]`) e `packager.toml` — porque o
`cargo-packager` não lê o workspace quando se usa `-c`. O script **compara os dois e aborta se
divergirem**: um instalador que mente a versão só aparece na máquina do cliente.

O app compara a **sua própria** `CARGO_PKG_VERSION` com a do manifesto. Publicar sem subir a versão
faz o app achar que já está atualizado.
