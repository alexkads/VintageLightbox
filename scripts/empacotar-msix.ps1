#Requires -Version 5.1
<#
Gera um MSIX x64 na propria maquina Windows, com Windows SDK e Rust GNU.
  .\scripts\empacotar-msix.ps1 -Certificado <thumbprint>
  .\scripts\empacotar-msix.ps1 -CertificadoDeTeste
  .\scripts\empacotar-msix.ps1 -Binario C:\build\ui-gpui.exe -Cli C:\build\vlb.exe -CertificadoDeTeste
O certificado de teste usa identidade separada e NAO e confiado automaticamente.
#>
[CmdletBinding()]
param(
    [string]$Binario,
    [string]$Cli,
    [string]$PastaDll = 'C:\msys64\mingw64\bin',
    [string]$Certificado,
    [switch]$CertificadoDeTeste,
    [string]$VersaoPacote,
    [string]$Saida,
    [ValidateSet('release', 'instalador')][string]$Perfil = 'release'
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Raiz = Split-Path $PSScriptRoot -Parent
if ($env:OS -ne 'Windows_NT') { throw 'Execute em Windows x64 com o Windows SDK instalado.' }
if ($Certificado -and $CertificadoDeTeste) { throw 'Escolha Certificado ou CertificadoDeTeste.' }
if (-not $Certificado -and -not $CertificadoDeTeste) { throw 'Informe -Certificado <thumbprint> ou -CertificadoDeTeste.' }
if ([bool]$Binario -ne [bool]$Cli) { throw 'Informe -Binario e -Cli juntos, ou deixe ambos para compilar.' }

function Executar([string]$Programa, [string[]]$Argumentos) {
    & $Programa @Argumentos
    if ($LASTEXITCODE -ne 0) { throw "$Programa terminou com codigo $LASTEXITCODE" }
}
function FerramentaSdk([string]$Nome) {
    $comando = Get-Command $Nome -ErrorAction SilentlyContinue
    if ($comando) { return $comando.Source }
    $arquivo = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\$Nome" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $arquivo) { throw "Falta $Nome. Instale o Windows SDK." }
    return $arquivo.FullName
}
function Xml([string]$Texto) { return [Security.SecurityElement]::Escape($Texto) }
$MakeAppx = FerramentaSdk 'makeappx.exe'
$SignTool = FerramentaSdk 'signtool.exe'
$Toml = Get-Content (Join-Path $Raiz 'Cargo.toml') -Raw
if ($Toml -notmatch '(?ms)^\[workspace.package\]\s*.*?^version\s*=\s*"(\d+\.\d+\.\d+)"') { throw 'Versao do workspace nao encontrada.' }
$Versao = $Matches[1]
if (-not $VersaoPacote) { $VersaoPacote = "$Versao.0" }
if ($VersaoPacote -notmatch '^\d+\.\d+\.\d+\.\d+$' -or
    @($VersaoPacote.Split('.') | Where-Object { [long]$_ -gt 65535 }).Count -gt 0 -or
    [long]$VersaoPacote.Split('.')[0] -eq 0 -and [version]$VersaoPacote -eq [version]'0.0.0.0') {
    throw 'VersaoPacote deve ter quatro numeros de 0 a 65535 e ser maior que zero.'
}
$NomePacote = 'RecordarFotos.VintageLightbox'
$AliasCli = 'vintagelightbox-vlb.exe'
if ($CertificadoDeTeste) {
    $NomePacote += '.Testes'
    $AliasCli = 'vintagelightbox-vlb-testes.exe'
    $Assunto = 'CN=RecordarFotos Testes'
    $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object {
        $_.Subject -eq $Assunto -and $_.HasPrivateKey -and $_.NotAfter -gt (Get-Date).AddDays(1)
    } | Sort-Object NotAfter -Descending | Select-Object -First 1
    if (-not $cert) {
        $cert = New-SelfSignedCertificate -Type Custom -Subject $Assunto -FriendlyName 'VintageLightbox MSIX - somente testes' `
            -CertStoreLocation Cert:\CurrentUser\My -KeyUsage DigitalSignature -KeyAlgorithm RSA -KeyLength 2048 `
            -HashAlgorithm SHA256 -NotAfter (Get-Date).AddYears(1) `
            -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
    }
} else {
    $Certificado = $Certificado.Replace(' ', '')
    $cert = Get-Item "Cert:\CurrentUser\My\$Certificado"
}
if (-not $cert.HasPrivateKey -or $cert.NotAfter -le (Get-Date) -or $cert.NotBefore -gt (Get-Date)) {
    throw 'O certificado precisa estar valido e ter chave privada no CurrentUser\My.'
}
if (-not $Binario) {
    $Alvo = 'x86_64-pc-windows-gnu'
    $env:PATH = "$PastaDll;$env:USERPROFILE\.cargo\bin;$env:PATH"
    $env:LIBCLANG_PATH = $PastaDll
    $env:GPUI_FXC_PATH = FerramentaSdk 'fxc.exe'
    $Target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Raiz 'target' }
    Executar 'cargo' @("+stable-$Alvo", 'build', '--profile', $Perfil, '--target', $Alvo, '--target-dir', $Target,
        '--manifest-path', (Join-Path $Raiz 'Cargo.toml'), '-p', 'ui-gpui', '--bin', 'ui-gpui', '-p', 'vlb-controle', '--bin', 'vlb')
    $Binario = Join-Path $Target "$Alvo\$Perfil\ui-gpui.exe"
    $Cli = Join-Path $Target "$Alvo\$Perfil\vlb.exe"
}
$Binario = (Resolve-Path $Binario).Path
$Cli = (Resolve-Path $Cli).Path
if (-not $Saida) { $Saida = Join-Path $Raiz 'dist\windows' }
$Saida = [IO.Path]::GetFullPath($Saida)
New-Item -ItemType Directory -Force $Saida | Out-Null
$Trabalho = Join-Path ([IO.Path]::GetTempPath()) ("vlb-msix-" + [guid]::NewGuid())
$Pacote = Join-Path $Saida "$NomePacote-$VersaoPacote-x64.msix"
try {
    New-Item -ItemType Directory -Path "$Trabalho\Assets" -Force | Out-Null
    Copy-Item $Binario "$Trabalho\VintageLightbox-GPUI.exe"
    Copy-Item $Cli "$Trabalho\vlb.exe"
    Copy-Item (Join-Path $Raiz 'LICENSE') "$Trabalho\LICENSE.txt"
    foreach ($dll in 'libstdc++-6.dll', 'libgcc_s_seh-1.dll', 'libwinpthread-1.dll') {
        Copy-Item (Join-Path $PastaDll $dll) $Trabalho
    }
    # PNGs nas dimensoes exigidas pelo manifesto, a partir do icone existente.
    Add-Type -AssemblyName System.Drawing
    $icone = [Drawing.Image]::FromFile((Join-Path $Raiz 'empacotamento\icones\256x256.png'))
    try {
        foreach ($t in 44, 150, 50) {
            $bitmap = New-Object Drawing.Bitmap($t, $t)
            $g = [Drawing.Graphics]::FromImage($bitmap)
            try {
                $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $g.DrawImage($icone, 0, 0, $t, $t)
                $bitmap.Save("$Trabalho\Assets\Logo$t.png", [Drawing.Imaging.ImageFormat]::Png)
            } finally { $g.Dispose(); $bitmap.Dispose() }
        }
    } finally { $icone.Dispose() }
    $Publisher = Xml $cert.Subject
    $Nome = if ($CertificadoDeTeste) { 'VintageLightbox (Teste MSIX)' } else { 'VintageLightbox' }
    @"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
 xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
 xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
 xmlns:uap5="http://schemas.microsoft.com/appx/manifest/uap/windows10/5"
 xmlns:desktop4="http://schemas.microsoft.com/appx/manifest/desktop/windows10/4"
 xmlns:desktop6="http://schemas.microsoft.com/appx/manifest/desktop/windows10/6"
 IgnorableNamespaces="uap rescap uap5 desktop4 desktop6">
 <Identity Name="$NomePacote" Publisher="$Publisher" Version="$VersaoPacote" ProcessorArchitecture="x64" />
 <Properties><DisplayName>$Nome</DisplayName><PublisherDisplayName>RecordarFotos</PublisherDisplayName>
 <Logo>Assets\Logo50.png</Logo><desktop6:FileSystemWriteVirtualization>disabled</desktop6:FileSystemWriteVirtualization></Properties>
 <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" /></Dependencies>
 <Resources><Resource Language="pt-BR" /></Resources>
 <Applications><Application Id="App" Executable="VintageLightbox-GPUI.exe" EntryPoint="Windows.FullTrustApplication" desktop4:SupportsMultipleInstances="true">
 <uap:VisualElements DisplayName="$Nome" Description="Editor de fotos da RecordarFotos"
  BackgroundColor="transparent" Square150x150Logo="Assets\Logo150.png" Square44x44Logo="Assets\Logo44.png" />
 <Extensions>
  <uap5:Extension Category="windows.appExecutionAlias" Executable="vlb.exe" EntryPoint="Windows.FullTrustApplication">
   <uap5:AppExecutionAlias desktop4:Subsystem="console">
    <uap5:ExecutionAlias Alias="$AliasCli" />
   </uap5:AppExecutionAlias>
  </uap5:Extension>
 </Extensions>
 </Application></Applications>
 <Capabilities><rescap:Capability Name="runFullTrust" /><rescap:Capability Name="unvirtualizedResources" /></Capabilities>
</Package>
"@ | Set-Content "$Trabalho\AppxManifest.xml" -Encoding UTF8
    # Recusa empacotar um executavel que nao consegue carregar as DLLs ou esta desatualizado.
    $processo = Start-Process -FilePath "$Trabalho\VintageLightbox-GPUI.exe" -ArgumentList '--versao' `
        -NoNewWindow -Wait -PassThru -RedirectStandardOutput "$Trabalho\versao.txt" -RedirectStandardError "$Trabalho\erros.txt"
    $VersaoExe = Get-Content "$Trabalho\versao.txt" -Raw
    if ($processo.ExitCode -ne 0 -or "$VersaoExe".Trim() -ne $Versao) {
        throw "Binario nao responde a versao $Versao (codigo $($processo.ExitCode), resposta: $VersaoExe)."
    }
    Remove-Item "$Trabalho\versao.txt", "$Trabalho\erros.txt"
    Executar $MakeAppx @('pack', '/o', '/d', $Trabalho, '/p', $Pacote)
    Executar $SignTool @('sign', '/fd', 'SHA256', '/sha1', $cert.Thumbprint, $Pacote)
    if ($CertificadoDeTeste) {
        Export-Certificate -Cert $cert -FilePath (Join-Path $Saida "$NomePacote.cer") -Force | Out-Null
        Write-Warning 'Assinatura de teste. Confie no .cer somente na VM de testes (LocalMachine\TrustedPeople).'
    } else {
        Executar $SignTool @('verify', '/pa', '/v', $Pacote)
    }
    [ordered]@{ pacote = (Split-Path $Pacote -Leaf); versaoApp = $Versao; versaoPacote = $VersaoPacote;
        identidade = $NomePacote; publisher = $cert.Subject; certificado = $cert.Thumbprint;
        teste = [bool]$CertificadoDeTeste; sha256 = (Get-FileHash $Pacote -Algorithm SHA256).Hash;
        sha256Binario = (Get-FileHash $Binario -Algorithm SHA256).Hash } |
        ConvertTo-Json | Set-Content "$Pacote.json" -Encoding UTF8
    Write-Host "Pronto: $Pacote"
} finally {
    Remove-Item $Trabalho -Recurse -Force
}
