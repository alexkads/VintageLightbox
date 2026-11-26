#!/usr/bin/env bash
#
# 📦 Publica o RPM do Fedora nos dois lugares onde o app o procura (#55), na
# ordem do dono (26/11/2026): o Cloudflare R2 primeiro, o GitHub Release depois.
#
#   ./scripts/publicar-rpm.sh dist/fedora/vintagelightbox-0.1.205-1.fc45.x86_64.rpm
#
# Ao lado de cada .rpm vai o .sha256 (gerado pelo empacotar-rpm.sh), que o
# ajudante da atualização confere antes do dnf.
#
# - **R2**: bucket `vintagelightbox`, em `rpm/fc<N>/`, pelo `wrangler` (precisa
#   de `wrangler login` ou de CLOUDFLARE_API_TOKEN com escrita no bucket).
# - **GitHub**: o Release `v<versão>`, criado sobre o `main` se ainda não
#   existe, pelo `gh` autenticado.
#
# Cada destino é independente: um que falha não impede o outro, e o script sai
# com erro se algum falhou (o app tenta o seguinte da ordem, e por fim o script).
set -uo pipefail

RPM="${1:-}"
[ -f "$RPM" ] || { echo "uso: publicar-rpm.sh <arquivo .rpm>" >&2; exit 2; }
[ -f "$RPM.sha256" ] || { echo "falta $RPM.sha256 (gere com o empacotar-rpm.sh)" >&2; exit 2; }
ARQUIVO=$(basename "$RPM")
if ! [[ "$ARQUIVO" =~ ^vintagelightbox-([0-9]+\.[0-9]+\.[0-9]+)-1\.fc([0-9]+)\.x86_64\.rpm$ ]]; then
  echo "nome inesperado: $ARQUIVO" >&2; exit 2
fi
VERSAO="${BASH_REMATCH[1]}"; FEDORA="${BASH_REMATCH[2]}"
(cd "$(dirname "$RPM")" && sha256sum -c --quiet "$ARQUIVO.sha256") \
  || { echo "o sha256 não confere com o arquivo" >&2; exit 2; }

BUCKET="vintagelightbox"
PUBLICO="https://pub-f97274c3a83f47ff903a64fc1578efb0.r2.dev"
REPO="alexkads/VintageLightbox"
falhou=0

echo "▸ R2: rpm/fc$FEDORA/$ARQUIVO"
if command -v wrangler >/dev/null \
   && wrangler r2 object put "$BUCKET/rpm/fc$FEDORA/$ARQUIVO" --remote --file "$RPM" \
        --content-type application/x-rpm >/dev/null \
   && wrangler r2 object put "$BUCKET/rpm/fc$FEDORA/$ARQUIVO.sha256" --remote --file "$RPM.sha256" \
        --content-type text/plain >/dev/null \
   && curl -fsS -o /dev/null "$PUBLICO/rpm/fc$FEDORA/$ARQUIVO.sha256"; then
  echo "  ✅ $PUBLICO/rpm/fc$FEDORA/$ARQUIVO"
else
  echo "  ❌ o R2 não recebeu (o wrangler está autenticado com escrita no bucket $BUCKET?)"
  falhou=1
fi

echo "▸ GitHub: Release v$VERSAO"
if ! gh release view "v$VERSAO" -R "$REPO" >/dev/null 2>&1; then
  gh release create "v$VERSAO" -R "$REPO" --target main --latest=false \
    --title "VintageLightbox $VERSAO" \
    --notes "RPM do Fedora, gerado na máquina de testes. O app se atualiza sozinho; instalar à mão: sudo dnf install <link do .rpm>." \
    >/dev/null || true
fi
if gh release upload "v$VERSAO" -R "$REPO" --clobber "$RPM" "$RPM.sha256" >/dev/null; then
  echo "  ✅ https://github.com/$REPO/releases/download/v$VERSAO/$ARQUIVO"
else
  echo "  ❌ o GitHub não recebeu"
  falhou=1
fi
exit "$falhou"
