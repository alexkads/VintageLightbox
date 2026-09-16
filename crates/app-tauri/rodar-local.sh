#!/bin/sh
# Abre o app Tauri (depuração) contra a pilha local do e-commerce, e não contra
# produção. Antes: `make up` em recordarfotos-e-commerce.
#
#     crates/app-tauri/rodar-local.sh
#
# Sessão, catálogo e armazenamento do webview são outros (`src/ambiente.rs`), e
# a janela diz "PILHA LOCAL" no título. As portas são as do
# docker-compose.dev.yml; troque com VLB_POS_VENDA_URL e VLB_SITE_URL.
set -eu
AQUI="$(cd "$(dirname "$0")" && pwd)"
export VLB_POS_VENDA_URL="${VLB_POS_VENDA_URL:-http://localhost:8080}"
export VLB_SITE_URL="${VLB_SITE_URL:-http://localhost:8001}"
if ! curl -fsS -o /dev/null "$VLB_POS_VENDA_URL/health"; then
  echo "A API local não responde em $VLB_POS_VENDA_URL. Rode 'make up' no recordarfotos-e-commerce." >&2
  exit 1
fi
cd "$AQUI/../.."
exec cargo run -p app-tauri "$@"
