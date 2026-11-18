#!/usr/bin/env bash
#
# Escreve o `latest-linux.json` de um pacote já assinado (issue #37, Passo 3).
#
#   scripts/manifesto-do-pacote.sh <pacote.tar.gz> <pacote.tar.gz.minisig> <url-base> [saida.json]
#
# `url-base` é a pasta pública do R2 onde o pacote JÁ está (ex.:
# https://pub-….r2.dev). O manifesto sai no formato do `cargo_packager_updater`
# (`version`, `pub_date`, `platforms.linux-x86_64.{url,signature,format}`), mais
# o campo `glibc` — a maior GLIBC_ que o binário pede, lida do `.glibc` ao lado
# do pacote —, que o `instalar.sh` usa para cair para a compilação antes de
# baixar. O `format` leva `appimage` só para passar na leitura da biblioteca: o
# app ignora o campo (a troca do binário é do próprio app).
#
# 🚨 **Nunca toca em `latest.json`**, o manifesto congelado (0.1.24, só macOS) que
#    os `.dmg` antigos leem. O de Linux é outro arquivo, no mesmo bucket.
set -euo pipefail

[ "$#" -ge 3 ] || { sed -n '3,6p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 1; }
PACOTE="$1"; SIG="$2"; BASE="${3%/}"; SAIDA="${4:-latest-linux.json}"
[ -f "$PACOTE" ] && [ -f "$SIG" ] || { echo "❌ pacote ou assinatura não existe." >&2; exit 1; }
case "$(basename "$SAIDA")" in latest.json) echo "❌ latest.json é o manifesto congelado; escolha outro nome." >&2; exit 1 ;; esac

ARQ="$(basename "$PACOTE")"
VERSAO="$(echo "$ARQ" | sed -n 's/^vintagelightbox-\(.*\)-linux-x86_64\.tar\.gz$/\1/p')"
[ -n "$VERSAO" ] || { echo "❌ o nome '$ARQ' não é vintagelightbox-<versão>-linux-x86_64.tar.gz." >&2; exit 1; }
GLIBC="$(cat "${PACOTE%.tar.gz}.glibc" 2>/dev/null || true)"
[ -n "$GLIBC" ] || { echo "❌ falta ${PACOTE%.tar.gz}.glibc ao lado do pacote." >&2; exit 1; }
ASSINATURA="$(base64 < "$SIG" | tr -d '\n')"

cat > "$SAIDA" <<JSON
{
  "version": "$VERSAO",
  "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "platforms": {
    "linux-x86_64": {
      "url": "$BASE/$ARQ",
      "signature": "$ASSINATURA",
      "format": "appimage",
      "glibc": "$GLIBC"
    }
  }
}
JSON
echo "✅ $SAIDA ($VERSAO, glibc $GLIBC) → $BASE/$ARQ"
