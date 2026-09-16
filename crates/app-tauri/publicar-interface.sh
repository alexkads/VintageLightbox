#!/bin/sh
# Compila a tela empacotada (recordarfotos-e-commerce/frontend/desktop) e a traz
# para `interface/`, que é o que o app empacota (DESKTOP_TAURI §0, etapa E).
#
# 🔑 O `interface/` vai versionado: o repositório do site é privado, e o
# instalador do balcão só alcança este.
#
#     crates/app-tauri/publicar-interface.sh [caminho/do/recordarfotos-e-commerce]
set -eu
AQUI="$(cd "$(dirname "$0")" && pwd)"
SITE="${1:-$AQUI/../../../recordarfotos-e-commerce}"
( cd "$SITE/frontend" && pnpm --filter @recordarfotos/desktop build )
rm -rf "$AQUI/interface"
cp -R "$SITE/frontend/desktop/dist" "$AQUI/interface"
cp "$AQUI/diagnostico/index.html" "$AQUI/interface/diagnostico.html"
echo "interface/ atualizada a partir de $SITE ($(git -C "$SITE" rev-parse --short HEAD))"
