# Compara cada exportação da régua (Lightroom) com o nosso motor, à medida que
# chegam, até o registro dizer "fim". Junta tudo em regua.csv.
# Pasta de trabalho (CSVs e imagens intermediárias): VLB_RASCUNHO, ou uma no TEMP.
$Rascunho = if ($env:VLB_RASCUNHO) { $env:VLB_RASCUNHO } else { Join-Path $env:TEMP 'regua-vintagelightbox' }
New-Item -ItemType Directory -Force $Rascunho | Out-Null
$env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
$exe = "C:\Projects\VintageLightbox\target\release\examples\comparar_com_o_lightroom.exe"
$regua = "C:\Users\alexk\OneDrive\Pictures\Comparar Presets\regua"
$originais = Join-Path $regua "originais"
$rascunho = "$Rascunho"
$csv = Join-Path $rascunho "regua.csv"
$trabalho = Join-Path $rascunho "regua"

$feitos = @{}
$tabela = New-Object System.Collections.ArrayList
if (Test-Path $csv) {
    foreach ($l in Import-Csv $csv -Encoding UTF8) { [void]$tabela.Add($l); $feitos[$l.Arquivo] = $true }
}

function Medir($original, $exportado, $pasta) {
    $cru = & $exe $original $exportado $pasta atual 2>$null
    $txt = $cru -join "`n"
    $g = [regex]::Match($txt, '\|Δ\| médio R G B\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)')
    $r = [ordered]@{}
    $r.DeltaGeral = if ($g.Success) { [Math]::Round(([double]$g.Groups[1].Value + [double]$g.Groups[2].Value + [double]$g.Groups[3].Value) / 3, 1) } else { $null }
    foreach ($f in 'pretos', 'sombras', 'médios', 'realces', 'brancos') {
        $m = [regex]::Match($txt, "(?m)^$f\s+([\d.]+)%\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+([\d.]+)")
        $r["Luma_$f"] = if ($m.Success) { [double]$m.Groups[2].Value } else { $null }
        $r["Quente_$f"] = if ($m.Success) { [double]$m.Groups[3].Value } else { $null }
        $r["Verde_$f"] = if ($m.Success) { [double]$m.Groups[4].Value } else { $null }
    }
    $r.Ignorados = ([regex]::Match($txt, '(?m)^ignorados: (.*)$')).Groups[1].Value
    $r.Erro = if ($g.Success) { '' } else { ($cru | Select-Object -Last 2) -join ' ' }
    $r
}

while ($true) {
    $fim = (Test-Path "$regua\registro.txt") -and ((Get-Content "$regua\registro.txt" -Encoding UTF8 -Raw) -match '(?m) fim$')
    $novos = Get-ChildItem $regua -Recurse -Filter *.jpg |
        Where-Object { $_.DirectoryName -ne $originais -and -not $feitos[$_.FullName] } |
        Sort-Object DirectoryName, Name
    foreach ($e in $novos) {
        # O Lightroom ainda pode estar escrevendo o arquivo.
        if (((Get-Date) - $e.LastWriteTime).TotalSeconds -lt 10) { continue }
        $foto = Split-Path $e.DirectoryName -Leaf
        $orig = Get-ChildItem $originais | Where-Object { $_.BaseName -eq $foto } | Select-Object -First 1
        $caso = $e.BaseName -replace '^\d+-', ''
        $m = Medir $orig.FullName $e.FullName (Join-Path $trabalho "$foto\$($e.BaseName)")
        foreach ($pesado in 'nosso.jpg', 'diferenca-x4.png') { $q = Join-Path $trabalho "$foto\$($e.BaseName)\atual\$pesado"; if (Test-Path $q) { Remove-Item -LiteralPath $q } }
        $linha = [ordered]@{ Foto = $foto; Tipo = $orig.Extension.ToUpper(); Caso = $caso }
        foreach ($k in $m.Keys) { $linha[$k] = $m[$k] }
        $linha.Arquivo = $e.FullName
        [void]$tabela.Add([pscustomobject]$linha)
        $feitos[$e.FullName] = $true
        $tabela | Export-Csv -NoTypeInformation -Encoding UTF8 $csv
        Write-Host ("{0} {1}: {2}" -f $foto, $caso, $m.DeltaGeral)
    }
    if ($fim -and @($novos).Count -eq 0) { break }
    Start-Sleep 15
}
Write-Host "régua comparada: $($tabela.Count) casos"
