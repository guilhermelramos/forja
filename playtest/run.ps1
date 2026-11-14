<#
.SYNOPSIS
  Um testador: abre a Forja em modo de teste e deixa um agente usá-la ao vivo.

.DESCRIPTION
  Inicia `forja-app --test-driver` com dados isolados e, até o prazo, roda
  sessões do OpenCode com INSTRUCTIONS.md + a missão. O agente opera o app só
  pelas ferramentas de `forja-mcp --player` (olhar, clicar, digitar, teclas)
  e registra achados em findings.jsonl. Cada sessão tem um orçamento de ações
  (-MaxSteps) e a próxima recebe um resumo estruturado da anterior, com os
  títulos já registrados. Se o app cair, isso vira um achado `crash` e o app
  é reaberto.

  Saída em -Out: findings.jsonl, actions.jsonl (cada ação no app),
  sessions/*.txt (transcrições), sessions/usage.jsonl (passos e tokens de
  cada sessão) e app-stderr.txt.

.EXAMPLE
  ./playtest/run.ps1 -Mission playtest/missions/caos.md -Minutes 20 -App target/debug/forja-app.exe -Mcp target/debug/forja-mcp.exe
#>
param(
    [Parameter(Mandatory)] [string]$Mission,
    [int]$Minutes = 80,
    [int]$SessionMinutes = 20,
    # Actions (click, type, key, scroll, wait) an agent may take in one session.
    [int]$MaxSteps = 40,
    [string[]]$Models = @("opencode/glm-5.3-flash-free", "opencode/deepseek-v4.1-flash-free",
        "opencode/minimax-m3.1-flash-free", "opencode/mimo-v2.6-flash-free"),
    # Testers start at different models so the free limits are spread out.
    [int]$ModelOffset = 0,
    [string]$App = "target/release/forja-app.exe",
    [string]$Mcp = "target/release/forja-mcp.exe",
    [string]$Out = "playtest-out",
    [string]$Locale = "pt-BR",
    [string]$PickDir = "C:\playground\exemplo"
)

$ErrorActionPreference = "Stop"
$deadline = (Get-Date).AddMinutes($Minutes)
$name = [System.IO.Path]::GetFileNameWithoutExtension($Mission)
$Out = [System.IO.Path]::GetFullPath($Out)
$App = (Resolve-Path $App).Path
$Mcp = (Resolve-Path $Mcp).Path
New-Item -ItemType Directory -Force $Out, "$Out\sessions", "$Out\data", "$Out\user", "$Out\room", "$Out\config" | Out-Null
$findings = Join-Path $Out "findings.jsonl"
$pipe = "\\.\pipe\forja-play-$name-$PID"

# The tester gets its own OpenCode config home: no developer skills, plugins or
# MCP servers on top of the `forja` server. Keeps every run reproducible and
# stops unrelated tool catalogs from costing tokens.
$configHome = Join-Path $Out "config"

# Apenas o que o app e o agente precisam; nada de tokens do GitHub.
Remove-Item Env:GITHUB_TOKEN, Env:GH_TOKEN, Env:ACTIONS_RUNTIME_TOKEN -ErrorAction SilentlyContinue

function Start-Forja {
    $psi = [System.Diagnostics.ProcessStartInfo]::new($App)
    foreach ($arg in @("--test-driver", "--locale", $Locale)) { $psi.ArgumentList.Add($arg) }
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $psi.Environment["FORJA_PIPE"] = $pipe
    $psi.Environment["FORJA_DATA_DIR"] = "$Out\data"
    $psi.Environment["FORJA_CONTENT_DIR"] = "$Out\user"
    $psi.Environment["FORJA_TEST_LOG"] = "$Out\actions.jsonl"
    $psi.Environment["FORJA_TEST_PICK_DIR"] = $PickDir
    $psi.Environment["FORJA_TEST_PICK_FILE"] = Join-Path $PickDir "README.md"
    $psi.Environment["FORJA_TEST_SAVE_DIR"] = "$Out\downloads"
    $process = [System.Diagnostics.Process]::Start($psi)
    $script:appErr = $process.StandardError.ReadToEndAsync()
    Start-Sleep -Seconds 6
    return $process
}

function Get-LineCount([string]$Path) {
    if (Test-Path $Path) { return @(Get-Content $Path).Count }
    return 0
}

function Get-NewFindingTitles([string]$Path, [int]$From) {
    if (-not (Test-Path $Path)) { return @() }
    return @(Get-Content $Path | Select-Object -Skip $From | ForEach-Object {
        try { ($_ | ConvertFrom-Json).title } catch { $null }
    } | Where-Object { $_ })
}

# What the agent actually did, straight from the app's action log.
function Get-SessionActions([string]$Path, [int]$From) {
    if (-not (Test-Path $Path)) { return @{ count = 0; methods = @{}; targets = @() } }
    $lines = @(Get-Content $Path | Select-Object -Skip $From)
    $methods = @{}
    $targets = [System.Collections.Generic.List[string]]::new()
    foreach ($line in $lines) {
        try { $item = $line | ConvertFrom-Json } catch { continue }
        $name = $item.method -replace '^ui\.', ''
        $methods[$name] = 1 + [int]($methods[$name])
        $target = $item.params.target
        if (-not $target) { $target = $item.params.keys }
        if (-not $target) { $target = $item.params.text }
        if ($target) {
            $target = "$target" -replace '\s+', ' '
            if ($target.Length -gt 28) { $target = $target.Substring(0, 28) + '…' }
            if (-not $targets.Contains($target)) { $targets.Add($target) }
        }
    }
    return @{ count = $lines.Count; methods = $methods; targets = @($targets | Select-Object -First 12) }
}

# The agent's own closing block, plus facts that do not depend on the model.
function Get-SessionSummary([string]$Text, [string]$Facts) {
    $lines = $Text -split "\r?\n"
    $start = -1
    for ($i = $lines.Count - 1; $i -ge 0; $i--) {
        if ($lines[$i] -match '^\s*#{1,4}\s*(Resumo|Summary)') { $start = $i; break }
    }
    $block = @()
    if ($start -ge 0) {
        for ($i = $start; $i -lt $lines.Count -and $block.Count -lt 16; $i++) {
            $line = $lines[$i].TrimEnd()
            if ($i -gt $start -and $line -match '^\s*#{1,4}\s') { break }
            if ($line.Trim()) { $block += $line }
        }
    }
    if ($block.Count -lt 2) { $block = @("## Resumo da sessão") }
    $block += "- Fatos: $Facts"
    return ($block -join "`n")
}

# Exact tokens and steps of one session, from OpenCode's own session data.
# Never breaks the test: on any error it just records nothing.
function Save-SessionUsage([string]$OpenCode, [string]$Room, [string]$Out, [int]$Session, [long]$Since, [string]$ConfigHome) {
    $previous = $env:XDG_CONFIG_HOME
    try {
        $env:XDG_CONFIG_HOME = $ConfigHome
        Push-Location $Room
        Start-Sleep -Milliseconds 400
        $list = @(& $OpenCode session list --standalone --format json 2>$null | ConvertFrom-Json)
        $mine = $list |
            Where-Object { $_.directory -eq $Room -and [long]$_.created -ge $Since } |
            Sort-Object { [long]$_.created } |
            Select-Object -Last 1
        if (-not $mine) { return }
        $export = & $OpenCode session export $mine.id --standalone 2>$null | ConvertFrom-Json
        if (-not $export.info.tokens) { return }
        $tokens = $export.info.tokens
        $usage = [pscustomobject]@{
            session = $Session
            id = $mine.id
            title = $mine.title
            steps = @($export.messages | Where-Object { $_.type -eq 'assistant' }).Count
            input = $tokens.input
            output = $tokens.output
            reasoning = $tokens.reasoning
            cache_read = $tokens.cache.read
            cache_write = $tokens.cache.write
            cost = $export.info.cost
            model = "$($export.info.model.providerID)/$($export.info.model.id)"
        }
        $usage | ConvertTo-Json -Compress | Add-Content "$Out\sessions\usage.jsonl"
        Write-Host ("  tokens: entrada {0}, cache {1}, saída {2}, raciocínio {3}; {4} passos" -f
            $tokens.input, $tokens.cache.read, $tokens.output, $tokens.reasoning, $usage.steps)
    } catch {
        # Usage is observability only.
    } finally {
        Pop-Location
        if ($null -eq $previous) {
            Remove-Item Env:XDG_CONFIG_HOME -ErrorAction SilentlyContinue
        } else {
            $env:XDG_CONFIG_HOME = $previous
        }
    }
}

# Config do OpenCode só nesta pasta: o servidor MCP do testador.
$config = @{
    '$schema' = "https://opencode.ai/config.json"
    mcp = @{ servers = @{ forja = @{
        type = "local"
        command = @($Mcp, "--player", "--findings", $findings, "--max-steps", "$MaxSteps")
        environment = @{ FORJA_PIPE = $pipe }
    } } }
}
$config | ConvertTo-Json -Depth 8 | Set-Content "$Out\room\opencode.json" -Encoding utf8
Push-Location "$Out\room"
git init -q 2>$null
Pop-Location

$opencode = Join-Path (npm root -g) "@opencode/cli/bin/opencode.exe"
if (-not (Test-Path $opencode)) { $opencode = (Get-Command opencode -CommandType Application | Select-Object -First 1).Source }

$instructions = Get-Content (Join-Path $PSScriptRoot "INSTRUCTIONS.md") -Raw
$missionText = Get-Content $Mission -Raw
$forja = Start-Forja
$summary = ""
$session = 0
$exhausted = @{}
$quickFailures = 0
$Models = @($Models | Where-Object { $_ })
# Free models only: anything that is not an "-free" model is refused, so no paid quota is used.
$Models = @($Models | Where-Object { $_ -and $_.Trim() -match '-free$' } | ForEach-Object { $_.Trim() })
if ($Models.Count -eq 0) { throw "no free model configured (ids must end in -free)" }

while ((Get-Date) -lt $deadline) {
    $session++
    $known = @()
    if (Test-Path $findings) {
        $known = Get-Content $findings | ForEach-Object { ($_ | ConvertFrom-Json).title }
    }
    $left = [int]($deadline - (Get-Date)).TotalMinutes
    $prompt = "$instructions`n`n$missionText`n`n"
    if ($known.Count) { $prompt += "Achados já registrados (não repita):`n- " + ($known -join "`n- ") + "`n`n" }
    if ($summary) { $prompt += "Resumo da sessão anterior (continue a partir daqui, explore o que ainda não foi testado):`n$summary`n`n" }
    $prompt += "Orçamento desta sessão: no máximo $MaxSteps ações (clique, digitação, tecla, rolagem, espera). " +
        "Ao chegar a 0, pare e escreva o resumo. Tempo restante: cerca de $left minutos. " +
        "Comece com ui_look (a tela inteira só ao chegar numa tela nova).`n`n" +
        "Termine com:`n## Resumo da sessão`n- Telas visitadas: ...`n- Testado: ...`n- Achados: ...`n- A seguir: ..."


    $available = @($Models | Where-Object { -not $exhausted[$_] })
    if ($available.Count -eq 0) {
        Write-Host "playtest[$name]: todos os modelos atingiram o limite gratuito; encerrando"
        break
    }
    $model = $available[($session - 1 + $ModelOffset) % $available.Count]
    $sessionStart = Get-Date
    $sessionStartMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $findingLines = Get-LineCount $findings
    $actionLines = Get-LineCount "$Out\actions.jsonl"
    Write-Host "playtest[$name]: sessão $session com $model ($left min restantes)"
    $psi = [System.Diagnostics.ProcessStartInfo]::new($opencode)
    foreach ($arg in @("run", "--standalone", "--auto", "--archived", "-m", $model, $prompt)) { $psi.ArgumentList.Add($arg) }
    $psi.WorkingDirectory = "$Out\room"
    $psi.Environment["XDG_CONFIG_HOME"] = $configHome
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $process = [System.Diagnostics.Process]::Start($psi)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $limit = [Math]::Max(1, [Math]::Min($SessionMinutes, $left + 1))
    if (-not $process.WaitForExit($limit * 60 * 1000)) {
        $process.Kill($true)
        Write-Host "  sessão ${session}: tempo esgotado"
    }
    $text = ($stdout.Result + "`n--- stderr ---`n" + $stderr.Result) -replace "\x1b\[[0-9;]*[A-Za-z]", ""
    Set-Content "$Out\sessions\$('{0:d2}' -f $session)-$($model -replace '[/:]', '_').txt" $text -Encoding utf8
    $newFindings = @(Get-NewFindingTitles $findings $findingLines)
    $actions = Get-SessionActions "$Out\actions.jsonl" $actionLines
    $actionParts = @($actions.methods.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Value) $($_.Name)" })
    $facts = "$($actions.count) chamadas"
    if ($actionParts.Count) { $facts += " (" + ($actionParts -join ", ") + ")" }
    if ($actions.targets.Count) { $facts += "; alvos: " + ($actions.targets -join ", ") }
    if ($newFindings.Count) { $facts += "; achados: " + ($newFindings -join " | ") } else { $facts += "; nenhum achado novo" }
    $summary = Get-SessionSummary ($stdout.Result -replace "\x1b\[[0-9;]*[A-Za-z]", "") $facts
    Save-SessionUsage $opencode "$Out\room" $Out $session $sessionStartMs $configHome

    $plain = $text
    if ($plain -match "(?i)usage limit reached|free usage limit|insufficient (credit|quota)|subscribe to OpenCode") {
        $exhausted[$model] = $true
        Write-Host "  $model atingiu o limite de uso; próximo modelo"
    } elseif (((Get-Date) - $sessionStart).TotalSeconds -lt 45) {
        $quickFailures++
        Write-Host "  sessão curta demais ($quickFailures seguidas)"
        if ($quickFailures -ge 5) { Write-Host "playtest[$name]: o agente não está conseguindo trabalhar; encerrando"; break }
        Start-Sleep -Seconds (15 * $quickFailures)
    } else {
        $quickFailures = 0
    }

    if ($forja.HasExited -and $forja.ExitCode -eq 0) {
        # Closed normally (e.g. the tester clicked the window's close button): reopen.
        Write-Host "  o app foi fechado normalmente; reabrindo"
        $forja = Start-Forja
    } elseif ($forja.HasExited) {
        $tail = ""
        if (Test-Path "$Out\actions.jsonl") { $tail = (Get-Content "$Out\actions.jsonl" -Tail 5) -join "`n" }
        $crash = @{
            title = "O app fechou durante o teste ($name, sessão $session)"
            kind = "crash"; severity = "critical"
            steps = @("Ver as últimas ações em actions.jsonl", $tail)
            expected = "A Forja continua aberta"
            actual = "Processo terminou com código $($forja.ExitCode). stderr: $(($appErr.Result -split "`n" | Select-Object -Last 8) -join ' | ')"
            screen = "desconhecida"
        } | ConvertTo-Json -Compress
        Add-Content $findings $crash
        Write-Host "  o app caiu; reabrindo"
        $forja = Start-Forja
    }
    if ($text -match "(?i)rate limit|429") { Start-Sleep -Seconds 60 }
}

if (-not $forja.HasExited) { $forja.Kill($true) }
# Aggregate token use. In CI the runner is fresh, so --all is only this run;
# locally the exact source is sessions/usage.jsonl.
$previous = $env:XDG_CONFIG_HOME
$env:XDG_CONFIG_HOME = $configHome
Push-Location "$Out\room"
& $opencode stats --standalone --all --project . --cost --models --json 2>$null | Set-Content "$Out\usage.json" -Encoding utf8
Pop-Location
if ($null -eq $previous) {
    Remove-Item Env:XDG_CONFIG_HOME -ErrorAction SilentlyContinue
} else {
    $env:XDG_CONFIG_HOME = $previous
}
Set-Content "$Out\app-stderr.txt" $appErr.Result -Encoding utf8
$count = if (Test-Path $findings) { (Get-Content $findings).Count } else { 0 }
Write-Host "playtest[$name]: $session sessões, $count achados"
