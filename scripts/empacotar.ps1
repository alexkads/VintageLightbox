<#
.SYNOPSIS
    O gerador de instaladores do VintageLightbox para Windows: .msi e .exe.

.DESCRIPTION
    O par de `scripts/empacotar.sh`, que gera o macOS e o Linux. Este roda numa
    máquina Windows — VM neste Mac ou qualquer PC —, usa a **mesma**
    `empacotamento/packager.toml` e a **mesma** chave de assinatura, e produz os
    dois instaladores que o site publica.

    Por que não sai do Mac: o `build.rs` do gpui compila os sete shaders HLSL
    com o `fxc.exe`, que só existe como binário do Windows, e faz isso dentro de
    `#[cfg(target_os = "windows")]` — a máquina que **compila**. Do macOS o
    trecho nem roda. O motivo inteiro está em `empacotamento/README.md`.

.EXAMPLE
    .\scripts\empacotar.ps1
    .\scripts\empacotar.ps1 -Conferir     # só diz o que falta instalar

.NOTES
    Se o Windows recusar com "execution of scripts is disabled on this system",
    é a política de execução, e não o script:

        powershell -ExecutionPolicy Bypass -File .\scripts\empacotar.ps1

    O `make windows` do Makefile faz exatamente esta chamada. Ele é atalho, não
    requisito — o Windows 11 não vem com `make`, e quem quiser um instala com
    `winget install ezwinports.make`.

    O binário sai como `ui-gpui.exe`, e a `packager.toml` diz `ui-gpui` sem
    extensão de propósito — o cargo-packager acrescenta o `.exe` sozinho no
    Windows. Não "corrija" isso na configuração: quebraria o macOS e o Linux.
#>
[CmdletBinding()]
param(
    # Só confere os pré-requisitos e sai, sem compilar nada.
    [switch]$Conferir,
    # A chave que assina a atualização. Sem ela saem instaladores, e nenhum app
    # já instalado aceita esta versão como atualização.
    [string]$Chave = "$env:USERPROFILE\.vintagelightbox\atualizacao.key",
    # A senha da chave, se ela tiver uma. Vazia é o padrão do `signer generate --ci`.
    [string]$SenhaDaChave = ""
)

$ErrorActionPreference = "Stop"

$Raiz    = Split-Path -Parent $PSScriptRoot
$Config  = Join-Path $Raiz "empacotamento\packager.toml"
$Dist    = Join-Path $Raiz "dist\windows-x86_64"
$Palco   = Join-Path $Raiz "target\empacotamento"
$Bin     = "ui-gpui.exe"
# 🚨 `-gnu`, e não `-msvc`. O `rsraw-sys` monta os ~200 `.cpp` do LibRaw e faz
#    `panic!("MSVC is not supported")` — no Windows também, não só cruzando.
#    MinGW é o único caminho, e é por isso que o g++ do MSYS2 é pré-requisito.
$Alvo    = "x86_64-pc-windows-gnu"

function Diga($t)  { Write-Host "`n> $t" -ForegroundColor Cyan }
function Erro($t)  { Write-Host "X $t" -ForegroundColor Red }
function Aviso($t) { Write-Host "! $t" -ForegroundColor Yellow }
function Ok($t)    { Write-Host "OK $t" -ForegroundColor Green }

