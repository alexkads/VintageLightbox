:<<"::FIM-DO-CMD"
@echo off
rem  VintageLightbox (Tauri) - instalar compilando nesta maquina.
rem
rem  Um arquivo so para os tres sistemas:
rem    Windows ........ baixe e de dois cliques neste arquivo
rem    Linux e macOS .. curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox.cmd | sh
rem
rem  No Windows, esta parte roda o PowerShell que esta mais abaixo e espera uma
rem  tecla no fim, para a janela nao sumir com a mensagem. O PowerShell vem da
rem  versao mais nova do arquivo no GitHub; sem internet, desta copia. Assim a
rem  copia baixada do Release nunca fica velha.
title Instalando o VintageLightbox (Tauri)
powershell -NoProfile -ExecutionPolicy Bypass -Command "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; $t=$null; try { $t=(Invoke-WebRequest -UseBasicParsing 'https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox.cmd').Content } catch { }; if (-not $t) { $t=[IO.File]::ReadAllText('%~f0') }; $n=[char]10; $i=$t.IndexOf($n+'#==POWERSHELL=='); $f=$t.IndexOf($n+'#==FIM-POWERSHELL==',$i); try { Invoke-Expression $t.Substring($i, $f-$i) } catch { Write-Host ''; Write-Host ('X ' + $_) -ForegroundColor Red; exit 1 }"
echo.
pause
goto :eof
::FIM-DO-CMD
#
# VintageLightbox (Tauri) — instalar compilando nesta máquina. Um arquivo só.
#
# 🔑 **Três sistemas, um arquivo** (dono, 2026-09-16: "um único script que faça
#    tudo, sem o usuário de Windows e Linux ter conhecimento de nada"). Cada
#    interpretador lê só a parte dele:
#
#    - o `cmd` (dois cliques no Windows) lê a primeira linha como rótulo, roda o
#      bloco acima e para no `goto :eof`;
#    - o `sh` (Linux e macOS) passa pelo bloco acima e pelo do PowerShell como
#      dois *heredocs* entregues ao `:`, que não faz nada, e roda o resto;
#    - o PowerShell recebe do `cmd` só o trecho entre as marcas `#==`.
#
# 🚨 **Fins de linha LF, e o arquivo não usa rótulo além do `:eof`.** O `sh`
#    quebra com CRLF. O `cmd` aceita LF, desde que não precise procurar rótulo
#    (`goto :eof` não procura). O `.gitattributes` fixa o LF.
#
# 🚨 **Nenhuma linha do PowerShell pode ser exatamente a marca de fim** do
#    heredoc abaixo, senão o `sh` sai dele antes da hora.
: <<'#==FIM-POWERSHELL=='
#==POWERSHELL==
# 🚨 **Nenhum `exit` aqui.** Este trecho roda por `iex`, e `exit` fecharia a
#    janela junto, sem o operador ler o erro. Falha é `throw`, e quem mostra a
#    mensagem e espera uma tecla é o trecho do `cmd`, lá em cima.
#
# As opções vêm de variáveis de ambiente, porque o `iex` não recebe parâmetros:
#   VLB_VERSAO (padrão dev), VLB_DESTINO, VLB_SECO=1
$Versao  = if ($env:VLB_VERSAO) { $env:VLB_VERSAO } else { "dev" }
$Destino = if ($env:VLB_DESTINO) { $env:VLB_DESTINO } else { "$env:LOCALAPPDATA\Programs\VintageLightbox-Tauri" }
$Seco    = $env:VLB_SECO -eq "1"
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"   # o Invoke-WebRequest fica 10x mais lento com a barra

$Repo  = "https://github.com/alexkads/VintageLightbox"
$Casa  = "$env:USERPROFILE\.vintagelightbox"
$Fonte = Join-Path $Casa "fonte-tauri"
$Nome  = "VintageLightbox (Tauri)"

# 🚨 `-gnu`, e não `-msvc`: o `rsraw-sys` compila o LibRaw e recusa o MSVC com
#    `panic!("MSVC is not supported")`. O g++ do MSYS2 é pré-requisito por isso.
$Alvo = "x86_64-pc-windows-gnu"
# 🚨 **O Rust inteiro é `-gnu`, e não só o alvo.** Com o `-msvc` de host (o
#    padrão do rustup), os `build.rs` e as macros são ligados pelo `link.exe` do
#    Visual Studio, que a máquina do balcão não tem. A toolchain `-gnu` traz o
#    próprio ligador, e o Visual Studio deixa de ser preciso.
$Toolchain = "stable-$Alvo"
# A menor versão que as dependências aceitam (`rust-version` do `notify-rust`).
$RustMinimo = [version]"1.89"

