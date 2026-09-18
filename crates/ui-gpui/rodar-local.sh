#!/bin/sh
# Abre o app GPUI (depuração) contra a pilha local do e-commerce, e não contra
# produção. Antes: `make up` em recordarfotos-e-commerce.
#
#     crates/ui-gpui/rodar-local.sh
#
# O par de `crates/app-tauri/rodar-local.sh`, e pelo mesmo motivo: as duas
# interfaces se validam uma à outra (DESKTOP_TAURI, D7), e uma delas não pode
# ser mais difícil de abrir local que a outra.
#
# 🚨 **As duas variáveis andam juntas, sempre.** Só a API em `localhost` abria a
# autorização em `recordarfotos.com.br`, e o operador entrava na conta de
# produção achando que estava local (achado do dono, 17/set/2026). O `main.rs`
# passou a deduzir o site da API no mesmo dia; estas linhas continuam aqui para
# o endereço ser explícito em vez de deduzido. As portas são as do
# docker-compose.dev.yml; troque com VLB_POS_VENDA_URL e VLB_SITE_URL.
#
# 🔑 **A sessão da pilha local tem item próprio no chaveiro** e não encosta na de
# produção (`main.rs`, `cofre_da_sessao`) — o mesmo que o Tauri já fazia.
#
# ⚠️ **O catálogo é o desta máquina, e continua sendo.** Ele é local desde
# sempre — o que muda aqui é para qual servidor o app fala. Para abrir um
# catálogo descartável em vez do seu acervo, passe VLB_CATALOG:
#
#     VLB_CATALOG=/tmp/catalogo-de-teste crates/ui-gpui/rodar-local.sh
#
# 🔬 `debug` é de propósito: este é o alvo de mexer no código. Quem **mede** usa
# `make rodar` e `make medir`, em release — em debug uma miniatura custa 38 ms
# contra 0,67 ms, e o número não mediria este código (ver o Makefile).
set -eu
AQUI="$(cd "$(dirname "$0")" && pwd)"
export VLB_POS_VENDA_URL="${VLB_POS_VENDA_URL:-http://localhost:8080}"
export VLB_SITE_URL="${VLB_SITE_URL:-http://localhost:8001}"
if ! curl -fsS -o /dev/null "$VLB_POS_VENDA_URL/health"; then
  echo "A API local não responde em $VLB_POS_VENDA_URL. Rode 'make up' no recordarfotos-e-commerce." >&2
  exit 1
fi
cd "$AQUI/../.."
exec cargo run -p ui-gpui "$@"
