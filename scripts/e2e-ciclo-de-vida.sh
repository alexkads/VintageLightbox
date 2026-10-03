#!/usr/bin/env bash
# 🎬 O e2e do ciclo de vida da sessão: a janela do app contra a API de verdade,
# num Postgres descartável. É a garantia de cada versão lançada (dono,
# 03/out/2026): criação da sessão, seleção com o cliente, revelação, venda no
# caixa, estorno, fechamento e pós-venda.
#
#   make e2e-ciclo              # ou: scripts/e2e-ciclo-de-vida.sh
#   scripts/e2e-ciclo-de-vida.sh --origin-dev
#
# `--origin-dev` é o que o `make producao` do e-commerce usa: o app e a API saem
# do `origin/dev` dos dois repositórios (o que vai virar `main`), em worktrees
# próprias e descartáveis — e não do que estiver no disco destes checkouts, que
# outras sessões dividem.
#
# Precisa do e-commerce ao lado (`../recordarfotos-e-commerce`, ou
# `VLB_ECOMMERCE=<pasta>`) e do Docker de pé. Nada sai da máquina: o
# `servidor-do-ciclo` (backend/crates/e2e-tests) sobe Postgres e Redis pelo
# testcontainers, a API numa porta livre, semeia o cenário e autoriza o app; o
# cenário `e2e::ciclo_de_vida` roda com um catálogo novo numa pasta temporária.
#
# 🚨 Um build por vez (a máquina é dividida com outras sessões): o script espera
# os `cargo build|test` que já estiverem rodando, e compila o servidor antes do
# app, nunca junto.
set -euo pipefail

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
ECOMMERCE="${VLB_ECOMMERCE:-$RAIZ/../recordarfotos-e-commerce}"
export CARGO_INCREMENTAL=0

ORIGIN_DEV=0
while [ $# -gt 0 ]; do
    case "$1" in
        --origin-dev) ORIGIN_DEV=1 ;;
        *) echo "opção desconhecida: $1" >&2; exit 2 ;;
    esac
    shift
done

falhar() {
    echo "❌ $*" >&2
    exit 1
}

[ -d "$ECOMMERCE/backend" ] || falhar "não achei o e-commerce em $ECOMMERCE (defina VLB_ECOMMERCE)"
docker info >/dev/null 2>&1 || falhar "o Docker não está de pé (macOS: open -a Docker)"

# 💾 O disco nunca enche: o binário de teste do app passa de 1 GB.
livre_gb=$(df -Pk / | awk 'NR==2 { print int($4 / 1048576) }')
[ "$livre_gb" -ge 10 ] || falhar "só ${livre_gb} GB livres no disco; libere espaço antes (mínimo 10 GB)"

# ⏳ Os builds de outras sessões terminam primeiro.
outros_builds() { ps -eo command | grep -E "^[^ ]*cargo (build|test|run|check|clippy)" | grep -v grep || true; }
for _ in $(seq 1 120); do
    [ -z "$(outros_builds)" ] && break
    echo "⏳ esperando outro build terminar: $(outros_builds | head -1)"
    sleep 15
done

TMP="$(mktemp -d "${TMPDIR:-/tmp}/e2e-ciclo.XXXXXX")"
SERVIDOR_PID=""
WORKTREES=()
encerrar() {
    local status=$?
    # Fechar o stdin é o "pode ir": o servidor solta os contêineres e sai.
    exec 3>&- 2>/dev/null || true
    if [ -n "$SERVIDOR_PID" ]; then
        for _ in $(seq 1 60); do
            kill -0 "$SERVIDOR_PID" 2>/dev/null || break
            sleep 0.5
        done
        kill "$SERVIDOR_PID" 2>/dev/null || true
    fi
    if [ "$status" -ne 0 ] && [ -s "$TMP/servidor.log" ]; then
        echo "── as últimas linhas do servidor ──" >&2
        tail -20 "$TMP/servidor.log" >&2
    fi
    for par in "${WORKTREES[@]+"${WORKTREES[@]}"}"; do
        git -C "${par%%|*}" worktree remove --force "${par#*|}" >/dev/null 2>&1 || true
    done
    rm -rf "$TMP"
    exit "$status"
}
trap encerrar EXIT
trap 'exit 130' INT TERM

