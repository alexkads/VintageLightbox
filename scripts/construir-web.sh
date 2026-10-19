#!/usr/bin/env bash
# Compila o motor de revelação para o navegador e entrega ao site.
#
# Uso: scripts/construir-web.sh [--sem-pilha] [caminho do frontend do e-commerce]
#      (padrão: $RECORDARFOTOS_FRONTEND, senão ../recordarfotos-e-commerce/frontend)
#
# 📸 O crate e o script de verdade moram no núcleo da foto desde 18/out/2026:
# este só escolhe a cópia (a tag do Cargo.toml, ou NUCLEO_DA_FOTO) e repassa.
# O que sai e para onde — frontend/public/revelacao/ —
# está no cabeçalho do `scripts/construir-web.sh` do núcleo.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/nucleo-da-foto.sh
construir_pelo_nucleo construir-web.sh "$@"
