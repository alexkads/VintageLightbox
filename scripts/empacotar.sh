#!/usr/bin/env bash
#
# O gerador de instaladores. **Ele gera só o sistema em que está rodando.**
#
#   no macOS:  ./scripts/empacotar.sh mac-universal  → .app + .dmg (Intel e ARM)
#              ./scripts/empacotar.sh mac-arm        → só Apple Silicon
#              ./scripts/empacotar.sh mac-intel      → só Intel
#   no Linux:  ./scripts/empacotar.sh linux          → .deb + .AppImage
#   no Windows: .\scripts\empacotar.ps1              → .msi + .exe
#
# 🔑 **Nativo em toda plataforma, e isso é desenho — decisão do dono,
#    7/set/2026: *"quero deixar tudo nativo mesmo"*.**
#
#    Não é falta de saída. As duas alternativas foram construídas e funcionaram:
#    o Windows por `cargo-xwin` (gerou um .exe de 34,9 MB) e o Linux por um
#    contêiner Docker (gerou o .deb). As duas foram recusadas pelo mesmo motivo,
#    e é o que decide: **o que sai delas ninguém abre para conferir.** Um
#    contêiner compila Linux e não tem X11, Wayland nem GPU; um .exe cruzado não
#    roda no Mac. Uma máquina de verdade gera **e** confere.
#
#    O registro do que cada alternativa custava está em
#    `empacotamento/README.md` — leia antes de reconstruir qualquer uma delas.
#
# Opções:
#   --publicar    sobe o resultado para recordarfotos.com.br/vintageLightbox
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

# 🔑 A chave que **assina a atualização** — minisign (ed25519), e nada a ver com
#    a Apple ou a Microsoft. É ela que faz o app instalado recusar um pacote que
#    não saiu daqui: sem essa assinatura, quem controlasse o link de download
#    controlaria o que roda na máquina do fotógrafo.
#
# ⚠️ Fica **fora do repositório**, em ~/.vintagelightbox/. A pública está em
#    `empacotamento/chave-publica.txt` e é compilada dentro do app. Perder a
#    privada significa que nenhum app já instalado aceita atualização de novo —
#    guarde uma cópia em lugar seguro.
CHAVE="${VLB_CHAVE_ATUALIZACAO:-$HOME/.vintagelightbox/atualizacao.key}"

ASSINAR=0; LIMPO=0; SECO=0; PUBLICAR=0; ALVOS=()

for arg in "$@"; do
  case "$arg" in
    --publicar) PUBLICAR=1 ;;
    --assinar) ASSINAR=1 ;;
    --limpo)   LIMPO=1 ;;
    --seco)    SECO=1 ;;
    -h|--help) sed -n '2,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
  # 🔑 `-k` faz duas coisas de uma vez, e as duas são necessárias para o
  #    autoupdate: assina cada pacote (o `.sig` que o app confere antes de
  #    instalar) e, quando a saída é uma **pasta** — o `.app` do macOS —, cria o
  #    `VintageLightbox.app.tar.gz` que é o formato que o updater sabe aplicar.
  #    Sem a chave sai instalador, mas não sai atualização.
  local assinatura=()
  if [[ -f "$CHAVE" ]]; then
    # 🚨 `--password` **sempre**, mesmo vazio. Sem ele o minisign pede a senha
    #    no terminal, e num shell não interativo a leitura de `/dev/tty` falha
    #    com `Device not configured (os error 6)` — que é o que se vê no lugar
    #    de "faltou a senha". O efeito é pior que um erro: os pacotes saem, e é
    #    só o `.sig` que não sai. Instalador pronto, atualização morta.
    assinatura=(-k "$CHAVE" --password "${VLB_SENHA_DA_CHAVE:-}")
  else
    aviso "sem chave de atualização em $CHAVE — os pacotes saem sem .sig,"
    echo  "     e nenhum app instalado vai aceitar esta versão como atualização."
    echo  "     Gere uma com: cargo packager signer generate --path \"$CHAVE\""
  fi

  # `--target` aqui não escolhe compilador nenhum (o binário já existe): é o que
  # põe a arquitetura no nome do arquivo — `_aarch64.dmg`, `_universal.dmg`.
  diga "empacotando → dist/$saida"
  correr cargo packager -c "$CONFIG" --target "$triple" -o "$DIST/$saida" \
    "${flags[@]}" ${assinatura[@]+"${assinatura[@]}"}
}

