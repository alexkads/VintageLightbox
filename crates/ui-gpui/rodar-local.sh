#!/bin/sh
# Abre o app GPUI (depuração) contra a pilha local do e-commerce, e não contra
# produção. **Sobe a pilha sozinho se ela não estiver de pé.**
#
#     crates/ui-gpui/rodar-local.sh
#
# O par de `crates/app-tauri/rodar-local.sh`, e pelo mesmo motivo: as duas
# interfaces se validam uma à outra (DESKTOP_TAURI, D7), e uma delas não pode
# ser mais difícil de abrir local que a outra.
#
# ## 🚨 Por que ele subiu de "confere" para "prepara"
#
# Até 19/set/2026 este script só media a temperatura: se a API não respondesse,
# ele mandava rodar `make up` e saía. Na prática isso rendia uma tarde assim —
# subir a pilha à mão, descobrir que o **site** (8001) não tinha subido junto,
# descobrir depois que o banco não tinha **usuário nenhum**, criar um pela API,
# e então descobrir que autorizar o desktop exige `ManageMedia`, que só o
# `ADMIN` tem. O dono, no fim: *"olha o tempo perdido"*.
#
# Nenhum desses passos era decisão de ninguém — eram pré-requisitos sabidos. Um
# script que conhece o pré-requisito e manda o humano executá-lo está guardando
# trabalho, não evitando risco.
#
# ## O que ele garante antes de compilar
#
# 1. **Os contêineres de pé** — `docker compose up -d` no e-commerce, que é
#    idempotente: o que já está no ar não é reiniciado.
# 2. **O seed aplicado** — o `migrate` roda a cada subida e cria
#    `admin@recordarfotos.com` / `admin123` como **ADMIN**, que é a conta que a
#    tela de login do site anuncia e a única que autoriza o desktop
#    (`backend/supabase/dev-contas.sql`).
# 3. **A API e o site respondendo** — os dois, e não só a API. O app abre o
#    navegador no site para autorizar: com a 8001 fora do ar, o fluxo morre no
#    meio e a tela fica girando.
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
# Para pular a preparação (a pilha já está como você quer, ou você a roda à
# mão), passe VLB_SEM_PILHA=1 — ele volta a só conferir e falhar.
#
# 🔬 `debug` é de propósito: este é o alvo de mexer no código. Quem **mede** usa
# `make rodar` e `make medir`, em release — em debug uma miniatura custa 38 ms
# contra 0,67 ms, e o número não mediria este código (ver o Makefile).
set -eu
AQUI="$(cd "$(dirname "$0")" && pwd)"
RAIZ="$(cd "$AQUI/../.." && pwd)"

export VLB_POS_VENDA_URL="${VLB_POS_VENDA_URL:-http://localhost:8080}"
export VLB_SITE_URL="${VLB_SITE_URL:-http://localhost:8001}"

# O e-commerce é irmão deste repositório. Quem o mantém noutro lugar passa
# RECORDARFOTOS_ECOMMERCE.
ECOMMERCE="${RECORDARFOTOS_ECOMMERCE:-$(cd "$RAIZ/.." 2>/dev/null && pwd)/recordarfotos-e-commerce}"

responde() {
  curl -fsS -m 3 -o /dev/null "$1" 2>/dev/null
}

# Espera um endereço responder. `$3` é o teto em segundos — a API compila o
# backend Rust na primeira subida, e isso passa de dois minutos numa máquina
# fria.
esperar() {
  endereco="$1"; nome="$2"; teto="$3"; gasto=0
  while ! responde "$endereco"; do
    if [ "$gasto" -ge "$teto" ]; then
      echo "⛔ $nome não respondeu em ${teto}s ($endereco)." >&2
      echo "   Veja o que houve com: docker compose -f docker-compose.dev.yml logs --tail 40" >&2
      return 1
    fi
    # ⚠️ `if`, e não `[ … ] && printf`: sob `set -e` o `&&` que dá falso
    # devolve 1 e **derruba o script** — e o caso comum, o endereço já no ar,
    # cai exatamente nesse falso.
    if [ "$gasto" -eq 0 ]; then
      printf '⏳ esperando %s' "$nome"
    fi
    printf '.'
    sleep 3
    gasto=$((gasto + 3))
  done
  if [ "$gasto" -gt 0 ]; then
    printf ' pronto (%ss)\n' "$gasto"
  fi
  return 0
}

if [ "${VLB_SEM_PILHA:-}" = "1" ]; then
  responde "$VLB_POS_VENDA_URL/health" || {
    echo "A API local não responde em $VLB_POS_VENDA_URL. Rode 'make up' no recordarfotos-e-commerce." >&2
    exit 1
  }
elif [ -f "$ECOMMERCE/docker-compose.dev.yml" ]; then
  echo "🐳 preparando a pilha local em $ECOMMERCE"
  # `up -d` não reinicia o que já está no ar, e o `migrate` roda de novo a cada
  # chamada — é ele que aplica o seed das contas de desenvolvimento.
  (cd "$ECOMMERCE" && docker compose -f docker-compose.dev.yml up -d)
  esperar "$VLB_POS_VENDA_URL/health" "a API" 600
  esperar "$VLB_SITE_URL" "o site" 300
  echo "✅ pilha pronta — entre com admin@recordarfotos.com / admin123"
else
  echo "⛔ não achei o recordarfotos-e-commerce em $ECOMMERCE." >&2
  echo "   Aponte com RECORDARFOTOS_ECOMMERCE=/caminho/do/repo, ou suba a pilha à mão." >&2
  exit 1
fi

cd "$RAIZ"
exec cargo run -p ui-gpui "$@"
