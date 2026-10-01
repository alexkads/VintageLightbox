# Acha JPGs exportados pelo Lightroom (XMP com crs:) e o original de cada um.
# Pasta de trabalho (CSVs e imagens intermediárias): VLB_RASCUNHO, ou uma no TEMP.
$Rascunho = if ($env:VLB_RASCUNHO) { $env:VLB_RASCUNHO } else { Join-Path $env:TEMP 'regua-vintagelightbox' }
New-Item -ItemType Directory -Force $Rascunho | Out-Null
param([string]$Saida)

$raizes = @(
    "C:\Users\alexk\OneDrive\Documentos",
    "C:\Users\alexk\Downloads",
    "C:\Users\alexk\Desktop",
    "C:\Users\alexk\OneDrive\Desktop",
    "C:\Users\alexk\OneDrive\Pictures"
)
$fotos = "C:\Users\alexk\OneDrive\Pictures"

function Ler-Xmp($caminho) {
    try {
        $fs = [System.IO.File]::OpenRead($caminho)
        $n = [Math]::Min($fs.Length, 262144)
        $buf = New-Object byte[] $n
        [void]$fs.Read($buf, 0, $n)
        $fs.Close()
    } catch { return $null }
    $t = [System.Text.Encoding]::UTF8.GetString($buf)
    $i = $t.IndexOf('<x:xmpmeta')
    if ($i -lt 0) { return $null }
    $k = $t.IndexOf('</x:xmpmeta>', $i)
    if ($k -lt 0) { return $null }
    $t.Substring($i, $k - $i)
}

function Attr($t, $nome) {
    $m = [regex]::Match($t, "$nome=`"([^`"]*)`"")
    if ($m.Success) { $m.Groups[1].Value } else { $null }
}

$pares = @()
foreach ($r in $raizes) {
    if (-not (Test-Path $r)) { continue }
    Get-ChildItem $r -Recurse -File -Include *.jpg, *.jpeg -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -notmatch '\\\d{4}\\\d{4}-\d{2}-\d{2}\\' -and $_.Length -gt 100KB } |
        ForEach-Object {
            $x = Ler-Xmp $_.FullName
            if (-not $x -or $x -notmatch 'crs:ProcessVersion') { return }
            $bruto = Attr $x 'crs:RawFileName'
            $data = Attr $x 'xmp:CreateDate'
            if (-not $data) { $data = Attr $x 'photoshop:DateCreated' }
            if (-not $bruto -or -not $data) { return }
            $dia = $data.Substring(0, 10)
            $ano = $dia.Substring(0, 4)
            $orig = Join-Path $fotos "$ano\$dia\$bruto"
            $look = ''
            $li = $x.IndexOf('<crs:Look>')
            if ($li -ge 0) { $look = [regex]::Match($x.Substring($li), 'crs:Name="([^"]*)"').Groups[1].Value }
            $pares += [pscustomobject]@{
                Exportado = $_.FullName
                Original  = $orig
                Existe    = (Test-Path $orig)
                Bruto     = $bruto
                Data      = $data
                Look      = $look -replace '&amp;', '&'
                Cinza     = ($x -match 'ConvertToGrayscale="True"')
            }
        }
}
$pares | Export-Csv -NoTypeInformation -Encoding UTF8 $Saida
$pares | Group-Object Existe | Select-Object Name, Count
$pares | Where-Object Existe | Group-Object { [System.IO.Path]::GetExtension($_.Bruto).ToUpper() } | Select-Object Name, Count