# 🌿 `--origin-dev`: o código que vai para o `main`, em worktrees próprias. O
# cache de compilação vem clonado do checkout (APFS: cópia sem custo de disco);
# fora do macOS a worktree usa o `target` do checkout.
worktree_do_dev() {
    local repo="$1" destino="$2" alvo_do_repo="$3" alvo_da_worktree="$4"
    git -C "$repo" fetch -q origin dev
    git -C "$repo" worktree add -q --detach "$destino" origin/dev
    WORKTREES+=("$repo|$destino")
    if [ "$(uname -s)" = Darwin ] && [ -d "$alvo_do_repo/debug" ]; then
        mkdir -p "$alvo_da_worktree"
        cp -cR "$alvo_do_repo/debug" "$alvo_da_worktree/debug"
        rm -rf "$alvo_da_worktree/debug/incremental"
    fi
}
if [ "$ORIGIN_DEV" = 1 ]; then
    echo "🌿 o app e a API do origin/dev, em worktrees próprias…"
    worktree_do_dev "$RAIZ" "$TMP/app" "$RAIZ/target" "$TMP/app/target"
    worktree_do_dev "$ECOMMERCE" "$TMP/ecommerce" "$ECOMMERCE/backend/target" "$TMP/ecommerce/backend/target"
    if [ "$(uname -s)" != Darwin ]; then
        export CARGO_TARGET_DIR_APP="$RAIZ/target" CARGO_TARGET_DIR_API="$ECOMMERCE/backend/target"
    fi
    RAIZ="$TMP/app"
    ECOMMERCE="$TMP/ecommerce"
fi
BACKEND="$ECOMMERCE/backend"
[ -f "$BACKEND/crates/e2e-tests/src/bin/servidor-do-ciclo/main.rs" ] ||
    falhar "o servidor do ciclo não está em $BACKEND"

echo "🔨 compilando o servidor do ciclo (e-commerce)…"
no_backend() {
    (
        cd "$BACKEND"
        if [ -n "${CARGO_TARGET_DIR_API:-}" ]; then export CARGO_TARGET_DIR="$CARGO_TARGET_DIR_API"; fi
        "$@"
    )
}
no_backend cargo build -q -p e2e-tests --bin servidor-do-ciclo
ALVO_DO_BACKEND=$(no_backend cargo metadata --format-version 1 --no-deps |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')

echo "🐳 subindo Postgres, Redis e a API…"
mkfifo "$TMP/entrada"
"$ALVO_DO_BACKEND/debug/servidor-do-ciclo" <"$TMP/entrada" >"$TMP/servidor.json" 2>"$TMP/servidor.log" &
SERVIDOR_PID=$!
exec 3>"$TMP/entrada"
for _ in $(seq 1 360); do
    [ -s "$TMP/servidor.json" ] && break
    kill -0 "$SERVIDOR_PID" 2>/dev/null || falhar "o servidor do ciclo morreu antes de responder"
    sleep 0.5
done
[ -s "$TMP/servidor.json" ] || falhar "o servidor do ciclo não respondeu em 3 minutos"
echo "✅ API de teste em $(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["api"])' "$TMP/servidor.json")"

echo "🎬 o ciclo de vida da sessão no app…"
cd "$RAIZ"
if [ -n "${CARGO_TARGET_DIR_APP:-}" ]; then export CARGO_TARGET_DIR="$CARGO_TARGET_DIR_APP"; fi
VLB_E2E_CICLO="$TMP/servidor.json" VLB_CATALOG="$TMP/catalogo" \
    cargo test -p ui-gpui --lib e2e::ciclo_de_vida -- --ignored --test-threads=1 --nocapture
echo "✅ ciclo de vida da sessão: criação, seleção, revelação, caixa, estorno, fechamento e pós-venda"
