<#
.SYNOPSIS
  Capturas e árvore acessível do item 7, sempre fora da tela e com dados isolados.
.EXAMPLE
  ./playtest/prove-icons.ps1 -App target/debug/forja-app.exe -Stage after
#>
param(
    [Parameter(Mandatory)] [string]$App,
    [Parameter(Mandatory)] [ValidateSet('before', 'after')] [string]$Stage,
    [string]$Out = 'docs/reports/196-icons'
)
$ErrorActionPreference = 'Stop'
$App = (Resolve-Path $App).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
# Windows captures use physical pixels (150% DPI on the proof machine).
# Keep the native PNG and also publish a logical-pixel copy at the requested size.
Add-Type -AssemblyName System.Drawing
$env:FORJA_TEST_HELPERS = 'none'
Remove-Item Env:FORJA_TEST_WINDOW -ErrorAction SilentlyContinue

function Get-Snapshot([string]$Pipe) {
    $client = [System.IO.Pipes.NamedPipeClientStream]::new('.', $Pipe, [System.IO.Pipes.PipeDirection]::InOut)
    try {
        $client.Connect(10000)
        $writer = [System.IO.StreamWriter]::new($client, [System.Text.UTF8Encoding]::new($false))
        $reader = [System.IO.StreamReader]::new($client, [System.Text.Encoding]::UTF8)
        $writer.WriteLine('{"jsonrpc":"2.0","id":1,"method":"ui.snapshot","params":{}}')
        $writer.Flush()
        $reply = $reader.ReadLine() | ConvertFrom-Json -Depth 50
        if ($reply.error) { throw $reply.error.message }
        return $reply.result
    } finally { $client.Dispose() }
}

foreach ($size in @('1280x720', '1920x1080')) {
    $root = Join-Path $env:TEMP "forja-icons-proof-$Stage-$size-$PID"
    $pipe = "forja-icons-$Stage-$size-$PID"
    $env:FORJA_TEST_WINDOW_SIZE = $size
    $env:FORJA_PIPE = "\\.\pipe\$pipe"
    $env:FORJA_DATA_DIR = Join-Path $root 'data'
    $env:FORJA_CONTENT_DIR = Join-Path $root 'user'
    $stem = Join-Path $Out "$Stage-$size"
    $appProcess = $null
    try {
        $appProcess = Start-Process $App -ArgumentList '--demo --test-driver --locale pt-BR --theme dark --bench-tab editor.tab --right-tab explorer.panel' -PassThru
        & "$PSScriptRoot/drive.ps1" -Pipe $pipe -Steps '[{"m":"ui.key","keys":"Escape"},{"m":"ui.snapshot","view":true}]' | ForEach-Object { ([string]$_).TrimEnd() } | Out-File "$stem-drive.txt" -Encoding utf8
        $snapshot = Get-Snapshot $pipe
        $snapshot | ConvertTo-Json -Depth 50 | Out-File "$stem-snapshot.json" -Encoding utf8
        $buttons = @($snapshot.elements | Where-Object role -eq 'Button')
        $logical = $size.Split('x')
        $dpi = $snapshot.window.width / [double]$logical[0]
        [PSCustomObject]@{
            stage = $Stage; size = $size; native_window = $snapshot.window; pixels_per_point = $dpi; buttons = $buttons.Count
            unnamed_buttons = @($buttons | Where-Object { [string]::IsNullOrWhiteSpace($_.name) }).Count
            window_controls = @($buttons | Where-Object name -in @('Minimizar', 'Maximizar', 'Fechar janela') | ForEach-Object {
                @{ name = $_.name; rect = $_.rect; logical_size = @(
                    [Math]::Round(($_.rect[2] - $_.rect[0]) / $dpi, 2),
                    [Math]::Round(($_.rect[3] - $_.rect[1]) / $dpi, 2)
                ) }
            })
        } | ConvertTo-Json -Depth 10 | Out-File "$stem-metrics.json" -Encoding utf8
    } finally {
        if ($appProcess -and !$appProcess.HasExited) { $appProcess.Kill(); $appProcess.WaitForExit() }
    }
    try {
        foreach ($page in @('bench', 'board')) {
            $png = "$stem-$page-native.png"
            $tree = "$stem-$page-tree.json"
            $extra = if ($page -eq 'board') { '--rail board.page' } else { '--bench-tab editor.tab --right-tab explorer.panel' }
            $shot = Start-Process $App -ArgumentList "--demo --screenshot `"$png`" --ui-tree `"$tree`" --locale pt-BR --theme dark $extra" -PassThru -Wait
            if ($shot.ExitCode -ne 0 -or !(Test-Path $png)) { throw "capture failed: $png (exit $($shot.ExitCode))" }
            $native = [System.Drawing.Image]::FromFile($png)
            $logical = $size.Split('x')
            $normalized = [System.Drawing.Bitmap]::new([int]$logical[0], [int]$logical[1])
            $graphics = [System.Drawing.Graphics]::FromImage($normalized)
            try {
                $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $graphics.DrawImage($native, 0, 0, $normalized.Width, $normalized.Height)
                $normalized.Save("$stem-$page.png", [System.Drawing.Imaging.ImageFormat]::Png)
            } finally { $graphics.Dispose(); $normalized.Dispose(); $native.Dispose() }
        }
    } finally {
        if (Test-Path $root) { Remove-Item -LiteralPath $root -Recurse -Force }
    }
    Write-Output "$Stage ${size}: captured bench, board, accessible tree and drive transcript"
}
