param(
    [string]$BinDir = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if ([string]::IsNullOrWhiteSpace($BinDir)) {
    $BinDir = Join-Path $repoRoot "target\debug"
}

$forjaApp = Join-Path $BinDir "forja-app.exe"
$forjaCli = Join-Path $BinDir "forja.exe"
$forjaMcp = Join-Path $BinDir "forja-mcp.exe"

foreach ($exe in @($forjaApp, $forjaCli, $forjaMcp)) {
    if (-not (Test-Path $exe)) {
        throw "Missing built binary: $exe. Run cargo build --workspace first."
    }
}

$runId = [guid]::NewGuid().ToString("N").Substring(0, 12)
$sandbox = Join-Path ([IO.Path]::GetTempPath()) "forja-e2e-$runId"
$dataDir = Join-Path $sandbox "data"
$contentDir = Join-Path $sandbox "content"
$homeDir = Join-Path $sandbox "home"
$appDataDir = Join-Path $sandbox "appdata"
$localAppDataDir = Join-Path $sandbox "localappdata"
$codexDir = Join-Path $sandbox "codex-home"
$binShimDir = Join-Path $sandbox "bin"
$repoDir = Join-Path $sandbox "repo"
$pipeName = "\\.\pipe\forja-e2e-$runId"

foreach ($dir in @(
    $dataDir,
    (Join-Path $contentDir "helpers"),
    $homeDir,
    $appDataDir,
    $localAppDataDir,
    (Join-Path $codexDir "sessions"),
    $binShimDir,
    (Join-Path $repoDir "src")
)) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

function Invoke-Git {
    param([string[]]$GitArgs)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $out = & git -C $repoDir @GitArgs 2>&1
    $code = $LASTEXITCODE
    $ErrorActionPreference = $prev
    if ($code -ne 0) {
        throw "git $($GitArgs -join ' ') failed ($code): $out"
    }
}

# Initialize isolated fixture repository.
Set-Content -Path (Join-Path $repoDir "README.md") -Value "# Projeto E2E`nforja smoke target`n" -Encoding UTF8
Set-Content -Path (Join-Path $repoDir "src\main.txt") -Value "needle-forja-e2e`nother line`n" -Encoding UTF8
Invoke-Git @("init", "-b", "main")
Invoke-Git @("config", "user.name", "Forja E2E")
Invoke-Git @("config", "user.email", "e2e@forja.invalid")
Invoke-Git @("add", ".")
Invoke-Git @("commit", "-m", "initial e2e fixture")

# Create fake helper (.cmd wrapper + stdin handler) so no real external agent CLI is ever launched.
$fakeCmd = Join-Path $binShimDir "fake-helper.cmd"
$fakePs1 = Join-Path $binShimDir "fake-helper-body.ps1"

$fakePs1Content = @'
$ErrorActionPreference = "Stop"
$buffer = ""
while ($null -ne ($line = [Console]::In.ReadLine())) {
    $buffer += "`n" + $line
    if ($buffer -match "--task\s+(task-\d+)") {
        $taskId = $Matches[1]
        & forja muster handoff --result feito --summary "Fake helper concluded" --evidence "fake-helper.cmd ran" --task $taskId
        exit $LASTEXITCODE
    }
    if ($line -match "Ola ajudante" -or $line -match "LOCAL-") {
        Write-Output "FAKE_HELPER_PROMPT_SEEN: $line"
        Start-Sleep -Milliseconds 300
        exit 0
    }
}
'@
Set-Content -Path $fakePs1 -Value $fakePs1Content -Encoding UTF8

$fakeCmdContent = "@echo off`r`necho FAKE_HELPER_READY`r`npowershell.exe -NoProfile -ExecutionPolicy Bypass -File `"%~dp0fake-helper-body.ps1`"`r`n"
Set-Content -Path $fakeCmd -Value $fakeCmdContent -Encoding ASCII

$escapedFakeCmd = $fakeCmd.Replace("\", "\\")
$helperProfile = @"
id = "claude-code"
name = "Fake Helper"
maker = "Forja E2E"
site = "https://forja.invalid"

[detect]
executables = ["cmd.exe"]

[start]
command = "cmd.exe"
args = ["/d", "/c", "$escapedFakeCmd"]

[prompt]
mode = "stdin"
"@
Set-Content -Path (Join-Path $contentDir "helpers\claude-code.toml") -Value $helperProfile -Encoding UTF8

$settingsToml = @"
[helper.default]
profile = "claude-code"

[terminal.shell]
default = "cmd"
"@
Set-Content -Path (Join-Path $dataDir "settings.toml") -Value $settingsToml -Encoding UTF8

# Write Codex JSONL fixture under isolated CODEX_HOME.
$escapedRepo = $repoDir.Replace("\", "\\")
$codexLines = @(
    "{`"timestamp`":`"2026-11-12T10:00:00Z`",`"type`":`"session_meta`",`"payload`":{`"id`":`"e2e-session-1`",`"cwd`":`"$escapedRepo`"}}",
    "{`"timestamp`":`"2026-11-12T10:00:01Z`",`"type`":`"response_item`",`"payload`":{`"role`":`"user`",`"content`":[{`"type`":`"input_text`",`"text`":`"Explicar arquitetura da Forja E2E`"}]}}",
    "{`"timestamp`":`"2026-11-12T10:00:02Z`",`"type`":`"response_item`",`"payload`":{`"role`":`"assistant`",`"content`":[{`"type`":`"output_text`",`"text`":`"Sessao indexada acervo-token-e2e`"}]}}"
)
Set-Content -Path (Join-Path $codexDir "sessions\rollout-e2e.jsonl") -Value $codexLines -Encoding UTF8

# Isolate environment variables for this process and all child processes.
$envBackup = @{}
foreach ($name in @(
    "FORJA_DATA_DIR",
    "FORJA_CONTENT_DIR",
    "FORJA_PIPE",
    "CODEX_HOME",
    "USERPROFILE",
    "HOME",
    "APPDATA",
    "LOCALAPPDATA",
    "GIT_TERMINAL_PROMPT",
    "PATH"
)) {
    $envBackup[$name] = [Environment]::GetEnvironmentVariable($name, "Process")
}

$env:FORJA_DATA_DIR = $dataDir
$env:FORJA_CONTENT_DIR = $contentDir
$env:FORJA_PIPE = $pipeName
$env:CODEX_HOME = $codexDir
$env:USERPROFILE = $homeDir
$env:HOME = $homeDir
$env:APPDATA = $appDataDir
$env:LOCALAPPDATA = $localAppDataDir
$env:GIT_TERMINAL_PROMPT = "0"
$env:PATH = "$BinDir;$binShimDir;$env:PATH"

function Invoke-ForjaRaw {
    param([string[]]$CliArgs)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $stdout = & $forjaCli @CliArgs 2>&1
    $code = $LASTEXITCODE
    $ErrorActionPreference = $prev
    [PSCustomObject]@{
        ExitCode = $code
        Output   = ($stdout | Out-String).Trim()
    }
}

function Invoke-ForjaJson {
    param([string[]]$CliArgs)
    $allArgs = $CliArgs + @("--json")
    $res = Invoke-ForjaRaw $allArgs
    if ($res.ExitCode -ne 0) {
        throw "forja $($allArgs -join ' ') failed ($($res.ExitCode)): $($res.Output)"
    }
    if ([string]::IsNullOrWhiteSpace($res.Output)) {
        return $null
    }
    return ($res.Output | ConvertFrom-Json)
}

function Wait-Until {
    param(
        [string]$Description,
        [int]$TimeoutMs = 20000,
        [int]$IntervalMs = 200,
        [scriptblock]$Condition
    )
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $value = & $Condition
        if ($value) {
            return $value
        }
        Start-Sleep -Milliseconds $IntervalMs
    }
    throw "Timed out waiting for: $Description"
}

$appStdout = Join-Path $sandbox "app.stdout.log"
$appStderr = Join-Path $sandbox "app.stderr.log"
$appProc = $null

try {
    Write-Output "[e2e] Starting isolated forja-app (pipe=$pipeName)"
    $appProc = Start-Process -FilePath $forjaApp `
        -ArgumentList @("--locale", "pt-BR") `
        -PassThru `
        -RedirectStandardOutput $appStdout `
        -RedirectStandardError $appStderr

    # 1. status & metodos
    $methods = Wait-Until -Description "forja metodos" -TimeoutMs 25000 -Condition {
        if ($appProc.HasExited) {
            $err = if (Test-Path $appStderr) { Get-Content $appStderr -Raw } else { "" }
            throw "forja-app exited early ($($appProc.ExitCode)): $err"
        }
        $r = Invoke-ForjaRaw @("metodos", "--json")
        if ($r.ExitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($r.Output)) {
            return ($r.Output | ConvertFrom-Json)
        }
        return $null
    }
    $methodIds = @($methods.methods | ForEach-Object { $_.name })
    foreach ($expected in @(
        "project.add", "bench.create", "bench.list", "bench.stage.set",
        "terminal.open", "terminal.send", "terminal.read",
        "request.list", "request.local.create", "request.forge",
        "helper.launch", "helper.status",
        "muster.run", "muster.task", "muster.start", "muster.status",
        "routine.save", "routine.run", "routine.runs",
        "file.open", "search.run", "search.results",
        "archive.reindex", "archive.list", "archive.search"
    )) {
        if ($methodIds -notcontains $expected) {
            throw "Expected RPC method missing from catalog: $expected"
        }
    }
    Write-Output "[step 1] PASS: forja metodos ($($methodIds.Count) methods)"

    # Verify fake helper override is active before launching any helper.
    $helperList = Invoke-ForjaJson @("helper.list")
    $claudeProfile = $helperList.profiles | Where-Object { $_.id -eq "claude-code" } | Select-Object -First 1
    if (-not $claudeProfile -or $claudeProfile.name -ne "Fake Helper") {
        throw "Fake helper override is not active! Got profile: $($claudeProfile | ConvertTo-Json -Compress)"
    }

    # 2. project.add
    $project = Invoke-ForjaJson @("project.add", "--path", $repoDir, "--name", "Projeto E2E")
    $projectId = $project.id
    if ([string]::IsNullOrWhiteSpace($projectId)) {
        throw "project.add did not return id"
    }
    Write-Output "[step 2] PASS: project.add -> id=$projectId"

    # 3. bench.create & bench.list
    $createdBench = Invoke-ForjaJson @("bench.create", "--project_id", $projectId, "--name", "bancada-e2e")
    $bench = Wait-Until -Description "bench 'bancada-e2e' listed and created on disk" -Condition {
        $listed = Invoke-ForjaJson @("bench.list")
        $found = $listed.benches | Where-Object { $_.name -eq "bancada-e2e" } | Select-Object -First 1
        if ($found -and (Test-Path $found.path)) { return $found }
        return $null
    }
    $benchId = $bench.bench_id
    $benchPath = $bench.path
    $null = Invoke-ForjaJson @("bench.activate", "--bench_id", $benchId)
    Write-Output "[step 3] PASS: bench.create & bench.list -> id=$benchId path=$benchPath"

    # 4. terminal.open, terminal.send, terminal.read
    $termOpened = Invoke-ForjaJson @("terminal.open", "--cwd", $benchPath, "--title", "Term E2E")
    $termId = $termOpened.terminal_id
    if ([string]::IsNullOrWhiteSpace($termId)) {
        throw "terminal.open returned no terminal_id"
    }
    $null = Invoke-ForjaJson @("terminal.send", "--terminal_id", $termId, "--text", "echo forja-e2e-terminal-ok", "--enter")
    $termRead = Wait-Until -Description "terminal output contains echo marker" -Condition {
        $out = Invoke-ForjaJson @("terminal.read", "--terminal_id", $termId, "--lines", "50")
        if ($out.text -match "forja-e2e-terminal-ok") { return $out }
        return $null
    }
    Write-Output "[step 4] PASS: terminal open/send/read ($termId)"

    # 5. bench.stage.set forging -> review
    $stageForging = Invoke-ForjaJson @("bench.stage.set", "--bench_id", $benchId, "--stage", "forging")
    if ($stageForging.stage -ne "forging") {
        throw "Expected stage forging, got: $($stageForging.stage)"
    }
    $stageReview = Invoke-ForjaJson @("bench.stage.set", "--bench_id", $benchId, "--stage", "review")
    if ($stageReview.stage -ne "review") {
        throw "Expected stage review, got: $($stageReview.stage)"
    }
    Write-Output "[step 5] PASS: bench stage transition forging -> review"

    # 6. request.local.create & request.forge
    $null = Wait-Until -Description "request local store loaded" -Condition {
        $reqs = Invoke-ForjaJson @("request.list")
        return [bool]$reqs.local_loaded
    }
    $localReq = Invoke-ForjaJson @("request.local.create", "--title", "Pedido Smoke E2E", "--body", "Validar forge completo")
    $reqKey = $localReq.key
    if ([string]::IsNullOrWhiteSpace($reqKey)) {
        throw "request.local.create returned no key"
    }
    $forged = Invoke-ForjaJson @("request.forge", "--counter", "local", "--key", $reqKey)
    $forgedBench = Wait-Until -Description "forged bench appears on disk" -Condition {
        $listed = Invoke-ForjaJson @("bench.list")
        $found = $listed.benches | Where-Object { $_.name -like "*local-1*" } | Select-Object -First 1
        if ($found -and (Test-Path $found.path)) { return $found }
        return $null
    }
    $null = Wait-Until -Description "helper run launched for forged bench" -Condition {
        $st = Invoke-ForjaJson @("helper.status", "--bench_path", $forgedBench.path)
        if ($st.runs.Count -ge 1) { return $st }
        return $null
    }
    # Re-activate our primary bench for subsequent steps.
    $null = Invoke-ForjaJson @("bench.activate", "--bench_id", $benchId)
    Write-Output "[step 6] PASS: request.local.create ($reqKey) -> request.forge ($($forgedBench.name))"

    # 7. helper.launch + helper.status
    $launchedHelper = Invoke-ForjaJson @("helper.launch", "--profile_id", "claude-code", "--bench_path", $benchPath, "--prompt", "Ola ajudante")
    $helperTermId = $launchedHelper.terminal_id
    if ([string]::IsNullOrWhiteSpace($helperTermId) -or ($launchedHelper.command_line -notmatch "fake-helper\.cmd")) {
        throw "helper.launch did not launch fake-helper.cmd: $($launchedHelper | ConvertTo-Json -Compress)"
    }
    $helperStatus = Invoke-ForjaJson @("helper.status", "--terminal_id", $helperTermId)
    if ($helperStatus.runs.Count -lt 1 -or $helperStatus.runs[0].profile_id -ne "claude-code") {
        throw "helper.status did not report active fake helper run"
    }
    Write-Output "[step 7] PASS: helper.launch & helper.status (terminal=$helperTermId status=$($helperStatus.runs[0].status))"

    # 8. muster.run, muster.task, muster.start, fake helper handoff, muster.status
    $null = Wait-Until -Description "muster store loaded" -Condition {
        $ms = Invoke-ForjaJson @("muster.status")
        return [bool]$ms.loaded
    }
    $musterRun = Invoke-ForjaJson @("muster.run", "--title", "Mutirao E2E", "--objective", "Testar handoff", "--bench_path", $benchPath)
    $musterRunId = $musterRun.id
    $musterTask = Invoke-ForjaJson @("muster.task", "--run", $musterRunId, "--title", "Tarefa E2E", "--spec", "Executar entrega fake")
    $taskId = $musterTask.id
    # Plans need approval before tasks start (muster.plan.require_approval defaults to on).
    $null = Invoke-ForjaJson @("muster.plan.approve", "--run", $musterRunId)
    $musterStarted = Invoke-ForjaJson @("muster.start", "--task", $taskId, "--profile_id", "claude-code", "--bench_path", $benchPath)
    try {
        $delivered = Wait-Until -Description "muster task delivered by fake helper" -TimeoutMs 15000 -Condition {
            $ms = Invoke-ForjaJson @("muster.status", "--run", $musterRunId)
            $t = $ms.tasks | Where-Object { $_.id -eq $taskId } | Select-Object -First 1
            if ($t -and $t.state -eq "delivered" -and $t.result -eq "feito") { return $t }
            return $null
        }
    } catch {
        $ms = Invoke-ForjaRaw @("muster.status", "--run", $musterRunId, "--json")
        $termOut = ""
        if ($musterStarted.assignment.terminal_id) {
            $termOut = (Invoke-ForjaRaw @("terminal.read", "--terminal_id", $musterStarted.assignment.terminal_id, "--lines", "50", "--json")).Output
        }
        throw "Muster timeout! status=$($ms.Output)`nterm=$termOut`norig=$($_.Exception.Message)"
    }
    Write-Output "[step 8] PASS: muster run/task/start/handoff/status (run=$musterRunId task=$taskId state=$($delivered.state))"

    # 9. routine.save, routine.run, routine.runs
    $null = Wait-Until -Description "routine store loaded" -Condition {
        $rl = Invoke-ForjaJson @("routine.list")
        return [bool]$rl.loaded
    }
    $routinePayload = @{
        name    = "Rotina E2E"
        enabled = $true
        target  = @{
            kind       = "fixed"
            bench_path = $benchPath
        }
        action  = @{
            kind    = "command"
            command = "echo rotina-e2e-ok && exit 0"
        }
    } | ConvertTo-Json -Compress -Depth 5
    $savedRoutine = Invoke-ForjaJson @("routine.save", "--routine", $routinePayload)
    if (-not $savedRoutine.saved) {
        throw "routine.save failed: $($savedRoutine | ConvertTo-Json -Compress)"
    }
    $routineId = $savedRoutine.id
    $ranRoutine = Invoke-ForjaJson @("routine.run", "--id", $routineId)
    $routineRunId = $ranRoutine.run_id
    $finishedRun = Wait-Until -Description "routine command run succeeded" -TimeoutMs 20000 -Condition {
        $runs = Invoke-ForjaJson @("routine.runs", "--routine_id", $routineId)
        $r = $runs.runs | Where-Object { $_.id -eq $routineRunId } | Select-Object -First 1
        if ($r -and $r.status.state -eq "succeeded") { return $r }
        if ($r -and $r.status.state -eq "failed") {
            throw "Routine run failed unexpectedly: $($r | ConvertTo-Json -Compress)"
        }
        return $null
    }
    Write-Output "[step 9] PASS: routine.save/run/runs (id=$routineId run=$routineRunId state=succeeded)"

    # 10. file.open
    $readmeFile = Join-Path $benchPath "README.md"
    $openedFile = Invoke-ForjaJson @("file.open", "--path", $readmeFile)
    Write-Output "[step 10] PASS: file.open ($readmeFile)"

    # 11. search.run & search.results
    $searchStarted = Invoke-ForjaJson @("search.run", "--pattern", "needle-forja-e2e", "--bench_path", $benchPath)
    if (-not $searchStarted.started) {
        throw "search.run did not start"
    }
    $searchRes = Wait-Until -Description "search completed with matches" -Condition {
        $sr = Invoke-ForjaJson @("search.results")
        if (-not $sr.running -and $sr.searched) { return $sr }
        return $null
    }
    if ($searchRes.matches -lt 1 -or ($searchRes.files | Where-Object { $_.rel -match "main\.txt" }).Count -lt 1) {
        throw "search.results missing expected match in src/main.txt"
    }
    Write-Output "[step 11] PASS: search.run & search.results (matches=$($searchRes.matches))"

    # 12. archive.reindex, archive.list, archive.search
    $reindexed = Invoke-ForjaJson @("archive.reindex")
    $archiveListed = Wait-Until -Description "archive lists e2e-session-1" -Condition {
        $al = Invoke-ForjaJson @("archive.list")
        if ($al.sessions | Where-Object { $_.session_id -eq "e2e-session-1" }) { return $al }
        return $null
    }
    $archiveFound = Invoke-ForjaJson @("archive.search", "--query", "acervo-token-e2e")
    if (-not ($archiveFound.hits | Where-Object { $_.session_id -eq "e2e-session-1" })) {
        throw "archive.search did not return e2e-session-1: $($archiveFound | ConvertTo-Json -Compress)"
    }
    Write-Output "[step 12] PASS: archive reindex/list/search (e2e-session-1)"

    # 13. forja-mcp tools/list
    $mcpRequests = @(
        '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"e2e-smoke","version":"0.1.0"}}}',
        '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    ) -join "`n"
    $mcpOut = $mcpRequests | & $forjaMcp
    if ($LASTEXITCODE -ne 0) {
        throw "forja-mcp exited with $LASTEXITCODE"
    }
    $mcpLines = @($mcpOut | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | ForEach-Object { $_ | ConvertFrom-Json })
    $initReply = $mcpLines | Where-Object { $_.id -eq 1 } | Select-Object -First 1
    $toolsReply = $mcpLines | Where-Object { $_.id -eq 2 } | Select-Object -First 1
    if (-not $initReply.result.protocolVersion -or -not $toolsReply.result.tools -or $toolsReply.result.tools.Count -lt 10) {
        throw "forja-mcp initialize or tools/list response invalid: $mcpOut"
    }
    Write-Output "[step 13] PASS: forja-mcp tools/list ($($toolsReply.result.tools.Count) tools)"
    Write-Output "[e2e] ALL 13 SMOKE STEPS PASSED"
}
finally {
    if ($appProc -and -not $appProc.HasExited) {
        try {
            $appProc.Kill()
            $null = $appProc.WaitForExit(5000)
        } catch {}
    }
    foreach ($entry in $envBackup.GetEnumerator()) {
        [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, "Process")
    }
    Start-Sleep -Milliseconds 300
    Remove-Item -Recurse -Force -Path $sandbox -ErrorAction SilentlyContinue
}
