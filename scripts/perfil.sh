#!/usr/bin/env bash
# Onde a tela gasta tempo durante a carga — o `perf` sobre `e2e::carga`.
#
# O teste de carga (`make carga`) diz SE a tela ficou presa; isto diz ONDE,
# função por função. Três etapas, cada uma com teto de tempo: nada aqui fica
# rodando para sempre.
#
#   1. compila o teste no perfil `carga` (release sem LTO, com linhas)  ≤ 5 min
#   2. roda a carga sob o `perf record`                                 ≤ 2 min
#   3. imprime as funções mais caras                                    ≤ 1 min
#
# Pede o `perf` instalado e o kernel deixando medir:
#   sudo dnf install -y perf
#   sudo sysctl kernel.perf_event_paranoid=1   # vale até reiniciar
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v perf >/dev/null; then
    echo "❌ o perf não está instalado: sudo dnf install -y perf" >&2
    exit 1
fi
if [ "$(cat /proc/sys/kernel/perf_event_paranoid)" -gt 1 ]; then
    echo "❌ o kernel não deixa medir: sudo sysctl kernel.perf_event_paranoid=1" >&2
    exit 1
fi

echo "▶ 1/3 compilando (perfil carga, até 5 min)…"
binario=$(timeout 5m nice -n 10 cargo test --profile carga -j 6 -p ui-gpui --lib --no-run \
    --message-format=json 2>/dev/null |
    python3 -c 'import sys, json
for linha in sys.stdin:
    try: m = json.loads(linha)
    except ValueError: continue
    if m.get("executable") and m.get("target", {}).get("name") == "ui_gpui":
        print(m["executable"])' | tail -1)
if [ -z "$binario" ]; then
    echo "❌ a compilação falhou ou passou de 5 minutos — rode 'make carga' para ver o erro" >&2
    exit 1
fi

dados=target/perfil-carga.data
echo "▶ 2/3 rodando a carga sob o perf (até 2 min)…"
# 99 amostras por segundo com pilha DWARF: o bastante para ver quem pesa, e
# pouco o bastante para o arquivo não passar de algumas dezenas de MB.
VLB_CARGA=1 timeout 2m perf record -q -F 99 --call-graph dwarf,32768 -o "$dados" -- \
    "$binario" --test-threads=1 --nocapture carga::importacao |
    grep -E "🏋️|tecla:|volta do|^   |test result|panicked" || true

echo "▶ 3/3 as funções que mais seguram a thread (até 1 min)…"
# `-g none`: só a conta por função, sem as árvores de chamada (essas ficam no
# modo interativo, abaixo). O `|| true` é o SIGPIPE do `head` cortando a saída.
timeout 1m perf report -i "$dados" --stdio --children --percent-limit 3 \
    --sort symbol -g none 2>/dev/null | grep -v "^$" | grep -v "^#" | head -25 || true
echo
echo "🔎 Detalhe interativo: perf report -i $dados"
