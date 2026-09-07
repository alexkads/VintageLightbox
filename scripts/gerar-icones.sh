#!/usr/bin/env bash
#
# Gera, a partir de um único SVG, todos os ícones que os três sistemas pedem.
#
#   macOS   → .icns  (10 imagens dentro, de 16 a 1024, com as @2x)
#   Windows → .ico   (6 imagens dentro, de 16 a 256)
#   Linux   → PNGs   (32/128/256/512, é o que o .deb e o AppImage instalam)
#
# 🔑 O SVG é a fonte, e o resto é derivado: trocar o desenho é trocar um arquivo
#    e rodar isto. Nenhum PNG se edita à mão — quem editar perde na próxima vez.
#
# Exige: rsvg-convert (brew install librsvg), magick (brew install imagemagick),
#        iconutil (vem com o macOS).
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICONES="$RAIZ/empacotamento/icones"
FONTE="$ICONES/icone.svg"

for f in rsvg-convert magick; do
  command -v "$f" >/dev/null || { echo "❌ falta '$f' — brew install librsvg imagemagick"; exit 1; }
done
[[ -f "$FONTE" ]] || { echo "❌ não achei $FONTE"; exit 1; }

renderizar() { rsvg-convert -w "$1" -h "$1" "$FONTE" -o "$2"; }

echo "🎨 PNGs (Linux e o mestre)"
for t in 32 128 256 512 1024; do renderizar "$t" "$ICONES/${t}x${t}.png"; done
cp "$ICONES/1024x1024.png" "$ICONES/icone.png"
# O AppImage e o .desktop procuram por `128x128@2x` quando a tela é HiDPI.
renderizar 256 "$ICONES/128x128@2x.png"

echo "🍎 .icns (macOS)"
CONJUNTO="$(mktemp -d)/icone.iconset"; mkdir -p "$CONJUNTO"
# Os nomes são fixos: o `iconutil` recusa o conjunto se um deles faltar ou
# vier com outro nome. 16, 32, 128, 256 e 512 — cada um em 1x e 2x.
for par in "16 16x16" "32 16x16@2x" "32 32x32" "64 32x32@2x" \
           "128 128x128" "256 128x128@2x" "256 256x256" "512 256x256@2x" \
           "512 512x512" "1024 512x512@2x"; do
  set -- $par; renderizar "$1" "$CONJUNTO/icon_$2.png"
done
iconutil -c icns "$CONJUNTO" -o "$ICONES/icone.icns"

echo "🪟 .ico (Windows)"
# Um .ico é um contêiner: as seis medidas vão dentro do mesmo arquivo, e o
# Explorer escolhe a que couber. Um .ico de 256 sozinho aparece borrado na
# barra de tarefas, que pede 32.
TMP="$(mktemp -d)"
for t in 16 24 32 48 64 256; do renderizar "$t" "$TMP/$t.png"; done
magick "$TMP/16.png" "$TMP/24.png" "$TMP/32.png" "$TMP/48.png" "$TMP/64.png" "$TMP/256.png" \
  "$ICONES/icone.ico"

echo
ls -lh "$ICONES" | sed 's/^/   /'
echo "✅ ícones gerados em empacotamento/icones/"
