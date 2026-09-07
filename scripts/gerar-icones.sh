#!/usr/bin/env bash
#
# Gera, a partir de um único mestre, todos os ícones que os três sistemas pedem.
#
#   macOS   → .icns  (10 imagens dentro, de 16 a 1024, com as @2x)
#   Windows → .ico   (6 imagens dentro, de 16 a 256)
#   Linux   → PNGs   (32/128/256/512, é o que o .deb e o AppImage instalam)
#   o site  → site/icone.png
#
# 🔑 **O mestre é `empacotamento/icones/icone-mestre.png`, e o resto é derivado.**
#    Trocar o ícone é trocar esse arquivo e rodar isto. Nenhum dos derivados se
#    edita à mão — quem editar perde na próxima vez que alguém rodar o script.
#
# ⚠️ O mestre tem de ser **quadrado e de pelo menos 1024×1024**. O `.icns` pede
#    uma imagem de 1024 para a variante `512x512@2x`, e ampliar um mestre menor
#    entrega um ícone borrado justamente no tamanho em que ele mais aparece — a
#    tela de "Sobre" e o Finder em visualização de ícones grandes.
#
# Exige: magick (brew install imagemagick) e iconutil (vem com o macOS).
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICONES="$RAIZ/empacotamento/icones"
MESTRE="$ICONES/icone-mestre.png"

command -v magick >/dev/null || { echo "❌ falta o 'magick' — brew install imagemagick"; exit 1; }
[[ -f "$MESTRE" ]] || { echo "❌ não achei $MESTRE"; exit 1; }

LADO=$(magick identify -format '%w' "$MESTRE")
ALTURA=$(magick identify -format '%h' "$MESTRE")
if [[ "$LADO" != "$ALTURA" ]]; then
  echo "❌ o mestre não é quadrado (${LADO}×${ALTURA}) — o ícone sairia esticado"; exit 1
fi
if (( LADO < 1024 )); then
  echo "❌ o mestre tem $LADO px e precisa de ao menos 1024"; exit 1
fi

# `-filter Lanczos` e não o padrão: em redução forte de imagem com textura — e
# esta tem metal escovado, vidro e granulado — o padrão do ImageMagick borra os
# contornos. O Lanczos preserva a borda do aparelho nos tamanhos pequenos, que é
# onde o ícone tem de continuar reconhecível.
reduzir() { magick "$MESTRE" -filter Lanczos -resize "${1}x${1}" -strip "$2"; }

echo "🎨 PNGs (Linux, o site e o mestre de 1024)"
for t in 32 128 256 512 1024; do reduzir "$t" "$ICONES/${t}x${t}.png"; done
cp "$ICONES/1024x1024.png" "$ICONES/icone.png"
# O AppImage e o `.desktop` procuram por `128x128@2x` quando a tela é HiDPI.
cp "$ICONES/256x256.png" "$ICONES/128x128@2x.png"
# A página do GitHub Pages usa o mesmo desenho.
mkdir -p "$RAIZ/site" && cp "$ICONES/256x256.png" "$RAIZ/site/icone.png"

echo "🍎 .icns (macOS)"
CONJUNTO="$(mktemp -d)/icone.iconset"; mkdir -p "$CONJUNTO"
# Os nomes são fixos: o `iconutil` recusa o conjunto se um deles faltar ou vier
# com outro nome. 16, 32, 128, 256 e 512 — cada um em 1x e 2x.
for par in "16 16x16" "32 16x16@2x" "32 32x32" "64 32x32@2x" \
           "128 128x128" "256 128x128@2x" "256 256x256" "512 256x256@2x" \
           "512 512x512" "1024 512x512@2x"; do
  set -- $par; reduzir "$1" "$CONJUNTO/icon_$2.png"
done
iconutil -c icns "$CONJUNTO" -o "$ICONES/icone.icns"

echo "🪟 .ico (Windows)"
# Um .ico é um contêiner: as seis medidas vão dentro do mesmo arquivo, e o
# Explorer escolhe a que couber. Um .ico só de 256 aparece borrado na barra de
# tarefas, que pede 32.
TMP="$(mktemp -d)"
for t in 16 24 32 48 64 256; do reduzir "$t" "$TMP/$t.png"; done
magick "$TMP/16.png" "$TMP/24.png" "$TMP/32.png" "$TMP/48.png" "$TMP/64.png" "$TMP/256.png" \
  "$ICONES/icone.ico"

echo
ls -lh "$ICONES" | sed 's/^/   /'
echo "✅ ícones gerados a partir de $(basename "$MESTRE") (${LADO}×${LADO})"
