<# Capturas locais do molde de página; nunca grava evidência rastreada. #>
param(
    [Parameter(Mandatory)] [string]$App,
    [Parameter(Mandatory)] [ValidateSet('before','after')] [string]$Stage,
    [string]$Out = 'target/reports/197'
)
$ErrorActionPreference='Stop'
$App=(Resolve-Path $App).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$Out=(Resolve-Path $Out).Path
$env:FORJA_TEST_HELPERS='none'
Remove-Item Env:FORJA_TEST_WINDOW -ErrorAction SilentlyContinue
Add-Type -AssemblyName System.Drawing
$pages=@('board.page','request.page','helper.tab','routine.page','muster.page','archive.tab','skill.page','usage.page','extension.page')
foreach($size in @('1280x720','1920x1080')) {
    $env:FORJA_TEST_WINDOW_SIZE=$size
    foreach($page in $pages) {
        $root=Join-Path $env:TEMP "forja-pages-$Stage-$PID"
        $env:FORJA_DATA_DIR=Join-Path $root 'data'
        $env:FORJA_CONTENT_DIR=Join-Path $root 'user'
        $stem=Join-Path $Out "$Stage-$size-$page"
        try {
            $p=Start-Process $App -ArgumentList "--demo --screenshot `"$stem-native.png`" --ui-tree `"$stem-tree.json`" --rail $page --locale pt-BR --theme dark" -PassThru -Wait
            if($p.ExitCode -ne 0 -or !(Test-Path "$stem-native.png")) { throw "capture failed: $stem ($($p.ExitCode))" }
            $native=[System.Drawing.Image]::FromFile("$stem-native.png")
            $wh=$size.Split('x')
            $image=[System.Drawing.Bitmap]::new([int]$wh[0],[int]$wh[1])
            $g=[System.Drawing.Graphics]::FromImage($image)
            try {
                $g.InterpolationMode=[System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $g.DrawImage($native,0,0,$image.Width,$image.Height)
                $image.Save("$stem.png",[System.Drawing.Imaging.ImageFormat]::Png)
            } finally { $g.Dispose(); $image.Dispose(); $native.Dispose() }
            Write-Output "$Stage $size $page"
        } finally { if(Test-Path $root) { Remove-Item -LiteralPath $root -Recurse -Force } }
    }
}
