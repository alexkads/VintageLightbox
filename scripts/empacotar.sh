#!/usr/bin/env bash
#
# O gerador de instaladores — um comando, os quatro alvos, tudo nesta máquina.
#
#   ./scripts/empacotar.sh mac-arm        → .app + .dmg (Apple Silicon)
#   ./scripts/empacotar.sh mac-intel      → .app + .dmg (Intel)
#   ./scripts/empacotar.sh mac-universal  → .app + .dmg (as duas arquiteturas num binário)
#   ./scripts/empacotar.sh linux          → .deb + .AppImage (x86_64, via Docker)
#   ./scripts/empacotar.sh windows        → .msi + .exe  (exige Windows — leia a mensagem)
#   ./scripts/empacotar.sh tudo           → o que esta máquina consegue
#
# Opções:
#   --assinar     assina e notariza o macOS (exige Developer ID + credenciais)
#   --limpo       apaga dist/ antes
#   --seco        mostra o que faria, sem compilar nada
#
# O empacotador é o `cargo-packager` (crabnebula), que é o `tauri-bundler`
# extraído para servir app que não é Tauri. A configuração inteira está em
# `empacotamento/packager.toml`; este script só compila o binário certo, aponta
# a pasta dele e escolhe os formatos.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG="$RAIZ/empacotamento/packager.toml"
DIST="$RAIZ/dist"
CRATE="ui-gpui"
BIN="ui-gpui"

ASSINAR=0; LIMPO=0; SECO=0; ALVOS=()

for arg in "$@"; do
  case "$arg" in
    --assinar) ASSINAR=1 ;;
    --limpo)   LIMPO=1 ;;
    --seco)    SECO=1 ;;
    -h|--help) sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*)        echo "❌ opção desconhecida: $arg"; exit 1 ;;
    *)         ALVOS+=("$arg") ;;
  esac
