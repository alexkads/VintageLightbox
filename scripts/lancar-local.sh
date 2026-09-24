#!/usr/bin/env bash
#
# Publica o que houver em `dist/`: no Cloudflare R2 (principal) e no GitHub
# (espelho: Releases e a página do Pages).
#
#   ./scripts/lancar-local.sh                       # publica o que está em dist/
#   ./scripts/lancar-local.sh --notas "corrige X"   # com texto no aviso do app
#   ./scripts/lancar-local.sh --conferir-r2         # só confere o wrangler e o bucket
#   ./scripts/lancar-local.sh --aceitar-perda       # publica mesmo tirando plataforma
#   ./scripts/lancar-local.sh --estrear-plataforma  # publica plataforma que nunca esteve no ar
#
# 🔑 **É o caminho de quando o GitHub Actions não está disponível**, e foi
#    escrito porque ele não estava: em 7/set/2026 a conta ficou bloqueada por
#    cobrança e o `instaladores.yml` não podia rodar. Faz o mesmo que ele, da
#    máquina de quem lança.
#
# 🔑 **O R2 é o principal desde 24/set/2026; o GitHub é espelho.** O Pages
#    também roda como Action, por baixo (`pages-build-deployment`): se a trava de
#    cobrança o pegar, o manifesto de lá congela. Com o R2 ligado, o que o
#    GitHub recusar vira aviso e o lançamento segue — o balcão já está servido.
#    O envio é pelo `wrangler` autenticado de quem lança (`wrangler login`):
#    nenhuma chave S3 fica guardada em disco.
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
R2_BUCKET="vintagelightbox"
NOTAS=""
SO_CONFERIR=0
ACEITAR_PERDA=0
ESTREAR=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --notas) NOTAS="${2:-}"; shift ;;
    --conferir-r2) SO_CONFERIR=1 ;;
    --aceitar-perda) ACEITAR_PERDA=1 ;;
    --estrear-plataforma) ESTREAR=1 ;;
    *) printf 'opção desconhecida: %s\n' "$1" >&2; exit 1 ;;
  esac
  shift
done

diga() { printf '\n\033[1;36m▸ %s\033[0m\n' "$*"; }
aviso() { printf '\033[1;33m⚠️  %s\033[0m\n' "$*" >&2; }
erro() { printf '\033[1;31m❌ %s\033[0m\n' "$*" >&2; }

cd "$RAIZ"
VERSAO=$(sed -n '/^\[workspace\.package\]/,/^\[/p' Cargo.toml | sed -n 's/^version *= *"\(.*\)"/\1/p' | head -1)
TAG="v$VERSAO"
# O pacote carrega a versão do packager.toml e o app compara a do Cargo.toml:
# divergentes, o balcão instala uma e continua se achando na outra.
VERSAO_DO_PACOTE=$(sed -n 's/^version *= *"\(.*\)"/\1/p' empacotamento/packager.toml | head -1)
[[ "$VERSAO" == "$VERSAO_DO_PACOTE" ]] || { printf '\033[1;31m❌ Cargo.toml diz %s e packager.toml diz %s\033[0m\n' "$VERSAO" "$VERSAO_DO_PACOTE" >&2; exit 1; }

# ─── R2 ──────────────────────────────────────────────────────────────────────
#
# O endereço público sai da MESMA lista que o app compila
# (`empacotamento/enderecos-de-atualizacao.txt`): se os dois discordassem, o app
# perguntaria num lugar e o lançamento publicaria em outro.
MANIFESTO_R2=$(grep -v '^[[:space:]]*#' empacotamento/enderecos-de-atualizacao.txt | grep -m1 '\.r2\.dev/' || true)
BASE_R2="${MANIFESTO_R2%/latest.json}"

if [[ -n "$MANIFESTO_R2" ]]; then
  # 🚨 **Sem o wrangler, não publica nada.** O app que conhece o R2 pergunta lá
  #    primeiro, e um manifesto velho no R2 responde "nada novo" com sucesso —
  #    o updater não passa ao Pages por isso. Publicar só no GitHub deixaria
  #    esses balcões presos, em silêncio.
  command -v wrangler >/dev/null || { erro "o R2 está ligado na lista, mas falta o wrangler (npm i -g wrangler && wrangler login)"; exit 1; }
fi

# Envia um arquivo ao bucket, pela conta em que o `wrangler login` entrou.
# ⚠️ `--remote` é obrigatório: sem ele o wrangler grava num R2 simulado, local,
#    e diz que deu certo.
enviar() {  # enviar <arquivo> <destino no bucket> <content-type> <cache-control>
  wrangler r2 object put "$R2_BUCKET/$2" --remote --file "$1" \
    --content-type "$3" --cache-control "$4" >/dev/null 2>&1 \
    || { erro "o R2 recusou $2 (wrangler whoami mostra a conta certa?)"; return 1; }
}

tipo_de() {
  case "$1" in
    *.json) echo "application/json" ;;
    *.html) echo "text/html; charset=utf-8" ;;
    *.png) echo "image/png" ;;
    *.sh|*.sig|*.txt) echo "text/plain; charset=utf-8" ;;
    *.md) echo "text/markdown; charset=utf-8" ;;
    *) echo "application/octet-stream" ;;
  esac
}