function Diga($t)  { Write-Host "`n> $t" -ForegroundColor Cyan }
function Erro($t)  { Write-Host "X $t" -ForegroundColor Red }
function Aviso($t) { Write-Host "! $t" -ForegroundColor Yellow }
function Ok($t)    { Write-Host "OK $t" -ForegroundColor Green }
function Correr([scriptblock]$bloco) {
    if ($Seco) { Write-Host "   [seco] $bloco" } else { & $bloco | Out-Host }
}

# O winget vem no Windows 11 e no 10 atualizado, mas falta no LTSC e em
# instalacoes antigas. Sem ele, o erro do PowerShell nao diria o que fazer.
function Precisa-Winget($oque) {
    if (Get-Command winget -ErrorAction SilentlyContinue) { return }
    Erro "para instalar $oque, este instalador usa o winget, e ele nao existe nesta maquina."
    Write-Host "   Instale o 'Instalador de Aplicativo' pela Microsoft Store:"
    Write-Host "   https://apps.microsoft.com/detail/9NBLGGH4NNS1"
    Write-Host "   Ou instale $oque a mao, e depois rode este arquivo de novo."
    if (-not $Seco) { throw "falta o winget" }
}

# ── O que precisa existir antes de compilar ──────────────────────────────────
#
# 🚨 Tudo é conferido antes do `cargo build`. Uma compilação que morre no fim por
#    falta de ferramenta custa meia hora do balcão.
Diga "conferindo o que esta maquina tem"

# O WebView2 vem no Windows 11. No 10 pode faltar, e sem ele a janela nao abre.
$chaves = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
)
if (-not ($chaves | Where-Object { Test-Path $_ })) {
    Aviso "nao achei o WebView2. Instalando pelo winget."
    Precisa-Winget "o WebView2"
    Correr { winget install --silent --accept-package-agreements --accept-source-agreements Microsoft.EdgeWebView2Runtime }
} else {
    Ok "WebView2 instalado"
}

# O Rust, pelo rustup. Um `cargo` instalado sem rustup nao serve: a toolchain
# `-gnu` so se escolhe por ele.
$cargoDoUsuario = "$env:USERPROFILE\.cargo\bin"
if (Test-Path (Join-Path $cargoDoUsuario "rustup.exe")) { $env:PATH = "$cargoDoUsuario;$env:PATH" }
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Aviso "nao ha rustup nesta maquina. Instalando o Rust."
    if (-not $Seco) {
        $init = Join-Path $env:TEMP "rustup-init.exe"
        Invoke-WebRequest "https://win.rustup.rs/x86_64" -OutFile $init
        & $init -y --no-modify-path --profile minimal --default-host $Alvo --default-toolchain $Toolchain | Out-Host
        $env:PATH = "$cargoDoUsuario;$env:PATH"
    } else {
        Write-Host "   [seco] baixaria e rodaria rustup-init.exe --default-host $Alvo"
    }
}
function Versao-Do-Rust {
    $v = & rustup run $Toolchain rustc --version 2>$null
    if ($v -match '^rustc (\d+)\.(\d+)') { return [version]"$($Matches[1]).$($Matches[2])" }
    return $null
}
if (Get-Command rustup -ErrorAction SilentlyContinue) {
    $instaladas = @(& rustup toolchain list 2>$null) -join "`n"
    if ($instaladas -notmatch [regex]::Escape($Toolchain)) {
        Aviso "instalando o Rust $Toolchain"
        Correr { rustup toolchain install $Toolchain --profile minimal }
    } elseif ((Versao-Do-Rust) -lt $RustMinimo) {
        Aviso "o Rust desta maquina e anterior ao $RustMinimo. Atualizando."
        Correr { rustup update $Toolchain }
    }
    $versaoRust = Versao-Do-Rust
    if (-not $Seco -and ($null -eq $versaoRust -or $versaoRust -lt $RustMinimo)) {
        Erro "o Rust $Toolchain continua ausente ou anterior ao $RustMinimo."
        Write-Host "   Num terminal: rustup toolchain install $Toolchain"
        throw "Rust ausente ou antigo"
    }
    Ok "Rust: $(& rustup run $Toolchain rustc --version 2>$null)"
}

