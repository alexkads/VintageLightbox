#!/usr/bin/env bash
#
# Publica o que houver em `dist/` — no GitHub Releases e na página do Pages.
#
#   ./scripts/lancar-local.sh                       # publica o que está em dist/
#   ./scripts/lancar-local.sh --notas "corrige X"   # com texto no aviso do app
#
# 🔑 **É o caminho de quando o GitHub Actions não está disponível**, e foi
#    escrito porque ele não estava: em 7/set/2026 a conta ficou bloqueada por
#    cobrança e o `instaladores.yml` não podia rodar. Faz o mesmo que ele, da
#    máquina de quem lança.
#
# 🚨 **O manifesto é montado a partir de `dist/` INTEIRO, e reescrito por
#    completo.** Publicar só o Linux num `dist/` que não tem mais o macOS
#    **apaga o macOS do manifesto** — a página perde o download e todo Mac
#    instalado passa a receber "nada novo", em silêncio.
#
#    Então: gere cada plataforma na máquina dela, **junte tudo em `dist/` aqui**,
#    e só então rode isto. As pastas convivem:
#
#      dist/macos-universal/   ← do Mac
#      dist/linux-x86_64/      ← trazido da máquina Linux
#      dist/windows-x86_64/    ← trazido da máquina Windows
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="alexkads/VintageLightbox"
NOTAS=""
[[ "${1:-}" == "--notas" ]] && NOTAS="${2:-}"

diga() { printf '\n\033[1;36m▸ %s\033[0m\n' "$*"; }
erro() { printf '\033[1;31m❌ %s\033[0m\n' "$*" >&2; }

cd "$RAIZ"
VERSAO=$(sed -n '/^\[workspace\.package\]/,/^\[/p' Cargo.toml | sed -n 's/^version *= *"\(.*\)"/\1/p' | head -1)
TAG="v$VERSAO"

# ⚠️ O `dist/` pode ter sobra de uma versão anterior — o empacotador não limpa.
#    Um `.dmg` da 0.1.0 ao lado do da 0.1.1 entraria no manifesto como se fosse
#    da versão nova, e o app baixaria o arquivo errado.
VELHOS=$(find dist -type f -name "*_*" ! -name "*_${VERSAO}_*" ! -name "*.app.tar.gz*" 2>/dev/null | head -5)
if [[ -n "$VELHOS" ]]; then
  erro "há arquivos de outra versão em dist/ — eles entrariam no manifesto da $VERSAO:"
  echo "$VELHOS" | sed 's/^/     /'
  echo "   Apague-os (ou rode ./scripts/empacotar.sh --limpo) e tente de novo."
  exit 1
fi

diga "montando o manifesto da $VERSAO"
python3 scripts/montar-manifesto.py --repo "$REPO" --tag "$TAG" --notas "$NOTAS"

diga "commitando docs/ — é de onde o Pages serve"
git add docs/
if git diff --cached --quiet; then
  echo "   nada mudou em docs/"
else
  git commit -q -m "Publicar a $VERSAO: manifesto e lista de downloads

Gerado por scripts/lancar-local.sh a partir do que havia em dist/.
Plataformas: $(python3 -c "import json;print(', '.join(sorted(json.load(open('docs/latest.json'))['platforms'])) or 'nenhuma')")"
  git push origin HEAD
fi

diga "publicando o Release $TAG"
ARQUIVOS=$(find dist -type f \( -name '*.dmg' -o -name '*.deb' -o -name '*.AppImage' \
             -o -name '*.msi' -o -name '*.exe' -o -name '*.tar.gz' -o -name '*.sig' \) | sort)
[[ -n "$ARQUIVOS" ]] || { erro "dist/ não tem pacote nenhum"; exit 1; }

if gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
  # Acrescentar plataforma a um lançamento que já existe: `--clobber` troca o
  # arquivo se ele já estiver lá, em vez de recusar.
  echo "   o release já existe — acrescentando arquivos"
  # shellcheck disable=SC2086
  gh release upload "$TAG" --repo "$REPO" --clobber $ARQUIVOS
  gh release edit "$TAG" --repo "$REPO" --notes-file docs/notas-do-lancamento.md
else
  # shellcheck disable=SC2086
  gh release create "$TAG" --repo "$REPO" \
    --title "VintageLightbox $VERSAO" \
    --notes-file docs/notas-do-lancamento.md \
    $ARQUIVOS
fi

echo
printf '\033[1;32m✅ %s no ar\033[0m\n' "$VERSAO"
echo "   arquivos:  https://github.com/$REPO/releases/tag/$TAG"
echo "   página:    https://alexkads.github.io/VintageLightbox/"