# ── macOS ────────────────────────────────────────────────────────────────────
#
# 🚨 **Desmontar o que ficou de uma tentativa anterior, antes de tentar de novo.**
#
# Montar o .dmg é a última etapa, e ela falha por fora do nosso controle: o
# Spotlight indexa o volume recém-montado e o `hdiutil detach` responde
# *Resource busy*. Quando isso acontece o volume **fica montado** — e a próxima
# tentativa não falha pelo mesmo motivo, falha por já existir
# `/Volumes/VintageLightbox`, que é um erro que não aponta para a causa.
#
# Aconteceu na primeira geração deste projeto (7/set/2026) e custou uma rodada
# inteira de compilação para ser entendido.
desmontar_restos() {
  local volume="/Volumes/VintageLightbox"
  if [[ -d "$volume" ]]; then
    aviso "sobrou $volume de uma tentativa anterior — desmontando"
    correr hdiutil detach "$volume" -force >/dev/null 2>&1 || true
  fi
}

mac() {                # mac <triple> <subpasta>
  local triple="$1" saida="$2"
  [[ "$(uname -s)" == "Darwin" ]] || { erro "'$saida' só se gera no macOS."; return 1; }
  rustup target list --installed | grep -qx "$triple" || correr rustup target add "$triple"
  compilar "$triple"
  desmontar_restos
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
  desmontar_restos
  empacotar "$uni" "universal-apple-darwin" "macos-universal" app dmg
}

