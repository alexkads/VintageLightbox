#!/usr/bin/env bash
# Compila o leitor de RAW para a importação do site e entrega ao frontend.
#
# Uso: scripts/construir-raw.sh [--sem-pilha] [caminho do frontend do e-commerce]
#      (padrão: $RECORDARFOTOS_FRONTEND, senão ../recordarfotos-e-commerce/frontend)
#
# 📸 O crate e o script de verdade moram no núcleo da foto desde 18/out/2026:
# este só escolhe a cópia (a tag do Cargo.toml, ou NUCLEO_DA_FOTO) e repassa.
# O que sai e para onde — frontend/public/raw/ —
# está no cabeçalho do `scripts/construir-raw.sh` do núcleo.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/nucleo-da-foto.sh
construir_pelo_nucleo construir-raw.sh "$@"