# ── Os pré-requisitos, um a um, com o comando que instala cada um ────────────
#
# 🚨 **Todo comando externo aqui dentro termina em `| Out-Host`, e isso não é
#    estilo.** Em PowerShell uma função devolve **tudo** que ela escreve no
#    pipeline, não só o `return`. Um `& rustup target add` solto faz a função
#    devolver `@("saida do rustup...", $false)` — e `-not` de um array não vazio
#    é `$false`, então `if (-not (Conferir-Prerequisitos))` **não entraria no
#    ramo de falha**. O script compilaria por meia hora com um pré-requisito
#    faltando, para morrer no fim.
#
#    `Out-Host` escreve na tela sem passar pelo pipeline, que é exatamente o que
#    se quer: o operador vê o progresso, e o valor de retorno continua sendo o
#    booleano.
function Conferir-Prerequisitos {
    $faltam = @()

    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $faltam += @{
            nome = "Rust"
            porque = "compila o app"
            como = "winget install Rustlang.Rustup"
        }
    } else {
        # O alvo `-gnu` não vem instalado por padrão nem no Windows.
        $alvos = & rustup target list --installed 2>$null
        if ($alvos -notcontains $Alvo) {
            if ($Conferir) {
                $faltam += @{
                    nome = "alvo $Alvo"
                    porque = "e o alvo do Windows aqui - o -msvc nao serve (rsraw-sys)"
                    como = "rustup target add $Alvo"
                }
            } else {
                Diga "instalando o alvo $Alvo"
                & rustup target add $Alvo | Out-Host
            }
        }
    }

    # O g++ do MinGW: o `cc` compila o C++ do LibRaw com ele. O mingw que o
    # rustup traz serve para **ligar** Rust, não para compilar C++.
    #
    # 🔑 O MSYS2 nao poe o `mingw64\bin` no PATH do Windows sozinho. Procurar no
    #    caminho padrao dele antes de desistir evita mandar o operador editar
    #    variavel de ambiente quando o compilador ja esta instalado.
    if (-not (Get-Command g++ -ErrorAction SilentlyContinue)) {
        $msys = "C:\msys64\mingw64\bin"
        if (Test-Path (Join-Path $msys "g++.exe")) {
            Aviso "g++ achado em $msys, mas fora do PATH - acrescentando so para esta execucao"
            $env:PATH = "$msys;$env:PATH"
        } else {
            $faltam += @{
                nome = "g++ do MinGW-w64"
                porque = "compila os ~200 arquivos .cpp do LibRaw (rsraw-sys)"
                como = "winget install MSYS2.MSYS2 ; depois, no terminal do MSYS2: pacman -S --noconfirm mingw-w64-x86_64-gcc"
            }
        }
    }

    # 🔑 O `fxc.exe` e o que prende isto ao Windows. Ele vem no Windows SDK; o
    #    `build.rs` do gpui o procura com `where.exe` e aceita o caminho por
    #    `GPUI_FXC_PATH`.
    #
    # ⚠️ O `where.exe` vai dentro de try/catch: com `$ErrorActionPreference =
    #    "Stop"`, um `where.exe` que nao esta la derruba o script inteiro em vez
    #    de virar "falta o fxc" — que e o que o operador precisa ler.
    $fxc = $env:GPUI_FXC_PATH
    if (-not ($fxc -and (Test-Path $fxc))) {
        $achado = $null
        try { $achado = (& where.exe fxc.exe 2>$null | Select-Object -First 1) } catch { }
        # O SDK instala em Windows Kits\10\bin\<versao>\x64\fxc.exe e nao poe
        # no PATH. Procurar la e a diferenca entre "funciona" e "diz que falta".
        if (-not $achado) {
            $achado = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Filter fxc.exe `
                        -Recurse -ErrorAction SilentlyContinue |
                      Where-Object { $_.FullName -match '\\x64\\' } |
                      Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
            if ($achado) {
                Aviso "fxc.exe achado fora do PATH - usando $achado"
                $env:GPUI_FXC_PATH = $achado
            }
        }
        if (-not $achado) {
            $faltam += @{
                nome = "fxc.exe (Windows SDK)"
                porque = "compila os sete shaders HLSL do gpui - sem ele o app nao compila"
                como = "winget install Microsoft.WindowsSDK, ou aponte GPUI_FXC_PATH para o fxc.exe"
            }
        }
    }

    if (-not (Get-Command cargo-packager -ErrorAction SilentlyContinue)) {
        if ($Conferir) {
            $faltam += @{
                nome = "cargo-packager"
                porque = "monta o .msi e o .exe"
                como = "cargo install cargo-packager --locked"
            }
        } else {
            Diga "instalando o cargo-packager"
            & cargo install cargo-packager --locked | Out-Host
        }
    }

    if ($faltam.Count -gt 0) {
        Erro "faltam $($faltam.Count) pre-requisito(s):"
        foreach ($f in $faltam) {
            Write-Host ""
            Write-Host "   $($f.nome) - $($f.porque)"
            Write-Host "     $($f.como)" -ForegroundColor Gray
        }
        Write-Host ""
        Write-Host "   O WiX e o NSIS nao estao nesta lista de proposito: o proprio" -ForegroundColor Gray
        Write-Host "   cargo-packager os baixa na primeira execucao." -ForegroundColor Gray
        return $false
    }

    Ok "todos os pre-requisitos no lugar"
    return $true
}

# ── A conferência que evita instalador com a versão errada dentro ────────────
function Conferir-Versao {
    $cargo = Get-Content (Join-Path $Raiz "Cargo.toml") -Raw
    $w = [regex]::Match($cargo, '(?ms)\[workspace\.package\].*?^version\s*=\s*"([^"]+)"').Groups[1].Value
    $p = [regex]::Match((Get-Content $Config -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    if ($w -ne $p) {
        Erro "versao divergente: Cargo.toml diz '$w', packager.toml diz '$p'."
        Write-Host "   Acerte os dois - um instalador que mente a versao so aparece na maquina do cliente."
        exit 1
    }
    Write-Host "   versao $w"
}

# ── Execução ─────────────────────────────────────────────────────────────────
Diga "VintageLightbox - instalador do Windows"
Conferir-Versao

if (-not [bool](Conferir-Prerequisitos)) { exit 1 }
if ($Conferir) { exit 0 }

# 🚨 `CARGO_TARGET_DIR` proprio, e nao o `target\` de sempre.
#
# Se este repositorio for uma pasta compartilhada com o Mac - que e o arranjo
# natural numa VM -, os dois enxergam o MESMO `target\`. E mesmo usando
# `--target` em ambos, os artefatos de **host** (build scripts, proc-macros) vao
# para `target\release\` nos dois: um Windows por cima de um macOS faz o cargo
# recompilar tudo a cada troca, quando nao deixa estado quebrado. Foi
# exatamente o que aconteceu com o conteiner do Linux antes de isolar o dele.
$env:CARGO_TARGET_DIR = Join-Path $Raiz "target\windows"

Diga "compilando ui-gpui para $Alvo (release; a primeira vez demora)"
& cargo build --release -p ui-gpui --target $Alvo --manifest-path (Join-Path $Raiz "Cargo.toml")
if ($LASTEXITCODE -ne 0) { Erro "a compilacao falhou"; exit 1 }

# O binário é copiado para o palco que a `packager.toml` aponta — o mesmo
# arranjo do `empacotar.sh`, e pela mesma razao: o `--binaries-dir` do
# cargo-packager 0.11.8 e aceito e ignorado.
New-Item -ItemType Directory -Force -Path $Palco | Out-Null
Copy-Item (Join-Path $env:CARGO_TARGET_DIR "$Alvo\release\$Bin") (Join-Path $Palco $Bin) -Force

$assinatura = @()
if (Test-Path $Chave) {
    # 🚨 `--password` sempre, mesmo vazio. Sem ele o minisign pede a senha no
    #    terminal, e num shell nao interativo a leitura falha - os pacotes saem
    #    e so o .sig nao sai. Instalador pronto, atualizacao morta. Foi o que
    #    aconteceu na primeira geracao do macOS, em 7/set/2026.
    #
    # ⚠️ **Um token so, `--password=`, e nao dois argumentos.** O Windows
    #    PowerShell 5.1 **descarta** argumento de string vazia ao chamar
    #    programa nativo: `--password ""` chegaria ao cargo-packager como
    #    `--password` sozinho, e o defeito acima voltaria - so nesta maquina, e
    #    so em quem ainda usa o PowerShell 5.1. Com `=` o valor vazio sobrevive.
    $assinatura = @("-k", $Chave, "--password=$SenhaDaChave")
} else {
    Aviso "sem chave de atualizacao em $Chave"
    Write-Host "     Os pacotes saem sem .sig, e nenhum app instalado vai aceitar esta versao."
    Write-Host "     Copie a chave da maquina que gera o macOS - tem de ser a MESMA, senao"
    Write-Host "     o app recusa a atualizacao por assinatura invalida."
}

Diga "empacotando -> dist\windows-x86_64"
& cargo packager -c $Config --target $Alvo -o $Dist --formats wix --formats nsis @assinatura
if ($LASTEXITCODE -ne 0) { Erro "o empacotamento falhou"; exit 1 }

Diga "o que saiu"
# `-Include` com `-Recurse` so filtra quando o caminho termina em `\*`; sem
# isso ele devolve tudo. `Where-Object` sobre a extensao nao tem essa pegadinha.
Get-ChildItem -Path $Dist -Recurse -File |
    Where-Object { $_.Extension -in ".msi", ".exe", ".sig" } |
    ForEach-Object { "{0,8:N1} MB  {1}" -f ($_.Length / 1MB), $_.Name }

Write-Host ""
Ok "pronto"
Write-Host "   Leve dist\windows-x86_64\ para o Mac (a pasta compartilhada da VM serve) e rode la:"
Write-Host "     ./scripts/publicar.py"