# O MSYS2 traz as duas ferramentas de C++ que o LibRaw pede. Ele nao poe o
# `mingw64\bin` no PATH do Windows sozinho, entao e procurado no lugar de sempre.
$msys = "C:\msys64"
$mingw = Join-Path $msys "mingw64\bin"
$msysBash = Join-Path $msys "usr\bin\bash.exe"
function Pacote-Msys($pacote, $oque) {
    if (-not (Test-Path $msysBash)) {
        Aviso "nao ha MSYS2 nesta maquina. Instalando pelo winget."
        Precisa-Winget "o MSYS2"
        Correr { winget install --silent --accept-package-agreements --accept-source-agreements MSYS2.MSYS2 }
    }
    Aviso "instalando $oque pelo MSYS2"
    # `-Sy`: a lista de pacotes que vem com o MSYS2 envelhece, e pedir um
    # pacote que o espelho ja trocou termina em 404.
    Correr { & $msysBash -lc "pacman -Sy --needed --noconfirm $pacote" }
}

# O g++ do MinGW, que compila o C++ do LibRaw.
if (-not (Get-Command g++ -ErrorAction SilentlyContinue)) {
    if (-not (Test-Path (Join-Path $mingw "g++.exe"))) {
        Pacote-Msys "mingw-w64-x86_64-gcc" "o g++ do MinGW"
    }
    $env:PATH = "$mingw;$env:PATH"
}
if (-not $Seco -and -not (Get-Command g++ -ErrorAction SilentlyContinue)) {
    Erro "o g++ do MinGW continua faltando. Sem ele o LibRaw nao compila."
    Write-Host "   Instale o MSYS2 (https://www.msys2.org) e, no terminal dele:"
    Write-Host "   pacman -S mingw-w64-x86_64-gcc"
    throw "falta o g++ do MinGW"
}
Ok "g++: $((Get-Command g++ -ErrorAction SilentlyContinue).Source)"

# 🚨 A libclang. O `rsraw-sys` gera as ligacoes do LibRaw com o bindgen, que
#    carrega a `libclang.dll`. Sem ela a compilacao morre no meio com "Unable to
#    find libclang". A maquina do GitHub ja traz o LLVM; a do balcao, nao.
$llvm = Join-Path $env:ProgramFiles "LLVM\bin"
$libclang = @($env:LIBCLANG_PATH, $mingw, $llvm) |
    Where-Object { $_ -and (Test-Path (Join-Path $_ "libclang.dll")) } |
    Select-Object -First 1
if (-not $libclang) {
    Pacote-Msys "mingw-w64-x86_64-clang" "a libclang"
    $libclang = $mingw
}
if (-not $Seco -and -not (Test-Path (Join-Path $libclang "libclang.dll"))) {
    Erro "a libclang continua faltando. Sem ela o LibRaw nao compila."
    Write-Host "   No terminal do MSYS2: pacman -S mingw-w64-x86_64-clang"
    throw "falta a libclang"
}
$env:LIBCLANG_PATH = $libclang
# A libclang do MSYS2 depende de outras DLLs da mesma pasta.
if ($libclang -eq $mingw -and ($env:PATH -split ';') -notcontains $mingw) { $env:PATH = "$env:PATH;$mingw" }
Ok "libclang: $libclang"

# ── Baixar o codigo ──────────────────────────────────────────────────────────
Diga "baixando o codigo de $Versao"
$url = if ($Versao -match '^v[0-9]') { "$Repo/archive/refs/tags/$Versao.zip" } else { "$Repo/archive/refs/heads/$Versao.zip" }
if ($Seco) {
    Write-Host "   [seco] baixaria $url para $Fonte"
} else {
    New-Item -ItemType Directory -Force -Path $Casa | Out-Null
    if (Test-Path $Fonte) { Remove-Item -Recurse -Force $Fonte }
    $zip = Join-Path $env:TEMP "vintagelightbox-tauri.zip"
    $aberto = Join-Path $env:TEMP "vintagelightbox-tauri"
    Invoke-WebRequest $url -OutFile $zip
    if (Test-Path $aberto) { Remove-Item -Recurse -Force $aberto }
    Expand-Archive $zip -DestinationPath $aberto
    # O zip do GitHub traz uma pasta so, com o nome do repositorio e da versao.
    Move-Item (Get-ChildItem $aberto | Select-Object -First 1).FullName $Fonte
    if (-not (Test-Path (Join-Path $Fonte "crates\app-tauri\Cargo.toml"))) {
        throw "a versao $Versao nao tem o crates\app-tauri"
    }
    Ok "codigo em $Fonte"
}

