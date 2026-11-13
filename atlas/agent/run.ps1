<#
.SYNOPSIS
  Loop do agente do atlas: documenta as telas pendentes até o prazo ou até completar.

.DESCRIPTION
  Para cada item de todo.json (sem página ou desatualizada), monta o pedido
  (INSTRUCTIONS.md + `xtask atlas brief`), chama `opencode run` com a captura
  anexada e grava a resposta com `xtask atlas accept`. Faz commit e push a cada
  -PushEvery páginas quando -Push está ligado. O modelo não recebe permissão
  de escrita: só o `accept` grava arquivos.

.EXAMPLE
  ./atlas/agent/run.ps1 -Out atlas-out -Xtask target/debug/xtask.exe -Minutes 10 -MaxPages 1
#>
param(
    [string]$Out = "atlas-site",
    [string]$Xtask = "target/release/xtask.exe",
    [int]$Minutes = 300,
    [int]$MaxPages = 0,
    [string[]]$Models = @("opencode/glm-5.3-flash-free", "opencode/deepseek-v4.1-flash-free"),
    [int]$CallTimeoutSeconds = 420,
    [switch]$Push,
    [int]$PushEvery = 3
)

$ErrorActionPreference = "Stop"
# Push credentials stay in this script only: removed from the environment so the
# model process (and anything it runs) never inherits them.
$pushToken = $env:ATLAS_PUSH_TOKEN
$pushRepo = $env:ATLAS_PUSH_REPO
Remove-Item Env:ATLAS_PUSH_TOKEN -ErrorAction SilentlyContinue
$deadline = (Get-Date).AddMinutes($Minutes)
$instructions = Get-Content (Join-Path $PSScriptRoot "INSTRUCTIONS.md") -Raw
$work = Join-Path ([System.IO.Path]::GetTempPath()) "forja-atlas-agent"
New-Item -ItemType Directory -Force $work | Out-Null
# Free models only: anything that is not an "-free" model is refused, so no paid quota is used.
$Models = @($Models | Where-Object { $_ -and $_.Trim() -match '-free$' } | ForEach-Object { $_.Trim() })
if ($Models.Count -eq 0) { throw "no free model configured (ids must end in -free)" }
$failures = @{}
$written = 0
$sincePush = 0

# The npm shims (.ps1/.cmd) cannot be started directly with an argument list.
$opencodeExe = Join-Path (npm root -g) "@opencode/cli/bin/opencode.exe"
if (-not (Test-Path $opencodeExe)) {
    $opencodeExe = (Get-Command opencode -CommandType Application | Select-Object -First 1).Source
}

function Invoke-Agent([string]$Model, [string]$PromptFile, [string]$Image) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new($opencodeExe)
    foreach ($arg in @("run", "--archived", "-m", $Model, "--file", $PromptFile, "--file", $Image,
            "Siga as instruções do arquivo de texto anexado e documente a captura de tela anexada.")) {
        $psi.ArgumentList.Add($arg)
    }
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $process = [System.Diagnostics.Process]::Start($psi)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($CallTimeoutSeconds * 1000)) {
        $process.Kill($true)
        Write-Host "  timeout after $CallTimeoutSeconds s"
        return ""
    }
    if ($process.ExitCode -ne 0) {
        Write-Host "  opencode exit $($process.ExitCode): $(($stderr.Result -split "`n" | Select-Object -Last 3) -join ' | ')"
    }
    return $stdout.Result
}

function Publish([string]$Message) {
    & $Xtask atlas status --out $Out | Out-Null
    & $Xtask atlas index --out $Out
    if (-not $Push) { return }
    git -C $Out add -A
    git -C $Out diff --cached --quiet
    if ($LASTEXITCODE -eq 0) { return }
    git -C $Out -c user.name="forja-atlas" -c user.email="atlas@users.noreply.github.com" commit -q -m $Message
    if ($pushToken -and $pushRepo) {
        $basic = [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("x-access-token:$pushToken"))
        git -C $Out -c "http.extraheader=AUTHORIZATION: basic $basic" push -q "https://github.com/$pushRepo.git" HEAD:atlas
    } else {
        git -C $Out push -q origin HEAD:atlas
    }
    if ($LASTEXITCODE -ne 0) { Write-Warning "push failed" }
}

while ((Get-Date) -lt $deadline) {
    if ($MaxPages -gt 0 -and $written -ge $MaxPages) { break }
    & $Xtask atlas status --out $Out
    $todo = Get-Content (Join-Path $Out "todo.json") -Raw | ConvertFrom-Json
    if ($todo.complete) { Write-Host "atlas: complete"; break }
    $items = @(@($todo.missing) + @($todo.stale) | Where-Object { $_ -and ($failures[$_.state] -as [int]) -lt 2 })
    if ($items.Count -eq 0) { Write-Host "atlas: nothing left that has not failed twice"; break }
    $item = $items[0]
    Write-Host "atlas: $($item.state)"

    $promptFile = Join-Path $work "pedido.md"
    $brief = (& $Xtask atlas brief --out $Out --state $item.state) -join "`n"
    Set-Content -Path $promptFile -Value "$instructions`n`n---`n`n$brief" -Encoding utf8
    $image = (Resolve-Path (Join-Path $Out $item.png)).Path

    $accepted = $false
    foreach ($model in $Models) {
        $answer = Invoke-Agent -Model $model -PromptFile $promptFile -Image $image
        $answerFile = Join-Path $work "resposta.txt"
        Set-Content -Path $answerFile -Value $answer -Encoding utf8
        & $Xtask atlas accept --out $Out --state $item.state --from $answerFile --model $model
        if ($LASTEXITCODE -eq 0) { $accepted = $true; break }
        Write-Host "  ${model}: answer rejected"
    }
    if ($accepted) {
        $written++
        $sincePush++
        if ($sincePush -ge $PushEvery) {
            Publish "atlas: $written páginas nesta rodada"
            $sincePush = 0
        }
    } else {
        $failures[$item.state] = ($failures[$item.state] -as [int]) + 1
    }
}

Publish "atlas: fim da rodada ($written páginas)"
Write-Host "atlas: $written pages written"
