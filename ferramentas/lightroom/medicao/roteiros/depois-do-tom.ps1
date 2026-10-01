# Espera a régua de tom, reinicia o Lightroom com o plug-in novo e manda a
# força fina da vinheta nas quatro fotos de quadrantes.
# Pasta de trabalho (CSVs e imagens intermediárias): VLB_RASCUNHO, ou uma no TEMP.
$Rascunho = if ($env:VLB_RASCUNHO) { $env:VLB_RASCUNHO } else { Join-Path $env:TEMP 'regua-vintagelightbox' }
New-Item -ItemType Directory -Force $Rascunho | Out-Null
$reg = "C:\Users\alexk\OneDrive\Pictures\Comparar Presets\regua-tom\registro.txt"
while (-not ((Get-Content $reg -Encoding UTF8 -Raw) -match '(?m) fim\r?$|interrompida')) { Start-Sleep 10 }
$mod = "$env:APPDATA\Adobe\Lightroom\Modules\VintageLightbox-Calibracao.lrplugin"
while (Test-Path "$mod\pedido-em-andamento.txt") { Start-Sleep 3 }

$p = Get-Process Lightroom -ErrorAction SilentlyContinue
if ($p) {
    [void]$p.CloseMainWindow()
    $n = 0
    while (-not $p.HasExited -and $n -lt 60) { Start-Sleep 2; $p.Refresh(); $n++ }
}
Copy-Item "C:\Projects\VintageLightbox\ferramentas\lightroom\VintageLightbox-Calibracao.lrplugin\*.lua" $mod -Force
foreach ($f in 'pedido-feito.txt', 'pedido-em-andamento.txt') { if (Test-Path "$mod\$f") { [System.IO.File]::Delete("$mod\$f") } }
$o = "C:\Users\alexk\OneDrive\Pictures\Comparar Presets\regua-forca2\originais"
$l = @("saida=C:\Users\alexk\OneDrive\Pictures\Comparar Presets\regua-forca2", "casos=vinheta-forca-fina") + (Get-ChildItem $o | Sort-Object Name | ForEach-Object { "foto=$($_.FullName)" })
[System.IO.File]::WriteAllLines("$mod\pedido.txt", $l, (New-Object System.Text.UTF8Encoding $false))
Start-Process ([System.IO.Path]::Combine($env:ProgramFiles, "Adobe", "Adobe Lightroom Classic", "Lightroom.exe"))

$reg2 = "C:\Users\alexk\OneDrive\Pictures\Comparar Presets\regua-forca2\registro.txt"
while (-not ((Test-Path $reg2) -and ((Get-Content $reg2 -Encoding UTF8 -Raw) -match '(?m) fim\r?$|interrompida'))) { Start-Sleep 10 }
"tom: " + (Get-Content $reg -Tail 1 -Encoding UTF8)
"força fina: " + (Get-Content $reg2 -Tail 1 -Encoding UTF8)
"falhas: " + @((Get-Content $reg, $reg2 -Encoding UTF8) | Where-Object { $_ -match '\t' -and $_ -notmatch '\tok$' }).Count
