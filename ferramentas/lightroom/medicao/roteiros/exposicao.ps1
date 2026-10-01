# Só a Exposição, nos dois programas: para cada foto da régua `casos=exposicao`
# e cada valor, a diferença do VintageLightbox para o Lightroom no processo 0 (a
# conta antiga) e no 1 (a medida). → exposicao.csv
# Pasta de trabalho (CSVs e imagens intermediárias): VLB_RASCUNHO, ou uma no TEMP.
$Rascunho = if ($env:VLB_RASCUNHO) { $env:VLB_RASCUNHO } else { Join-Path $env:TEMP 'regua-vintagelightbox' }
New-Item -ItemType Directory -Force $Rascunho | Out-Null

$env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
$exe = "C:\Projects\VintageLightbox\target\release\examples\comparar_com_o_lightroom.exe"
$base = "C:\Users\alexk\OneDrive\Pictures\Comparar Presets\regua-exposicao"
$valores = '-4.00', '-2.00', '-1.00', '-0.50', '+0.50', '+1.00', '+2.00', '+4.00'
$linhas = @()
foreach ($orig in Get-ChildItem "$base\originais" -Filter 'exp_*') {
    $pasta = Join-Path $base $orig.BaseName
    foreach ($v in $valores) {
        $exportado = Get-ChildItem $pasta -Filter "*exposicao$v.jpg" | Select-Object -First 1
        foreach ($processo in 0, 1) {
            $env:VLB_FORCAR = "processo=$processo"
            $cru = & $exe $orig.FullName $exportado.FullName (Join-Path $Rascunho "exposicao\$($orig.BaseName)\$v-p$processo") atual 2>$null
            $txt = $cru -join "`n"
            $g = [regex]::Match($txt, '\|Δ\| médio R G B\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)')
            $faixa = { param($f) $m = [regex]::Match($txt, "(?m)^$f\s+([\d.]+)%\s+(-?[\d.]+)"); if ($m.Success) { [double]$m.Groups[2].Value } else { $null } }
            $linhas += [pscustomobject]@{
                Foto      = $orig.BaseName -replace '^exp_', ''
                Exposicao = $v
                Processo  = $processo
                Geral     = if ($g.Success) { [Math]::Round(([double]$g.Groups[1].Value + [double]$g.Groups[2].Value + [double]$g.Groups[3].Value) / 3, 1) } else { $null }
                Sombras   = & $faixa 'sombras'
                Medios    = & $faixa 'médios'
                Realces   = & $faixa 'realces'
                Brancos   = & $faixa 'brancos'
            }
            Write-Host ("{0} {1} p{2}: {3}" -f $orig.BaseName, $v, $processo, $linhas[-1].Geral)
        }
    }
}
$env:VLB_FORCAR = ''
$linhas | Export-Csv -NoTypeInformation -Encoding UTF8 (Join-Path $Rascunho 'exposicao.csv')
