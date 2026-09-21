:<<"::FIM-DO-CMD"
@echo off
rem  VintageLightbox (Zed GPUI) - instalar compilando nesta maquina.
rem
rem  Um arquivo so para os tres sistemas:
rem    Windows ........ baixe e de dois cliques neste arquivo
rem    Linux e macOS .. curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox-gpui.cmd | sh
rem
rem  No Windows, esta parte roda o PowerShell que esta mais abaixo e espera uma
rem  tecla no fim, para a janela nao sumir com a mensagem. O PowerShell vem da
rem  versao mais nova do arquivo no GitHub; sem internet, desta copia. Assim a
rem  copia baixada do Release nunca fica velha.
rem
rem  Gerado por scripts/gerar-instaladores.py - edite scripts/instalador-modelo.cmd.in.
title Instalando o VintageLightbox (Zed GPUI)
setlocal
set "VLB_SCRIPT=%~f0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; try { $t=$null; if ($env:VLB_SECO -ne '1') { try { $t=(Invoke-WebRequest -UseBasicParsing -TimeoutSec 30 'https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox-gpui.cmd').Content } catch { } }; if (-not $t) { $t=[IO.File]::ReadAllText($env:VLB_SCRIPT) }; $n=[char]10; $i=$t.IndexOf($n+'#==POWERSHELL=='); if ($i -lt 0) { throw 'instalador invalido: falta o bloco PowerShell' }; $f=$t.IndexOf($n+'#==FIM-POWERSHELL==',$i); if ($f -le $i) { throw 'instalador incompleto' }; Invoke-Expression $t.Substring($i, $f-$i) } catch { Write-Host ''; Write-Host ('X ' + $_) -ForegroundColor Red; exit 1 }"
set "VLB_RESULTADO=%ERRORLEVEL%"
echo.
pause
exit /b %VLB_RESULTADO%
::FIM-DO-CMD
#
# VintageLightbox (Zed GPUI) — instalar compilando nesta máquina. Um arquivo só.
#
# 🔧 **Gerado.** Este arquivo sai de `scripts/instalador-modelo.cmd.in` por
#    `python3 scripts/gerar-instaladores.py`. Edite o modelo e gere de novo: o
#    teste `scripts/testar-instalador.py` recusa um arquivo que divirja dele.
#
# 🔑 **Três sistemas, um arquivo** (dono, 2026-09-16: "um único script que faça
#    tudo, sem o usuário de Windows e Linux ter conhecimento de nada"). Cada
#    interpretador lê só a parte dele:
#
#    - o `cmd` (dois cliques no Windows) lê a primeira linha como rótulo, roda o
#      bloco acima e para no `exit /b`;
#    - o `sh` (Linux e macOS) passa pelo bloco acima e pelo do PowerShell como
#      dois *heredocs* entregues ao `:`, que não faz nada, e roda o resto;
#    - o PowerShell recebe do `cmd` só o trecho entre as marcas `#==`.
#
# 🚨 **Fins de linha LF, e o arquivo não usa saltos para rótulos.** O `sh`
#    quebra com CRLF. O `cmd` aceita LF, desde que não precise procurar rótulo
#    (`exit /b` não procura). O `.gitattributes` fixa o LF.
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
$Destino = if ($env:VLB_DESTINO) { $env:VLB_DESTINO } else { "$env:LOCALAPPDATA\Programs\VintageLightbox-GPUI" }
$Seco    = $env:VLB_SECO -eq "1"
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"   # o Invoke-WebRequest fica 10x mais lento com a barra

$Repo  = "https://github.com/alexkads/VintageLightbox"
$Casa  = "$env:USERPROFILE\.vintagelightbox"
$Fonte = Join-Path $Casa "fonte-gpui"
$Nome  = "VintageLightbox (Zed GPUI)"

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
    if ($Seco) { Write-Host "   [seco] $bloco"; return }
    # ErrorActionPreference nao intercepta codigos de erro de executaveis no PS 5.1.
    $global:LASTEXITCODE = 0
    & $bloco | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "comando falhou (codigo $LASTEXITCODE): $bloco" }
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

# 🚨 O `fxc.exe`, do Windows SDK. O `build.rs` do gpui compila os shaders HLSL
#    com ele, e sem ele a compilacao morre com "Failed to find fxc.exe". O SDK
#    nao poe o fxc no PATH; o gpui aceita o caminho por `GPUI_FXC_PATH`.
function Achar-Fxc {
    if ($env:GPUI_FXC_PATH -and (Test-Path $env:GPUI_FXC_PATH)) { return $env:GPUI_FXC_PATH }
    $achado = $null
    # Dentro de try: com "Stop", o where.exe sem resultado derrubaria o script.
    try { $achado = (& where.exe fxc.exe 2>$null | Select-Object -First 1) } catch { }
    if ($achado) { return $achado }
    Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Filter fxc.exe -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } |
        Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
}
$fxc = Achar-Fxc
if (-not $fxc) {
    Aviso "nao achei o fxc.exe (Windows SDK). Instalando pelo winget."
    Precisa-Winget "o Windows SDK"
    Correr { winget install --silent --accept-package-agreements --accept-source-agreements Microsoft.WindowsSDK.10.0.26100 }
    $fxc = Achar-Fxc
}
if (-not $Seco -and -not $fxc) {
    Erro "o fxc.exe continua faltando. Sem ele os shaders do gpui nao compilam."
    Write-Host "   Instale o Windows SDK (https://developer.microsoft.com/windows/downloads/windows-sdk/)"
    Write-Host "   ou aponte GPUI_FXC_PATH para o fxc.exe, e rode este arquivo de novo."
    throw "falta o fxc.exe"
}
if ($fxc) { $env:GPUI_FXC_PATH = $fxc; Ok "fxc.exe: $fxc" }