# ── Compilar ─────────────────────────────────────────────────────────────────
Diga "compilando (10 a 30 minutos na primeira vez)"
$env:CARGO_TARGET_DIR = Join-Path $Casa "target-tauri"
Correr { cargo "+$Toolchain" build --release --manifest-path (Join-Path $Fonte "Cargo.toml") -p app-tauri --bin app-tauri --target $Alvo }
$binario = Join-Path $env:CARGO_TARGET_DIR "$Alvo\release\app-tauri.exe"
if ($Seco) { Write-Host "`n   [seco] nada foi feito."; return }
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $binario)) {
    throw "a compilacao falhou, ou nao deixou $binario"
}

# ── Instalar ─────────────────────────────────────────────────────────────────
Diga "instalando em $Destino"
New-Item -ItemType Directory -Force -Path $Destino | Out-Null
$exe = Join-Path $Destino "VintageLightbox-Tauri.exe"
Copy-Item $binario $exe -Force
Copy-Item (Join-Path $Fonte "empacotamento\icones\icone.ico") (Join-Path $Destino "icone.ico") -Force

# O executavel liga com as DLLs do MinGW (libstdc++, libgcc, winpthread). Fora
# do PATH do MSYS2 ele nao abriria, entao elas vao junto.
foreach ($dll in "libstdc++-6.dll", "libgcc_s_seh-1.dll", "libwinpthread-1.dll") {
    $origem = Join-Path $mingw $dll
    if (Test-Path $origem) { Copy-Item $origem $Destino -Force }
}

$atalho = Join-Path ([Environment]::GetFolderPath("Programs")) "$Nome.lnk"
$shell = New-Object -ComObject WScript.Shell
$lnk = $shell.CreateShortcut($atalho)
$lnk.TargetPath = $exe
$lnk.WorkingDirectory = $Destino
$lnk.IconLocation = (Join-Path $Destino "icone.ico")
$lnk.Save()

Write-Host ""
Ok "$Nome instalado em $Destino"
Write-Host "   Abra pelo Menu Iniciar: $Nome"
Write-Host ""
Write-Host "   Para atualizar, rode este mesmo comando de novo: a segunda compilacao"
Write-Host "   reaproveita o cache em $env:CARGO_TARGET_DIR."
#==FIM-POWERSHELL==
#
# ── A parte do macOS e do Linux ──────────────────────────────────────────────
#
# 🔑 **Por que compilar na máquina** (dono, 2026-09-16): cada balcão gera o
#    próprio app, e ninguém precisa de instalador assinado nem de CI. O que sai
#    do compilador daqui não tem a marca de quarentena do macOS e abre no
#    primeiro duplo-clique.
#
# 🔑 **Servido pelo `raw.githubusercontent.com`, e não pelo Pages**: o Pages
#    publica por GitHub Actions, e este caminho não pode depender dele.
#
# ⚠️ **É a janela Tauri** (recordarfotos-e-commerce/docs/DESKTOP_TAURI.md), que
#    abre `/dashboard/sessoes-fotograficas` do site. O app GPUI continua em
#    `docs/instalar.sh`, e os dois convivem instalados.
#
# ⚠️ **Custa tempo e disco.** A primeira compilação leva de 10 a 30 minutos e usa
#    alguns GiB em `~/.vintagelightbox/target-tauri`. As seguintes reaproveitam.
#
# Opções (com `curl | sh`, passe-as depois de `sh -s --`):
#
#     --versao <ref>     branch ou tag a compilar. Padrão: dev
#     --destino <pasta>  onde instalar. Padrão: /Applications (macOS) ou
#                        ~/.local (Linux)
#     --seco             diz o que faria, sem fazer
#
# Ler antes de rodar:
#
#     curl -fsSL <url> -o instalar.cmd && less instalar.cmd && sh instalar.cmd

set -eu

