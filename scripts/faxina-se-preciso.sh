#!/usr/bin/env bash
#
# Mantém `target/` dentro de um tamanho de trabalho.
#
# ## O que aconteceu para isto existir
#
# Em 2026-09-08 o disco encheu **duas vezes no mesmo dia**, no meio do trabalho —
# a segunda a ponto de nenhum comando conseguir nem gravar a própria saída. O
# dono: *"estou achando esse problema muito chato, toda hora preciso limpar"*.
#
# 🚨 **Aqui a árvore não é uma, são oito.** Este repositório compila para o Mac
# (duas arquiteturas mais a universal), para o Linux, para o wasm e para o
# empacotamento — cada um com o seu `target/<alvo>/`, e cada um com o seu
# `incremental/`. Os fragmentos incrementais aceleram a edição seguinte e
# **nunca são apagados sozinhos**: crescem por build, por perfil e por conjunto
# de `RUSTFLAGS`, e o `cargo` não tem teto configurável para eles.
#
# ## Por que apagar só o incremental, e não `cargo clean`
#
# `cargo clean` derruba junto as 478 dependências compiladas — a próxima build
# levaria minutos em vez de segundos, para recuperar espaço que o incremental já
# devolve. O que se apaga aqui volta a existir na primeira build, aos poucos.
#
# ## O gatilho é o tamanho, e não o calendário
#
# Sem `--agora`, o script mede e **só age acima do limite**. É o que permite
# chamá-lo de dentro do `make api-test` sem custo: na maioria das execuções ele
# imprime uma linha e sai.
#
# Uso:
#   ./scripts/faxina-se-preciso.sh              # age só acima de 12 GiB
#   ./scripts/faxina-se-preciso.sh --limite 8   # outro teto, em GiB
#   ./scripts/faxina-se-preciso.sh --agora      # apaga independente do tamanho
#
# Nunca falha o comando que o chamou: sai 0 mesmo sem `target/` nenhum.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$RAIZ/target}"
LIMITE_GIB=12
AGORA=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --limite) LIMITE_GIB="${2:-12}"; shift 2 ;;
        --agora)  AGORA=1; shift ;;
        -h|--help) sed -n '3,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "argumento desconhecido: $1" >&2; exit 0 ;;
    esac
done

if [[ ! -d "$TARGET" ]]; then
    echo "🧹 [Faxina] Não há $TARGET — nada a fazer."
    exit 0
fi

# Em KiB: é o que `du -sk` dá em qualquer Unix, sem depender do `-h` do macOS.
tamanho_kib() { du -sk "$1" 2>/dev/null | awk '{print $1}'; }
em_gib() { awk -v k="$1" 'BEGIN { printf "%.1f", k / 1048576 }'; }

ANTES_KIB="$(tamanho_kib "$TARGET")"
ANTES_KIB="${ANTES_KIB:-0}"
LIMITE_KIB=$(awk -v g="$LIMITE_GIB" 'BEGIN { printf "%d", g * 1048576 }')

if [[ "$AGORA" -eq 0 && "$ANTES_KIB" -lt "$LIMITE_KIB" ]]; then
    echo "🧹 [Faxina] target com $(em_gib "$ANTES_KIB") GiB, abaixo do teto de ${LIMITE_GIB} GiB — nada a fazer."
    exit 0
fi

echo "🧹 [Faxina] target com $(em_gib "$ANTES_KIB") GiB — limpando os fragmentos incrementais."

# Um por perfil e por alvo: `debug/`, `release/`, `aarch64-apple-darwin/…`,
# `x86_64-apple-darwin/…`, `wasm32-unknown-unknown/…`. O `-maxdepth 3` é o que
# alcança os cruzados, que ficam um nível mais fundo que os nativos.
find "$TARGET" -maxdepth 3 -type d -name incremental -print0 2>/dev/null |
    while IFS= read -r -d '' pasta; do
        echo "   • $(em_gib "$(tamanho_kib "$pasta")") GiB em ${pasta#"$RAIZ"/}"
        rm -rf "$pasta"
    done

# `cargo-sweep` tira o que sobrou de builds antigas (toolchain trocada, branch
# abandonado) sem tocar no que a árvore de hoje usa. Opcional de propósito: se
# não estiver instalado, o incremental acima já é a maior parte.
if command -v cargo-sweep >/dev/null 2>&1; then
    echo "   • cargo sweep: artefatos sem uso há mais de 14 dias"
    (cd "$RAIZ" && cargo sweep --time 14 >/dev/null 2>&1) || true
else
    echo "   • (dica: \`cargo install cargo-sweep\` tira também os artefatos de builds antigas)"
fi

DEPOIS_KIB="$(tamanho_kib "$TARGET")"
DEPOIS_KIB="${DEPOIS_KIB:-0}"
echo "🧹 [Faxina] $(em_gib "$ANTES_KIB") GiB → $(em_gib "$DEPOIS_KIB") GiB (liberados $(em_gib "$((ANTES_KIB - DEPOIS_KIB))") GiB)."
exit 0