# O Rust, pelo rustup. Um `cargo` instalado sem rustup nao serve: a toolchain
# `-gnu` so se escolhe por ele.
$cargoDoUsuario = "$env:USERPROFILE\.cargo\bin"
if (Test-Path (Join-Path $cargoDoUsuario "rustup.exe")) { $env:PATH = "$cargoDoUsuario;$env:PATH" }
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Aviso "nao ha rustup nesta maquina. Instalando o Rust."
    if (-not $Seco) {
        $init = Join-Path $env:TEMP "rustup-init.exe"
        Invoke-WebRequest -UseBasicParsing "https://win.rustup.rs/x86_64" -OutFile $init
        Correr { & $init -y --no-modify-path --profile minimal --default-host $Alvo --default-toolchain $Toolchain }
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
    # MSYS2 so suporta atualizacao completa. A primeira passagem pode atualizar
    # o runtime; a segunda abre outro bash e termina os pacotes restantes.
    Correr { & $msysBash -lc "pacman -Syu --noconfirm" }
    Correr { & $msysBash -lc "pacman -Syu --needed --noconfirm $pacote" }
}

# Use o mesmo MinGW de onde serao copiadas as DLLs, mesmo se houver outro no PATH.
if (-not (Test-Path (Join-Path $mingw "g++.exe"))) {
    Pacote-Msys "mingw-w64-x86_64-gcc mingw-w64-x86_64-clang" "o g++ do MinGW e a libclang"
}
$env:PATH = "$mingw;$env:PATH"
if (-not $Seco -and -not (Get-Command g++ -ErrorAction SilentlyContinue)) {
    Erro "o g++ do MinGW continua faltando. Sem ele o LibRaw nao compila."
    Write-Host "   Instale o MSYS2 (https://www.msys2.org) e, no terminal dele:"
    Write-Host "   pacman -S mingw-w64-x86_64-gcc"
    throw "falta o g++ do MinGW"
}
Ok "g++: $((Get-Command g++ -ErrorAction SilentlyContinue).Source)"