# ── Linux, nativo ────────────────────────────────────────────────────────────
#
# 🔑 **Nativo, num Linux de verdade** — igual ao Windows. Decisão do dono,
#    7/set/2026: cada alvo é gerado onde pode ser gerado **e conferido**.
#
#    Antes isto rodava num contêiner Docker a partir do Mac, e funcionava. Saiu
#    pelo mesmo motivo que a cross-compilação do Windows: o `.deb` que saía dali
#    ninguém abria para ver se instala e roda. Um contêiner compila Linux; ele
#    não tem X11, nem Wayland, nem GPU — não abre o app.
#
# O que a máquina Linux precisa ter, e o que cada coisa resolve. Em Debian ou
# Ubuntu (22.04 ou mais novo):
#
#   build-essential pkg-config curl   o básico de compilar
#   clang libclang-dev                🚨 o `rsraw-sys` gera as ligações do LibRaw
#                                     com bindgen, que carrega a `libclang.so` em
#                                     tempo de execução — sem ela o build morre
#                                     com "Unable to find libclang", que não
#                                     parece falta de pacote
#   libx11-dev libxkbcommon-dev       o que o GPUI abre no Linux
#   libxkbcommon-x11-dev libxcb1-dev
#   libwayland-dev wayland-protocols
#   libxcursor-dev libxrandr-dev libxi-dev
#   libfontconfig1-dev libfreetype6-dev
#   libasound2-dev libssl-dev libvulkan-dev
#   libdbus-1-dev libsecret-1-dev     o chaveiro (Secret Service), que o `keyring` usa
#   patchelf fuse libfuse2 appstream  🚨 o AppImage. O `linuxdeploy` usa o
#   desktop-file-utils zsync           `patchelf` para consertar rpaths, e sem ele
#                                      morre com "subprocess failed (exit code 2)"
#                                      — mensagem que não cita o que faltou
#
# ⚠️ **A glibc da máquina que compila vira o piso do binário.** Compilar num
#    Ubuntu 24.04 gera um `.deb` que não instala no 22.04. Use a distribuição
#    mais **velha** que você pretende atender — 22.04 (glibc 2.35) cobre Ubuntu
#    22.04+, Debian 12+ e Fedora 36+.
conferir_linux() {
  local faltam=()
  command -v cc  >/dev/null || faltam+=("build-essential")
  command -v clang >/dev/null || faltam+=("clang libclang-dev")
  command -v patchelf >/dev/null || faltam+=("patchelf  (o AppImage morre sem ele, e sem dizer que foi ele)")
  for lib in x11 xkbcommon wayland-client fontconfig vulkan openssl dbus-1; do
    pkg-config --exists "$lib" 2>/dev/null || faltam+=("lib${lib}-dev")
  done
  if [[ ${#faltam[@]} -gt 0 ]]; then
    erro "faltam ${#faltam[@]} pré-requisito(s) nesta máquina Linux:"
    printf '     %s\n' "${faltam[@]}"
    echo
    echo "   Em Debian/Ubuntu, o pacotão que resolve tudo:"
    echo "     sudo apt install build-essential pkg-config clang libclang-dev \\"
    echo "       libx11-dev libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev \\"
    echo "       libwayland-dev wayland-protocols libxcursor-dev libxrandr-dev libxi-dev \\"
    echo "       libfontconfig1-dev libfreetype6-dev libasound2-dev libssl-dev \\"
    echo "       libvulkan-dev libdbus-1-dev libsecret-1-dev \\"
    echo "       patchelf fuse libfuse2 desktop-file-utils zsync appstream"
    return 1
  fi
  return 0
}

linux() {
  if [[ "$(uname -s)" != "Linux" ]]; then
    erro "o Linux não sai deste sistema — rode este mesmo script numa máquina Linux."
    cat <<'MOTIVO'

   Lá, o comando é o mesmo:

       ./scripts/empacotar.sh linux      # .deb + .AppImage

   Ele confere os pré-requisitos e diz o `apt install` que resolve o que faltar.

   ⚠️ Copie ~/.vintagelightbox/atualizacao.key para a máquina Linux — tem de ser
   a MESMA chave, senão todo Linux instalado recusa a atualização.

   Depois traga dist/linux-*/ de volta e rode ./scripts/publicar.py.

MOTIVO
    return 1
  fi

  conferir_linux || return 1

  local arco saida
  arco="$(uname -m)"
  case "$arco" in
    x86_64)  saida="linux-x86_64";  triple="x86_64-unknown-linux-gnu" ;;
    aarch64) saida="linux-aarch64"; triple="aarch64-unknown-linux-gnu" ;;
    *) erro "arquitetura não prevista: $arco"; return 1 ;;
  esac

  # 🚨 `--bin ui-gpui`: o `-p ui-gpui` compila **cinco** binários (o app e os
  #    quatro de medição), e com `lto = true` cada um linka o programa inteiro.
  #    Numa máquina modesta isso estoura a memória e o `rustc` morre com
  #    `SIGKILL` — o instalador não usa nenhum dos quatro.
  diga "compilando $CRATE nativo ($arco)"
  correr cargo build --release -p "$CRATE" --bin "$BIN" --manifest-path "$RAIZ/Cargo.toml"

  # ⚠️ **Duas chamadas, e não uma com dois `--formats`.** O cargo-packager
  #    aborta a execução inteira no primeiro formato que falha, e o AppImage é o
  #    frágil dos dois (baixa o `linuxdeploy` da rede e monta um squashfs). Numa
  #    chamada só, uma falha dele levava junto o `.deb` **e a assinatura de
  #    ambos**, que só roda no fim.
  empacotar "$RAIZ/target/release" "$triple" "$saida" deb
  empacotar "$RAIZ/target/release" "$triple" "$saida" appimage \
    || aviso "o AppImage não saiu; o .deb saiu e está assinado"
}

