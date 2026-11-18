#!/usr/bin/env bash
#
# Ensaio do instalador do Linux (issue #37): o ciclo inteiro, sem rede e sem
# tocar no seu HOME. Roda na fedora (ou em qualquer Linux com `minisign`).
#
#   scripts/testar-instalar-linux.sh                    usa um binário de mentira (rápido)
#   scripts/testar-instalar-linux.sh --binario CAMINHO  usa o app de verdade (compilado)
#
# O que ele faz: gera um par de chaves de TESTE, empacota, assina, serve o
# pacote e o `latest-linux.json` numa porta local e roda o `docs/instalar.sh`
# real, com um HOME novo e **sem `cargo` nem `rustc` no PATH**, em oito casos:
#
#   1. instalação nova            → instala, o binário responde, nada compilou
#   2. atualização por cima       → o instalado vira `.anterior`
#   3. pacote adulterado          → recusa e cai para a compilação; nada instalado
#   4. assinado por outra chave   → recusa e cai para a compilação
#   5. binário que diz outra versão → recusa e cai para a compilação
#   6. sem pacote publicado       → cai para a compilação, sem erro
#   7. glibc menor que a do pacote → cai para a compilação, dizendo o motivo
#   8. `--seco`                   → não instala nada
#
# 🔑 A chave de teste nunca é a do app. A chave de verdade (ID 33D71240F44E3949)
#    fica só no Mac do dono; aqui só se confere que a CÓPIA dela dentro do
#    `instalar.sh` é idêntica à de `empacotamento/chave-publica.txt`.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARIO=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --binario) BINARIO="${2:?--binario pede um caminho}"; shift ;;
    *) echo "opção desconhecida: $1" >&2; exit 1 ;;
  esac
  shift
done

command -v minisign >/dev/null || { echo "❌ falta o minisign (sudo dnf install minisign)." >&2; exit 1; }
command -v openssl  >/dev/null || { echo "❌ falta o openssl." >&2; exit 1; }

T="$(mktemp -d)"
PID_SERVIDOR=""
limpar() { [ -n "$PID_SERVIDOR" ] && kill "$PID_SERVIDOR" 2>/dev/null || true; rm -rf "$T"; }
trap limpar EXIT

FALHAS=0
passou() { printf '  ✅ %s\n' "$1"; }
falhou() { printf '  ❌ %s\n' "$1"; FALHAS=$((FALHAS + 1)); }
espera() { # $1 descrição  $2 saída  $3 trecho que deve aparecer
  if printf '%s' "$2" | grep -qF -- "$3"; then passou "$1"; else falhou "$1 (faltou: $3)"; printf '%s\n' "$2" | sed 's/^/      | /' | tail -15; fi
}
nao_espera() {
  if printf '%s' "$2" | grep -qF -- "$3"; then falhou "$1 (apareceu: $3)"; else passou "$1"; fi
}

