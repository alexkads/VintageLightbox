#!/bin/sh
#
# VintageLightbox — instalar compilando na sua própria máquina.
#
#     curl -fsSL https://alexkads.github.io/VintageLightbox/instalar.sh | sh
#
# 🐧 **No Linux (Fedora 44 ou mais novo) este script instala o app JÁ COMPILADO**
#    (issue #37): baixa o pacote `.tar.gz` do R2, confere a assinatura minisign
#    **antes** de tocar em qualquer coisa, instala na pasta do usuário (sem sudo)
#    e abre em segundos. Só cai para a compilação se não houver pacote para a
#    versão, a assinatura não bater, a glibc for menor que a do Fedora 44 ou o
#    binário não responder a `--versao`. Uma queda, sem laço.
#
# ⚠️ **No macOS este caminho segue compilando a versão publicada.** Desde
#    2026-09-17 há um instalador de arquivo único para os três sistemas, que
#    compila a branch `main` e dispensa o Xcode:
#      `scripts/instalar-vintagelightbox-gpui.cmd` (docs/INSTALAR-GPUI.md).
#    Este script continua publicado porque o endereço curto está em uso.
#
# 🔑 **Por que este caminho existe.** O `.dmg` pronto abre com um susto: o macOS
#    diz que "não pôde verificar se o item está livre de malware" e oferece só
#    *Mover para o Lixo*. Não é o app ter problema — é ele não estar assinado
#    com um **Developer ID**, que custa US$ 99 por ano à Apple. Decisão do dono,
#    8/set/2026: não pagar.
#
#    O que não custa nada é compilar aqui. O Gatekeeper interroga o que chegou
#    pela rede **com a marca de quarentena** (`com.apple.quarantine`), que o
#    navegador põe no que baixa. Um app que saiu do compilador desta máquina
#    nunca teve essa marca: ele abre no primeiro duplo-clique, sem nenhuma volta
#    por Ajustes do Sistema. É a mesma diferença que faz `cargo run` funcionar
#    hoje e o `.dmg` não.
#
# ⚠️ **Só macOS, e é de propósito.** No Linux o `.deb` e o `.AppImage` instalam
#    sem interrogatório nenhum; no Windows o `.msi` pede um "Mais informações →
#    Executar assim mesmo", uma vez. Nenhum dos dois tem o problema que este
#    script resolve — compilar meia hora para chegar no mesmo lugar seria custo
#    sem troco. Os instaladores dos três sistemas estão em
#    https://alexkads.github.io/VintageLightbox/
#
# ⚠️ **Custa tempo e disco.** A primeira compilação leva de 15 a 40 minutos
#    (é um app nativo inteiro, com o LibRaw junto) e usa ~10 GiB em
#    `~/.vintagelightbox/target`. As seguintes reaproveitam o que já está lá.
#
# Opções (com `curl | sh`, passe-as depois de `sh -s --`):
#
#     curl -fsSL <url> | sh -s -- --versao main     compila o topo do dev
#     curl -fsSL <url> | sh -s -- --destino ~/Apps  onde instalar o .app
#     curl -fsSL <url> | sh -s -- --seco            diz o que faria, sem fazer
#
# 🔑 **Ler antes de rodar é legítimo, e este script não se ofende:**
#
#     curl -fsSL https://alexkads.github.io/VintageLightbox/instalar.sh -o instalar.sh
#     less instalar.sh && sh instalar.sh

set -eu

REPO="https://github.com/alexkads/VintageLightbox"
MANIFESTO="https://alexkads.github.io/VintageLightbox/latest.json"
PAGINA="https://alexkads.github.io/VintageLightbox/"

# A mesma pasta onde vive a chave de atualização de quem gera os pacotes — é a
# casa do projeto nesta máquina. Dentro dela: `fonte/` (descartável) e `target/`
# (o cache do compilador, que é o que faz a segunda compilação ser rápida).
CASA="${VLB_CASA:-$HOME/.vintagelightbox}"
FONTE="$CASA/fonte"

REF=""; DESTINO=""; SECO=0; COMPILAR=0

if [ -t 1 ]; then
  C='\033[1;36m'; V='\033[1;32m'; A='\033[1;33m'; E='\033[1;31m'; N='\033[1m'; Z='\033[0m'