# Sobe um arquivo pequeno e o lê de volta **pelo endereço público**: prova de
# uma vez que o wrangler escreve e que o bucket está aberto para o app ler.
conferir_r2() {
  local marca prova
  marca="conferido em $(date -u +%FT%TZ)"
  prova=$(mktemp)
  echo "$marca" >"$prova"
  enviar "$prova" conferencia.txt "text/plain; charset=utf-8" "no-cache" \
    || { rm -f "$prova"; return 1; }
  rm -f "$prova"
  [[ "$(curl -fsS "$BASE_R2/conferencia.txt?$RANDOM" 2>/dev/null)" == "$marca" ]] \
    || { erro "enviou, mas $BASE_R2 não devolve o arquivo — o acesso público (r2.dev) está ligado?"; return 1; }
  echo "   R2 ok: escreve em $R2_BUCKET e lê em $BASE_R2"
}

if [[ $SO_CONFERIR == 1 ]]; then
  [[ -n "$MANIFESTO_R2" ]] || { erro "nenhuma linha .r2.dev ativa em empacotamento/enderecos-de-atualizacao.txt"; exit 1; }
  conferir_r2
  exit
fi

# ─── Conferências de dist/ ───────────────────────────────────────────────────

# ⚠️ O `dist/` pode ter sobra de uma versão anterior — o empacotador não limpa.
#    Um `.dmg` da 0.1.0 ao lado do da 0.1.1 entraria no manifesto como se fosse
#    da versão nova, e o app baixaria o arquivo errado.
VELHOS=$(find dist -type f -name "*_*" ! -name "*_${VERSAO}_*" ! -name "*.app.tar.gz*" 2>/dev/null | head -5 || true)
if [[ -n "$VELHOS" ]]; then
  erro "há arquivos de outra versão em dist/ — eles entrariam no manifesto da $VERSAO:"
  echo "$VELHOS" | sed 's/^/     /'
  echo "   Apague-os (ou rode ./scripts/empacotar.sh --limpo) e tente de novo."
  exit 1
fi

ARQUIVOS=$(find dist -type f \( -name '*.dmg' -o -name '*.deb' -o -name '*.AppImage' \
             -o -name '*.msi' -o -name '*.exe' -o -name '*.tar.gz' -o -name '*.sig' \) 2>/dev/null | sort)
[[ -n "$ARQUIVOS" ]] || { erro "dist/ não tem pacote nenhum"; exit 1; }

[[ -n "$MANIFESTO_R2" ]] && { diga "conferindo o R2 antes de mexer em qualquer coisa"; conferir_r2; }

diga "montando o manifesto da $VERSAO"
python3 scripts/montar-manifesto.py --repo "$REPO" --tag "$TAG" --notas "$NOTAS" \
  ${MANIFESTO_R2:+--base "$BASE_R2"}

# ─── Travas: o que prejudicaria quem já tem o app instalado ──────────────────
#
# Compara o manifesto novo com o que está no ar, pelo primeiro endereço da
# lista (o que o app consulta primeiro). Recusar aqui não deixa rastro: o
# `docs/` volta ao que era.
diga "comparando com o que está no ar"
NO_AR=$(grep -v '^[[:space:]]*#' empacotamento/enderecos-de-atualizacao.txt | grep -m1 'https://')
if ! python3 - "$NO_AR" "$ACEITAR_PERDA" "$ESTREAR" <<'PY'
import json, sys, urllib.error, urllib.request
endereco, aceitar_perda, estrear = sys.argv[1], sys.argv[2] == "1", sys.argv[3] == "1"
novo = json.load(open("docs/latest.json"))
# ⚠️ Com User-Agent próprio: o r2.dev responde 403 ao do `urllib`.
pedido = urllib.request.Request(f"{endereco}?sem-cache", headers={"User-Agent": "lancar-local"})
try:
    with urllib.request.urlopen(pedido, timeout=20) as r:
        velho = json.load(r)
except urllib.error.HTTPError as e:
    if e.code != 404:
        print(f"❌ não li {endereco} (HTTP {e.code}) — sem saber o que está no ar, não publico", file=sys.stderr)
        sys.exit(1)
    print(f"   {endereco} ainda não existe: primeiro lançamento ali")
    sys.exit(0)
except Exception as e:
    print(f"❌ não li {endereco} ({e}) — sem saber o que está no ar, não publico", file=sys.stderr)
    sys.exit(1)
num = lambda v: tuple(int(p) for p in v.split("-")[0].split("."))
# 🚨 Versão menor que a publicada: o app não volta versão, e o manifesto
#    passaria a anunciar algo mais velho do que quem já atualizou tem.
if num(novo["version"]) < num(velho["version"]):
    print(f"❌ a {novo['version']} é MENOR que a {velho['version']} que está no ar — suba a versão", file=sys.stderr)
    sys.exit(1)
