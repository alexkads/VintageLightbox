#!/usr/bin/env bash
#
# O pacote PRONTO do VintageLightbox para Linux (issue #37): um `.tar.gz` com o
# binário, o atalho do menu, os ícones, a licença e os avisos de licença das
# dependências. É o que o `docs/instalar.sh` baixa, confere pela assinatura e
# instala em `~/.local` — sem compilar nada no balcão.
#
#   scripts/empacotar-pacote.sh                      compila (release) e empacota
#   scripts/empacotar-pacote.sh --binario CAMINHO    usa um binário já compilado
#   scripts/empacotar-pacote.sh --saida PASTA        onde gravar (padrão: dist/pacote)
#   scripts/empacotar-pacote.sh --seco               mostra o que faria
#
# Sai em `dist/pacote/vintagelightbox-<versão>-linux-x86_64.tar.gz`, mais um
# `.glibc` ao lado (a maior `GLIBC_` que o binário pede, que o manifesto leva).
#
# 🔑 **Gerar não é assinar nem publicar.** A chave privada do minisign fica só
#    no Mac do dono (issue #37, Passo 0 e Passo 3): este script roda na fedora,
#    que só compila e valida. O pacote sai **sem** assinatura; o Mac assina, sobe
#    para o R2 e escreve o `latest-linux.json` (`scripts/manifesto-do-pacote.sh`).
#
# 🔑 **O piso do Linux é o Fedora 44** (dono, 18/11/2026): compile na fedora, que
#    tem a glibc do piso. Compilar numa distribuição mais nova faria o pacote
#    pedir uma glibc que o Fedora 44 não tem; o script confere isso no fim.
#
# 🔑 **O código não viaja.** O binário sai com `strip = true` (perfil `release`)
#    e com `--remap-path-prefix`, que troca `~/.cargo/registry/src` por `/r` e a
#    pasta do projeto por `/f` nos caminhos de panic e de `file!()`.
#
# 🚨 Só roda depois de `make lint`, `make testar` e `make e2e` verdes no `dev`.
#    Aqui não há como conferir isso: quem lança é quem garante.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_ID="vintagelightbox-gpui"   # o `APP_ID` de menu.rs: o GNOME casa a janela pelo nome do .desktop
CRATE="ui-gpui"
BIN="ui-gpui"
ICONES="$RAIZ/empacotamento/icones"
SAIDA="$RAIZ/dist/pacote"
GLIBC_PISO="2.43"                # a do Fedora 44 (`ldd --version`); o binário não pode pedir mais

if [ -t 1 ]; then
  C=$'\e[1;36m'; V=$'\e[32m'; A=$'\e[33m'; E=$'\e[31m'; Z=$'\e[0m'
else
  C=''; V=''; A=''; E=''; Z=''
fi
diga()  { printf "\n${C}▸ %s${Z}\n" "$*"; }
ok()    { printf "${V}✅ %s${Z}\n" "$*"; }
aviso() { printf "${A}⚠️  %s${Z}\n" "$*"; }
erro()  { printf "${E}❌ %s${Z}\n" "$*" >&2; }
correr() { if [ "$SECO" -eq 1 ]; then echo "   [seco] $*"; else "$@"; fi; }

SECO=0; BINARIO=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --seco)    SECO=1 ;;
    --binario) [ -n "${2:-}" ] || { erro "--binario pede um caminho."; exit 1; }; BINARIO="$2"; shift ;;
    --saida)   [ -n "${2:-}" ] || { erro "--saida pede uma pasta."; exit 1; }; SAIDA="$2"; shift ;;
    -h|--ajuda) sed -n '3,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) erro "opção desconhecida: $1"; exit 1 ;;
  esac
  shift
done

# ── 1. A máquina ─────────────────────────────────────────────────────────────
diga "conferindo a máquina"
[ "$(uname -s)" = "Linux" ] || { erro "este pacote é do Linux e se gera no Linux (macOS e Windows têm o seu, ainda por fazer)."; exit 1; }
[ "$(uname -m)" = "x86_64" ] || { erro "só x86_64 por enquanto (esta é $(uname -m))."; exit 1; }
for f in tar objdump strings python3 install; do
  command -v "$f" >/dev/null || { erro "falta '$f'."; exit 1; }