# ── Windows: não é aqui ──────────────────────────────────────────────────────
#
# Este script gera macOS e Linux. O Windows é `scripts/empacotar.ps1`, e roda
# numa máquina Windows — VM neste Mac ou qualquer PC.
#
# 🚨 **Cross-compilação funciona, e foi recusada.** Não é falta de tentativa nem
#    falta de saída — é decisão do dono, 7/set/2026: *"quero deixar tudo nativo
#    mesmo"*.
#
#      mingw-w64 + windows-gnu       → falha em `shaders_bytes.rs`
#      cargo-zigbuild + zig          → o mesmo erro, byte por byte
#      cross-rs …-windows-msvc       → a imagem nem existe
#      cargo-xwin + LLVM + 2 remendos → ✅ **gerou um .exe de 34,9 MB**
#
#    O que o caminho que deu certo custava: bifurcar o `gpui` (o framework da
#    interface inteira) e o `rsraw-sys`, e entregar por um caminho de shader que
#    o upstream só usa em desenvolvimento. E, o que decide, **nada disso se
#    confere aqui**: o .exe saiu e ninguém neste Mac consegue abri-lo.
#
#    Uma máquina Windows resolve as três coisas de uma vez, e o build é
#    conferido onde foi gerado. `empacotamento/README.md` tem o registro inteiro.
windows() {
  erro "o Windows não sai deste script — use scripts/empacotar.ps1, numa máquina Windows."
  cat <<'MOTIVO'

   Lá dentro, um comando:

       .\scripts\empacotar.ps1 -Conferir    # diz o que falta instalar
       .\scripts\empacotar.ps1              # gera o .msi e o .exe

   Ele usa a MESMA empacotamento/packager.toml e a MESMA chave de assinatura —
   copie ~/.vintagelightbox/atualizacao.key para %USERPROFILE%\.vintagelightbox\
   na máquina Windows. Chave diferente faz todo Windows instalado recusar a
   atualização, e o operador vê só "não consegui atualizar".

   Depois traga dist\windows-x86_64\ de volta e rode ./scripts/publicar.py.
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
    windows)       windows; exit 1 ;;
    # 🔑 `tudo` é **o que esta máquina gera**, e numa máquina isso é um sistema
    #    só. Não existe "tudo" que atravesse plataforma — foi justamente o que
    #    saiu de cena em 7/set/2026.
    tudo)
      case "$(uname -s)" in
        Darwin) mac_universal || FALHOU+=("mac-universal") ;;
        Linux)  linux         || FALHOU+=("linux") ;;
        *)      erro "sistema não previsto: $(uname -s)"; exit 1 ;;
      esac
      aviso "os outros sistemas saem nas máquinas deles — veja empacotamento/README.md"
      ;;
    *) erro "alvo desconhecido: $alvo"; exit 1 ;;
  esac
done

echo
if [[ -d "$DIST" ]]; then
  diga "o que saiu em dist/"
  find "$DIST" -type f \( -name '*.dmg' -o -name '*.deb' -o -name '*.AppImage' \
       -o -name '*.msi' -o -name '*.exe' -o -name '*.tar.gz' \) -exec ls -lh {} \; \
    | awk '{printf "   %-8s %s\n", $5, $NF}'
  find "$DIST" -maxdepth 2 -name '*.app' -exec echo "   (bundle) {}" \;
  SIGS=$(find "$DIST" -name '*.sig' | wc -l | tr -d ' ')
  echo "   $SIGS assinatura(s) de atualização (.sig)"
fi

if [[ ${#FALHOU[@]} -gt 0 ]]; then
  echo; aviso "não saíram: ${FALHOU[*]}"
  # 🚨 Não publica pela metade. Um lançamento com o macOS dentro e o Windows
  # fora vira `ultima.json` sem `windows-x86_64`, e todo Windows instalado passa
  # a receber 204 — "nada novo" — para uma versão que existe. Publicar é ato
  # separado justamente para poder ser refeito depois que o alvo que faltou sair.
  [[ $PUBLICAR -eq 1 ]] && aviso "e por isso não publiquei — rode ./scripts/publicar.py quando estiver completo"
  exit 1
fi

if [[ $PUBLICAR -eq 1 ]]; then
  diga "publicando em recordarfotos.com.br/vintageLightbox"
  if [[ $SECO -eq 1 ]]; then
    correr "$RAIZ/scripts/publicar.py" --seco
  else
    "$RAIZ/scripts/publicar.py"
  fi
fi
ok "pronto"