# 🚨 Plataforma que some do manifesto: quem a usa passa a ouvir "nada novo"
#    para sempre, sem aviso. É o que acontece ao publicar um dist/ incompleto.
perdidas = sorted(set(velho.get("platforms", {})) - set(novo["platforms"]))
if perdidas:
    if aceitar_perda:
        print(f"⚠️  aceito por --aceitar-perda: {', '.join(perdidas)} deixam de receber atualização", file=sys.stderr)
    else:
        print(f"❌ o manifesto novo perde {', '.join(perdidas)}, que estão no ar na {velho['version']}.", file=sys.stderr)
        print("   Traga o dist/ dessas plataformas, ou rode com --aceitar-perda se for de propósito.", file=sys.stderr)
        sys.exit(1)
# 🚨 Plataforma que estreia no manifesto: todo app daquele sistema que já está
#    na rua passa a receber o pacote — inclusive os instalados COMPILANDO na
#    máquina (o instalador .cmd), que não são o pacote. No Windows o NSIS
#    instala ao lado, noutra pasta, e o atalho antigo fica; no Linux o .AppImage
#    sobrescreve ~/.local/bin/vintagelightbox-gpui e só abre com FUSE. Estrear é
#    decisão do dono, com o plano para esses balcões na mão.
novas = sorted(set(novo["platforms"]) - set(velho.get("platforms", {})))
if novas and not estrear:
    print(f"❌ {', '.join(novas)} nunca estiveram no ar: os balcões desse sistema instalados pelo", file=sys.stderr)
    print("   script que compila receberiam o pacote. Ver empacotamento/README.md, \"Plataforma nova\",", file=sys.stderr)
    print("   e rode com --estrear-plataforma quando o dono tiver decidido.", file=sys.stderr)
    sys.exit(1)
print(f"   no ar: {velho['version']} ({', '.join(sorted(velho.get('platforms', {}))) or 'nenhuma'})")
print(f"   novo:  {novo['version']} ({', '.join(sorted(novo['platforms'])) or 'nenhuma'})")
PY
then
  git checkout -q -- docs/
  exit 1
fi

# ─── R2: principal ───────────────────────────────────────────────────────────

if [[ -n "$MANIFESTO_R2" ]]; then
  diga "enviando os pacotes ao R2 ($R2_BUCKET/$TAG/)"
  # Pasta por versão, e nome não se repete entre versões: pode ser cache eterno.
  for arquivo in $ARQUIVOS; do
    echo "   $(basename "$arquivo")"
    enviar "$arquivo" "$TAG/$(basename "$arquivo")" "$(tipo_de "$arquivo")" \
      "public, max-age=31536000, immutable"
  done

  diga "enviando a página e o manifesto ao R2"
  for arquivo in docs/index.html docs/icone.png docs/instalar.sh docs/downloads.json docs/notas-do-lancamento.md; do
    [[ -f "$arquivo" ]] && enviar "$arquivo" "$(basename "$arquivo")" "$(tipo_de "$arquivo")" "public, max-age=300"
  done
  # 🔑 **O manifesto por último**, e sem cache: enquanto ele não sobe, o app
  #    continua vendo a versão anterior — cujos pacotes seguem lá. Subi-lo
  #    antes dos pacotes abriria uma janela em que o app acha a versão nova e
  #    o download dá 404.
  enviar docs/latest.json latest.json "application/json" "no-cache"

  publicado=$(curl -fsS "$BASE_R2/latest.json?$RANDOM" | python3 -c "import json,sys;print(json.load(sys.stdin)['version'])")
  [[ "$publicado" == "$VERSAO" ]] || { erro "o R2 responde a versão $publicado, e não a $VERSAO"; exit 1; }
  printf '\033[1;32m✅ %s no R2 — o balcão já recebe\033[0m\n' "$VERSAO"
fi

# ─── GitHub: espelho ─────────────────────────────────────────────────────────
#
# Com o R2 ligado, uma falha daqui é aviso: o app 0.1.9 e anteriores só lêem o
# Pages, mas o manifesto de lá aponta para os pacotes do R2, então só o que
# importa é o `latest.json` chegar ao Pages. Sem o R2, é o único caminho, e
# falhar aborta como sempre.
espelho() {
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
}

if [[ -n "$MANIFESTO_R2" ]]; then
  # ⚠️ Não é `( espelho ) || aviso`: à esquerda de um `||` o bash desliga o
  #    `set -e`, e o espelho seguiria depois de um `git push` recusado.
  set +e
  ( set -e; espelho )
  status_do_espelho=$?
  set -e
  [[ $status_do_espelho == 0 ]] \
    || aviso "o espelho no GitHub falhou — o R2 já está servindo a $VERSAO; repita quando o GitHub voltar"
else
  aviso "R2 desligado: publicando só no GitHub (ver empacotamento/enderecos-de-atualizacao.txt)"
  espelho
fi

echo
printf '\033[1;32m✅ %s no ar\033[0m\n' "$VERSAO"
[[ -n "$MANIFESTO_R2" ]] && echo "   R2:        $BASE_R2/index.html"
echo "   arquivos:  https://github.com/$REPO/releases/tag/$TAG"
echo "   página:    https://alexkads.github.io/VintageLightbox/"
