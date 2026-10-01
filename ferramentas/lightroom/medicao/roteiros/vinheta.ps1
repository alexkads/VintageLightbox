# Para cada caso da régua da vinheta: o perfil do Lightroom e o nosso.
# Pasta de trabalho (CSVs e imagens intermediárias): VLB_RASCUNHO, ou uma no TEMP.
$Rascunho = if ($env:VLB_RASCUNHO) { $env:VLB_RASCUNHO } else { Join-Path $env:TEMP 'regua-vintagelightbox' }
New-Item -ItemType Directory -Force $Rascunho | Out-Null
$env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
$rasc = "$Rascunho"
$exe = "C:\Projects\VintageLightbox\target\release\examples\comparar_com_o_lightroom.exe"
$ferr = "$rasc\perfis\target\release\perfis.exe"
$base = "C:\Users\alexk\OneDrive\Pictures\Comparar Presets\regua-vinheta"
$orig = "$base\originais\cinza-128.jpg"
$saidaTxt = "$rasc\vinheta.txt"
"" | Set-Content $saidaTxt -Encoding UTF8
foreach ($e in Get-ChildItem "$base\cinza-128" -Filter *.jpg | Sort-Object Name) {
    $pasta = "$rasc\vinheta\$($e.BaseName)"
    $null = & $exe $orig $e.FullName $pasta atual 2>$null
    $bloco = @("== $($e.BaseName)", "Lightroom:") + (& $ferr perfil $e.FullName)
    if (Test-Path "$pasta\atual\nosso.jpg") {
        $bloco += "nosso:"
        $bloco += (& $ferr perfil "$pasta\atual\nosso.jpg")
        Remove-Item -LiteralPath "$pasta\atual\nosso.jpg"
        Remove-Item -LiteralPath "$pasta\atual\diferenca-x4.png" -ErrorAction SilentlyContinue
    } else {
        $bloco += "nosso: (não revelou)"
    }
    $bloco | Add-Content $saidaTxt -Encoding UTF8
    Write-Host $e.BaseName
}