REPO="https://github.com/alexkads/VintageLightbox"
CASA="${VLB_CASA:-$HOME/.vintagelightbox}"
FONTE="$CASA/fonte-tauri"
NOME="VintageLightbox (Tauri)"
# O que a barra de menus e o Dock mostram. O arquivo continua com o nome acima,
# porque o app GPUI já se chama VintageLightbox.app na mesma pasta.
NOME_EXIBIDO="VintageLightbox"
IDENTIFICADOR="br.com.recordarfotos.vintagelightbox.tauri"

REF="dev"; DESTINO=""; SECO=0

if [ -t 1 ]; then
  C='\033[1;36m'; V='\033[1;32m'; A='\033[1;33m'; E='\033[1;31m'; N='\033[1m'; Z='\033[0m'
else
  C=''; V=''; A=''; E=''; N=''; Z=''
fi

diga()  { printf "\n${C}▸ %s${Z}\n" "$*"; }
ok()    { printf "${V}✅ %s${Z}\n" "$*"; }
aviso() { printf "${A}⚠️  %s${Z}\n" "$*"; }
erro()  { printf "${E}❌ %s${Z}\n" "$*" >&2; }
correr() { if [ "$SECO" -eq 1 ]; then echo "   [seco] $*"; else "$@"; fi; }

