<#
.SYNOPSIS
  Dirige a Forja em modo de teste pelo pipe, passo a passo (para reproduzir bugs).

.DESCRIPTION
  Abra o app antes, com um pipe próprio e dados isolados:
    $env:FORJA_PIPE = '\\.\pipe\forja-drive'
    $env:FORJA_DATA_DIR = "$env:TEMP\forja-drive\data"
    $env:FORJA_CONTENT_DIR = "$env:TEMP\forja-drive\user"
    $env:FORJA_TEST_PICK_DIR = 'C:\playground\exemplo'   # ./playtest/seed.ps1
    Start-Process target\debug\forja-app.exe -ArgumentList '--test-driver'
  Depois rode passos em JSON: m = método ui.* (snapshot, click, type, key, scroll, wait)
  e os demais campos são os parâmetros. Cada passo imprime o que apareceu e sumiu.

.EXAMPLE
  ./playtest/drive.ps1 -Pipe forja-drive -Steps '[{"m":"ui.click","target":"Ajustes"},{"m":"ui.key","keys":"Escape"},{"m":"ui.snapshot","view":true}]'
#>
param(
    [string]$Pipe = "forja-drive",
    [Parameter(Mandatory)] [string]$Steps
)

function Invoke-Ui([string]$Method, [hashtable]$Params) {
    $client = [System.IO.Pipes.NamedPipeClientStream]::new(".", $Pipe, [System.IO.Pipes.PipeDirection]::InOut)
    $client.Connect(10000)
    $writer = [System.IO.StreamWriter]::new($client, [System.Text.UTF8Encoding]::new($false))
    $reader = [System.IO.StreamReader]::new($client, [System.Text.Encoding]::UTF8)
    $line = @{ jsonrpc = "2.0"; id = 1; method = $Method; params = $Params } | ConvertTo-Json -Compress -Depth 6
    $writer.WriteLine($line)
    $writer.Flush()
    $answer = $reader.ReadLine()
    $client.Dispose()
    return $answer | ConvertFrom-Json -Depth 30
}

foreach ($step in ($Steps | ConvertFrom-Json)) {
    $params = @{}
    $showView = $false
    foreach ($property in $step.PSObject.Properties) {
        if ($property.Name -eq "m") { continue }
        if ($property.Name -eq "view") { $showView = [bool]$property.Value; continue }
        $params[$property.Name] = $property.Value
    }
    $reply = Invoke-Ui $step.m $params
    Write-Output ">> $($step.m) $($params | ConvertTo-Json -Compress)"
    if ($reply.error) { Write-Output "   ERRO: $($reply.error.message)"; continue }
    $result = $reply.result
    if ($result.action.clicked) { Write-Output "   clicou: $($result.action.clicked.role) '$($result.action.clicked.name)'" }
    Write-Output "   apareceu: $(($result.changes.appeared | Select-Object -First 15) -join ' | ')"
    Write-Output "   sumiu: $(($result.changes.disappeared | Select-Object -First 15) -join ' | ')"
    if ($showView) { Write-Output $result.view }
}