# 🚨 Um g++ presente nao e um g++ que compila. Um MSYS2 atualizado pela metade
#    deixa o g++.exe no lugar e o cc1plus sem as DLLs de que depende, e o erro so
#    aparecia no meio do LibRaw, meia hora depois ("error occurred in cc-rs" no
#    fuji_compressed.cpp, Windows do dono, 2026-09-17). Aqui ele compila um
#    arquivo de teste com as mesmas opcoes do LibRaw antes de tudo.
function Testar-Gpp {
    $ErrorActionPreference = "Continue"
    $pasta = Join-Path ([IO.Path]::GetTempPath()) ("vlb-gpp-" + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $pasta | Out-Null
    try {
        $arquivo = Join-Path $pasta "teste.cpp"
        $codigo = "#include <cstdint>`n#include <thread>`n#include <vector>`nint main() { std::vector<int64_t> v(4); std::thread t([] {}); t.join(); return (int)v.size() - 4; }`n"
        [IO.File]::WriteAllText($arquivo, $codigo)
        $global:LASTEXITCODE = 0
        $saida = & g++ -O3 -m64 -pthread -c $arquivo -o (Join-Path $pasta "teste.o") 2>&1 | Out-String
        return [pscustomobject]@{ Ok = ($LASTEXITCODE -eq 0); Saida = $saida.Trim() }
    } catch {
        return [pscustomobject]@{ Ok = $false; Saida = "$_" }
    } finally {
        Remove-Item -LiteralPath $pasta -Recurse -Force -ErrorAction SilentlyContinue
    }
}
if (-not $Seco) {
    $teste = Testar-Gpp
    if (-not $teste.Ok) {
        Aviso "o g++ do MinGW nao compilou um arquivo de teste:"
        Write-Host $teste.Saida
        Aviso "atualizando o MSYS2 e reinstalando o compilador"
        # Sem --needed: o pacote instalado pode ser justamente o quebrado.
        Correr { & $msysBash -lc "pacman -Syu --noconfirm" }
        Correr { & $msysBash -lc "pacman -Syu --noconfirm" }
        Correr { & $msysBash -lc "pacman -S --noconfirm mingw-w64-x86_64-gcc mingw-w64-x86_64-gcc-libs mingw-w64-x86_64-binutils mingw-w64-x86_64-clang" }
        $teste = Testar-Gpp
        if (-not $teste.Ok) {
            Erro "o g++ do MinGW continua sem compilar:"
            Write-Host $teste.Saida
            Write-Host "   Se a mensagem fala em acesso negado, o antivirus pode estar bloqueando:"
            Write-Host "   em Seguranca do Windows > Protecao contra virus e ameacas > Exclusoes,"
            Write-Host "   adicione $Casa e $msys, e rode este arquivo de novo."
            throw "o g++ do MinGW nao compila"
        }
    }
    Ok "g++ compila"
}

# 🚨 O `windres`. O gpui embute o manifesto do Windows no executavel
#    (`embed-resource`), e com a toolchain `-gnu` quem compila o recurso e o
#    `windres` do MinGW. Ele vem nos binutils, que o gcc do MSYS2 ja traz.
if (-not (Test-Path (Join-Path $mingw "windres.exe"))) {
    Pacote-Msys "mingw-w64-x86_64-binutils" "o windres do MinGW"
}
if (-not $Seco -and -not (Test-Path (Join-Path $mingw "windres.exe"))) {
    Erro "o windres do MinGW continua faltando. Sem ele o manifesto do gpui nao compila."
    Write-Host "   No terminal do MSYS2: pacman -S mingw-w64-x86_64-binutils"
    throw "falta o windres"
}
Ok "windres: $(Join-Path $mingw 'windres.exe')"

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
    $temporario = Join-Path $Casa ("download-" + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $temporario | Out-Null
    try {
        $zip = Join-Path $temporario "fonte.zip"
        $aberto = Join-Path $temporario "aberto"
        Invoke-WebRequest -UseBasicParsing $url -OutFile $zip
        Expand-Archive $zip -DestinationPath $aberto
        $nova = (Get-ChildItem $aberto -Directory | Select-Object -First 1).FullName
        if (-not $nova -or -not (Test-Path (Join-Path $nova "crates\ui-gpui\Cargo.toml"))) {
            throw "a versao $Versao nao tem o crates\ui-gpui"
        }
        # O repositorio ainda nao versiona Cargo.lock. Reutilize a resolucao
        # local; o Cargo a ajusta quando os manifestos mudam.
        if (-not (Test-Path (Join-Path $nova "Cargo.lock")) -and (Test-Path (Join-Path $Fonte "Cargo.lock"))) {
            Copy-Item (Join-Path $Fonte "Cargo.lock") (Join-Path $nova "Cargo.lock")
        }
        # O ZIP recria datas. Preserve a arvore anterior se o conteudo nao mudou,
        # para o Cargo nao refazer a compilacao e o LTO sem necessidade.
        function Assinatura-Fonte($pasta) {
            if (-not (Test-Path $pasta)) { return }
            Get-ChildItem $pasta -Recurse -File -Force | ForEach-Object {
                $relativo = $_.FullName.Substring($pasta.Length).Replace('\', '/')
                $relativo + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
            } | Sort-Object
        }
        $antes = @(Assinatura-Fonte $Fonte)
        $depois = @(Assinatura-Fonte $nova)
        if ($antes.Count -gt 0 -and -not (Compare-Object $antes $depois)) {
            Ok "codigo sem alteracoes; preservando o cache"
        } else {
            if (Test-Path $Fonte) { Remove-Item -Recurse -Force $Fonte }
            Move-Item $nova $Fonte
        }
    } finally {
        Remove-Item -LiteralPath $temporario -Recurse -Force
    }
    Ok "codigo em $Fonte"
}

# ── Compilar ─────────────────────────────────────────────────────────────────
Diga "compilando (15 a 40 minutos na primeira vez)"
$env:CARGO_TARGET_DIR = Join-Path $Casa "target-gpui"

# O rustfmt so embeleza as ligacoes que o bindgen gera; sem ele sai um aviso
# assustador no meio da compilacao. Falhar aqui nao impede nada.
if (-not $Seco) {
    try {
        $ErrorActionPreference = "Continue"
        & rustup component add rustfmt --toolchain $Toolchain 2>&1 | Out-Null
    } catch { } finally { $ErrorActionPreference = "Stop" }
}

# 🚨 Uma compilacao por 4 GiB de memoria, como no Linux e no macOS: com LTO,
#    cada rustc grande passa de 2 GiB, e o cc-rs do LibRaw soma dezenas de g++.
#    Um CARGO_BUILD_JOBS ja definido vale mais do que esta conta.
function Trabalhos-Pela-Memoria {
    try {
        $gib = [math]::Floor((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)
    } catch { return $null }
    $trabalhos = [math]::Max(1, [math]::Floor($gib / 4))
    if ($trabalhos -ge [Environment]::ProcessorCount) { return $null }
    return [pscustomobject]@{ Trabalhos = $trabalhos; Memoria = $gib }
}
if (-not $env:CARGO_BUILD_JOBS) {
    $conta = Trabalhos-Pela-Memoria
    if ($conta) {
        $env:CARGO_BUILD_JOBS = "$($conta.Trabalhos)"
        Ok "compilando $($conta.Trabalhos) de cada vez ($($conta.Memoria) GiB de memoria)"
    }
}

# 🚨 Se falhar, tenta de novo uma vez: sem os restos do LibRaw (um .o pela metade,
#    travado pelo antivirus, derruba a segunda tentativa tambem) e com uma
#    compilacao de cada vez, que e o que sobra quando falta memoria.
$compilar = { cargo "+$Toolchain" build --profile instalador --manifest-path (Join-Path $Fonte "Cargo.toml") -p ui-gpui --bin ui-gpui --target $Alvo }
try {
    Correr $compilar
} catch {
    if ($Seco) { throw }
    Aviso "a compilacao falhou. Tentando de novo, uma compilacao de cada vez."
    Get-ChildItem (Join-Path $env:CARGO_TARGET_DIR "$Alvo\instalador\build") -Directory -Filter "rsraw-sys-*" -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    # 🚨 **E os crates DESTE repositorio saem do cache** (balcao do dono,
    #    18/set/2026): o codigo chegou novo e o cargo compilou a interface contra
    #    um `biblioteca-core` **velho**, guardado no alvo — "no field
    #    `sem_estudio` on type `SessaoFotografica`", com o campo no arquivo ao
    #    lado. O ZIP recria datas, este instalador preserva a arvore quando o
    #    conteudo nao muda, e nesse vaivem o carimbo que o cargo usa para decidir
    #    "isto nao mudou" desencontra do que esta em disco.
    #
    # 🔑 **So os locais**: apagar o carimbo deles recompila os crates da pasta
    #    `crates/` e preserva o caro (wgpu, gpui, LibRaw) — segundos, e nao os
    #    quarenta minutos de uma compilacao do zero.
    $carimbos = Join-Path $env:CARGO_TARGET_DIR "$Alvo\instalador\.fingerprint"
    if (Test-Path $carimbos) {
        foreach ($local in (Get-ChildItem (Join-Path $Fonte "crates") -Directory -ErrorAction SilentlyContinue)) {
            foreach ($padrao in @("$($local.Name)-*", "$($local.Name -replace '-','_')-*")) {
                Get-ChildItem $carimbos -Directory -Filter $padrao -ErrorAction SilentlyContinue |
                    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
        Ok "cache dos crates locais limpo"
    }
    $env:CARGO_BUILD_JOBS = "1"
    try {
        Correr $compilar
    } catch {
        Erro "a compilacao falhou de novo."
        Write-Host "   A mensagem do compilador esta acima, nas linhas 'cargo:warning='."
        Write-Host "   Se ela fala em acesso negado, o antivirus pode estar bloqueando:"
        Write-Host "   em Seguranca do Windows > Protecao contra virus e ameacas > Exclusoes,"
        Write-Host "   adicione $Casa e $msys, e rode este arquivo de novo."
        throw
    }
}
$binario = Join-Path $env:CARGO_TARGET_DIR "$Alvo\instalador\ui-gpui.exe"
if ($Seco) { Write-Host "`n   [seco] nada foi feito."; return }
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $binario)) {
    throw "a compilacao falhou, ou nao deixou $binario"
}

# ── Instalar ─────────────────────────────────────────────────────────────────
Diga "instalando em $Destino"
New-Item -ItemType Directory -Force -Path $Destino | Out-Null
$exe = Join-Path $Destino "VintageLightbox-GPUI.exe"
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
# ⚠️ **É o app GPUI** (`crates/ui-gpui`), o editor nativo do balcão. No macOS
#    ele se chama `VintageLightbox (Zed GPUI).app` (dono, 2026-09-17: o nome
#    diz qual build está aberta), com o identificador do `.dmg`. Um
#    `VintageLightbox.app` antigo, do `.dmg`, não é apagado: some quando o dono
#    o tirar à mão.
#
# 🔑 **No macOS bastam as Command Line Tools**: os shaders Metal são compilados
#    pelo próprio macOS quando o app abre (feature
#    `shaders-em-tempo-de-execucao`), e o Xcode não é preciso.
#
# ⚠️ **Custa tempo e disco.** A primeira compilação leva de 15 a 40 minutos e usa
#    alguns GiB em `~/.vintagelightbox/target-gpui`. As seguintes reaproveitam.
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

# 🚨 **O script inteiro é um bloco `{ … }`**, e o `}` está na última linha. O
#    `sh` lê um bloco inteiro antes de executá-lo; sem isso, com `curl | sh`, um
#    erro no meio fazia o `sh` sair enquanto o `curl` ainda escrevia, e a
#    última linha na tela era "curl: Failed writing body".
{
REPO="https://github.com/alexkads/VintageLightbox"
CASA="${VLB_CASA:-$HOME/.vintagelightbox}"
FONTE="$CASA/fonte-gpui"
NOME="VintageLightbox (Zed GPUI)"
# O que a barra de menus e o Dock mostram.
NOME_EXIBIDO="VintageLightbox (Zed GPUI)"
IDENTIFICADOR="br.com.recordarfotos.vintagelightbox"
# No Linux: o nome do binário, do atalho e do ícone.
NOME_LINUX="vintagelightbox-gpui"

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

# Acrescenta uma extensão à lista das ligadas do GNOME, sem o Shell saber —
# vale no próximo login. Não duplica, e não mexe se o `gsettings` não existir.
ligar_pelo_gsettings() {
  command -v gsettings >/dev/null 2>&1 || return 1
  atual=$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null) || return 1
  case "$atual" in
    *"'$1'"*) return 0 ;;
    "@as []"|"[]"|"") novo="['$1']" ;;
    *) novo="${atual%]}, '$1']" ;;
  esac
  correr gsettings set org.gnome.shell enabled-extensions "$novo" 2>/dev/null
}

