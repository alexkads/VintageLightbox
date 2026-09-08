#!/usr/bin/env bash
# Roda a prova do motor (`public/revelacao/teste.html`) num Chrome de verdade,
# **nos dois backends**, e falha se algum não passar.
#
# Uso: scripts/conferir-no-navegador.sh [caminho do frontend do e-commerce]
#      (padrão: $RECORDARFOTOS_FRONTEND, senão ../recordarfotos-e-commerce/frontend)
#
# 🚨 **Por que isto existe, e por que `cargo test` não substitui.**
#
# O WGSL sai deste repositório e é compilado por **dois** compiladores
# diferentes, que não concordam:
#
#   - **naga** (do wgpu) — usado no WebGL2 e em TODOS os testes nativos, do
#     desktop ao `revelacao-core`;
#   - **Tint** (do Dawn) — usado quando o navegador tem **WebGPU**, porque aí o
#     wgpu entrega o WGSL cru ao navegador e não compila nada.
#
# Em 6/set/2026 uma linha do grão misturava `*` e `^` sem parênteses. O naga
# aceitou, os 922 testes passaram, o WebGL2 desenhou certo — e no Chrome com
# WebGPU o Tint recusou o shader inteiro ("mixing '*' and '^' requires
# parenthesis"). O pipeline nascia inválido, o render pass não escrevia nada, e
# a revelação abria **preta**: sem erro na tela, com todos os sliders no neutro.
# Só um navegador de verdade pega isso.
set -euo pipefail

cd "$(dirname "$0")/.."

FRONTEND="${1:-${RECORDARFOTOS_FRONTEND:-../recordarfotos-e-commerce/frontend}}"
PUBLICO="$FRONTEND/public/revelacao"
[[ -f "$PUBLICO/teste.html" ]] || { echo "não achei $PUBLICO/teste.html" >&2; exit 1; }

CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
[[ -x "$CHROME" ]] || { echo "não achei o Chrome em $CHROME (defina CHROME=)" >&2; exit 1; }

PORTA="${PORTA:-8791}"
TEMP="$(mktemp -d)"
python3 -m http.server "$PORTA" --directory "$PUBLICO" >/dev/null 2>&1 &
SERVIDOR=$!
trap 'kill $SERVIDOR 2>/dev/null || true; rm -rf "$TEMP"' EXIT
sleep 1

falhou=0
for modo in webgpu webgl; do
    extra=(--enable-unsafe-webgpu)
    [[ $modo == webgl ]] && extra=(--disable-features=WebGPU)

    "$CHROME" --headless=new "${extra[@]}" --virtual-time-budget=30000 \
        --user-data-dir="$TEMP/perfil-$modo" \
        --dump-dom "http://localhost:$PORTA/teste.html" >"$TEMP/$modo.html" 2>/dev/null

    saida=$(python3 - "$TEMP/$modo.html" <<'PY'
import re, sys
html = open(sys.argv[1]).read()
try:
    corpo = html.split('<pre id="saida">')[1].split('</pre>')[0]
except IndexError:
    print("a página não carregou")
    raise SystemExit
print(re.sub(r"<[^>]+>", "", corpo).strip())
PY
)
    echo "── $modo"
    echo "$saida" | sed 's/^/   /'

    # 🔑 Três "OK" e nenhum "FALHOU": as linhas que a prova mede sozinha
    # (exposição, orientação, enquadramento). "Gradiente desenhado" não conta —
    # ela só diz que a chamada não lançou, e era o que o shader inválido
    # continuava dizendo enquanto entregava preto.
    oks=$(grep -c "OK$" <<<"$saida" || true)
    if grep -q "FALHOU" <<<"$saida" || [[ "$oks" -lt 3 ]]; then
        echo "   🚨 $modo não passou" >&2
        falhou=1
    fi
done

[[ $falhou -eq 0 ]] || { echo "a prova no navegador falhou" >&2; exit 1; }
echo "✅ os dois backends revelam certo"
