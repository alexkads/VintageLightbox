#!/usr/bin/env bash
# Compila a biblioteca de fotos para o navegador e entrega ao site.
#
# Uso: scripts/construir-biblioteca.sh [caminho do frontend do e-commerce]
#      (padrão: $RECORDARFOTOS_FRONTEND, senão ../recordarfotos-e-commerce/frontend)
#
# O que sai, e para onde:
#
#   frontend/public/biblioteca/biblioteca_web.js        o glue do wasm-bindgen (--target web)
#   frontend/public/biblioteca/biblioteca_web_bg.wasm   a grade em wgpu + o core
#   frontend/public/biblioteca/VERSAO                   o commit deste repositório
#   frontend/src/lib/biblioteca/biblioteca_web.d.ts     os tipos do glue, para o TypeScript
#   frontend/src/lib/biblioteca/versao.ts               a mesma VERSAO, importável
#
# 🔑 É o mesmo molde do `construir-web.sh` (o motor de revelação), e pelas
# mesmas razões: `--target web` dispensa configurar o bundler do Next, e os
# artefatos são commitados no repositório do site porque a Vercel não tem Rust.
#
# ⚠️ **O `?v=` não é enfeite**: um glue novo com um `.wasm` velho no cache chama
# exportações que não existem. As duas URLs carregam a versão.
# 🚨 **Se a pilha local estiver no ar, reinicie o `web`.**
#
# O `versao.ts` deste script é lido por `import`, e o Turbopack **não o
# recompila** quando ele muda por fora do editor: o contêiner continua servindo
# o `?v=` antigo, o navegador continua carregando o wasm de antes, e o motor
# novo simplesmente não entra. Isso custou uma ida e volta com o dono em
# 2026-09-12 — ele testou uma correção três vezes contra o binário velho.
#
#   docker compose -f docker-compose.dev.yml exec web \
#     sh -c 'rm -rf /app/.next/dev/cache /app/.next/dev/static /app/.next/dev/server'
#   docker compose -f docker-compose.dev.yml restart web
#
# 🚨 **E o `restart` sozinho não basta** — provado em 2026-09-12. Depois dele o
# HTML do servidor já vinha com o `?v=` novo e o **chunk do cliente** continuava
# sendo o antigo, vindo do cache do Turbopack: o selo de desenvolvimento da tela
# do cliente mostrava o commit velho enquanto o log do contêiner mostrava o
# novo. Só apagando `.next/dev` o chunk velho desaparece — e vale um
# `Cmd+Shift+R` na janela, que pode ter o dela em cache.
#
# Em produção não acontece: a Vercel constrói do zero a cada deploy.
set -euo pipefail

cd "$(dirname "$0")/.."

FRONTEND="${1:-${RECORDARFOTOS_FRONTEND:-../recordarfotos-e-commerce/frontend}}"
if [[ ! -d "$FRONTEND/public" ]]; then
    echo "frontend não encontrado em $FRONTEND (passe o caminho ou RECORDARFOTOS_FRONTEND)" >&2
    exit 1
fi
PUBLICO="$FRONTEND/public/biblioteca"
FONTE="$FRONTEND/src/lib/biblioteca"
SAIDA="target/biblioteca-web"

for ferramenta in wasm-pack wasm-opt; do
    command -v "$ferramenta" >/dev/null || {
        echo "falta $ferramenta — brew install wasm-pack binaryen" >&2
        exit 1
    }
done
rustup target add wasm32-unknown-unknown >/dev/null

echo "→ testes do core (é onde as decisões moram)"
cargo test -q -p biblioteca-core

echo "→ wasm-pack build (perfil release-web)"
wasm-pack build crates/biblioteca-web \
    --target web \
    --profile release-web \
    --out-dir "../../$SAIDA" \
    --out-name biblioteca_web

echo "→ wasm-opt -Oz"
mkdir -p "$PUBLICO" "$FONTE"
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    -o "$PUBLICO/biblioteca_web_bg.wasm" "$SAIDA/biblioteca_web_bg.wasm"
cp "$SAIDA/biblioteca_web.js" "$PUBLICO/biblioteca_web.js"
cp "$SAIDA/biblioteca_web.d.ts" "$FONTE/biblioteca_web.d.ts"

echo "→ VERSAO"
VERSAO="$(git rev-parse --short HEAD)"
if [[ -n "$(git status --porcelain crates/biblioteca-core crates/biblioteca-web)" ]]; then
    VERSAO="$VERSAO-sujo"
fi
echo "$VERSAO" > "$PUBLICO/VERSAO"
cat > "$FONTE/versao.ts" <<TS
// Gerado por scripts/construir-biblioteca.sh do VintageLightbox-Rust — não editar.
// É o commit da biblioteca que está em public/biblioteca/, e vai na URL dos dois
// arquivos (\`?v=\`) para o glue e o .wasm nunca descasarem no cache.
export const VERSAO = "$VERSAO";
TS

echo
ls -la "$PUBLICO"
echo
echo "biblioteca $VERSAO entregue em $PUBLICO"