# A ajuda mora aqui, e não é lida do arquivo: com `curl | sh` o script não
# existe em disco.
ajuda() {
  cat <<AJUDA
VintageLightbox (Zed GPUI) — compila e instala nesta máquina (macOS ou Linux).

  curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox-gpui.cmd | sh

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

[ -n "$REF" ] || { erro "a versão não pode ser vazia."; exit 1; }

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

# O `diff` não entra aqui: ele só decide se o código baixado é igual ao da vez
# anterior, para não recompilar à toa, e o Fedora mínimo não o traz. Sem ele o
# código é trocado sempre, e a instalação segue.
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

# `--features` a mais no build; só o GPUI no macOS usa.
RECURSOS=""
if [ "$SISTEMA" = "Darwin" ]; then
  # 🔑 **Sem Xcode** (dono, 2026-09-17: *"não posso depender do xcode"*). O
  #    GPUI compilava os shaders Metal no build, com o `metal` do Xcode, que não
  #    vem nas Command Line Tools. Com a feature `shaders-em-tempo-de-execucao`
  #    quem os compila é o Metal do próprio macOS, quando o app abre. O build
  #    precisa só do compilador de C++ e da libclang.
  if ! xcrun -f clang++ >/dev/null 2>&1; then
    erro "faltam as Command Line Tools do Xcode (o compilador de C++)."
    echo "   Instale com:  xcode-select --install"
    echo "   Depois rode este script de novo. O Xcode inteiro não é preciso."
    exit 1
  fi
  ok "Command Line Tools: $(xcode-select -p)"
  RECURSOS="--features shaders-em-tempo-de-execucao"
else
  # 🔑 O que o GPUI abre no Linux (X11, Wayland, teclado, fontes, Vulkan), o
  #    chaveiro do `keyring` (D-Bus e Secret Service), o TLS do atualizador e o
  #    clang do LibRaw. A lista é a do CI (`.github/workflows/instaladores.yml`),
  #    sem as ferramentas do AppImage, que aqui não se monta. Cada peça é
  #    conferida por si; faltando qualquer uma, a lista inteira é pedida ao
  #    gerenciador, que pula o que já existe.
  FALTA=""
  command -v pkg-config >/dev/null 2>&1 || FALTA="$FALTA pkg-config"
  command -v c++ >/dev/null 2>&1 || FALTA="$FALTA compilador-de-C++"
  # PTP não é um disco montado: o app usa o gphoto2 para detectar a câmera e
  # o gio/GVfs para as URIs camera:/ e gphoto2:// escolhidas pelo usuário.
  command -v gphoto2 >/dev/null 2>&1 || FALTA="$FALTA gphoto2"
  command -v gio >/dev/null 2>&1 || FALTA="$FALTA gio"
  # `gio` sozinho não basta: é o backend `gvfsd-gphoto2` que transforma a
  # câmera PTP em uma pasta navegável. O nome do pacote muda entre as famílias
  # de distribuição, por isso conferimos tanto o binário quanto o gerenciador
  # de pacotes. VLB_PTP_BACKEND_OK só existe nos testes automatizados.
  tem_backend_ptp() {
    [ "${VLB_PTP_BACKEND_OK:-}" = 1 ] && return 0
    for BIN in /usr/lib/gvfs/gvfsd-gphoto2 /usr/libexec/gvfsd-gphoto2 \
               /lib/gvfs/gvfsd-gphoto2 /libexec/gvfsd-gphoto2; do
      [ -x "$BIN" ] && return 0
    done
    if command -v dpkg-query >/dev/null 2>&1; then
      dpkg-query -W -f='${db:Status-Status}' gvfs-backends 2>/dev/null | grep -qx installed && return 0
    fi
    if command -v rpm >/dev/null 2>&1; then
      rpm -q gvfs-gphoto2 2>/dev/null >/dev/null && return 0
    fi
    if command -v pacman >/dev/null 2>&1; then
      pacman -Q gvfs 2>/dev/null >/dev/null && return 0
    fi
    return 1
  }
  tem_backend_ptp || FALTA="$FALTA backend-ptp-gvfs"
  for LIB in x11 xcb xkbcommon xkbcommon-x11 wayland-client xcursor xrandr xi \
             fontconfig freetype2 alsa openssl vulkan dbus-1 libsecret-1; do
    pkg-config --exists "$LIB" 2>/dev/null || FALTA="$FALTA $LIB"
  done
  tem_libclang || FALTA="$FALTA libclang"

  # O `mesa-vulkan-drivers` é o que faz o Vulkan achar a placa de vídeo; sem um
  # driver Vulkan a janela do GPUI não abre, mesmo com tudo compilado.
  PACOTES_APT="build-essential pkg-config curl clang libclang-dev libx11-dev libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev wayland-protocols libxcursor-dev libxrandr-dev libxi-dev libfontconfig1-dev libfreetype6-dev libasound2-dev libssl-dev libvulkan-dev mesa-vulkan-drivers libdbus-1-dev libsecret-1-dev gphoto2 gvfs-backends libgphoto2-6"
  PACOTES_DNF="gcc-c++ pkgconf-pkg-config curl diffutils clang clang-devel libX11-devel libxcb-devel libxkbcommon-devel libxkbcommon-x11-devel wayland-devel wayland-protocols-devel libXcursor-devel libXrandr-devel libXi-devel fontconfig-devel freetype-devel alsa-lib-devel openssl-devel vulkan-loader-devel mesa-vulkan-drivers dbus-devel libsecret-devel gphoto2 gvfs-gphoto2 libgphoto2"
  PACOTES_PACMAN="base-devel curl clang libx11 libxcb libxkbcommon libxkbcommon-x11 wayland wayland-protocols libxcursor libxrandr libxi fontconfig freetype2 alsa-lib openssl vulkan-icd-loader dbus libsecret gphoto2 gvfs libgphoto2"
  PACOTES_A_MAO="libx11, libxcb, libxkbcommon e libxkbcommon-x11, wayland, libxcursor, libxrandr, libxi, fontconfig, freetype, alsa, openssl, vulkan (loader e driver), dbus e libsecret, gphoto2, gio/GVfs e libgphoto2, todos na versão dev, mais clang e libclang (dev)"
  conferir_depois() {
    for LIB in xkbcommon vulkan dbus-1; do
      pkg-config --exists "$LIB" 2>/dev/null || { erro "a biblioteca $LIB continua faltando depois da instalação."; exit 1; }
    done
    for BIN in gphoto2 gio; do
      command -v "$BIN" >/dev/null 2>&1 || { erro "o comando $BIN continua faltando depois da instalação."; exit 1; }
    done
    tem_backend_ptp || { erro "o backend PTP do GVfs continua faltando depois da instalação."; exit 1; }
  }
  resumo_linux() {
    ok "xkbcommon $(pkg-config --modversion xkbcommon 2>/dev/null || echo 'a instalar'), Vulkan $(pkg-config --modversion vulkan 2>/dev/null || echo 'a instalar')"
  }

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
    # 🚨 **Fedora imutável** (Silverblue, Kinoite, Bazzite): o sistema é uma
    #    imagem, e o `dnf install` recusa. Os pacotes entram pelo `rpm-ostree`
    #    e só valem depois de reiniciar, então o script para e diz o comando.
    # VLB_OSTREE existe para o teste.
    if [ -e "${VLB_OSTREE:-/run/ostree-booted}" ]; then
      erro "este Linux é imutável (Fedora Silverblue, Kinoite, Bazzite…): os pacotes não entram pelo dnf."
      echo "   Rode este comando, reinicie o computador e rode este script de novo:"
      echo "   sudo rpm-ostree install --idempotent $PACOTES_DNF"
      exit 1
    fi
    # 🚨 **A distribuição decide, e não o comando que existe.** O Fedora tem um
    #    pacote `apt` que instala o `apt-get` sem repositório nenhum: procurado
    #    primeiro, ele respondia "Unable to locate package" a toda a lista
    #    (máquina do Igor, Fedora, 2026-09-17). VLB_OS_RELEASE existe para o teste.
    GERENCIADOR=""
    # shellcheck disable=SC1090
    DISTRO="$(. "${VLB_OS_RELEASE:-/etc/os-release}" 2>/dev/null && echo " ${ID:-} ${ID_LIKE:-} ")" || DISTRO=""
    case "$DISTRO" in
      *" fedora "*|*" rhel "*|*" centos "*|*" rocky "*|*" almalinux "*|*" nobara "*|*" ultramarine "*)
        command -v dnf >/dev/null 2>&1 && GERENCIADOR=dnf ;;
      *" debian "*|*" ubuntu "*)
        command -v apt-get >/dev/null 2>&1 && GERENCIADOR=apt ;;
      *" arch "*)
        command -v pacman >/dev/null 2>&1 && GERENCIADOR=pacman ;;
    esac
    if [ -z "$GERENCIADOR" ]; then
      if command -v dnf >/dev/null 2>&1; then GERENCIADOR=dnf
      elif command -v pacman >/dev/null 2>&1; then GERENCIADOR=pacman
      elif command -v apt-get >/dev/null 2>&1; then GERENCIADOR=apt
      fi
    fi
    if [ "$GERENCIADOR" = apt ]; then
      # 🚨 `update` antes: numa máquina recém-instalada a lista de pacotes é a
      #    do dia da imagem, e o `install` responde "Unable to locate package".
      INSTALAR="${SUDO:+$SUDO }apt-get update && ${SUDO:+$SUDO }env DEBIAN_FRONTEND=noninteractive apt-get install -y $PACOTES_APT"
    elif [ "$GERENCIADOR" = dnf ]; then
      INSTALAR="${SUDO:+$SUDO }dnf install -y $PACOTES_DNF"
    elif [ "$GERENCIADOR" = pacman ]; then
      INSTALAR="${SUDO:+$SUDO }pacman -Syu --needed --noconfirm $PACOTES_PACMAN"
    else
      erro "não reconheci o gerenciador de pacotes desta distribuição."
      echo "   Instale o equivalente a: $PACOTES_A_MAO,"
      echo "   e um compilador de C++. Depois rode este script de novo."
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
      conferir_depois
      tem_libclang || { erro "a libclang continua faltando depois da instalação."; exit 1; }
    fi
  fi
  resumo_linux
  ok "libclang: $(tem_libclang && echo presente || echo 'a instalar')"

  # 🔑 **O GNOME não mostra ícone de bandeja sem uma extensão**, e o app vai
  #    para a bandeja ao minimizar e ao fechar com envio pendente. O Ubuntu traz
  #    a extensão ligada (`ubuntu-appindicators`); o Fedora, não. Sem ela o app
  #    continua rodando, mas o ícone não aparece. Só se instala pelo `dnf`: nas
  #    outras distribuições o script avisa e segue.
  EXTENSAO=appindicatorsupport@rgcjonas.gmail.com
  # VLB_EXTENSOES_GNOME existe para o teste.
  EXTENSOES="${VLB_EXTENSOES_GNOME:-/usr/share/gnome-shell/extensions}"
  case "${XDG_CURRENT_DESKTOP:-}" in
    *GNOME*)
      if [ -d "$EXTENSOES/$EXTENSAO" ] \
         || [ -d "$HOME/.local/share/gnome-shell/extensions/$EXTENSAO" ] \
         || [ -d "$EXTENSOES/ubuntu-appindicators@ubuntu.com" ]; then
        :
      elif command -v dnf >/dev/null 2>&1 && [ ! -e "${VLB_OSTREE:-/run/ostree-booted}" ]; then
        aviso "o GNOME não mostra o ícone da bandeja sem a extensão AppIndicator — instalando"
        if [ "$(id -u)" -eq 0 ]; then SUDO=""; else SUDO="sudo"; fi
        correr ${SUDO:+$SUDO} dnf install -y gnome-shell-extension-appindicator </dev/null \
          || aviso "não consegui instalar a extensão; o app funciona, mas sem o ícone da bandeja."
      else
        aviso "o GNOME não mostra o ícone da bandeja sem a extensão AppIndicator."
        echo "   Instale a 'AppIndicator and KStatusNotifierItem Support' pelo gerenciador de"
        echo "   extensões. Sem ela o app funciona, mas o ícone da bandeja não aparece."
      fi
      if [ -d "$EXTENSOES/$EXTENSAO" ] && command -v gnome-extensions >/dev/null 2>&1; then
        if gnome-extensions info "$EXTENSAO" 2>/dev/null | grep -q -i -E "(enabled|state).*(yes|active|enabled)"; then
          ok "extensão da bandeja do GNOME: ligada"
        else
          # Ligada agora, ela só aparece depois de sair e entrar de novo na
          # sessão: o GNOME no Wayland não recarrega extensões com a sessão aberta.
          #
          # 🚨 **Recém-instalada pelo `dnf`, o `enable` falha** (Fedora 44,
          # 2026-09-21): o GNOME Shell no Wayland só descobre extensões novas
          # ao entrar na sessão, e responde que ela não existe. A lista das
          # ligadas mora no dconf (`org.gnome.shell enabled-extensions`), e
          # gravá-la ali não depende do Shell: ela liga no próximo login.
          if correr gnome-extensions enable "$EXTENSAO" 2>/dev/null \
             || ligar_pelo_gsettings "$EXTENSAO"; then
            aviso "extensão da bandeja ligada: saia e entre de novo na sessão para o ícone aparecer."
          else
            aviso "não consegui ligar a extensão da bandeja por aqui."
            echo "   Abra o app Extensões e ligue 'AppIndicator and KStatusNotifierItem Support'."
          fi
        fi
      fi
      ;;
  esac
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
# Valide o download numa pasta temporária antes de substituir o código anterior.
diga "baixando o código de $REF"
case "$REF" in
  v[0-9]*) URL="$REPO/archive/refs/tags/$REF.tar.gz" ;;
  *)       URL="$REPO/archive/refs/heads/$REF.tar.gz" ;;
