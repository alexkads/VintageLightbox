#!/usr/bin/env bash
#
# O pacote .rpm do VintageLightbox (Zed GPUI), para Fedora — gerado e conferido
# na própria máquina, como o resto do empacotamento (ver `empacotar.sh`).
#
#   ./scripts/empacotar-rpm.sh                    compila e gera o .rpm
#   ./scripts/empacotar-rpm.sh --binario CAMINHO  usa um binário já compilado
#   ./scripts/empacotar-rpm.sh --instalar         gera e instala com o dnf
#   ./scripts/empacotar-rpm.sh --seco             mostra o que faria
#
# Sai em `dist/fedora/vintagelightbox-<versão>-1.fc<N>.x86_64.rpm`. Quem recebe
# instala com dois cliques, ou com `sudo dnf install ./<arquivo>.rpm` — sem
# compilar nada, e o `dnf` traz as bibliotecas que faltarem.
#
# 🔑 **Por que o `rpmbuild` e não o `cargo-packager`.** O cargo-packager, que
#    faz o .deb e o AppImage, não gera .rpm. O `rpmbuild` é a ferramenta do
#    próprio Fedora: descobre sozinho as bibliotecas que o binário linka e não
#    pede nada instalado pelo cargo.
#
# 🔑 **O nome dos arquivos é `vintagelightbox-gpui`, e não é gosto.** É o
#    `APP_ID` de `crates/ui-gpui/src/menu.rs`: o GNOME casa a janela com o
#    `.desktop` pelo nome dele. Com outro nome a janela abre sem ícone.
#
# ⚠️ **Um .rpm gerado no Fedora N instala no N e nos seguintes**, não nos
#    anteriores: ele herda as versões das bibliotecas desta máquina. Gere na
#    versão mais antiga que precisa atender.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACOTE="vintagelightbox"
APP_ID="vintagelightbox-gpui"
CRATE="ui-gpui"
BIN="ui-gpui"
ICONES="$RAIZ/empacotamento/icones"
TRABALHO="$RAIZ/target/rpm"
SAIDA="$RAIZ/dist/fedora"

# ── Mensagens ────────────────────────────────────────────────────────────────
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

ajuda() { sed -n '3,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

# ── Opções ───────────────────────────────────────────────────────────────────
SECO=0; INSTALAR=0; BINARIO=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --seco)     SECO=1 ;;
    --instalar) INSTALAR=1 ;;
    --binario)  [ -n "${2:-}" ] || { erro "--binario pede um caminho."; exit 1; }
                BINARIO="$2"; shift ;;
    -h|--ajuda) ajuda; exit 0 ;;
    *) erro "opção desconhecida: $1"; ajuda; exit 1 ;;
  esac
  shift
done