done
VERSAO=$(awk '/^\[workspace.package\]/ {w=1; next} /^\[/ {w=0}
              w && /^version *=/ {gsub(/[" ]/, "", $0); sub(/version=/, ""); print; exit}' "$RAIZ/Cargo.toml")
[ -n "$VERSAO" ] || { erro "não achei a versão em Cargo.toml ([workspace.package])."; exit 1; }
NOME="vintagelightbox-$VERSAO-linux-x86_64"
ok "versão $VERSAO → $NOME.tar.gz"

# ── 2. O binário ─────────────────────────────────────────────────────────────
if [ -n "$BINARIO" ]; then
  diga "usando o binário pronto"
  [ -x "$BINARIO" ] || { erro "não é um executável: $BINARIO"; exit 1; }
else
  diga "compilando em release (a primeira vez leva de 15 a 40 minutos)"
  # 🔑 A regra geral ($HOME) vem primeiro e as específicas depois: o rustc usa a
  #    ÚLTIMA que casar. Só o `registry/src` não bastava: as dependências por git
  #    (`nucleo-da-foto`, em `~/.cargo/git/checkouts`) e a pasta `build/` do
  #    `target` também deixavam o caminho da máquina dentro do binário.
  export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=/h --remap-path-prefix=$HOME/.cargo/registry/src=/r --remap-path-prefix=$HOME/.cargo/git/checkouts=/g --remap-path-prefix=$RAIZ=/f"
  correr cargo build --release -p "$CRATE" --bin "$BIN" --manifest-path "$RAIZ/Cargo.toml"
  BINARIO="${CARGO_TARGET_DIR:-$RAIZ/target}/release/$BIN"
fi
[ "$SECO" -eq 1 ] && { echo "   [seco] empacotaria $BINARIO"; exit 0; }
ok "$BINARIO ($(du -h "$BINARIO" | cut -f1))"

# ── 3. O binário serve ao piso? ──────────────────────────────────────────────
diga "conferindo a glibc que o binário pede"
GLIBC_BIN=$(objdump -T "$BINARIO" 2>/dev/null | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -V | tail -1)
[ -n "$GLIBC_BIN" ] || { erro "não achei nenhum símbolo GLIBC_ no binário: é mesmo um binário Linux dinâmico?"; exit 1; }
if [ "$(printf '%s\n%s\n' "$GLIBC_BIN" "$GLIBC_PISO" | sort -V | tail -1)" != "$GLIBC_PISO" ]; then
  erro "o binário pede glibc $GLIBC_BIN, mais nova que a do Fedora 44 ($GLIBC_PISO): compile na fedora."
  exit 1
fi
ok "pede glibc $GLIBC_BIN (a do Fedora 44 é $GLIBC_PISO)"

# O código não pode ter vazado: nenhum caminho do projeto nem do cargo no binário.
VAZOU="$(strings -a "$BINARIO" 2>/dev/null | grep -F -e "$HOME" -e "$RAIZ" | sed -E 's/.*(\/[^ ]{0,70}).*/\1/' | sort | uniq -c | sort -rn | head -5 || true)"
if [ -n "$VAZOU" ]; then
  aviso "o binário ainda cita caminhos desta máquina (foi compilado sem o --remap-path-prefix?):"
  printf '%s\n' "$VAZOU" | sed 's/^/     /'
fi

# ── 4. Montar ────────────────────────────────────────────────────────────────
diga "montando o pacote"
TRABALHO="$(mktemp -d)"; trap 'rm -rf "$TRABALHO"' EXIT
RAIZ_PACOTE="$TRABALHO/$NOME"
mkdir -p "$RAIZ_PACOTE/bin" "$RAIZ_PACOTE/share/applications"
install -m 755 "$BINARIO" "$RAIZ_PACOTE/bin/$APP_ID"
for t in 32x32 128x128 256x256 512x512; do
  install -Dm644 "$ICONES/$t.png" "$RAIZ_PACOTE/share/icons/hicolor/$t/apps/$APP_ID.png"
done
install -Dm644 "$ICONES/128x128@2x.png" "$RAIZ_PACOTE/share/icons/hicolor/128x128@2/apps/$APP_ID.png"
# O `Exec` é reescrito pelo instalador com o caminho inteiro; aqui fica o nome.
cat > "$RAIZ_PACOTE/share/applications/$APP_ID.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=VintageLightbox
GenericName=Editor de fotos
Comment=Editor de fotos da RecordarFotos
Exec=$APP_ID
Icon=$APP_ID
StartupWMClass=$APP_ID
Categories=Graphics;Photography;
Terminal=false
DESKTOP
cp "$RAIZ/LICENSE" "$RAIZ_PACOTE/LICENSE"
printf '%s\n' "$VERSAO" > "$RAIZ_PACOTE/VERSAO"
printf '%s\n' "$GLIBC_BIN" > "$RAIZ_PACOTE/GLIBC-MINIMA"

# 📜 As licenças (MIT, Apache…) exigem que o **aviso** acompanhe o binário; não
#    exigem divulgá-lo. Vai num arquivo do pacote, gerado do Cargo.lock.
diga "reunindo os avisos de licença das dependências"
cargo metadata --format-version 1 --locked --filter-platform x86_64-unknown-linux-gnu \
    --manifest-path "$RAIZ/Cargo.toml" 2>/dev/null \
  | python3 -I -c '
import json, sys
m = json.load(sys.stdin)
membros = set(m["workspace_members"])
linhas = []
for p in sorted(m["packages"], key=lambda p: (p["name"], p["version"])):
    if p["id"] in membros:
        continue
    linhas.append("%s %s — %s%s" % (
        p["name"], p["version"], p.get("license") or "licença não declarada",
        (" — " + p["repository"]) if p.get("repository") else ""))
print("VintageLightbox — avisos de licença das dependências")
print("O próprio VintageLightbox é MIT (arquivo LICENSE). Dependências:\n")
print("\n".join(linhas))
' > "$RAIZ_PACOTE/AVISOS-DE-LICENCA.txt" \
  || { erro "não consegui gerar os avisos de licença (cargo metadata falhou)."; exit 1; }
ok "$(($(wc -l < "$RAIZ_PACOTE/AVISOS-DE-LICENCA.txt") - 3)) dependências listadas"

mkdir -p "$SAIDA"
# Ordem e metadados fixos: o mesmo conteúdo dá o mesmo arquivo.
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime='2026-01-01 00:00:00' \
    -C "$TRABALHO" -czf "$SAIDA/$NOME.tar.gz" "$NOME"
printf '%s\n' "$GLIBC_BIN" > "$SAIDA/$NOME.glibc"
ok "$SAIDA/$NOME.tar.gz ($(du -h "$SAIDA/$NOME.tar.gz" | cut -f1))"

echo
echo "   Próximo passo (no Mac do dono): assinar com a chave do app, subir para o R2"
echo "   e gerar o manifesto:  scripts/manifesto-do-pacote.sh $NOME.tar.gz <.sig> <url-base>"
