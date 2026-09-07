#!/usr/bin/env bash
#
# O gerador de instaladores do **macOS e do Linux**. O Windows é outro script.
#
#   ./scripts/empacotar.sh mac-arm        → .app + .dmg (Apple Silicon)
#   ./scripts/empacotar.sh mac-intel      → .app + .dmg (Intel)
#   ./scripts/empacotar.sh mac-universal  → .app + .dmg (as duas arquiteturas num binário)
#   ./scripts/empacotar.sh linux          → .deb + .AppImage (x86_64, via Docker)
#   ./scripts/empacotar.sh linux-arm      → .deb + .AppImage (aarch64, via Docker)
#   ./scripts/empacotar.sh tudo           → mac-universal + linux
#
# 🔑 **O Windows tem script próprio: `scripts/empacotar.ps1`**, e roda num
#    Windows de verdade. Não é divisão por gosto nem falta de saída: a
#    cross-compilação **funciona** (provada em 7/set/2026 com `cargo-xwin`) e foi
#    **recusada** — decisão do dono, *"quero deixar tudo nativo mesmo"*. O
#    registro do que ela custava está em `empacotamento/README.md`.
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

# ── Linux, via Docker ────────────────────────────────────────────────────────
#
# 🚨 **`--bin ui-gpui` e `--jobs 2`, e os dois por causa de memória.**
#
# O `-p ui-gpui` sozinho compila **cinco** binários: o app e os quatro de
# medição (`medir-*`, `semear-catalogo`). Com `lto = true` e `codegen-units = 1`
# cada um deles linka o programa inteiro — e quatro `rustc` em paralelo numa VM
# de 7,7 GiB estouram a memória. Em 7/set/2026 o build morreu com
# `signal: 9, SIGKILL` compilando `medir-grade-da-sessao`, que **o instalador
# não usa**.
#
# ⚠️ O sintoma não aponta para a causa: `SIGKILL` parece defeito do compilador,
#    e o binário citado na mensagem é de uma ferramenta que ninguém pediu.
#
# A chave de atualização mora no `$HOME` do Mac, e o contêiner não enxerga o
# `$HOME` do Mac: ela entra por bind mount, somente leitura.
#
# ⚠️ **A decisão de assinar é tomada aqui, no host, e não dentro do contêiner.**
#    O comando do contêiner vai numa string entre aspas duplas, então tudo que
#    parece variável dele é expandido **aqui** antes de o docker rodar. Um
#    `${VLB_CHAVE:+…}` escrito lá dentro leria a variável do Mac, que não
#    existe, e sumiria em silêncio — levando junto o `-k` e, com ele, a
#    atualização automática do Linux.
CHAVE_MONTADA=()
FLAG_CHAVE=""
if [[ -f "$CHAVE" ]]; then
  CHAVE_MONTADA=(-v "$CHAVE:/chave.key:ro")
  FLAG_CHAVE="-k /chave.key --password '${VLB_SENHA_DA_CHAVE:-}'"
fi

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
    ${CHAVE_MONTADA[@]+"${CHAVE_MONTADA[@]}"} \
    -e CARGO_TARGET_DIR=/projeto/target/linux-x86_64 \
    -w /projeto vintagelightbox-linux \
    bash -c "cargo build --release -p $CRATE --bin $BIN --jobs 2 && \
             mkdir -p /projeto/target/empacotamento && \
             cp /projeto/target/linux-x86_64/release/$BIN /projeto/target/empacotamento/ && \
             cargo packager -c empacotamento/packager.toml --target x86_64-unknown-linux-gnu \
               -o /projeto/dist/linux-x86_64 --formats deb --formats appimage \
               $FLAG_CHAVE"
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
    ${CHAVE_MONTADA[@]+"${CHAVE_MONTADA[@]}"} \
    -e CARGO_TARGET_DIR=/projeto/target/linux-aarch64 \
    -w /projeto vintagelightbox-linux-arm64 \
    bash -c "cargo build --release -p $CRATE --bin $BIN --jobs 2 && \
             mkdir -p /projeto/target/empacotamento && \
             cp /projeto/target/linux-aarch64/release/$BIN /projeto/target/empacotamento/ && \
             cargo packager -c empacotamento/packager.toml --target aarch64-unknown-linux-gnu \
               -o /projeto/dist/linux-aarch64 --formats deb --formats appimage \
               $FLAG_CHAVE"
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
    linux-arm)     linux_arm                              || FALHOU+=("$alvo") ;;
    windows)       windows; exit 1 ;;
    tudo)
      # 🔑 `tudo` é **o que este script faz**: macOS e Linux. O Windows sai por
      #    `empacotar.ps1`, e não entra aqui nem como falha — contá-lo como tal
      #    faria `--publicar` nunca publicar daqui, que é o oposto de "tudo".
      mac_universal || FALHOU+=("mac-universal")
      linux         || FALHOU+=("linux")
      aviso "o Windows sai por scripts/empacotar.ps1, numa máquina Windows"
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
