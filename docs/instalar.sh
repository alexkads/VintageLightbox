#!/bin/sh
#
# VintageLightbox — instalar compilando na sua própria máquina.
#
#     curl -fsSL https://alexkads.github.io/VintageLightbox/instalar.sh | sh
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

REF=""; DESTINO=""; SECO=0

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

Só macOS: no Linux e no Windows os instaladores já abrem sem esse passo.

Opções (com curl | sh, passe-as depois de \`sh -s --\`):
  --versao <ref>     tag (v0.1.2) ou branch (main). Padrão: a versão publicada
  --destino <pasta>  onde instalar o .app. Padrão: /Applications
  --seco             diz o que faria, sem compilar nem instalar nada
  --ajuda            isto aqui

Custa 15 a 40 minutos na primeira vez e ~10 GiB em ~/.vintagelightbox/target.
Exige o Xcode (grátis) com o componente Metal; o Rust ele instala se faltar.
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
    -h|--ajuda|--help) ajuda; exit 0 ;;
    *) erro "opção desconhecida: $1"; echo "   as que existem: --versao, --destino, --seco, --ajuda"; exit 1 ;;
  esac
done

# ── Este sistema é o certo? ───────────────────────────────────────────────────
if [ "$(uname -s)" != "Darwin" ]; then
  erro "este script é do macOS — e nos outros sistemas ele não teria troco."
  echo
  echo "   No Linux o .deb e o .AppImage instalam direto; no Windows o .msi pede"
  echo "   'Executar assim mesmo' uma vez. O passo chato que compilar evita é do"
  echo "   macOS, e só dele."
  echo
  echo "   Baixe o seu em: $PAGINA"
  exit 1
fi

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
# 🔑 O `Info.plist` daqui é o mesmo que o `cargo-packager` gera para o `.dmg`
#    publicado — os campos saem de `empacotamento/packager.toml`, e o
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
