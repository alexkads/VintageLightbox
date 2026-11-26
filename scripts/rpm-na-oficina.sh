#!/usr/bin/env bash
#
# 📦 O RPM de cada versão do `main`, gerado e publicado na fedora (#55) — o job
# `rpm-vintagelightbox` do Jenkins da oficina roda isto a cada 15 minutos.
#
#   scripts/rpm-na-oficina.sh            gera e publica se a versão do main ainda não tem RPM
#   scripts/rpm-na-oficina.sh --forcar   gera e publica de novo mesmo assim
#
# Barato quando não há nada a fazer: um `git fetch` e dois HEAD. Só compila
# (no perfil `instalador`, 15 a 40 min) quando a versão do `Cargo.toml` do
# `main` ainda não tem o .sha256 no R2 **e** no GitHub.
#
# Saída: 0 = publicado ou já estava; 1 = erro; 3 = publicou em parte (um dos
# destinos recusou — o app ainda acha pelo outro, ou compila pelo script).
#
# Variáveis: VLB_RPM_CLONE (o clone do main), CARGO_TARGET_DIR (um só,
# reaproveitado), e as credenciais do R2 em ~/oficina/segredos/r2-vintagelightbox.env
# (R2_ACCOUNT_ID, R2_ACCESS_KEY_ID e R2_SECRET_ACCESS_KEY de uma chave do R2
# restrita ao bucket `vintagelightbox`, com leitura e escrita de objetos).
set -euo pipefail

FORCAR=0
[ "${1:-}" = "--forcar" ] && FORCAR=1

CLONE="${VLB_RPM_CLONE:-$HOME/oficina/trabalho/rpm-vintagelightbox/vlb}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/oficina/trabalho/rpm-vintagelightbox/target}"
export CARGO_INCREMENTAL=0
PUBLICO="https://pub-f97274c3a83f47ff903a64fc1578efb0.r2.dev"
REPO="alexkads/VintageLightbox"

[ -f "$HOME/oficina/bin/ambiente.sh" ] && . "$HOME/oficina/bin/ambiente.sh"
SEGREDO="$HOME/oficina/segredos/r2-vintagelightbox.env"
if [ -f "$SEGREDO" ]; then set -a; . "$SEGREDO"; set +a; fi

echo "▸ o main"
if [ ! -d "$CLONE/.git" ]; then
  mkdir -p "$(dirname "$CLONE")"
  git clone -q --branch main "https://github.com/$REPO.git" "$CLONE"
fi
git -C "$CLONE" fetch -q origin main
git -C "$CLONE" checkout -q --detach origin/main
VERSAO=$(awk '/^\[workspace.package\]/ {w=1; next} /^\[/ {w=0}
              w && /^version *=/ {gsub(/[" ]/, "", $0); sub(/version=/, ""); print; exit}' \
         "$CLONE/Cargo.toml")
FEDORA=$(rpm -E '%{?fedora}')
ARQUIVO="vintagelightbox-$VERSAO-1.fc$FEDORA.x86_64.rpm"
echo "  $VERSAO ($(git -C "$CLONE" log -1 --format='%h %s' | cut -c1-80)) · Fedora $FEDORA"

publicado() { curl -fsSL -o /dev/null --max-time 30 "$1/$ARQUIVO.sha256"; }
no_r2=0; no_github=0
publicado "$PUBLICO/rpm/fc$FEDORA" && no_r2=1
publicado "https://github.com/$REPO/releases/download/v$VERSAO" && no_github=1
if [ "$FORCAR" -eq 0 ] && [ "$no_r2" -eq 1 ] && [ "$no_github" -eq 1 ]; then
  echo "✅ a $VERSAO já está no R2 e no GitHub: nada a fazer"
  exit 0
fi
echo "  R2: $([ $no_r2 -eq 1 ] && echo tem || echo falta) · GitHub: $([ $no_github -eq 1 ] && echo tem || echo falta)"

echo "▸ gerando o RPM da $VERSAO"
# Recompila só se o binário do target não for desta versão.
nice -n 10 "$CLONE/scripts/empacotar-rpm.sh"
RPM="$CLONE/dist/fedora/$ARQUIVO"
[ -f "$RPM" ] || { echo "❌ o empacotar-rpm.sh não deixou $RPM" >&2; exit 1; }

echo "▸ publicando"
if "$CLONE/scripts/publicar-rpm.sh" "$RPM"; then
  echo "✅ a $VERSAO está no R2 e no GitHub"
  rm -rf "$CLONE/dist/fedora" "$CLONE/target"
  exit 0
fi
rm -rf "$CLONE/dist/fedora" "$CLONE/target"
exit 3
