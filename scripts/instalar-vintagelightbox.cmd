:<<"::FIM-DO-CMD"
@echo off
rem  VintageLightbox - o endereco antigo do instalador.
rem
rem  O instalador de verdade e scripts/instalar-vintagelightbox-gpui.cmd, o app
rem  do balcao. Este arquivo continua existindo porque o Release antigo e quem
rem  copiou o comando antigo apontam para ele: ele baixa aquele instalador e o
rem  roda.
title Instalando o VintageLightbox
setlocal
set "VLB_SCRIPT=%~f0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; try { $t=$null; if ($env:VLB_SECO -ne '1') { try { $t=(Invoke-WebRequest -UseBasicParsing -TimeoutSec 30 'https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/instalar-vintagelightbox.cmd').Content } catch { } }; if (-not $t) { $t=[IO.File]::ReadAllText($env:VLB_SCRIPT) }; $n=[char]10; $i=$t.IndexOf($n+'#==POWERSHELL=='); if ($i -lt 0) { throw 'instalador invalido: falta o bloco PowerShell' }; $f=$t.IndexOf($n+'#==FIM-POWERSHELL==',$i); if ($f -le $i) { throw 'instalador incompleto' }; Invoke-Expression $t.Substring($i, $f-$i) } catch { Write-Host ''; Write-Host ('X ' + $_) -ForegroundColor Red; exit 1 }"
set "VLB_RESULTADO=%ERRORLEVEL%"
echo.
pause
exit /b %VLB_RESULTADO%
::FIM-DO-CMD
#
# VintageLightbox — o endereço antigo do instalador, que hoje só despacha.
#
# 🔑 **Por que ele ainda existe.** O instalador de verdade é um arquivo único:
#
#      scripts/instalar-vintagelightbox-gpui.cmd    VintageLightbox (Zed GPUI)
#
#    Mas este nome está no Release antigo, em cópias já baixadas nos balcões e
#    no comando que as pessoas copiaram. Quem roda o comando antigo recebe o
#    app do balcão.
#
# 🔑 **Nenhuma lógica de instalação mora aqui.** O despachante baixa o arquivo
#    do app da branch `dev` e o roda; sem internet, usa a cópia ao lado deste
#    arquivo, se houver (um clone do repositório). Sem internet não haveria
#    instalação de todo jeito: o código do app também vem do GitHub.
#
# 🚨 **As marcas `#==` ficam.** A cópia antiga do Release baixa ESTE arquivo da
#    `dev` e roda o trecho entre elas — que agora baixa o instalador do app e
#    roda o trecho dele. Fins de linha LF, como nos outros dois.
: <<'#==FIM-POWERSHELL=='
#==POWERSHELL==
# Sem `exit`: este trecho roda por `iex` (veja os instaladores de cada app).
$Arquivo = "instalar-vintagelightbox-gpui.cmd"
Write-Host "> Este e o endereco antigo. O instalador do app e $Arquivo." -ForegroundColor Cyan
$texto = $null
if ($env:VLB_SECO -ne "1") {
    try {
        $texto = (Invoke-WebRequest -UseBasicParsing -TimeoutSec 30 "https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/$Arquivo").Content
    } catch { }
}
if (-not $texto -and $env:VLB_SCRIPT) {
    $aoLado = Join-Path (Split-Path -Parent $env:VLB_SCRIPT) $Arquivo
    if (Test-Path $aoLado) { $texto = [IO.File]::ReadAllText($aoLado) }
}
if (-not $texto) { throw "nao consegui baixar $Arquivo. Confira a internet e rode de novo." }
$quebra = [char]10
$inicio = $texto.IndexOf($quebra + '#==POWERSHELL==')
if ($inicio -lt 0) { throw "$Arquivo invalido: falta o bloco PowerShell" }
$fim = $texto.IndexOf($quebra + '#==FIM-POWERSHELL==', $inicio)
if ($fim -le $inicio) { throw "$Arquivo incompleto" }
Invoke-Expression $texto.Substring($inicio, $fim - $inicio)
#==FIM-POWERSHELL==
#
# ── A parte do macOS e do Linux ──────────────────────────────────────────────
#
# As opções (`--versao`, `--destino`, `--seco`, `--ajuda`) passam direto para o
# instalador do app: `curl … | sh -s -- --seco`.

set -eu

ARQUIVO="instalar-vintagelightbox-gpui.cmd"
URL="https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/$ARQUIVO"

printf '▸ Este é o endereço antigo. O instalador do app é %s:\n' "$ARQUIVO"
printf '    curl -fsSL %s | sh\n' "$URL"

TEMPORARIO="$(mktemp -d "${TMPDIR:-/tmp}/vintagelightbox.XXXXXX")"
trap 'rm -rf "$TEMPORARIO"' 0
trap 'exit 130' INT
trap 'exit 143' TERM
INSTALADOR="$TEMPORARIO/$ARQUIVO"

# Da `dev`, a versão mais nova; sem internet, a cópia ao lado deste arquivo —
# só quando ele foi rodado de um arquivo (com `curl | sh`, `$0` é o `sh`).
if ! curl --connect-timeout 30 --retry 2 -fsSL "$URL" -o "$INSTALADOR" 2>/dev/null; then
  rm -f "$INSTALADOR"
  case "$0" in
    *instalar-vintagelightbox.cmd)
      AO_LADO="$(dirname "$0")/$ARQUIVO"
      [ -f "$AO_LADO" ] && cp "$AO_LADO" "$INSTALADOR" ;;
  esac
fi
# A primeira linha confere que chegou um instalador, e não uma página de erro.
if [ ! -f "$INSTALADOR" ] || [ "$(head -n 1 "$INSTALADOR")" != ':<<"::FIM-DO-CMD"' ]; then
  printf '❌ não consegui baixar %s. Confira a internet e rode de novo.\n' "$URL" >&2
  exit 1
fi

# `</dev/null`: com `curl | sh`, a entrada deste `sh` é o resto do script. O
# `sudo` do instalador pede a senha pelo terminal, que não passa por ela.
sh "$INSTALADOR" "$@" </dev/null
