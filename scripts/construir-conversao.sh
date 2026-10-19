#!/usr/bin/env bash
# Compila o conversor de WebP em JPEG da galeria do cliente e entrega ao site.
#
# Uso: scripts/construir-conversao.sh [--sem-pilha] [caminho do frontend do e-commerce]
#      (padrão: $RECORDARFOTOS_FRONTEND, senão ../recordarfotos-e-commerce/frontend)
#
# 📸 O crate e o script de verdade moram no núcleo da foto desde 18/out/2026:
# este só escolhe a cópia (a tag do Cargo.toml, ou NUCLEO_DA_FOTO) e repassa.
# O que sai e para onde — frontend/public/conversao/ —
# está no cabeçalho do `scripts/construir-conversao.sh` do núcleo.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/nucleo-da-foto.sh
construir_pelo_nucleo construir-conversao.sh "$@"