else
  C=''; V=''; A=''; E=''; N=''; Z=''
fi

diga()  { printf "\n${C}▸ %s${Z}\n" "$*"; }
ok()    { printf "${V}✅ %s${Z}\n" "$*"; }
aviso() { printf "${A}⚠️  %s${Z}\n" "$*"; }
erro()  { printf "${E}❌ %s${Z}\n" "$*" >&2; }
correr() { if [ "$SECO" -eq 1 ]; then echo "   [seco] $*"; else "$@"; fi; }

# ⚠️ A ajuda é escrita aqui, e não lida do próprio arquivo: rodando por
#    `curl | sh` o script **não existe em disco** — `$0` é "sh", e um
#    `sed -n ... "$0"` responderia vazio justo no modo em que ele mais é usado.
ajuda() {
  cat <<AJUDA
VintageLightbox — instala compilando nesta máquina.

  curl -fsSL https://alexkads.github.io/VintageLightbox/instalar.sh | sh

Por que compilar: o .dmg pronto não é assinado com Developer ID (US\$ 99/ano da
Apple), e o macOS interroga o que vem baixado. O que sai do compilador daqui
nunca teve marca de quarentena — abre no primeiro duplo-clique.

No macOS compila aqui. No Linux instala o pacote pronto (veja abaixo).

Opções (com curl | sh, passe-as depois de \`sh -s --\`):
  --versao <ref>     tag (v0.1.2) ou branch (main). Padrão: a versão publicada
  --destino <pasta>  onde instalar o .app. Padrão: /Applications
  --seco             diz o que faria, sem compilar nem instalar nada
  --compilar         (Linux) pula o pacote pronto e compila nesta máquina
  --ajuda            isto aqui

No Linux (Fedora 44 ou mais novo) o app vem pronto: baixa, confere a assinatura
e instala em ~/.local, sem sudo e sem compilar. Se não houver pacote para a
versão, ele compila como antes.

Custa 15 a 40 minutos na primeira vez e ~10 GiB em ~/.vintagelightbox/target.
Exige o Xcode (grátis) com o componente Metal; o Rust ele instala se faltar.

⚠️ Este caminho compila a versão publicada e exige o Xcode. O mesmo app,
compilado da branch dev, em qualquer sistema e sem o Xcode:
  curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/instalar-vintagelightbox-gpui.cmd | sh
AJUDA
}

# ⚠️ `--versao` e `--destino` conferem se o valor veio: sem isso, `shift 2` com
#    um argumento só aborta por `set -e` e o script morre sem dizer o motivo.
faltou() { erro "$1 precisa de um valor. Ex.: $1 ${2}"; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --versao)    [ $# -ge 2 ] || faltou --versao v0.1.2; REF="$2"; shift 2 ;;
    --versao=*)  REF="${1#*=}"; shift ;;
    --destino)   [ $# -ge 2 ] || faltou --destino /Applications; DESTINO="$2"; shift 2 ;;
    --destino=*) DESTINO="${1#*=}"; shift ;;
    --seco)      SECO=1; shift ;;
    --compilar)  COMPILAR=1; shift ;;
    -h|--ajuda|--help) ajuda; exit 0 ;;
    *) erro "opção desconhecida: $1"; echo "   as que existem: --versao, --destino, --seco, --compilar, --ajuda"; exit 1 ;;
  esac
done

# ── Linux: o pacote pronto primeiro, a compilação como segunda via ───────────
#
# 🔑 **Por que não aparece aviso nenhum.** O aviso de "desenvolvedor não
#    verificado" vem de uma marca que o *navegador* põe no que baixa; no Linux
#    nem existe. O app chega por `curl`, é conferido por assinatura **nossa**
#    (minisign, a chave pública compilada no próprio app) e entra na pasta do
#    usuário, sem administrador. O código-fonte não viaja: só o binário.
#
# 🔑 **A chave pública é a de `empacotamento/chave-publica.txt`** (ID
#    33D71240F44E3949), a mesma que o app confere nas atualizações. Há um teste
#    (`scripts/testar-instalar-linux.sh`) que prende as duas ao mesmo valor.
BASE="${VLB_BASE:-https://pub-f97274c3a83f47ff903a64fc1578efb0.r2.dev}"
CHAVE_PUBLICA_B64="${VLB_CHAVE_PUBLICA:-dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDMzRDcxMjQwRjQ0RTM5NDkKUldSSk9VNzBRQkxYTTlSM1gxRGhyaDFDL3VYZnplTVp4WldZZGluRUkzZFpVb2lrQ3cvT2JyNUoK}"
COMPILAR_COM="${VLB_COMPILAR_COM:-curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/instalar-vintagelightbox-gpui.cmd | sh}"
APP_ID="vintagelightbox-gpui"

# O motivo da última recusa, para a mensagem de queda dizer o que houve.
MOTIVO=""

# Cai para a compilação — **uma vez só**. O `exec` troca este processo pelo
# instalador de compilação: não há como voltar e tentar o pacote de novo.
cair_para_compilacao() {
  aviso "$1"
  diga "compilando nesta máquina, como antes (15 a 40 minutos na primeira vez)"
  if [ "$SECO" -eq 1 ]; then
    echo "   [seco] $COMPILAR_COM"
    exit 0
  fi
  exec sh -c "$COMPILAR_COM"
}

# `1.2.3` → número comparável. Só os dois primeiros campos: a glibc é `2.43`.
glibc_numero() { echo "$1" | awk -F. '{ printf "%d", ($1 * 1000) + $2 }'; }

# Campo de uma plataforma no manifesto (JSON de uma linha ou de várias).
# Sem `jq` de propósito: o balcão não o tem, e o formato é o nosso.
campo_do_bloco() { # $1 bloco  $2 chave
  printf '%s' "$1" | sed -n 's/.*"'"$2"'"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1
}

# Confere o `.minisig` (em base64, como o manifesto o guarda) contra o arquivo.
# 🚨 Esta função é a **única porta** do pacote: nada é extraído, executado ou
#    copiado antes de ela devolver 0. Ed25519 e BLAKE2b vêm do `openssl`, que
#    todo Fedora tem — o cliente não precisa instalar o `minisign`.
verificar_assinatura() { # $1 arquivo  $2 assinatura-em-base64
  T="$(mktemp -d)" || { MOTIVO="sem pasta temporária"; return 1; }
  printf '%s' "$CHAVE_PUBLICA_B64" | base64 -d 2>/dev/null | sed -n 2p | base64 -d > "$T/pk.bin" 2>/dev/null
  printf '%s' "$2" | base64 -d 2>/dev/null | sed -n 2p | base64 -d > "$T/sg.bin" 2>/dev/null
  if [ "$(wc -c < "$T/pk.bin")" -ne 42 ] || [ "$(wc -c < "$T/sg.bin")" -ne 74 ]; then
    MOTIVO="a assinatura do pacote está malformada"; rm -rf "$T"; return 1
  fi
  ID_CHAVE="$(head -c 10 "$T/pk.bin" | tail -c 8 | od -An -tx1 | tr -d ' \n')"
  ID_ASSIN="$(head -c 10 "$T/sg.bin" | tail -c 8 | od -An -tx1 | tr -d ' \n')"
  if [ "$ID_CHAVE" != "$ID_ASSIN" ]; then
    MOTIVO="o pacote foi assinado por outra chave"; rm -rf "$T"; return 1
  fi
  ALG="$(head -c 2 "$T/sg.bin")"
  # DER de uma chave pública Ed25519 = cabeçalho fixo (12 bytes) + os 32 da chave.
  { printf '%s' 'MCowBQYDK2VwAyEA' | base64 -d; tail -c 32 "$T/pk.bin"; } > "$T/pk.der"
  tail -c 64 "$T/sg.bin" > "$T/sig.raw"
  case "$ALG" in
    ED) openssl dgst -blake2b512 -binary "$1" > "$T/msg" 2>/dev/null ;; # minisign padrão: assina o BLAKE2b-512
    Ed) cp "$1" "$T/msg" ;;
    *)  MOTIVO="formato de assinatura desconhecido"; rm -rf "$T"; return 1 ;;
  esac
  if openssl pkeyutl -verify -pubin -inkey "$T/pk.der" -keyform DER -rawin -in "$T/msg" -sigfile "$T/sig.raw" >/dev/null 2>&1; then
    rm -rf "$T"; return 0
  fi
  MOTIVO="a assinatura não confere com o pacote (arquivo adulterado ou corrompido)"
  rm -rf "$T"; return 1
}

instalar_pacote_linux() {
  if [ "$COMPILAR" -eq 1 ]; then
    cair_para_compilacao "pedido de compilação (--compilar)"
  fi

  diga "conferindo esta máquina"
  for f in curl tar openssl base64 od sed awk mktemp; do
    command -v "$f" >/dev/null 2>&1 || cair_para_compilacao "falta '$f' para baixar e conferir o pacote"
  done
  case "$(uname -m)" in
    x86_64) ARQ="x86_64" ;;
    *) cair_para_compilacao "só há pacote para x86_64 (esta máquina é $(uname -m))" ;;
  esac
  # glibc desta máquina. `ldd --version` do musl (Alpine) não diz "GNU libc".
  LDD="$(ldd --version 2>&1 | head -1)"
  case "$LDD" in
    *"GNU libc"*|*"GLIBC"*|*"glibc"*) GLIBC_AQUI="$(printf '%s' "$LDD" | sed -n 's/.* \([0-9][0-9]*\.[0-9][0-9]*\)[^0-9.]*$/\1/p; s/.* \([0-9][0-9]*\.[0-9][0-9]*\)$/\1/p' | head -1)" ;;
    *) cair_para_compilacao "esta máquina não usa a glibc ($LDD)" ;;
  esac
  [ -n "$GLIBC_AQUI" ] || cair_para_compilacao "não consegui ler a versão da glibc ($LDD)"
  ok "Linux $ARQ, glibc $GLIBC_AQUI"

  diga "perguntando ao manifesto qual é a versão do pacote"
  MANIFESTO_LINUX="$BASE/latest-linux.json"
  if ! JSON="$(curl -fsSL --max-time 30 "$MANIFESTO_LINUX" 2>/dev/null | tr -d '\n\r')" || [ -z "$JSON" ]; then
    cair_para_compilacao "ainda não há pacote publicado para o Linux ($MANIFESTO_LINUX)"
  fi
  VERSAO_PACOTE="$(campo_do_bloco "$JSON" version)"
  BLOCO="$(printf '%s' "$JSON" | sed -n 's/.*"linux-'"$ARQ"'"[[:space:]]*:[[:space:]]*{\([^}]*\)}.*/\1/p')"
  [ -n "$VERSAO_PACOTE" ] && [ -n "$BLOCO" ] \
    || cair_para_compilacao "o manifesto não traz pacote para linux-$ARQ"
  URL_PACOTE="$(campo_do_bloco "$BLOCO" url)"
  ASSINATURA="$(campo_do_bloco "$BLOCO" signature)"
  GLIBC_MINIMA="$(campo_do_bloco "$BLOCO" glibc)"
  [ -n "$URL_PACOTE" ] && [ -n "$ASSINATURA" ] \
    || cair_para_compilacao "o manifesto do pacote está incompleto"
  case "$URL_PACOTE" in https://*|http://127.0.0.1*|http://localhost*) : ;; *) cair_para_compilacao "o endereço do pacote não é HTTPS" ;; esac
  ok "pacote da versão $VERSAO_PACOTE"

  # 🔑 **O piso do Linux é o Fedora 44** (dono, 18/11/2026). O pacote diz a
  #    maior glibc que o binário pede; menor que isso, ele nem abriria.
  if [ -n "$GLIBC_MINIMA" ] && [ "$(glibc_numero "$GLIBC_AQUI")" -lt "$(glibc_numero "$GLIBC_MINIMA")" ]; then
    cair_para_compilacao "a glibc daqui ($GLIBC_AQUI) é menor que a do pacote ($GLIBC_MINIMA, a do Fedora 44)"
  fi

  if [ "$SECO" -eq 1 ]; then
    echo "   [seco] baixaria $URL_PACOTE, conferiria a assinatura e instalaria em ${DESTINO:-$HOME/.local}"
    exit 0
  fi

  TRABALHO="$(mktemp -d)" || cair_para_compilacao "sem pasta temporária"
  trap 'rm -rf "$TRABALHO"' EXIT INT TERM

  diga "baixando o pacote"
  if ! curl -fsSL --max-time 900 -o "$TRABALHO/pacote.tar.gz" "$URL_PACOTE"; then
    cair_para_compilacao "não consegui baixar $URL_PACOTE"
  fi
  ok "$(du -h "$TRABALHO/pacote.tar.gz" | cut -f1) baixados"

  diga "conferindo a assinatura (nada é aberto antes disto)"
  verificar_assinatura "$TRABALHO/pacote.tar.gz" "$ASSINATURA" \
    || cair_para_compilacao "pacote recusado: $MOTIVO"
  ok "assinatura confere com a chave do app"

  diga "abrindo o pacote"
  mkdir "$TRABALHO/pacote"
  # `--no-same-owner` e sem caminho absoluto nem `..`: o pacote é nosso, mas a
  # extração não confia no nome de nada que vem dentro.
  tar -xzf "$TRABALHO/pacote.tar.gz" -C "$TRABALHO/pacote" --strip-components=1 --no-same-owner 2>/dev/null \
    || cair_para_compilacao "o pacote não abre"
  NOVO="$TRABALHO/pacote/bin/$APP_ID"
  [ -x "$NOVO" ] || cair_para_compilacao "o pacote não traz o binário $APP_ID"

  # 🔑 A prova de vida que a atualização também faz: o binário novo responde
  #    a `--versao` com a versão anunciada, antes de trocar o instalado.
  VERSAO_BINARIO="$("$NOVO" --versao 2>/dev/null | head -1)" || VERSAO_BINARIO=""
  if [ "$VERSAO_BINARIO" != "$VERSAO_PACOTE" ]; then
    cair_para_compilacao "o binário do pacote não abre aqui (respondeu '${VERSAO_BINARIO:-nada}', esperado $VERSAO_PACOTE)"
  fi
  ok "o binário abre e diz $VERSAO_BINARIO"

  [ -n "$DESTINO" ] || DESTINO="$HOME/.local"
  diga "instalando em $DESTINO"
  # 🔑 **O mesmo lugar do instalador por compilação**, e não uma pasta própria
  #    com link: `atualizacao::compilar::binario_instalado()` procura o app em
  #    `~/.local/bin/vintagelightbox-gpui` e troca ali, guardando o `.anterior`.
  mkdir -p "$DESTINO/bin" "$DESTINO/share/applications" "$DESTINO/share/icons/hicolor" \
    || { erro "não consegui criar as pastas em $DESTINO"; exit 1; }
  install -m 755 "$NOVO" "$DESTINO/bin/.$APP_ID.novo" \
    || { rm -f "$DESTINO/bin/.$APP_ID.novo"; erro "não consegui copiar o app; o instalado continua como estava"; exit 1; }
  if [ -f "$DESTINO/bin/$APP_ID" ]; then
    cp -p "$DESTINO/bin/$APP_ID" "$DESTINO/bin/$APP_ID.anterior" 2>/dev/null || true
  fi
  mv -f "$DESTINO/bin/.$APP_ID.novo" "$DESTINO/bin/$APP_ID" \
    || { rm -f "$DESTINO/bin/.$APP_ID.novo"; erro "a troca falhou; o instalado continua como estava"; exit 1; }

  if [ -d "$TRABALHO/pacote/share/icons/hicolor" ]; then
    (cd "$TRABALHO/pacote/share/icons/hicolor" && find . -type f -name '*.png') | while read -r icone; do
      mkdir -p "$DESTINO/share/icons/hicolor/$(dirname "$icone")"
      cp "$TRABALHO/pacote/share/icons/hicolor/$icone" "$DESTINO/share/icons/hicolor/$icone"
    done
  fi
  # O `Exec` leva o caminho inteiro: `~/.local/bin` nem sempre está no PATH do menu.
  sed "s|^Exec=.*|Exec=$DESTINO/bin/$APP_ID|" "$TRABALHO/pacote/share/applications/$APP_ID.desktop" \
    > "$DESTINO/share/applications/$APP_ID.desktop"
  command -v update-desktop-database >/dev/null 2>&1 \
    && update-desktop-database "$DESTINO/share/applications" >/dev/null 2>&1 || true
  command -v gtk-update-icon-cache >/dev/null 2>&1 \
    && gtk-update-icon-cache -q -t "$DESTINO/share/icons/hicolor" >/dev/null 2>&1 || true

  printf "\n${V}✅ VintageLightbox %s instalado em %s${Z}\n\n" "$VERSAO_PACOTE" "$DESTINO"
  printf "   ${N}Abra pelo menu de aplicativos, ou:${Z}  %s\n\n" "$DESTINO/bin/$APP_ID"
  echo "   Veio pronto: nada foi compilado aqui, e não houve aviso de segurança —"
  echo "   o pacote foi conferido pela assinatura do próprio app."
  echo "   As próximas versões o app baixa sozinho; se faltar pacote, ele compila."
  case ":$PATH:" in
    *":$DESTINO/bin:"*) : ;;
    *) echo; aviso "$DESTINO/bin não está no seu PATH; o menu de aplicativos abre o app normalmente." ;;
  esac
  trap - EXIT INT TERM
  rm -rf "$TRABALHO"
  exit 0
}

# ── Este sistema é o certo? ───────────────────────────────────────────────────
case "$(uname -s)" in
  Linux)  instalar_pacote_linux ;;
  Darwin) : ;;
  *)
    erro "este script é do macOS e do Linux."
    echo
    echo "   No Windows use o PowerShell:  irm https://alexkads.github.io/VintageLightbox/instalar.ps1 | iex"
    echo "   Os instaladores estão em: $PAGINA"
    exit 1 ;;
esac

# ── O que precisa existir antes de qualquer compilação ────────────────────────
#
# 🚨 Toda conferência acontece **antes** do primeiro `cargo build`. Uma
#    compilação de meia hora que morre no fim por falta de ferramenta é o pior
#    desfecho possível — e foi o que já aconteceu gerando o instalador do
#    Windows (empacotamento/README.md).
diga "conferindo o que esta máquina tem"

for f in curl tar; do
  command -v "$f" >/dev/null 2>&1 || { erro "falta '$f', que vem com o macOS — algo está muito fora do lugar."; exit 1; }
done

# 🚨 **O GPUI compila os shaders Metal em tempo de build**, e o compilador
#    `metal` **não vem** nas Command Line Tools: ele vem no Xcode, e a partir do
#    Xcode 26 é um componente que se baixa à parte. Sem ele o build morre depois
#    de compilar centenas de crates, com "cannot execute tool 'metal' due to
#    missing Metal Toolchain" — que é uma mensagem clara, meia hora tarde demais.
#
#    `xcrun -f metal` responde pelos dois de uma vez: se o `xcode-select` aponta
#    para as Command Line Tools, ou se o componente não foi baixado, ele falha.
if ! xcrun -f metal >/dev/null 2>&1; then
  erro "falta o compilador Metal — o app não compila sem ele."
  echo
  printf "   ${N}O Xcode é grátis${Z} (o que custa US\$ 99 é o certificado, que este caminho dispensa).\n"
  echo
  echo "   1. Instale o Xcode pela App Store:"
  echo "      https://apps.apple.com/app/xcode/id497799835"
  echo "   2. Aponte as ferramentas para ele:"
  echo "      sudo xcode-select -s /Applications/Xcode.app"
  echo "   3. Baixe o componente do Metal:"
  echo "      xcodebuild -downloadComponent MetalToolchain"
  echo
  echo "   Depois rode este script de novo."
  echo
  # 🔑 Quem chega aqui quase sempre pode usar o instalador do GPUI,
  #    que não precisa do Xcode (dono, 2026-09-16).
  printf "   ${N}Não quer instalar o Xcode?${Z} O instalador do GPUI não\n"
  echo "   precisa dele nem do Metal:"
  echo "      curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/instalar-vintagelightbox-gpui.cmd | sh"
  exit 1
fi
ok "Xcode e Metal: $(xcode-select -p)"

# O Rust. Se não houver, instala o rustup — que é o jeito oficial e não pede
# privilégio nenhum (tudo vai para ~/.cargo e ~/.rustup).
if ! command -v cargo >/dev/null 2>&1; then
  # shellcheck disable=SC1091
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env" || true
fi
if ! command -v cargo >/dev/null 2>&1; then
  aviso "não há Rust nesta máquina — instalando o rustup (em ~/.cargo, sem sudo)"
  if [ "$SECO" -eq 0 ]; then
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --no-modify-path >/dev/null
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
  else
    echo "   [seco] curl https://sh.rustup.rs | sh -s -- -y"
  fi
fi
command -v cargo >/dev/null 2>&1 && ok "Rust: $(cargo --version)" || aviso "sem cargo (modo seco)"

# 🔑 Espaço em disco conferido **aqui**, e não depois: o `target/` do cargo chega
#    a ~10 GiB, e ficar sem disco no meio de um build deixa a árvore num estado
#    que o próprio cargo não explica.
LIVRE="$(df -g "$HOME" 2>/dev/null | awk 'NR==2 {print $4}')"
case "$LIVRE" in
  ''|*[!0-9]*) : ;;
  *) if [ "$LIVRE" -lt 12 ]; then
       aviso "há ${LIVRE} GiB livres — a compilação usa ~10 GiB e pode não caber."
       echo "     Se ela falhar por espaço: rm -rf \"$CASA/target\""
     fi ;;
esac

# ── Qual versão ───────────────────────────────────────────────────────────────
#
# 🔑 A versão sai do **mesmo `latest.json`** que o app instalado consulta para se
#    atualizar. Uma fonte só: um lançamento novo chega aqui e lá pelo mesmo
#    arquivo, no mesmo instante.
if [ -z "$REF" ]; then
  diga "perguntando ao manifesto qual é a versão publicada"
  VERSAO_PUB="$(curl -fsSL "$MANIFESTO" | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
  if [ -z "$VERSAO_PUB" ]; then
    erro "não consegui ler a versão em $MANIFESTO"
    echo "   Dá para escolher na mão: sh instalar.sh --versao v0.1.2"
    exit 1
  fi
  REF="v$VERSAO_PUB"
fi
ok "versão a compilar: $REF"

# ── Baixar o código ───────────────────────────────────────────────────────────
#
# ⚠️ O tarball, e não `git clone`: assim não é preciso ter git, e o que chega é
#    exatamente a árvore da tag. `fonte/` é **descartável** — quem editar algo
#    ali perde na próxima execução. Quem quer mexer no código clona o repositório
#    de verdade: $REPO
diga "baixando o código de $REF"
case "$REF" in
  v[0-9]*) URL="$REPO/archive/refs/tags/$REF.tar.gz" ;;
  *)       URL="$REPO/archive/refs/heads/$REF.tar.gz" ;;
esac

correr mkdir -p "$CASA"
correr rm -rf "$FONTE"
correr mkdir -p "$FONTE"
if [ "$SECO" -eq 1 ]; then
  echo "   [seco] curl -fsSL $URL | tar -xz -C $FONTE --strip-components=1"
else
  if ! curl -fsSL "$URL" | tar -xz -C "$FONTE" --strip-components=1; then
    erro "não consegui baixar $URL"
    echo "   Confira o nome da versão em $PAGINA"
    exit 1
  fi
  [ -f "$FONTE/Cargo.toml" ] || { erro "o pacote baixado não tem um Cargo.toml dentro."; exit 1; }
  ok "código em $FONTE"
fi

# ── Compilar ──────────────────────────────────────────────────────────────────
#
# 🚨 `--bin ui-gpui`, e não só `-p ui-gpui`: o crate tem cinco binários (o app e
#    quatro de medição) e, com `lto = true`, cada um linka o programa inteiro.
#    Foi assim que um build morreu por falta de memória no CI — e a máquina de um
#    fotógrafo não tem mais RAM que um runner.
#
# 🔑 **Só a arquitetura desta máquina.** O `.dmg` publicado é universal porque
#    serve a todo mundo; aqui o alvo é um Mac só, e compilar as duas
#    arquiteturas dobraria o tempo para nada.
diga "compilando (é a parte demorada — 15 a 40 minutos na primeira vez)"
CARGO_TARGET_DIR="$CASA/target"
export CARGO_TARGET_DIR
correr cargo build --release --manifest-path "$FONTE/Cargo.toml" -p ui-gpui --bin ui-gpui

BINARIO="$CARGO_TARGET_DIR/release/ui-gpui"
if [ "$SECO" -eq 0 ] && [ ! -x "$BINARIO" ]; then
  erro "a compilação terminou mas não há binário em $BINARIO"
  exit 1
fi

# ── Montar o .app ─────────────────────────────────────────────────────────────
#
# 🔑 O `Info.plist` daqui é o mesmo que o `cargo-packager` gerava para o `.dmg`
#    (que não é mais publicado desde 27/set/2026), e o
#    identificador precisa bater: é por ele que o macOS lembra as permissões que
#    o fotógrafo já concedeu. Montar à mão evita instalar o empacotador inteiro
#    só para copiar sete arquivos.
diga "montando o VintageLightbox.app"
VERSAO="$(sed -n '/^\[workspace\.package\]/,/^\[/p' "$FONTE/Cargo.toml" 2>/dev/null | sed -n 's/^version *= *"\(.*\)"/\1/p' | head -1)"
[ -n "$VERSAO" ] || VERSAO="${REF#v}"
APP="$CASA/VintageLightbox.app"

if [ "$SECO" -eq 1 ]; then
  echo "   [seco] montaria $APP (versão $VERSAO)"
else
  rm -rf "$APP"
  mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
  cp "$BINARIO" "$APP/Contents/MacOS/ui-gpui"
  cp "$FONTE/empacotamento/icones/icone.icns" "$APP/Contents/Resources/icone.icns"
  cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>English</string>
	<key>CFBundleDisplayName</key>
	<string>VintageLightbox</string>
	<key>CFBundleExecutable</key>
	<string>ui-gpui</string>
	<key>CFBundleIconFile</key>
	<string>icone.icns</string>
	<key>CFBundleIdentifier</key>
	<string>br.com.recordarfotos.vintagelightbox</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>VintageLightbox</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSAO</string>
	<key>CFBundleVersion</key>
	<string>$(date +%Y%m%d.%H%M%S)</string>
	<key>CSResourcesFileMapped</key>
	<true/>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.photography</string>
	<key>LSMinimumSystemVersion</key>
	<string>10.15</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSHumanReadableCopyright</key>
	<string>© 2026 RecordarFotos</string>
</dict>
</plist>
PLIST

  # ⚠️ Assinatura **ad-hoc** (`-s -`), que é a mesma coisa que o `.dmg`
  #    publicado carrega hoje: ela não vem da Apple, não custa nada e não é
  #    conferida pelo Gatekeeper. Serve para o app ter uma identidade estável
  #    para o macOS pendurar as permissões (rede, /Volumes, ~/Pictures) — sem
  #    ela cada abertura parece um app diferente.
  codesign --force --sign - "$APP" >/dev/null 2>&1 \
    || aviso "não consegui assinar ad-hoc — o app abre, mas o macOS vai repetir os pedidos de permissão."
  ok "$APP"
fi

# ── Instalar ──────────────────────────────────────────────────────────────────
if [ -z "$DESTINO" ]; then
  if [ -w /Applications ]; then DESTINO="/Applications"; else DESTINO="$HOME/Applications"; fi
fi
diga "instalando em $DESTINO"
correr mkdir -p "$DESTINO"
correr rm -rf "$DESTINO/VintageLightbox.app"
# `ditto` e não `cp -R`: é o que copia bundle no macOS preservando os metadados
# que a assinatura acabou de gravar.
correr ditto "$APP" "$DESTINO/VintageLightbox.app"

if [ "$SECO" -eq 1 ]; then
  echo
  echo "   [seco] nada foi feito."
  exit 0
fi

CACHE="$(du -sh "$CARGO_TARGET_DIR" 2>/dev/null | awk '{print $1}')"

printf "\n${V}✅ VintageLightbox %s instalado em %s${Z}\n\n" "$VERSAO" "$DESTINO"
printf "   ${N}Abra pelo Launchpad, ou:${Z}  open -a VintageLightbox\n\n"
echo "   Ele abre no primeiro duplo-clique, sem aviso de segurança: este app não"
echo "   foi baixado, foi compilado aqui — não tem a marca de quarentena que faz"
echo "   o macOS interrogar o que vem da internet."
echo
echo "   A partir daqui ele se atualiza sozinho, e cada atualização é conferida"
echo "   por assinatura (minisign) antes de ser instalada — essa parte nunca"
echo "   dependeu da Apple. Recompilar de novo só se você quiser."
echo
echo "   Opcional, para abrir DNG com compressão lossy:  brew install libraw"
echo
[ -n "$CACHE" ] && echo "   O cache do compilador ficou com $CACHE em $CARGO_TARGET_DIR."
echo "   Ele acelera a próxima compilação; apagá-lo é seguro:  rm -rf \"$CASA\""