esac
correr mkdir -p "$CASA"
if [ "$SECO" -eq 1 ]; then
  echo "   [seco] baixaria e validaria $URL antes de atualizar $FONTE"
else
  TEMPORARIO="$(mktemp -d "$CASA/download.XXXXXX")"
  trap 'rm -rf "$TEMPORARIO"' 0
  trap 'exit 130' INT
  trap 'exit 143' TERM
  mkdir "$TEMPORARIO/fonte"
  # Separe curl e tar: o sh nao oferece pipefail, e tar pode aceitar um arquivo
  # completo mesmo quando o transporte termina com erro.
  if ! curl --connect-timeout 30 --retry 3 -fsSL "$URL" -o "$TEMPORARIO/fonte.tar.gz"; then
    erro "não consegui baixar $URL"
    exit 1
  fi
  tar -xz -f "$TEMPORARIO/fonte.tar.gz" -C "$TEMPORARIO/fonte" --strip-components=1
  [ -f "$TEMPORARIO/fonte/crates/ui-gpui/Cargo.toml" ] || { erro "a versão $REF não tem o crates/ui-gpui."; exit 1; }
  # Cargo.lock nao e versionado neste repositorio; preserve a resolucao local.
  if [ ! -f "$TEMPORARIO/fonte/Cargo.lock" ] && [ -f "$FONTE/Cargo.lock" ]; then
    cp "$FONTE/Cargo.lock" "$TEMPORARIO/fonte/Cargo.lock"
  fi
  if [ -d "$FONTE" ] && command -v diff >/dev/null 2>&1 && diff -qr "$FONTE" "$TEMPORARIO/fonte" >/dev/null 2>&1; then
    ok "código sem alterações; preservando o cache"
  else
    rm -rf "$FONTE"
    mv "$TEMPORARIO/fonte" "$FONTE"
  fi
  rm -rf "$TEMPORARIO"
  trap - 0 INT TERM
  ok "código em $FONTE"