VERSAO=$(awk '/^\[workspace.package\]/ {w=1; next} /^\[/ {w=0}
              w && /^version *=/ {gsub(/[" ]/, "", $0); sub(/version=/, ""); print; exit}' "$RAIZ/Cargo.toml")

echo "▸ a chave pública do instalar.sh é a do app?"
CHAVE_APP="$(tr -d '\n' < "$RAIZ/empacotamento/chave-publica.txt")"
if grep -qF "$CHAVE_APP" "$RAIZ/docs/instalar.sh"; then passou "docs/instalar.sh leva a chave de empacotamento/chave-publica.txt"; else falhou "a chave do instalar.sh difere da do app"; fi

# ── O binário ───────────────────────────────────────────────────────────────
if [ -z "$BINARIO" ]; then
  command -v cc >/dev/null || { echo "❌ sem compilador C para o binário de mentira; passe --binario." >&2; exit 1; }
  cat > "$T/falso.c" <<C
#include <stdio.h>
#include <string.h>
int main(int c, char **v) { for (int i = 1; i < c; i++) if (!strcmp(v[i], "--versao")) { puts("${FALSO_VERSAO:-$VERSAO}"); return 0; } return 0; }
C
  cc -O1 -o "$T/falso" "$T/falso.c"
  BINARIO="$T/falso"
fi

# ── Chaves de teste, pacote, assinatura, manifesto ──────────────────────────
echo "▸ empacotando e assinando com chave de TESTE"
mkdir -p "$T/srv"
minisign -G -f -W -p "$T/boa.pub" -s "$T/boa.key" >/dev/null
minisign -G -f -W -p "$T/outra.pub" -s "$T/outra.key" >/dev/null
CHAVE_TESTE="$(base64 -w0 < "$T/boa.pub")"
"$RAIZ/scripts/empacotar-pacote.sh" --binario "$BINARIO" --saida "$T/srv" >"$T/empacotar.log" 2>&1 \
  || { cat "$T/empacotar.log"; echo "❌ o empacotador falhou"; exit 1; }
PACOTE="$(ls "$T"/srv/vintagelightbox-*-linux-x86_64.tar.gz)"
minisign -S -s "$T/boa.key" -m "$PACOTE" -x "$PACOTE.minisig" >/dev/null
"$RAIZ/scripts/manifesto-do-pacote.sh" "$PACOTE" "$PACOTE.minisig" "http://127.0.0.1:0" "$T/srv/latest-linux.json" >/dev/null

# Servidor local numa porta livre; o manifesto é reescrito com a porta de verdade.
PORTA="$(python3 -I -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
sed -i "s|127.0.0.1:0|127.0.0.1:$PORTA|" "$T/srv/latest-linux.json"
( cd "$T/srv" && exec python3 -I -m http.server "$PORTA" --bind 127.0.0.1 >/dev/null 2>&1 ) &
PID_SERVIDOR=$!
for _ in $(seq 1 50); do curl -fs "http://127.0.0.1:$PORTA/latest-linux.json" >/dev/null 2>&1 && break; sleep 0.1; done

# Sem cargo, sem rustc: só o que vem com um Fedora de fábrica.
PATH_LIMPO="/usr/bin:/bin"
PATH="$PATH_LIMPO" command -v cargo >/dev/null 2>&1 && { echo "❌ cargo no PATH limpo; o ensaio não provaria nada"; exit 1; }

rodar() { # $1 nome do HOME  [env…] -- args
  local casa="$T/home-$1"; shift
  mkdir -p "$casa"
  env -i HOME="$casa" PATH="$PATH_LIMPO" LANG=C.UTF-8 \
      VLB_BASE="http://127.0.0.1:$PORTA" VLB_CHAVE_PUBLICA="$CHAVE_TESTE" \
      VLB_COMPILAR_COM="echo COMPILOU-NA-MAQUINA" "$@" 2>&1 || true
}
instalar() { local casa="$1"; shift; rodar "$casa" sh "$RAIZ/docs/instalar.sh" "$@"; }

echo "▸ 1. instalação nova (pelo pipe, como o cliente roda, sem cargo no PATH)"
SAIDA="$(mkdir -p "$T/home-nova"; env -i HOME="$T/home-nova" PATH="$PATH_LIMPO" LANG=C.UTF-8 VLB_BASE="http://127.0.0.1:$PORTA" VLB_CHAVE_PUBLICA="$CHAVE_TESTE" VLB_COMPILAR_COM="echo COMPILOU-NA-MAQUINA" sh -c "cat '$RAIZ/docs/instalar.sh' | sh" 2>&1 || true)"
espera "assinatura conferida" "$SAIDA" "assinatura confere com a chave do app"
espera "instalou" "$SAIDA" "instalado em $T/home-nova/.local"
nao_espera "não compilou" "$SAIDA" "COMPILOU-NA-MAQUINA"
APP="$T/home-nova/.local/bin/vintagelightbox-gpui"
[ -x "$APP" ] && [ "$("$APP" --versao)" = "$VERSAO" ] && passou "binário instalado responde --versao $VERSAO" || falhou "binário instalado não responde"
grep -q "^Exec=$APP\$" "$T/home-nova/.local/share/applications/vintagelightbox-gpui.desktop" && passou "atalho do menu aponta para o binário" || falhou "atalho do menu errado"
[ -f "$T/home-nova/.local/share/icons/hicolor/256x256/apps/vintagelightbox-gpui.png" ] && passou "ícone instalado" || falhou "ícone ausente"
[ ! -e "$T/home-nova/.cargo" ] && [ ! -e "$T/home-nova/.rustup" ] && passou "nenhum ~/.cargo nem ~/.rustup criado" || falhou "apareceu Rust no HOME"

echo "▸ 2. atualização por cima"
SAIDA="$(instalar nova)"
espera "instalou de novo" "$SAIDA" "instalado em"
[ -x "$APP.anterior" ] && passou "o instalado virou .anterior" || falhou "faltou o .anterior"
ls "$T/home-nova/.local/bin/" | grep -q '\.novo' && falhou "sobrou arquivo .novo" || passou "nada pela metade em ~/.local/bin"

echo "▸ 3. pacote adulterado depois de assinado"
cp "$PACOTE" "$PACOTE.bom"; printf 'adulterado' >> "$PACOTE"
SAIDA="$(instalar adulterado)"
espera "recusou pela assinatura" "$SAIDA" "pacote recusado: a assinatura não confere"
espera "caiu para a compilação" "$SAIDA" "COMPILOU-NA-MAQUINA"
[ ! -e "$T/home-adulterado/.local/bin/vintagelightbox-gpui" ] && passou "nada foi instalado" || falhou "instalou pacote adulterado"
mv "$PACOTE.bom" "$PACOTE"

echo "▸ 4. assinado por outra chave"
cp "$T/srv/latest-linux.json" "$T/manifesto.bom"
minisign -S -s "$T/outra.key" -m "$PACOTE" -x "$T/outra.minisig" >/dev/null
"$RAIZ/scripts/manifesto-do-pacote.sh" "$PACOTE" "$T/outra.minisig" "http://127.0.0.1:$PORTA" "$T/srv/latest-linux.json" >/dev/null
SAIDA="$(instalar outra)"
espera "recusou" "$SAIDA" "pacote recusado: o pacote foi assinado por outra chave"
espera "caiu para a compilação" "$SAIDA" "COMPILOU-NA-MAQUINA"

echo "▸ 5. o manifesto anuncia versão diferente da do binário"
sed 's/"version": "[^"]*"/"version": "9.9.9"/' "$T/manifesto.bom" > "$T/srv/latest-linux.json"
SAIDA="$(instalar versao)"
espera "recusou a troca" "$SAIDA" "o binário do pacote não abre aqui"
espera "caiu para a compilação" "$SAIDA" "COMPILOU-NA-MAQUINA"
[ ! -e "$T/home-versao/.local/bin/vintagelightbox-gpui" ] && passou "nada foi instalado" || falhou "instalou binário de outra versão"

echo "▸ 6. nenhum pacote publicado"
rm -f "$T/srv/latest-linux.json"
SAIDA="$(instalar sempacote)"
espera "avisou que não há pacote" "$SAIDA" "ainda não há pacote publicado para o Linux"
espera "caiu para a compilação" "$SAIDA" "COMPILOU-NA-MAQUINA"

echo "▸ 7. glibc daqui menor que a do pacote"
sed 's/"glibc": "[^"]*"/"glibc": "9.99"/' "$T/manifesto.bom" > "$T/srv/latest-linux.json"
SAIDA="$(instalar glibc)"
espera "disse o motivo" "$SAIDA" "é menor que a do pacote (9.99, a do Fedora 44)"
espera "caiu para a compilação" "$SAIDA" "COMPILOU-NA-MAQUINA"

echo "▸ 8. --seco"
cp "$T/manifesto.bom" "$T/srv/latest-linux.json"
SAIDA="$(instalar seco --seco)"
espera "disse o que faria" "$SAIDA" "[seco] baixaria"
[ ! -e "$T/home-seco/.local" ] && passou "não instalou nada" || falhou "o --seco instalou"

echo "▸ 9. --compilar pula o pacote"
SAIDA="$(instalar compilar --compilar)"
espera "foi direto para a compilação" "$SAIDA" "COMPILOU-NA-MAQUINA"

echo
if [ "$FALHAS" -eq 0 ]; then echo "✅ todos os casos passaram"; else echo "❌ $FALHAS falha(s)"; exit 1; fi