# A ajuda mora aqui, e não é lida do arquivo: com `curl | sh` o script não
# existe em disco.
ajuda() {
  cat <<AJUDA
VintageLightbox (Tauri) — compila e instala nesta máquina (macOS ou Linux).

  curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox.cmd | sh

Opções (com curl | sh, depois de \`sh -s --\`):
  --versao <ref>     branch ou tag. Padrão: dev
  --destino <pasta>  onde instalar. Padrão: /Applications ou ~/.local
  --seco             diz o que faria, sem compilar nem instalar
  --ajuda            isto aqui
AJUDA
}

faltou() { erro "$1 precisa de um valor. Ex.: $1 ${2}"; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --versao)    [ $# -ge 2 ] || faltou --versao dev; REF="$2"; shift 2 ;;
    --versao=*)  REF="${1#*=}"; shift ;;
    --destino)   [ $# -ge 2 ] || faltou --destino ~/Apps; DESTINO="$2"; shift 2 ;;
    --destino=*) DESTINO="${1#*=}"; shift ;;
    --seco)      SECO=1; shift ;;
    -h|--ajuda|--help) ajuda; exit 0 ;;
    *) erro "opção desconhecida: $1"; echo "   as que existem: --versao, --destino, --seco, --ajuda"; exit 1 ;;
  esac
done

SISTEMA="$(uname -s)"
case "$SISTEMA" in
  Darwin|Linux) : ;;
  *) erro "sistema desconhecido. No Windows, dê dois cliques neste arquivo."; exit 1 ;;
esac

# ── O que precisa existir antes de compilar ───────────────────────────────────
#
# 🚨 Tudo é conferido **antes** do `cargo build`: uma compilação de meia hora que
#    morre no fim por falta de biblioteca é o pior desfecho possível.
diga "conferindo o que esta máquina tem"

for f in curl tar; do
  command -v "$f" >/dev/null 2>&1 || { erro "falta '$f'."; exit 1; }
done

# A menor versão que as dependências aceitam (`rust-version` do `notify-rust`).
RUST_MINIMO_MAIOR=1; RUST_MINIMO_MENOR=89

rust_serve() {
  command -v rustc >/dev/null 2>&1 || return 1
  # shellcheck disable=SC2046
  set -- $(rustc --version 2>/dev/null | sed -n 's/^rustc \([0-9]*\)\.\([0-9]*\).*/\1 \2/p')
  [ $# -eq 2 ] || return 1
  [ "$1" -gt "$RUST_MINIMO_MAIOR" ] || { [ "$1" -eq "$RUST_MINIMO_MAIOR" ] && [ "$2" -ge "$RUST_MINIMO_MENOR" ]; }
}

# 🚨 A libclang. O `rsraw-sys` gera as ligações do LibRaw com o bindgen, que
#    carrega a `libclang.so`. Sem ela a compilação morre no meio com "Unable to
#    find libclang".
tem_libclang() {
  for f in "${LIBCLANG_PATH:-/nenhum}"/libclang*.so* /usr/lib/llvm-*/lib/libclang*.so* \
           /usr/lib/*-linux-gnu/libclang*.so* /usr/lib64/libclang*.so* /usr/lib/libclang*.so*; do
    [ -e "$f" ] && return 0
  done
  return 1
}

if [ "$SISTEMA" = "Darwin" ]; then
  # O LibRaw é C++, e o compilador e a libclang vêm nas Command Line Tools. A
  # janela Tauri não usa o compilador Metal (o GPUI usa), então o Xcode inteiro
  # não é preciso.
  if ! xcrun -f clang++ >/dev/null 2>&1; then
    erro "faltam as Command Line Tools do Xcode (o compilador de C++)."
    echo "   Instale com:  xcode-select --install"
    echo "   Depois rode este script de novo."
    exit 1
  fi
  ok "Command Line Tools: $(xcode-select -p)"
else
  # 🔑 Cada peça é conferida por si: ter o WebKitGTK não diz nada do compilador
  #    nem da libclang. Faltando qualquer uma, a lista inteira é pedida ao
  #    gerenciador, que pula o que já existe. A lista é a da documentação do
  #    Tauri 2, mais o clang.
  FALTA=""
  command -v pkg-config >/dev/null 2>&1 || FALTA="$FALTA pkg-config"
  command -v c++ >/dev/null 2>&1 || FALTA="$FALTA compilador-de-C++"
  pkg-config --exists webkit2gtk-4.1 2>/dev/null || FALTA="$FALTA WebKitGTK-4.1"
  { pkg-config --exists ayatana-appindicator3-0.1 || pkg-config --exists appindicator3-0.1; } 2>/dev/null \
    || FALTA="$FALTA appindicator"
  pkg-config --exists xdo 2>/dev/null || [ -e /usr/include/xdo.h ] || FALTA="$FALTA libxdo"
  tem_libclang || FALTA="$FALTA libclang"

  if [ -n "$FALTA" ]; then
    aviso "faltam bibliotecas de desenvolvimento:$FALTA"
    # Como root (um contêiner, por exemplo) não há `sudo`, nem precisa.
    if [ "$(id -u)" -eq 0 ]; then SUDO=""; else SUDO="sudo"; fi
    if [ -n "$SUDO" ] && ! command -v sudo >/dev/null 2>&1; then
      erro "não há 'sudo' nesta máquina para instalar os pacotes."
      echo "   Peça a quem administra a máquina para instalar os pacotes da lista acima,"
      echo "   ou rode este script como root."
      exit 1
    fi
    if command -v apt-get >/dev/null 2>&1; then
      PACOTES="build-essential pkg-config curl libssl-dev libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev clang libclang-dev"
      # 🚨 `update` antes: numa máquina recém-instalada a lista de pacotes é a
      #    do dia da imagem, e o `install` responde "Unable to locate package".
      INSTALAR="${SUDO:+$SUDO }apt-get update && ${SUDO:+$SUDO }env DEBIAN_FRONTEND=noninteractive apt-get install -y $PACOTES"
    elif command -v dnf >/dev/null 2>&1; then
      PACOTES="gcc-c++ pkgconf-pkg-config curl openssl-devel webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel libxdo-devel clang clang-devel"
      INSTALAR="${SUDO:+$SUDO }dnf install -y $PACOTES"
    elif command -v pacman >/dev/null 2>&1; then
      PACOTES="base-devel curl openssl webkit2gtk-4.1 gtk3 libappindicator-gtk3 librsvg xdotool clang"
      INSTALAR="${SUDO:+$SUDO }pacman -Syu --needed --noconfirm $PACOTES"
    else
      erro "não reconheci o gerenciador de pacotes desta distribuição."
      echo "   Instale o equivalente a: webkit2gtk-4.1 (dev), gtk3 (dev), librsvg (dev),"
      echo "   libayatana-appindicator (dev), libxdo (dev), openssl (dev), clang e"
      echo "   libclang (dev) e um compilador de C++. Depois rode este script de novo."
      exit 1
    fi
    [ -n "$SUDO" ] && echo "   Instalando os pacotes (pede a senha de administrador):"
    echo "   $INSTALAR"
    if [ "$SECO" -eq 1 ]; then
      echo "   [seco] não instalei nada"
    # 🚨 `</dev/null`: com `curl | sh`, a entrada é o resto deste script, e o
    #    `apt` que lesse dela o engoliria. O `sudo` pede a senha pelo terminal.
    elif ! sh -c "$INSTALAR" </dev/null; then
      erro "a instalação dos pacotes falhou."
      echo "   Se a senha foi recusada, esta conta não pode instalar programas: peça a"
      echo "   quem administra a máquina para rodar o comando acima, e rode este script de novo."
      exit 1
    fi
    if [ "$SECO" -eq 0 ]; then
      pkg-config --exists webkit2gtk-4.1 2>/dev/null || { erro "o WebKitGTK 4.1 continua faltando depois da instalação."; exit 1; }
      tem_libclang || { erro "a libclang continua faltando depois da instalação."; exit 1; }
    fi
  fi
  ok "WebKitGTK 4.1: $(pkg-config --modversion webkit2gtk-4.1 2>/dev/null || echo 'a instalar')"
  ok "libclang: $(tem_libclang && echo presente || echo 'a instalar')"
fi

# O Rust, pelo rustup, sem privilégio nenhum (tudo em ~/.cargo e ~/.rustup).
#
# 🚨 O `~/.cargo/env` é lido **sempre**, e não só quando falta `cargo`: ele põe o
#    rustup à frente no PATH. Sem isso, um `cargo` antigo da distribuição (o do
#    `apt` fica anos atrás) ganharia, e a compilação morreria no meio.
# shellcheck disable=SC1091
if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi
if ! rust_serve; then
  if command -v rustup >/dev/null 2>&1; then
    aviso "o Rust desta máquina é anterior ao $RUST_MINIMO_MAIOR.$RUST_MINIMO_MENOR — atualizando pelo rustup"
    correr rustup toolchain install stable --profile minimal
    correr rustup default stable
  else
    if command -v rustc >/dev/null 2>&1; then
      aviso "o Rust desta máquina ($(rustc --version)) é antigo e não veio do rustup — instalando o rustup ao lado (em ~/.cargo, sem sudo)"
    else
      aviso "não há Rust nesta máquina — instalando o rustup (em ~/.cargo, sem sudo)"
    fi
    if [ "$SECO" -eq 0 ]; then
      # `-y` também responde "sim" quando já há um Rust fora do rustup.
      curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal >/dev/null
      # shellcheck disable=SC1091
      . "$HOME/.cargo/env"
    else
      echo "   [seco] curl https://sh.rustup.rs | sh -s -- -y"
    fi
  fi
fi
if rust_serve; then
  ok "Rust: $(rustc --version)"
elif [ "$SECO" -eq 1 ]; then
  aviso "Rust ausente ou antigo (modo seco)"
else
  erro "o Rust continua ausente ou anterior ao $RUST_MINIMO_MAIOR.$RUST_MINIMO_MENOR."
  echo "   Rode:  rustup update stable   e depois este script de novo."
  exit 1
fi

# ── Baixar o código ───────────────────────────────────────────────────────────
#
# O tarball, e não `git clone`: não é preciso ter git. `fonte-tauri/` é
# descartável e some a cada execução.
diga "baixando o código de $REF"
case "$REF" in
  v[0-9]*) URL="$REPO/archive/refs/tags/$REF.tar.gz" ;;
  *)       URL="$REPO/archive/refs/heads/$REF.tar.gz" ;;
esac
correr mkdir -p "$CASA"
correr rm -rf "$FONTE"
correr mkdir -p "$FONTE"
if [ "$SECO" -eq 1 ]; then
  echo "   [seco] curl -fsSL $URL | tar -xz -C $FONTE --strip-components=1"
else
  if ! curl -fsSL "$URL" | tar -xz -C "$FONTE" --strip-components=1; then
    erro "não consegui baixar $URL"
    exit 1
  fi
  [ -f "$FONTE/crates/app-tauri/Cargo.toml" ] || { erro "a versão $REF não tem o crates/app-tauri."; exit 1; }
  ok "código em $FONTE"
fi

# ── Compilar ──────────────────────────────────────────────────────────────────
#
# `--bin app-tauri`, e só ele: com `lto = true`, cada binário linka o programa
# inteiro, e a máquina do balcão não tem RAM sobrando.
diga "compilando (10 a 30 minutos na primeira vez)"
CARGO_TARGET_DIR="$CASA/target-tauri"
export CARGO_TARGET_DIR
correr cargo build --release --manifest-path "$FONTE/Cargo.toml" -p app-tauri --bin app-tauri

BINARIO="$CARGO_TARGET_DIR/release/app-tauri"
if [ "$SECO" -eq 0 ] && [ ! -x "$BINARIO" ]; then
  erro "a compilação terminou mas não há binário em $BINARIO"
  exit 1
fi
VERSAO="$(sed -n '/^\[workspace\.package\]/,/^\[/p' "$FONTE/Cargo.toml" 2>/dev/null | sed -n 's/^version *= *"\(.*\)"/\1/p' | head -1)"
[ -n "$VERSAO" ] || VERSAO="0.0.0"

# ── Instalar ──────────────────────────────────────────────────────────────────
if [ "$SISTEMA" = "Darwin" ]; then
  [ -n "$DESTINO" ] || { if [ -w /Applications ]; then DESTINO="/Applications"; else DESTINO="$HOME/Applications"; fi; }
  APP="$CASA/$NOME.app"
  diga "montando $NOME.app"
  if [ "$SECO" -eq 1 ]; then
    echo "   [seco] montaria $APP e copiaria para $DESTINO"
    exit 0
  fi
  rm -rf "$APP"
  mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
  cp "$BINARIO" "$APP/Contents/MacOS/app-tauri"
  cp "$FONTE/empacotamento/icones/icone.icns" "$APP/Contents/Resources/icone.icns"
  # 🔑 O identificador é outro que o do app GPUI: os dois convivem instalados, e
  #    o macOS guarda as permissões de cada um separadas.
  cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDisplayName</key>
	<string>$NOME_EXIBIDO</string>
	<key>CFBundleExecutable</key>
	<string>app-tauri</string>
	<key>CFBundleIconFile</key>
	<string>icone.icns</string>
	<key>CFBundleIdentifier</key>
	<string>$IDENTIFICADOR</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>$NOME_EXIBIDO</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSAO</string>
	<key>CFBundleVersion</key>
	<string>$(date +%Y%m%d.%H%M%S)</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.photography</string>
	<key>LSMinimumSystemVersion</key>
	<string>10.15</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
PLIST
  # Assinatura ad-hoc: não vem da Apple, mas dá ao app uma identidade estável
  # para o macOS pendurar as permissões.
  codesign --force --sign - "$APP" >/dev/null 2>&1 \
    || aviso "não consegui assinar ad-hoc — o macOS vai repetir os pedidos de permissão."
  mkdir -p "$DESTINO"
  rm -rf "$DESTINO/$NOME.app"
  ditto "$APP" "$DESTINO/$NOME.app"
  printf "\n${V}✅ %s %s instalado em %s${Z}\n\n" "$NOME" "$VERSAO" "$DESTINO"
  printf "   ${N}Abra pelo Launchpad, ou:${Z}  open -a \"%s\"\n" "$NOME"
else
  [ -n "$DESTINO" ] || DESTINO="$HOME/.local"
  diga "instalando em $DESTINO"
  if [ "$SECO" -eq 1 ]; then
    echo "   [seco] copiaria o binário para $DESTINO/bin e criaria o atalho do menu"
    exit 0
  fi
  mkdir -p "$DESTINO/bin" "$DESTINO/share/applications" "$DESTINO/share/icons/hicolor/256x256/apps"
  install -m 755 "$BINARIO" "$DESTINO/bin/vintagelightbox-tauri"
  cp "$FONTE/empacotamento/icones/256x256.png" "$DESTINO/share/icons/hicolor/256x256/apps/vintagelightbox-tauri.png"
  cat > "$DESTINO/share/applications/vintagelightbox-tauri.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=$NOME
Comment=Pós-venda da RecordarFotos
Exec=$DESTINO/bin/vintagelightbox-tauri
Icon=vintagelightbox-tauri
Categories=Graphics;Photography;
Terminal=false
StartupWMClass=app-tauri
DESKTOP
  command -v update-desktop-database >/dev/null 2>&1 \
    && update-desktop-database "$DESTINO/share/applications" >/dev/null 2>&1 || true
  printf "\n${V}✅ %s %s instalado${Z}\n\n" "$NOME" "$VERSAO"
  printf "   ${N}Abra pelo menu de aplicativos, ou:${Z}  %s\n" "$DESTINO/bin/vintagelightbox-tauri"
fi

echo
echo "   Para atualizar, rode este mesmo comando de novo: a segunda compilação"
echo "   reaproveita o cache em $CARGO_TARGET_DIR."
echo "   Apagar o cache é seguro:  rm -rf \"$CARGO_TARGET_DIR\""