fi

# ── Compilar ──────────────────────────────────────────────────────────────────
#
# `--bin ui-gpui`, e só ele: com `lto = true`, cada binário linka o programa
# inteiro, e a máquina do balcão não tem RAM sobrando.
diga "compilando (15 a 40 minutos na primeira vez)"
CARGO_TARGET_DIR="$CASA/target-gpui"
export CARGO_TARGET_DIR

# 🚨 **Uma compilação por 4 GiB de memória.** O cargo abre uma por núcleo, e
#    com `lto = true` e `codegen-units = 1` cada `rustc` grande passa de 2 GiB.
#    Num Fedora com 8 GiB e 4 núcleos o `rustc` do gpui morreu por falta de
#    memória (`signal: 9, SIGKILL`); com 2 de cada vez, morreu o do ui-gpui. Um CARGO_BUILD_JOBS
#    já definido vale mais do que esta conta.
if [ -z "${CARGO_BUILD_JOBS:-}" ]; then
  MEMORIA_GIB=""
  if [ -r /proc/meminfo ]; then
    MEMORIA_GIB="$(awk '/^MemTotal:/ {print int($2 / 1048576)}' /proc/meminfo)"
  elif command -v sysctl >/dev/null 2>&1; then
    MEMORIA_GIB="$(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1073741824 ))"
  fi
  NUCLEOS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)"
  case "$MEMORIA_GIB:$NUCLEOS" in
    *[!0-9:]*|:*|*:) : ;;
    *)
      TRABALHOS=$(( MEMORIA_GIB / 4 ))
      [ "$TRABALHOS" -ge 1 ] || TRABALHOS=1
      if [ "$TRABALHOS" -lt "$NUCLEOS" ]; then
        CARGO_BUILD_JOBS="$TRABALHOS"
        export CARGO_BUILD_JOBS
        ok "compilando $TRABALHOS de cada vez (${MEMORIA_GIB} GiB de memória, $NUCLEOS núcleos)"
      fi ;;
  esac