done
[[ ${#ALVOS[@]} -eq 0 ]] && ALVOS=(tudo)

diga()  { printf '\n\033[1;36m▸ %s\033[0m\n' "$*"; }
erro()  { printf '\033[1;31m❌ %s\033[0m\n' "$*" >&2; }
ok()    { printf '\033[1;32m✅ %s\033[0m\n' "$*"; }
aviso() { printf '\033[1;33m⚠️  %s\033[0m\n' "$*"; }
correr() { if [[ $SECO -eq 1 ]]; then echo "   [seco] $*"; else "$@"; fi; }

# ── A conferência que evita instalador com a versão errada dentro ────────────
#
# O `packager.toml` carrega a versão porque o `cargo-packager` não lê o
# workspace por conta própria quando se usa `-c`. Duas fontes para o mesmo
# número é uma que envelhece — então elas se conferem aqui, e divergir aborta.
versao_workspace() {
  sed -n '/^\[workspace\.package\]/,/^\[/p' "$RAIZ/Cargo.toml" \
    | sed -n 's/^version *= *"\(.*\)"/\1/p' | head -1
}
versao_packager() {
  sed -n 's/^version *= *"\(.*\)"/\1/p' "$CONFIG" | head -1
}
conferir_versao() {
  local w p; w="$(versao_workspace)"; p="$(versao_packager)"
  if [[ "$w" != "$p" ]]; then
    erro "versão divergente: Cargo.toml diz '$w', packager.toml diz '$p'."
    echo "   Acerte os dois — um instalador que mente a versão só aparece na máquina do cliente."
    exit 1
  fi
  echo "   versão $w"
}

# ── Assinatura do macOS ──────────────────────────────────────────────────────
#
# ⚠️ "Apple Development" **não serve** para distribuir: ela assina para rodar na
#    máquina do próprio time, e o Gatekeeper recusa em qualquer outra. Para um
#    .dmg que o fotógrafo abre é preciso "Developer ID Application", que é outro
#    certificado, emitido no Apple Developer Program.
conferir_assinatura() {
  local ident="${APPLE_SIGNING_IDENTITY:-}"
  if [[ -z "$ident" ]]; then
    ident="$(security find-identity -v -p codesigning 2>/dev/null \
             | sed -n 's/.*"\(Developer ID Application:.*\)"/\1/p' | head -1)"
    [[ -n "$ident" ]] && export APPLE_SIGNING_IDENTITY="$ident"
  fi
  if [[ -z "$ident" ]]; then
    erro "--assinar pedido, mas não há certificado 'Developer ID Application' no chaveiro."
    echo
    echo "   O que existe aqui hoje:"
    security find-identity -v -p codesigning 2>/dev/null | sed 's/^/     /'
    echo
    echo "   'Apple Development' assina para a própria máquina; o Gatekeeper de"
    echo "   qualquer outra recusa. Emita um 'Developer ID Application' em"
    echo "   https://developer.apple.com/account/resources/certificates e importe-o."
    exit 1
  fi
  echo "   assinando como: $ident"
  # A notarização precisa de credencial da Apple. Três formas, e o packager
  # aceita qualquer uma; a de perfil de chaveiro é a que não deixa segredo no
  # ambiente (`xcrun notarytool store-credentials`).
  if [[ -z "${APPLE_TEAM_ID:-}" && -z "${APPLE_API_KEY:-}" && -z "${APPLE_KEYCHAIN_PROFILE:-}" ]]; then
    aviso "sem credencial de notarização — o .dmg sai assinado, mas não notarizado."
    echo "     Defina APPLE_ID + APPLE_PASSWORD + APPLE_TEAM_ID, ou APPLE_API_KEY +"
    echo "     APPLE_API_ISSUER, ou rode 'xcrun notarytool store-credentials'."
  fi
}

# ── Compilar ─────────────────────────────────────────────────────────────────
compilar() {           # compilar <triple>
  local triple="$1"
  diga "compilando $CRATE para $triple"
  correr cargo build --release -p "$CRATE" --target "$triple" --manifest-path "$RAIZ/Cargo.toml"
}

# ⚠️ O binário é **copiado** para `target/empacotamento/` antes de empacotar, e
#    não apontado por `--binaries-dir`: essa opção existe no CLI do
#    cargo-packager 0.11.8, é aceita e é ignorada (o CLI a lê para a struct e
#    nunca a escreve na config). O caminho fixo está em `packager.toml`.
PALCO="$RAIZ/target/empacotamento"

empacotar() {          # empacotar <pasta-do-binário> <triple> <subpasta-dist> <formatos...>
  local bindir="$1" triple="$2" saida="$3"; shift 3
  correr mkdir -p "$PALCO"
  correr cp "$bindir/$BIN" "$PALCO/$BIN"
  # ⚠️ `--formats` recebe **um valor por vez**: `--formats app dmg` faz o clap
  #    ler "dmg" como subcomando e morrer com "unrecognized subcommand".
  local flags=(); for f in "$@"; do flags+=(--formats "$f"); done
  # `--target` aqui não escolhe compilador nenhum (o binário já existe): é o que
  # põe a arquitetura no nome do arquivo — `_aarch64.dmg`, `_universal.dmg`.
  diga "empacotando → dist/$saida"
  correr cargo packager -c "$CONFIG" --target "$triple" -o "$DIST/$saida" "${flags[@]}"
}

# ── macOS ────────────────────────────────────────────────────────────────────
mac() {                # mac <triple> <subpasta>
  local triple="$1" saida="$2"
  [[ "$(uname -s)" == "Darwin" ]] || { erro "'$saida' só se gera no macOS."; return 1; }
  rustup target list --installed | grep -qx "$triple" || correr rustup target add "$triple"
  compilar "$triple"
  empacotar "$RAIZ/target/$triple/release" "$triple" "$saida" app dmg
}

mac_universal() {
  [[ "$(uname -s)" == "Darwin" ]] || { erro "'mac-universal' só se gera no macOS."; return 1; }
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    rustup target list --installed | grep -qx "$t" || correr rustup target add "$t"
    compilar "$t"
  done
  # 🔑 Um binário universal não é dois arquivos: é um só, com as duas
  #    arquiteturas dentro, e o `lipo` é quem costura. O `.app` que sai daqui
  #    abre nativo nos dois Macs — sem Rosetta, sem dois downloads.
  local uni="$RAIZ/target/universal-apple-darwin/release"
  diga "costurando o binário universal (lipo)"
  correr mkdir -p "$uni"
  correr lipo -create \
    "$RAIZ/target/aarch64-apple-darwin/release/$BIN" \
    "$RAIZ/target/x86_64-apple-darwin/release/$BIN" \
    -output "$uni/$BIN"
  [[ $SECO -eq 0 ]] && lipo -info "$uni/$BIN" | sed 's/^/   /'
  empacotar "$uni" "universal-apple-darwin" "macos-universal" app dmg
}

# ── Linux, via Docker ────────────────────────────────────────────────────────
linux() {
  command -v docker >/dev/null || { erro "'linux' precisa do Docker — https://docker.com"; return 1; }
  docker info >/dev/null 2>&1 || { erro "o Docker está instalado mas não está no ar."; return 1; }

  # ⚠️ `--platform linux/amd64` explícito nos dois comandos: num Mac ARM o Docker
  #    escolhe arm64 por padrão, e o alvo aqui é o PC do fotógrafo. Sai por
  #    emulação (lento) — mas sai certo. Para o ARM64 nativo, use `linux-arm`.
  diga "construindo a imagem da toolchain Linux (a primeira vez demora)"
  correr docker build --platform linux/amd64 \
    -f "$RAIZ/empacotamento/linux/Dockerfile" -t vintagelightbox-linux "$RAIZ/empacotamento/linux"

  # 🚨 `CARGO_TARGET_DIR` próprio, e não o `target/` do Mac. O repositório entra
  #    no contêiner por bind mount, então os dois enxergam a **mesma** pasta — e
  #    `target/release` de um Linux por cima de `target/release` de um macOS faz
  #    o cargo recompilar tudo a cada troca, quando não deixa estado quebrado.
  diga "compilando e empacotando dentro do contêiner (x86_64)"
  correr docker run --rm --platform linux/amd64 \
    -v "$RAIZ:/projeto" \
    -v vintagelightbox-cargo:/root/.cargo/registry \
    -e CARGO_TARGET_DIR=/projeto/target/linux-x86_64 \
    -w /projeto vintagelightbox-linux \
    bash -c "cargo build --release -p $CRATE && \
             mkdir -p /projeto/target/empacotamento && \
             cp /projeto/target/linux-x86_64/release/$BIN /projeto/target/empacotamento/ && \
             cargo packager -c empacotamento/packager.toml --target x86_64-unknown-linux-gnu \
               -o /projeto/dist/linux-x86_64 --formats deb --formats appimage"
}

linux_arm() {
  command -v docker >/dev/null || { erro "'linux-arm' precisa do Docker."; return 1; }
  diga "construindo a imagem da toolchain Linux (arm64)"
  correr docker build --platform linux/arm64 -f "$RAIZ/empacotamento/linux/Dockerfile" \
    -t vintagelightbox-linux-arm64 "$RAIZ/empacotamento/linux"
  diga "compilando e empacotando dentro do contêiner (aarch64)"
  correr docker run --rm --platform linux/arm64 \
    -v "$RAIZ:/projeto" \
    -v vintagelightbox-cargo-arm64:/root/.cargo/registry \
    -e CARGO_TARGET_DIR=/projeto/target/linux-aarch64 \
    -w /projeto vintagelightbox-linux-arm64 \
    bash -c "cargo build --release -p $CRATE && \
             mkdir -p /projeto/target/empacotamento && \
             cp /projeto/target/linux-aarch64/release/$BIN /projeto/target/empacotamento/ && \
             cargo packager -c empacotamento/packager.toml --target aarch64-unknown-linux-gnu \
               -o /projeto/dist/linux-aarch64 --formats deb --formats appimage"
}

# ── Windows ──────────────────────────────────────────────────────────────────
windows() {
  if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* || "$(uname -s)" == CYGWIN* ]]; then
    compilar x86_64-pc-windows-msvc
    empacotar "$RAIZ/target/x86_64-pc-windows-msvc/release" x86_64-pc-windows-msvc \
      "windows-x86_64" wix nsis
    return
  fi
  erro "o instalador do Windows não se gera no macOS — e não é falta de ferramenta."
  cat <<'MOTIVO'

   Dois impedimentos, os dois no código de terceiros:

   1. gpui 0.2.2 — o build.rs compila os shaders HLSL dentro de
      `#[cfg(target_os = "windows")]`, que num build script é a máquina que
      **compila**, não a que **roda**. Cruzando do macOS esse trecho nunca
      executa, e o binário sai sem shader nenhum.
      (~/.cargo/registry/src/*/gpui-0.2.2/build.rs, linhas 24-27)

   2. rsraw-sys 0.1 — o build.rs monta os ~200 .cpp do LibRaw e faz
      `panic!("MSVC is not supported")` quando o compilador é o da Microsoft.
      Sobraria o alvo `-gnu`, que o GPUI no Windows não sustenta.

   O caminho que funciona **nesta máquina**: uma VM Windows (UTM é gratuito,
   Parallels e VMware Fusion também servem), com Rust + Visual Studio Build
   Tools + WiX v3 dentro. O repositório se compartilha com a VM e roda-se lá:

       ./scripts/empacotar.sh windows      # em Git Bash, dentro da VM

   Este mesmo script e o mesmo empacotamento/packager.toml — ele detecta que
   está no Windows e segue.
MOTIVO
  return 1
}

# ── Execução ─────────────────────────────────────────────────────────────────
diga "VintageLightbox — gerador de instaladores"
conferir_versao
[[ $ASSINAR -eq 1 ]] && conferir_assinatura
[[ $LIMPO -eq 1 ]] && { diga "limpando dist/"; correr rm -rf "$DIST"; }
command -v cargo-packager >/dev/null || {
  erro "falta o cargo-packager — 'cargo install cargo-packager --locked'"; exit 1; }

FALHOU=()
for alvo in "${ALVOS[@]}"; do
  case "$alvo" in
    mac-arm)       mac aarch64-apple-darwin macos-arm64  || FALHOU+=("$alvo") ;;
    mac-intel)     mac x86_64-apple-darwin  macos-x86_64 || FALHOU+=("$alvo") ;;
    mac-universal) mac_universal                          || FALHOU+=("$alvo") ;;
    linux)         linux                                  || FALHOU+=("$alvo") ;;
    linux-arm)     linux_arm                              || FALHOU+=("$alvo") ;;
    windows)       windows                                || FALHOU+=("$alvo") ;;
    tudo)
      mac_universal || FALHOU+=("mac-universal")
      linux         || FALHOU+=("linux")
      windows       || FALHOU+=("windows")
      ;;
    *) erro "alvo desconhecido: $alvo"; exit 1 ;;
  esac
done

echo
if [[ -d "$DIST" ]]; then
  diga "o que saiu em dist/"
  find "$DIST" -type f \( -name '*.dmg' -o -name '*.deb' -o -name '*.AppImage' \
       -o -name '*.msi' -o -name '*.exe' \) -exec ls -lh {} \; \
    | awk '{printf "   %-8s %s\n", $5, $NF}'
  find "$DIST" -maxdepth 2 -name '*.app' -exec echo "   (bundle) {}" \;
fi

if [[ ${#FALHOU[@]} -gt 0 ]]; then
  echo; aviso "não saíram: ${FALHOU[*]}"
  exit 1
fi
ok "pronto"
