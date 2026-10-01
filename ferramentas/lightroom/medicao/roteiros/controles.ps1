# Cada controle do Básico sozinho, nos dois programas: para cada foto da régua
# `casos=controles` e cada caso, a diferença do VintageLightbox para o Lightroom
# no processo 0 (a conta antiga) e no 1 (a medida). → controles.csv
# Pasta de trabalho (CSVs e imagens intermediárias): VLB_RASCUNHO, ou uma no TEMP.
$Rascunho = if ($env:VLB_RASCUNHO) { $env:VLB_RASCUNHO } else { Join-Path $env:TEMP 'regua-vintagelightbox' }
New-Item -ItemType Directory -Force $Rascunho | Out-Null

$env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
$exe = "C:\Projects\VintageLightbox\target\release\examples\comparar_com_o_lightroom.exe"
$base = "C:\Users\alexk\OneDrive\Pictures\Comparar Presets\regua-controles"
$linhas = @()
foreach ($orig in Get-ChildItem "$base\originais" -Filter 'ctl_*') {
    $pasta = Join-Path $base $orig.BaseName
    foreach ($e in Get-ChildItem $pasta -Filter *.jpg | Where-Object Name -notmatch 'neutro' | Sort-Object Name) {
        $caso = $e.BaseName -replace '^\d+-', ''
        $m = [regex]::Match($caso, '^([A-Za-z]+?)(2012)?([+-]\d+)$')
        foreach ($processo in 0, 1) {
            $env:VLB_FORCAR = "processo=$processo"
            $saida = Join-Path $Rascunho "controles\$($orig.BaseName)\$caso-p$processo"
            $cru = & $exe $orig.FullName $e.FullName $saida atual 2>$null
            foreach ($pesado in 'nosso.jpg', 'diferenca-x4.png') {
                $q = Join-Path $saida "atual\$pesado"
                if (Test-Path $q) { [System.IO.File]::Delete($q) }
            }
            $txt = $cru -join "`n"
            $g = [regex]::Match($txt, '\|Δ\| médio R G B\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)')
            $faixa = { param($f, $col) $x = [regex]::Match($txt, "(?m)^$f\s+([\d.]+)%\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)"); if ($x.Success) { [double]$x.Groups[$col].Value } else { $null } }
            $linhas += [pscustomobject]@{
                Foto     = $orig.BaseName -replace '^ctl_', ''
                Controle = $m.Groups[1].Value
                Valor    = [int]$m.Groups[3].Value
                Processo = $processo
                Geral    = if ($g.Success) { [Math]::Round(([double]$g.Groups[1].Value + [double]$g.Groups[2].Value + [double]$g.Groups[3].Value) / 3, 1) } else { $null }
                Sombras  = & $faixa 'sombras' 2
                Medios   = & $faixa 'médios' 2
                Realces  = & $faixa 'realces' 2
                QuenteMedios = & $faixa 'médios' 3
                VerdeMedios  = & $faixa 'médios' 4
            }
        }
        Write-Host ("{0} {1}: p0 {2} p1 {3}" -f $orig.BaseName, $caso, $linhas[-2].Geral, $linhas[-1].Geral)
        $linhas | Export-Csv -NoTypeInformation -Encoding UTF8 (Join-Path $Rascunho 'controles.csv')
    }
}
$env:VLB_FORCAR = ''
