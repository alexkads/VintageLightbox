# Roda o comparador em todos os pares de pares.csv e junta num CSV.
# Pasta de trabalho (CSVs e imagens intermediárias): VLB_RASCUNHO, ou uma no TEMP.
$Rascunho = if ($env:VLB_RASCUNHO) { $env:VLB_RASCUNHO } else { Join-Path $env:TEMP 'regua-vintagelightbox' }
New-Item -ItemType Directory -Force $Rascunho | Out-Null
$Pares = (Join-Path $Rascunho 'pares.csv'); $Saida = (Join-Path $Rascunho 'resultado.csv'); $Pasta = (Join-Path $Rascunho 'todas')

$env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
$exe = "C:\Projects\VintageLightbox\target\release\examples\comparar_com_o_lightroom.exe"
$presets = "C:\Users\alexk\AppData\Roaming\Adobe\CameraRaw\ImportedSettings\Predefinições do usuário"

function Ler-Xmp($caminho) {
    $fs = [System.IO.File]::OpenRead($caminho)
    $n = [Math]::Min($fs.Length, 262144)
    $buf = New-Object byte[] $n
    [void]$fs.Read($buf, 0, $n)
    $fs.Close()
    $t = [System.Text.Encoding]::UTF8.GetString($buf)
    $i = $t.IndexOf('<x:xmpmeta')
    $k = $t.IndexOf('</x:xmpmeta>', $i)
    $t.Substring($i, $k - $i)
}

function Chaves($t) {
    $i = $t.IndexOf('<crs:Look>')
    if ($i -ge 0) { $k = $t.IndexOf('</crs:Look>'); $t = $t.Substring(0, $i) + $t.Substring($k) }
    $h = @{}
    foreach ($m in [regex]::Matches($t, 'crs:([A-Za-z0-9]+)="([^"]*)"')) { $h[$m.Groups[1].Value] = $m.Groups[2].Value }
    $h
}

$ign = 'UUID|Version|ProcessVersion|Name|ShortName|SortName|Group|Cluster|Supports.*|PresetType|HasSettings|RequiresRGBTables|ShowIn.*|Copyright|ContactInfo|Description|CameraModelRestriction|Stubbed|Amount|OverrideLookVignette'
$catalogo = foreach ($p in Get-ChildItem $presets -Filter *.xmp) {
    [pscustomobject]@{ Nome = $p.BaseName.Trim(); Chaves = (Chaves ([System.IO.File]::ReadAllText($p.FullName))); Vinheta = $p.Name -match '^Vinheta' }
}

function Qual-Preset($foto, $vinheta) {
    $melhor = $null; $nota = -1
    foreach ($p in $catalogo | Where-Object { $_.Vinheta -eq $vinheta }) {
        $comp = @($p.Chaves.Keys | Where-Object { $_ -notmatch "^($ign)$" -and ($vinheta -or $_ -notmatch '^PostCropVignette') })
        if ($comp.Count -eq 0) { continue }
        $iguais = @($comp | Where-Object { $foto[$_] -eq $p.Chaves[$_] }).Count
        $r = $iguais / $comp.Count
        if ($r -gt $nota) { $nota = $r; $melhor = $p.Nome }
    }
    "{0} ({1:P0})" -f $melhor, $nota
}

$linhas = @()
$pares = Import-Csv $Pares | Where-Object { $_.Existe -eq 'True' }
$i = 0
foreach ($par in $pares) {
    $i++
    $x = Ler-Xmp $par.Exportado
    $foto = Chaves $x
    $captura = [datetime]::Parse($par.Data.Substring(0, 19))
    $arquivo = Get-Item $par.Original
    $distancia = [Math]::Abs(($arquivo.LastWriteTime - $captura).TotalMinutes)

    $nome = "{0:D2}-{1}" -f $i, [System.IO.Path]::GetFileNameWithoutExtension($par.Exportado)
    $texto_cru = & $exe $par.Original $par.Exportado (Join-Path $Pasta $nome) atual 2>$null
    $texto = $texto_cru -join "`n"
    $geral = [regex]::Match($texto, '\|Δ\| médio R G B\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)')
    $faixa = @{}
    foreach ($f in 'pretos', 'sombras', 'médios', 'realces', 'brancos') {
        $m = [regex]::Match($texto, "(?m)^$f\s+([\d.]+)%\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+([\d.]+)")
        if ($m.Success) { $faixa[$f] = $m }
    }
    $num = { param($m, $g) if ($m) { [double]$m.Groups[$g].Value } else { $null } }
    $media = if ($geral.Success) { ([double]$geral.Groups[1].Value + [double]$geral.Groups[2].Value + [double]$geral.Groups[3].Value) / 3 } else { $null }
    $linhas += [pscustomobject]@{
        N            = $i
        Foto         = $nome
        Original     = [System.IO.Path]::GetExtension($par.Original).ToUpper()
        Sessao       = Split-Path (Split-Path $par.Exportado -Parent) -Leaf
        Preset       = Qual-Preset $foto $false
        Vinheta      = Qual-Preset $foto $true
        Perfil       = $par.Look
        PB           = $par.Cinza
        CapturaOk    = ($distancia -lt 3)
        Erro         = if ($geral.Success) { '' } else { ($texto_cru | Select-Object -Last 2) -join ' ' }
        DeltaGeral   = if ($media) { [Math]::Round($media, 1) } else { $null }
        LumaSombras  = & $num $faixa['sombras'] 2
        LumaMedios   = & $num $faixa['médios'] 2
        LumaRealces  = & $num $faixa['realces'] 2
        QuenteMedios = & $num $faixa['médios'] 3
        VerdeMedios  = & $num $faixa['médios'] 4
        Exportado    = $par.Exportado
    }
    Write-Host ("{0}/{1} {2}: {3}" -f $i, $pares.Count, $nome, $(if ($media) { [Math]::Round($media, 1) } else { 'erro' }))
}
$linhas | Export-Csv -NoTypeInformation -Encoding UTF8 $Saida
