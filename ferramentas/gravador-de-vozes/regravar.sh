#!/usr/bin/env bash
# 🎙️ Regrava as vozes de aviso do app com o Kokoro 82M (voz pm_santa).
#
#   ferramentas/gravador-de-vozes/regravar.sh            # todas as frases de frases.tsv
#   ferramentas/gravador-de-vozes/regravar.sh minhas.tsv # só as de outro arquivo
#
# Baixa o modelo se faltar (hexgrad/Kokoro-82M, versão fixa, hash conferido),
# compila o gravador em release e grava cada frase em .ogg (Vorbis q −0,1) em
# crates/ui-gpui/assets/vozes/, com um .ogg.ouvir.wav ao lado em ~/.cache para
# ouvir no Mac (o afplay toca Ogg Vorbis pela metade). Ver a skill vozes-gravadas.
set -euo pipefail
cd "$(dirname "$0")"
RAIZ=$(cd ../.. && pwd)
FRASES=${1:-$RAIZ/crates/ui-gpui/assets/vozes/frases.tsv}
MODELOS=${KOKORO_MODELOS:-$HOME/.cache/vintagelightbox/modelos}
OUVIR=${OUVIR:-$HOME/.cache/vintagelightbox/vozes-para-ouvir}
DESTINO=$RAIZ/crates/ui-gpui/assets/vozes
export QUALIDADE=${QUALIDADE:--0.1}

[ -d "$HOME/Projects/RavenRS/crates/ravenrs-ai" ] || {
  echo "precisa do RavenRS em ~/Projects/RavenRS (o motor do Kokoro mora no ravenrs-ai)" >&2; exit 1; }

# O mesmo modelo, versão e hashes do catálogo do RavenRS (crates/ravenrs-ai/src/catalog.rs).
K=$MODELOS/kokoro-82m; mkdir -p "$K"
B=https://huggingface.co/hexgrad/Kokoro-82M/resolve/f3ff3571791e39611d31c381e3a41a3af07b4987
while read -r nome url hash; do
  if [ -f "$K/$nome" ] && echo "$hash  $K/$nome" | shasum -a 256 -c - >/dev/null 2>&1; then continue; fi
  echo "⬇️  $nome"
  curl -sSL --fail -o "$K/$nome" "$B/$url"
  echo "$hash  $K/$nome" | shasum -a 256 -c - >/dev/null
done <<'LISTA'
config.json config.json 5abb01e2403b072bf03d04fde160443e209d7a0dad49a423be15196b9b43c17f
pm_santa.pt voices/pm_santa.pt d42103169c5c872abbafb9129133af7e942bb9d272c3cc3b95c203e7d7198c29
pf_dora.pt voices/pf_dora.pt 07e4ff987c5d5a8c3995efd15cc4f0db7c4c15e881b198d8ab7f67ecf51f5eb7
pm_alex.pt voices/pm_alex.pt cf0ba8c573c2480fc54123683a35cf1e2ae130428e441eb91f9149bdb188a526
model.pth kokoro-v1_0.pth 496dba118d1a58f5f3db2efc88dbdc216e0483fc89fe6e47ee1f2c53f18ad1e4
LISTA

# Um target só, fora do repositório (a regra do disco).
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/.cache/vintagelightbox/target-gravador}
CARGO_INCREMENTAL=0 cargo build --release -q
BIN=$CARGO_TARGET_DIR/release/gravador-de-vozes

TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
"$BIN" "$MODELOS" "$TMP" "$FRASES"
mkdir -p "$OUVIR"
for ogg in "$TMP"/*.ogg; do
  cp "$ogg" "$DESTINO/"
  "$BIN" "$ogg" >/dev/null && mv "$ogg.ouvir.wav" "$OUVIR/$(basename "$ogg" .ogg).wav"
done
echo "✅ $(ls "$TMP"/*.ogg | wc -l | tr -d ' ') frase(s) em $DESTINO"
echo "🔊 para ouvir no Mac: afplay $OUVIR/<nome>.wav"