# ── 1. Conferir a máquina ────────────────────────────────────────────────────
conferir() {
  diga "conferindo a máquina"
  [ "$(uname -s)" = "Linux" ] || { erro "o .rpm só se gera no Linux."; exit 1; }
  [ "$(uname -m)" = "x86_64" ] || { erro "só x86_64 por enquanto (esta é $(uname -m))."; exit 1; }

  local faltam=()
  command -v rpmbuild >/dev/null || faltam+=(rpm-build)
  command -v desktop-file-validate >/dev/null || faltam+=(desktop-file-utils)
  if [ "${#faltam[@]}" -gt 0 ]; then
    command -v dnf >/dev/null || { erro "falta ${faltam[*]}, e esta máquina não tem dnf."; exit 1; }
    aviso "faltam ferramentas: ${faltam[*]} — instalando (pede a senha)"
    correr sudo dnf install -y "${faltam[@]}"
  fi

  # A versão sai do workspace, a mesma que o `empacotar.sh` confere.
  VERSAO=$(awk '/^\[workspace.package\]/ {w=1; next} /^\[/ {w=0}
                w && /^version *=/ {gsub(/[" ]/, "", $0); sub(/version=/, ""); print; exit}' \
           "$RAIZ/Cargo.toml")
  [ -n "$VERSAO" ] || { erro "não achei a versão em Cargo.toml ([workspace.package])."; exit 1; }
  FEDORA=$(rpm --eval '%{?fedora}')
  # Quem assina o changelog do pacote: o autor dos commits desta máquina.
  EMPACOTADOR="$(git -C "$RAIZ" config user.name || echo RecordarFotos) <$(git -C "$RAIZ" config user.email || echo sem-email)>"
  ok "versão $VERSAO · Fedora ${FEDORA:-?} · $(rpmbuild --version)"
}

# ── 2. O binário ─────────────────────────────────────────────────────────────
compilar() {
  if [ -n "$BINARIO" ]; then
    diga "usando o binário pronto"
    [ -x "$BINARIO" ] || { erro "não é um executável: $BINARIO"; exit 1; }
  else
    diga "compilando (15 a 40 minutos na primeira vez)"
    correr cargo build --release -p "$CRATE" --bin "$BIN" --manifest-path "$RAIZ/Cargo.toml"
    BINARIO="$RAIZ/target/release/$BIN"
  fi
  [ "$SECO" -eq 1 ] || ok "$BINARIO ($(du -h "$BINARIO" | cut -f1))"
}

# ── 3. O atalho do menu e a receita ──────────────────────────────────────────
escrever_desktop() {
  cat > "$1" <<DESKTOP
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
}

# 🔑 **As `Requires` escritas à mão são as que o GPUI abre com `dlopen`.** O
#    `rpmbuild` só enxerga o que o binário linka; X11, Wayland, teclado e Vulkan
#    são abertos em tempo de execução, e sem eles o app instala e não abre. É a
#    mesma lista do `[deb] depends` de `empacotamento/packager.toml`, por soname
#    — o nome do pacote muda entre versões do Fedora, o soname não.
escrever_spec() {
  cat > "$1" <<SPEC
%global debug_package %{nil}
%global _build_id_links none

Name:           $PACOTE
Version:        $VERSAO
Release:        1%{?dist}
Summary:        Editor de fotos da RecordarFotos, com revelação em GPU
License:        MIT
URL:            https://recordarfotos.com.br
ExclusiveArch:  x86_64

Source0:        $APP_ID
Source1:        $APP_ID.desktop
Source2:        LICENSE

Requires:       libX11.so.6()(64bit)
Requires:       libxcb.so.1()(64bit)
Requires:       libxkbcommon.so.0()(64bit)
Requires:       libxkbcommon-x11.so.0()(64bit)
Requires:       libwayland-client.so.0()(64bit)
Requires:       libfontconfig.so.1()(64bit)
Requires:       libfreetype.so.6()(64bit)
Requires:       libasound.so.2()(64bit)
Requires:       libvulkan.so.1()(64bit)
# O driver Vulkan da Intel e da AMD; com NVIDIA quem traz é o akmod-nvidia.
Recommends:     mesa-vulkan-drivers
# O GNOME não mostra ícone de bandeja sem ela.
Recommends:     gnome-shell-extension-appindicator
# A importação direto da câmera pelo cabo (PTP).
Recommends:     gphoto2
Recommends:     gvfs-gphoto2

%description
VintageLightbox importa, organiza, tria, revela e entrega arquivo — com o motor
de revelação em GPU e integração com o pós-venda da RecordarFotos.

%prep
cp %{SOURCE2} .

%build

%install
install -Dm755 %{SOURCE0} %{buildroot}%{_bindir}/$APP_ID
install -Dm644 %{SOURCE1} %{buildroot}%{_datadir}/applications/$APP_ID.desktop
for t in 32 128 256 512; do
  install -Dm644 %{_sourcedir}/icone-\${t}.png \\
    %{buildroot}%{_datadir}/icons/hicolor/\${t}x\${t}/apps/$APP_ID.png
done
install -Dm644 %{_sourcedir}/icone-256@2.png \\
  %{buildroot}%{_datadir}/icons/hicolor/128x128@2/apps/$APP_ID.png

%files
%license LICENSE
%{_bindir}/$APP_ID
%{_datadir}/applications/$APP_ID.desktop
%{_datadir}/icons/hicolor/*/apps/$APP_ID.png

%changelog
* $(LC_ALL=C date '+%a %b %d %Y') $EMPACOTADOR - $VERSAO-1
- Pacote gerado por scripts/empacotar-rpm.sh
SPEC
}

montar() {
  diga "montando o pacote"
  if [ "$SECO" -eq 1 ]; then
    echo "   [seco] prepararia $TRABALHO e rodaria rpmbuild -bb"
    return
  fi
  rm -rf "$TRABALHO"
  mkdir -p "$TRABALHO"/{SOURCES,SPECS,BUILD,RPMS,SRPMS} "$SAIDA"
  local src="$TRABALHO/SOURCES"

  install -m755 "$BINARIO" "$src/$APP_ID"
  cp "$RAIZ/LICENSE" "$src/LICENSE"
  cp "$ICONES/32x32.png"       "$src/icone-32.png"
  cp "$ICONES/128x128.png"     "$src/icone-128.png"
  cp "$ICONES/256x256.png"     "$src/icone-256.png"
  cp "$ICONES/512x512.png"     "$src/icone-512.png"
  cp "$ICONES/128x128@2x.png"  "$src/icone-256@2.png"

  escrever_desktop "$src/$APP_ID.desktop"
  desktop-file-validate "$src/$APP_ID.desktop"
  escrever_spec "$TRABALHO/SPECS/$PACOTE.spec"

  rpmbuild -bb --quiet --define "_topdir $TRABALHO" "$TRABALHO/SPECS/$PACOTE.spec"

  RPM=$(find "$TRABALHO/RPMS" -name "$PACOTE-$VERSAO-*.rpm" | head -1)
  [ -n "$RPM" ] || { erro "o rpmbuild terminou sem gerar o pacote."; exit 1; }
  cp "$RPM" "$SAIDA/"
  RPM="$SAIDA/$(basename "$RPM")"
  ok "$(basename "$RPM") ($(du -h "$RPM" | cut -f1))"
}

# ── 4. Conferir o pacote ─────────────────────────────────────────────────────
# O que se confere aqui é o que já deu errado em outros formatos: arquivo
# faltando dentro do pacote, e dependência que o Fedora não sabe resolver.
conferir_pacote() {
  [ "$SECO" -eq 0 ] || return 0
  diga "conferindo o pacote"
  local arquivos; arquivos=$(rpm -qpl "$RPM")
  for f in "/usr/bin/$APP_ID" "/usr/share/applications/$APP_ID.desktop" \
           "/usr/share/icons/hicolor/256x256/apps/$APP_ID.png"; do
    grep -qx "$f" <<<"$arquivos" || { erro "falta no pacote: $f"; exit 1; }
  done
  ok "$(wc -l <<<"$arquivos") arquivos, binário, atalho e ícones dentro"

  # O `--test` resolve as dependências contra o banco do rpm sem instalar nada.
  local teste
  if teste=$(rpm -i --test "$RPM" 2>&1); then
    ok "todas as dependências existem nesta máquina"
  elif grep -q "is needed by" <<<"$teste"; then
    aviso "dependências que esta máquina não tem (o dnf as baixa na instalação):"
    grep "is needed by" <<<"$teste" | sed 's/^/   /'
  elif grep -q "already installed" <<<"$teste"; then
    ok "esta versão já está instalada aqui"
  else
    aviso "o rpm --test respondeu: $teste"
  fi
}

# ── 5. Instalar (opcional) ───────────────────────────────────────────────────
instalar() {
  [ "$INSTALAR" -eq 1 ] || return 0
  diga "instalando"
  correr sudo dnf install -y "$RPM"
  # ⚠️ O instalador `curl | sh` põe o mesmo atalho em ~/.local, e o de lá vence
  #    o de /usr no menu. Com os dois, o menu abre a cópia compilada.
  if [ -e "$HOME/.local/share/applications/$APP_ID.desktop" ]; then
    aviso "há também uma instalação pelo curl em ~/.local, e é ela que o menu abre."
    echo "   Para ficar só com o .rpm:"
    echo "   rm ~/.local/bin/$APP_ID ~/.local/share/applications/$APP_ID.desktop"
  fi
}

conferir
compilar
montar
conferir_pacote
instalar

if [ "$SECO" -eq 0 ]; then
  printf "\n${V}✅ pronto:${Z} %s\n\n" "$RPM"
  echo "   Para instalar:  sudo dnf install ./${RPM#"$RAIZ"/}"
  echo "   Para remover:   sudo dnf remove $PACOTE"
fi