fi

# O código de saída do cargo chega a quem chamou (o despachante e os testes o
# conferem).
# shellcheck disable=SC2086
# `--profile instalador`: thin LTO e 16 unidades (ver o `Cargo.toml`) — a
# compilação na máquina do balcão, e não o pacote distribuído.
if correr cargo build --profile instalador --manifest-path "$FONTE/Cargo.toml" -p ui-gpui --bin ui-gpui $RECURSOS; then
  :
else
  CODIGO=$?
  erro "a compilação falhou."
  echo "   Se a mensagem acima fala em 'signal: 9' ou 'SIGKILL', faltou memória. Feche"
  echo "   outros programas e rode de novo com uma compilação de cada vez:"
  echo "   curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox-gpui.cmd | CARGO_BUILD_JOBS=1 sh"
  exit "$CODIGO"
fi

BINARIO="$CARGO_TARGET_DIR/instalador/ui-gpui"
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
  cp "$BINARIO" "$APP/Contents/MacOS/ui-gpui"
  cp "$FONTE/empacotamento/icones/icone.icns" "$APP/Contents/Resources/icone.icns"
  # 🔑 O identificador é o mesmo do `.dmg` (`empacotamento/packager.toml`): é
  #    por ele que o macOS lembra as permissões já concedidas ao app GPUI.
  cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>English</string>
	<key>CFBundleDisplayName</key>
	<string>$NOME_EXIBIDO</string>
	<key>CFBundleExecutable</key>
	<string>ui-gpui</string>
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
	<key>CSResourcesFileMapped</key>
	<true/>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.photography</string>
	<key>LSMinimumSystemVersion</key>
	<string>10.15</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSHumanReadableCopyright</key>
	<string>© 2026 RecordarFotos</string>
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
  install -m 755 "$BINARIO" "$DESTINO/bin/$NOME_LINUX"
  cp "$FONTE/empacotamento/icones/256x256.png" "$DESTINO/share/icons/hicolor/256x256/apps/$NOME_LINUX.png"
  cat > "$DESTINO/share/applications/$NOME_LINUX.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=VintageLightbox (Zed GPUI)
Comment=Editor de fotos da RecordarFotos
Exec=$DESTINO/bin/$NOME_LINUX
Icon=$NOME_LINUX
StartupWMClass=$NOME_LINUX
Categories=Graphics;Photography;
Terminal=false
DESKTOP
  command -v update-desktop-database >/dev/null 2>&1 \
    && update-desktop-database "$DESTINO/share/applications" >/dev/null 2>&1 || true
  printf "\n${V}✅ %s %s instalado${Z}\n\n" "VintageLightbox (Zed GPUI)" "$VERSAO"
  printf "   ${N}Abra pelo menu de aplicativos, ou:${Z}  %s\n" "$DESTINO/bin/$NOME_LINUX"
fi

echo
echo "   Para atualizar, rode este mesmo comando de novo: a segunda compilação"
echo "   reaproveita o cache em $CARGO_TARGET_DIR."
echo "   Apagar o cache é seguro:  rm -rf \"$CARGO_TARGET_DIR\""
}
