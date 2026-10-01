# Para cada slider na rampa: a curva do Lightroom e a nossa.
# Pasta de trabalho (CSVs e imagens intermediárias): VLB_RASCUNHO, ou uma no TEMP.
$Rascunho = if ($env:VLB_RASCUNHO) { $env:VLB_RASCUNHO } else { Join-Path $env:TEMP 'regua-vintagelightbox' }
New-Item -ItemType Directory -Force $Rascunho | Out-Null
$env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
$rasc = "$Rascunho"
$exe = "C:\Projects\VintageLightbox\target\release\examples\comparar_com_o_lightroom.exe"
$ferr = "$rasc\perfis\target\release\perfis.exe"
$base = "C:\Users\alexk\OneDrive\Pictures\Comparar Presets\regua-sliders"
$orig = "$base\originais\rampa.jpg"
$saidaTxt = "$rasc\rampa.txt"
"entrada        0     8    16    32    48    64    96   128   160   192   224   240   255" | Set-Content $saidaTxt -Encoding UTF8
foreach ($e in Get-ChildItem "$base\rampa" -Filter *.jpg | Sort-Object Name) {
    $pasta = "$rasc\rampa\$($e.BaseName)"
    $txt = & $exe $orig $e.FullName $pasta atual 2>$null
    $campos = (($txt | Select-String '^ajustes').Line -replace '^ajustes \(atual\): ', '')
    $bloco = @("== $($e.BaseName)   [$campos]")
    if (Test-Path "$pasta\atual\nosso.jpg") {
        $bloco += (& $ferr curva $e.FullName "$pasta\atual\nosso.jpg")
        [System.IO.File]::Delete("$pasta\atual\nosso.jpg")
        [System.IO.File]::Delete("$pasta\atual\diferenca-x4.png")
    } else {
        $bloco += "  (não revelou)"
    }
    $bloco | Add-Content $saidaTxt -Encoding UTF8
}
