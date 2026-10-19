#!/usr/bin/env bash
# Compila a tela do cliente do balcão para o navegador e entrega ao site.
#
# Uso: scripts/construir-tela-do-cliente.sh [--sem-pilha] [caminho do frontend do e-commerce]
#      (padrão: $RECORDARFOTOS_FRONTEND, senão ../recordarfotos-e-commerce/frontend)
#
# 📸 O crate e o script de verdade moram no núcleo da foto desde 18/out/2026:
# este só escolhe a cópia (a tag do Cargo.toml, ou NUCLEO_DA_FOTO) e repassa.
# O que sai e para onde — frontend/public/tela-do-cliente/ —
# está no cabeçalho do `scripts/construir-tela-do-cliente.sh` do núcleo.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/nucleo-da-foto.sh
construir_pelo_nucleo construir-tela-do-cliente.sh "$@"
