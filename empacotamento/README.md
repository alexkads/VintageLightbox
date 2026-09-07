# Empacotamento — os instaladores, gerados nesta máquina

O que este diretório contém, e o comando que o usa:

```bash
./scripts/empacotar.sh mac-universal    # .app + .dmg, Intel e ARM no mesmo binário
./scripts/empacotar.sh linux            # .deb + .AppImage, via Docker
./scripts/empacotar.sh tudo             # o que este Mac consegue
./scripts/empacotar.sh --help
```

A saída vai para `dist/`, uma pasta por alvo.

## O que é o quê

| Arquivo | Serve a |
|---|---|
| `packager.toml` | **A configuração, única.** Nome, identificador, ícones, entitlements, dependências do `.deb`, geometria do `.dmg`, idiomas do instalador do Windows |
| `icones/icone.svg` | **A fonte do ícone.** Todo o resto (`.icns`, `.ico`, os PNGs) é derivado por `scripts/gerar-icones.sh` — nenhum PNG se edita à mão |
| `macos/entitlements.plist` | Os entitlements do Hardened Runtime, um por um com o motivo |
| `linux/Dockerfile` | A toolchain Linux inteira, para o `.deb` e o `.AppImage` saírem daqui |

O empacotador é o [`cargo-packager`](https://github.com/crabnebula-dev/cargo-packager) — o
`tauri-bundler` extraído para servir app que não é Tauri. É ele que produz `.app`, `.dmg`, `.msi`,
`.exe` (NSIS), `.deb`, `.AppImage` e `.pacman` a partir de uma configuração só.

```bash
cargo install cargo-packager --locked
brew install librsvg imagemagick   # só para regerar os ícones
```

## O que sai, alvo por alvo

| Alvo | Formatos | Onde compila | Estado |
|---|---|---|---|
| `mac-arm` | `.app`, `.dmg` | aqui, nativo | ✅ |
| `mac-intel` | `.app`, `.dmg` | aqui, nativo (cruzando para x86_64) | ✅ |
| `mac-universal` | `.app`, `.dmg` | aqui — as duas costuradas com `lipo` | ✅ **é o que se distribui** |
| `linux` | `.deb`, `.AppImage` | contêiner Docker `linux/amd64` | ✅ |
| `linux-arm` | `.deb`, `.AppImage` | contêiner Docker `linux/arm64` | ✅ |
| `windows` | `.msi`, `.exe` | **exige Windows** | ⛔ veja abaixo |

### Por que o Windows não sai do Mac

Não é falta de ferramenta — `cargo-xwin` e `cross` existem, e nenhum dos dois resolve. São dois
impedimentos, os dois em código de terceiros:

1. **`gpui 0.2.2`** compila os shaders HLSL dentro de `#[cfg(target_os = "windows")]` no `build.rs`.
   Num build script esse `cfg` é a máquina que **compila**, não a que **roda** — cruzando do macOS o
   trecho nunca executa e o binário sai sem shader nenhum.
2. **`rsraw-sys 0.1`** (LibRaw, ~200 arquivos `.cpp`) faz `panic!("MSVC is not supported")`. Sobraria
   o alvo `-gnu`, que o GPUI no Windows não sustenta.

**O caminho que funciona nesta máquina** é uma VM Windows — [UTM](https://mac.getutm.app) é gratuito;
Parallels e VMware Fusion servem igual. Dentro dela: Rust, Visual Studio Build Tools, WiX v3 e Git
Bash. O repositório se compartilha com a VM e roda-se **este mesmo script**, que detecta o Windows e
segue:

```bash
./scripts/empacotar.sh windows
```

## Assinatura e notarização do macOS

```bash
./scripts/empacotar.sh mac-universal --assinar
```

⚠️ **"Apple Development" não serve para distribuir.** Ela assina para a máquina do próprio time; o
Gatekeeper de qualquer outra recusa. É preciso um **"Developer ID Application"**, emitido em
[developer.apple.com](https://developer.apple.com/account/resources/certificates) — o script confere
o chaveiro e aborta com a lista do que encontrou se ele não estiver lá.

Sem notarização o `.dmg` abre com "não pode ser verificado" e exige botão direito → Abrir. Para
notarizar, uma destas três no ambiente:

```bash
# a que não deixa segredo no ambiente
xcrun notarytool store-credentials --apple-id "..." --team-id "..."
export APPLE_KEYCHAIN_PROFILE="o-nome-que-você-deu"

# ou App Store Connect API
export APPLE_API_KEY=... APPLE_API_ISSUER=...

# ou Apple ID + senha de app
export APPLE_ID=... APPLE_PASSWORD=... APPLE_TEAM_ID=...
```

## Trocar o ícone

Edite `icones/icone.svg` e rode `./scripts/gerar-icones.sh`. O `.icns` (10 medidas), o `.ico` (6) e
os PNGs do Linux saem todos dele.

## Subir a versão

A versão está em **dois** lugares — `Cargo.toml` (`[workspace.package]`) e `packager.toml` — porque o
`cargo-packager` não lê o workspace quando se usa `-c`. O script **compara os dois e aborta se
divergirem**: um instalador que mente a versão só aparece na máquina do cliente.
